//!
//!                 +------+-----+----------+------+---+   OPi 3  +---+------+----------+-----+------+
//!                | GPIO | wPi |   Name   | Mode | V | Physical | V | Mode | Name     | wPi | GPIO |
//!                +------+-----+----------+------+---+----++----+---+------+----------+-----+------+
//!                |      |     |     3.3V |      |   |  1 || 2  |   |      | 5V       |     |      |
//!                |  122 |   0 |    SDA.0 |  OFF | 0 |  3 || 4  |   |      | 5V       |     |      |
//!                |  121 |   1 |    SCL.0 |  OFF | 0 |  5 || 6  |   |      | GND      |     |      |
//!                |  118 |   2 |    PWM.0 | ALT2 | 0 |  7 || 8  | 0 | OFF  | PL02     | 3   | 354  |
//!                |      |     |      GND |      |   |  9 || 10 | 0 | OFF  | PL03     | 4   | 355  |
//!                |  120 |   5 |    RXD.3 |  OFF | 0 | 11 || 12 | 0 | OFF  | PD18     | 6   | 114  |
//!                |  119 |   7 |    TXD.3 |  OFF | 0 | 13 || 14 |   |      | GND      |     |      |
//!                |  362 |   8 |     PL10 |  OFF | 0 | 15 || 16 | 0 | OFF  | PD15     | 9   | 111  |
//!                |      |     |     3.3V |      |   | 17 || 18 | 0 | OFF  | PD16     | 10  | 112  |
//!                |  229 |  11 |   MOSI.1 |  OFF | 0 | 19 || 20 |   |      | GND      |     |      |
//!                |  230 |  12 |   MISO.1 |  OFF | 0 | 21 || 22 | 0 | OFF  | PD21     | 13  | 117  |
//!                |  228 |  14 |   SCLK.1 |  OFF | 0 | 23 || 24 | 0 | OFF  | CE.1     | 15  | 227  |
//!                |      |     |      GND |      |   | 25 || 26 | 0 | OFF  | PL08     | 16  | 360  |
//!                +------+-----+----------+------+---+----++----+---+------+----------+-----+------+
//!                | GPIO | wPi |   Name   | Mode | V | Physical | V | Mode | Name     | wPi | GPIO |
//!                +------+-----+----------+------+---+   OPi 3  +---+------+----------+-----+------+
//!

use clap::Parser;
use tokio::select;
use tokio::signal::unix::SignalKind;
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TryRecvError;
use crate::fixture::Windmill;

pub mod cli;
pub mod fixture;
pub mod ola;
pub mod pwm;
pub mod wiringpi;

const BRAKE_PIN: i32 = 3;
const MOTOR_DIRECTION_PIN: i32 = 4;
const FORWARD_DRIVING_PIN: i32 = 9;
const REVERSE_DRIVING_PIN: i32 = 10;
const SAFETY_PIN: i32 = 13;
const BRAKE_STOP: i32 = wiringpi::DIGITAL_LOW;
const BRAKE_RUN: i32 = wiringpi::DIGITAL_HIGH;
const MOTOR_DIRECTION_FORWARD: i32 = wiringpi::DIGITAL_LOW;
const MOTOR_DIRECTION_REVERSE: i32 = wiringpi::DIGITAL_HIGH;
const DRIVING_INACTIVE: i32 = wiringpi::DIGITAL_LOW;
const DRIVING_ACTIVE: i32 = wiringpi::DIGITAL_HIGH;
const SAFETY_NO: i32 = wiringpi::DIGITAL_LOW;
const SAFETY_GO: i32 = wiringpi::DIGITAL_HIGH;
const INPUT_MIN: u8 = u8::MIN;
const INPUT_MAX: u8 = u8::MAX;
const OUTPUT_MIN: u8 = u8::MIN;
const OUTPUT_MAX: u8 = 100;
const SCALE: f64 = (OUTPUT_MAX as f64 - OUTPUT_MIN as f64) / (INPUT_MAX as f64 - INPUT_MIN as f64);
const UPDATE_TICKS: u8 = 6;
const MAX_SPEED_CHANGE_PER_CYCLE: u8 = 1;

/// There's effectively two high level loops running in this process:
///
///   - The first loop starts up an OpenLightingArchitecture client and begins listening for DMX messages transmitted
///     over any patched interfaces. This program doesn't particularly care, but in case you're interested, we're
///     patched in to OSC, ArtNet, and sACN.
///   - The second loop is responsible for writing out the physical commands that represent the current desired state
///     of the system.
///
/// Note that there's no actual formal requirement for the system to work this way specifically, it's just the way I've
/// decided for it to behave. It's perfectly valid (and used in this system) for each of these loops to do other work.
/// In this case, both systems operate on an intermediate higher-level state defined as `Windmill`, which represents a
/// state of our windmill that this fixture is controlling. It can be the current state of the windmill, a previous
/// state, or a proposed state in different contexts.
#[tokio::main]
async fn main() -> Result<(), &'static str> {
  let args = cli::Args::parse();

  println!("We're off to see the wizard...");
  wiringpi::init()?;
  ola::ensure_patches_exist(args.universe).await?;

  // For the two systems to communicate, we set up an unbounded channel for `Windmill` state messages to be passed from
  // one end to the other. This channel is convenient because we only need one-way message passing: from the OLA
  // messages down to the physical receiving end. We're using an unbounded system here because we're able to process
  // messages quickly enough that there's no need to handle backpressure. Our OrangePi is probably insanely over-powered
  // for this, but this multi-threaded two-loop system is also part of what makes managing this lack of backpressure
  // possible in the first place.
  let (tx, mut rx) = mpsc::unbounded_channel::<Windmill>();

  // Start up an OpenLightingArchitecture client and pass the transmission end ownership over to it.
  let ola_task = tokio::task::spawn_blocking(move || {
    // Once start is called here, this task should never return. Under the hood it will call `Run` on the underlying
    // receive server. If this task returns, our fixture has failed.
    ola::start(tx, args.speed_channel, args.direction_channel)
  });

  // Start another process for the receiving end, which will use the OrangePi's physical GPIO pins to dive a PWM signal
  // for motor speed and other digital state signals. This task is also always listening, and should never return.
  let windmill_task = tokio::spawn(async move {
    wiringpi::pin_mode(BRAKE_PIN, wiringpi::PIN_MODE_OUTPUT);
    wiringpi::pin_mode(MOTOR_DIRECTION_PIN, wiringpi::PIN_MODE_OUTPUT);
    wiringpi::pin_mode(FORWARD_DRIVING_PIN, wiringpi::PIN_MODE_OUTPUT);
    wiringpi::pin_mode(REVERSE_DRIVING_PIN, wiringpi::PIN_MODE_OUTPUT);
    wiringpi::pin_mode(SAFETY_PIN, wiringpi::PIN_MODE_OUTPUT);
    set_direction_forward();
    set_brake(BRAKE_STOP);
    set_safety(SAFETY_GO);

    let driver = pwm::init(0, 0, 20000)?;

    let mut desired_state = Windmill::Off;
    let mut current_state = Windmill::Off;
    let mut tick = 0u8;

    loop {
      // Non-blocking, non-sleeping receive call, so we can continue to emit a full pulse at whatever frequency we're
      // currently emitting at. In this portion of the loop, all we're doing is updating the system's desired state to
      // be whatever we've most recently received from the controller.
      match rx.try_recv() {
        // Awesome! Some work to do!
        Ok(value) => desired_state = value,

        // This ain't good...
        Err(TryRecvError::Disconnected) => return Err("windmill lost connection to incoming DMX messages."),

        // This is actually okay. It's fine if no messages have come in. Some controllers will continuously output the
        // current desired state of the system, but they may only happen every second or so. We'll get a lot of "nothing
        // to do" responses.
        //
        // However, we shouldn't break here. Our system still may not be in the desired state, so this just means we
        // don't need to update that desired state.
        Err(TryRecvError::Empty) => {}
      }

      // Simple tick counter that will act as a linear easing function between state updates. We do this _after_ the
      // desired state so that we're always easing to the most recently desired state and don't get caught lagging
      // behind.
      tick = (tick + 1) % UPDATE_TICKS;

      if tick != 0 {
        continue;
      }

      // Now we need to reconcile the current state with the desired state.
      let new_state = state_change_evaluator(current_state, desired_state);

      if new_state != current_state {
        let duty_cycle = match new_state {
          Windmill::Off | Windmill::Cooldown(_) => 0,
          Windmill::Forward(speed) | Windmill::Reverse(speed) => {
            let scale = (OUTPUT_MIN as f64 + ((speed as f64 - INPUT_MIN as f64) * SCALE)) as u8;
            println!("Received {speed}, scaling to: {scale}");

            scale
          }

        };

        // Specifically do not break on this particular error.
        if let Err(why) = driver.set_duty_cycle(duty_cycle) {
          eprintln!("{}", why);
        }

        current_state = new_state;
      }

      // We're not going to be able to get more granular than this anyway, and updating the state every 10ms, especially
      // when factoring in acceleration/deceleration/state easing... is completely indistinguishable from realtime busy
      // waiting.
      tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }
  });

  // Establishes the set of signals one should listen to in a long-running process to gracefully handle most types of
  // easy shutdown scenarios.
  let ctrl_c = tokio::signal::ctrl_c();
  let mut terminate = tokio::signal::unix::signal(SignalKind::terminate())
    .map_err(|_| "could not wire up listener for terminate signal")?;
  let mut interrupt = tokio::signal::unix::signal(SignalKind::interrupt())
    .map_err(|_| "could not wire up listener for interrupt signal")?;

  // So here's the thing: if we've done our job correctly, neither of these processes will die, and we'll be happy
  // campers. If something goes wrong, `select!` will make sure that the first thing to die quickly kills the rest of
  // the program and returns that error as the application error.
  select! {
    ola_err = ola_task => ola_err.map_err(|_| "OpenLightingArchitecture thread panicked!")?,
    windmill_err = windmill_task => windmill_err.map_err(|_| "Windmill thread panicked!")?,
    _ = ctrl_c => graceful_shutdown(),
    _ = terminate.recv() => graceful_shutdown(),
    _ = interrupt.recv() => graceful_shutdown()
  }
}

fn state_change_evaluator(current_state: Windmill, desired_state: Windmill) -> Windmill {
  match (current_state, desired_state) {
    // You want the windmill off? It's off already!
    (Windmill::Off, Windmill::Off) => Windmill::Off,

    // Begin the cool down process after rapidly braking. This generally takes under a second.
    (Windmill::Cooldown(cycles), _) if cycles > 0 => Windmill::Cooldown(cycles - 1),

    // The cool down process has completed, back to normal operation.
    (Windmill::Cooldown(_), _) => Windmill::Off,

    // It's never desirable to be in the cool down state, it should only ever be a present state. If this somehow
    // happens, which we should be able to assert that it won't: we're broken somewhere. We can't actually fix it though
    // in this context, so just try to get the windmill off.
    (_, Windmill::Cooldown(_)) => Windmill::Off,

    // When going from off to on, we need to enable the brake/run relay and set our direction pin. We won't actually
    // worry about setting the speed yet -- that's easier to just let happen as a part of the next cycle (remember
    // this is happening every 10ms). To make this happen, we'll actually set the current state to `Forward(0)`.
    (Windmill::Off, Windmill::Forward(_)) => {
      set_direction_forward();
      set_brake(BRAKE_RUN);

      Windmill::Forward(0)
    },

    // Going in reverse is the same as going forward, but we swap the braking circuit (direction) pin polarity. This
    // will also run the motor controller in reverse.
    (Windmill::Off, Windmill::Reverse(_)) => {
      set_direction_reverse();
      set_brake(BRAKE_RUN);

      Windmill::Reverse(0)
    },

    // When going exactly as fast as you want to be going, you're winning!
    (Windmill::Forward(current), Windmill::Forward(desired)) if current == desired =>
      Windmill::Forward(current),

    // Same thing when we're spinning in reverse exactly as fast as we want to be.
    (Windmill::Reverse(current), Windmill::Reverse(desired)) if current == desired =>
      Windmill::Reverse(current),

    // When going too fast, slow down. We need to clamp this to the desired value to fall into the branches above next
    // cycle, otherwise if MAX_SPEED_CHANGE_PER_CYCLE != 1 we may bounce back and forth but never settle on the desired
    // actual speed.
    (Windmill::Forward(current), Windmill::Forward(desired)) if current > desired =>
      Windmill::Forward(std::cmp::max(current - MAX_SPEED_CHANGE_PER_CYCLE, desired)),

    // Spinning in reverse too quickly? Same as above, slow it down brother!
    (Windmill::Reverse(current), Windmill::Reverse(desired)) if current > desired =>
      Windmill::Reverse(std::cmp::max(current - MAX_SPEED_CHANGE_PER_CYCLE, desired)),

    // If we're not at the right speed, and we're not going too fast, we must need to accelerate. Same general principle
    // as slowing down, just not going slower. Faster!
    (Windmill::Forward(current), Windmill::Forward(desired)) =>
      Windmill::Forward(std::cmp::min(current + MAX_SPEED_CHANGE_PER_CYCLE, desired)),

    // Too slow in reverse? Hit the gas!
    (Windmill::Reverse(current), Windmill::Reverse(desired)) =>
      Windmill::Reverse(std::cmp::min(current + MAX_SPEED_CHANGE_PER_CYCLE, desired)),

    // If we're going and we want to stop, trigger the brake relay which should pull any residual momentum into the
    // braking resistor.
    (_, Windmill::Off) => {
      set_brake(BRAKE_STOP);
      Windmill::Cooldown(100)
    }

    // This is potentially the trickiest set of state changes: hard switch of direction. But actually it's not as bad
    // as it may seem. The goal of the cool down phase is to handle this transition. Once the cool down phase asses, the
    // system shut start moving the motor in the other direction.
    (Windmill::Forward(_), Windmill::Reverse(_)) | (Windmill::Reverse(_), Windmill::Forward(_)) => {
      set_brake(BRAKE_STOP);
      Windmill::Cooldown(100)
    }
  }
}

/// Simple clean up task for when the application is manually killed. This will turn off the brake and disable the
/// safety which relays the PWM signal. This should pull the motor controller off and discharge the motor to the braking
/// resistor. This isn't totally fool-proof, but at least if you hit CTRL-C in a panic it'll attempt to also panic stop
/// the hardware.
///
/// Believe it or not this is not based on a horrific incident that happened or anything, it just dawned on me that
/// something like this would be the right thing to do and I couldn't sleep until I did it. So now it's done.
fn graceful_shutdown() -> Result<(), &'static str> {
  println!("I'll get you my pretty!");
  set_brake(BRAKE_STOP);
  set_safety(SAFETY_NO);
  std::process::exit(0)
}

#[cfg(not(test))]
fn set_brake(value: i32) {
  wiringpi::digital_write(BRAKE_PIN, value);
}

#[cfg(test)]
fn set_brake(value: i32) {
  // no-op for testing
}

#[cfg(not(test))]
fn set_safety(value: i32) {
  wiringpi::digital_write(SAFETY_PIN, value);
}

#[cfg(test)]
fn set_safety(value: i32) {
  // no-op for testing
}

#[cfg(not(test))]
fn set_direction_forward() {
  wiringpi::digital_write(MOTOR_DIRECTION_PIN, MOTOR_DIRECTION_FORWARD);
  wiringpi::digital_write(FORWARD_DRIVING_PIN, DRIVING_ACTIVE);
  wiringpi::digital_write(REVERSE_DRIVING_PIN, DRIVING_INACTIVE);
}

#[cfg(not(test))]
fn set_direction_reverse() {
  wiringpi::digital_write(MOTOR_DIRECTION_PIN, MOTOR_DIRECTION_REVERSE);
  wiringpi::digital_write(FORWARD_DRIVING_PIN, DRIVING_INACTIVE);
  wiringpi::digital_write(REVERSE_DRIVING_PIN, DRIVING_ACTIVE);
}

#[cfg(test)]
fn set_direction_forward() {
  // no-op for testing
}

#[cfg(test)]
fn set_direction_reverse() {
  // no-op for testing
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn off_to_off() {
    assert_eq!(Windmill::Off, state_change_evaluator(Windmill::Off, Windmill::Off));
  }

  #[test]
  fn off_to_forward() {
    assert_eq!(Windmill::Forward(0), state_change_evaluator(Windmill::Off, Windmill::Forward(239)));
  }

  #[test]
  fn forward_stopped_to_go() {
    assert_eq!(
      Windmill::Forward(MAX_SPEED_CHANGE_PER_CYCLE),
      state_change_evaluator(Windmill::Forward(0), Windmill::Forward(239))
    );
  }
}
