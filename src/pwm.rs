use std::path::PathBuf;

/// The polarity of the PWM signal. For whatever it's worth, the OrangePi 3 LTS seems to default to `Inverse`. This has
/// the implication that an inverse signal with a default zero duty cycle is actually held high. This is extremely
/// annoying when booting up the OrangePi, as anything connected to it will potentially want to start to be driven.
///
/// I've combated this by triggering the run/brake relay where the run will be high. Those pins will start low, and
/// prevent the motor from actually running.
#[derive(Copy, Clone)]
pub enum Polarity {
  /// Under `Normal` `Polarity`, the duty cycle of a PWM signal represents the active-high time, and the remaining time
  /// is spent low.
  Normal,

  /// Under `Inverse` `Polarity`, the duty cycle of a PWM signal represents the active-low time, and the remaining time
  /// is spent high.
  Inverse
}

/// `Driver` is a PWM driver representation that can own a physical GPIO pin (that is compatible with hardware PWM) and
/// drive it at various frequencies and duty cycles. This happens in userspace, so performance is pretty decent from the
/// get-go because we don't have to continually jump into kernel space to interface with the pin.
///
/// Unfortunately, if you want to be dynamic here, you need to be able to support writing to dynamic paths quite often,
/// in the form of something like:
/// `/sys/class/pwm/pwmchip<CHIP>/pwm<CHANNEL>/enable`
///
/// The quick-n-dirty answer is a bunch of allocations for paths or strings. I originally tried to do this with const
/// generics, such that all these strings (which should be known at compile time) could be static, but to the best of
/// my knowledge this isn't yet available in stable Rust. If anyone out there reads this and knows better -- let me
/// know! It felt like something that was _almost_ there.
pub struct Driver {
  /// The `pwmchip` channel to be driven.
  channel: u8,

  /// The pin's `period` as a pre-allocated `String`. This driver focuses on runtime performance over flexibility, so it
  /// does not make any optimizations for dynamically changing the period. It should be possible to `Drop` this `Driver`
  /// and instantiate a new one for a given `chip` and `channel` with a new `frequency` to adjust the period: but this
  /// incurs a lot of allocations to precalculate the `Strings` that must be written to the `/sys` controls, so this
  /// should be avoided, generally.
  period_string: String,

  /// A pre-computed map of string representations of each of the `duty_cycle` values, with a granularity of 1%. This
  /// could in theory be adjusted to have more granular control (maybe map more natively to the DMX 0-255 signals), but
  /// from what I've seen with all (albeit a small amount) of the boards and controllers -- this signal is often exposed
  /// to the user on a scale of 0->100, so it's probably moot to expose that much granularity anyway, since the end user
  /// can't actually access it.
  duty_cycle_map: [String; 101],

  /// To make the compiler happy where it can't verify things at compile-time, this is the value that should be exported
  /// to the `duty_cycle` control when we can't make a mapping. For safety, this internally defaults to `"0"`.
  default_duty_cycle_string: String,

  /// A collection of pre-allocated paths to the various controls of the PWM chip and channel this `Driver` controls.
  paths: Paths
}

/// Internal helper struct to allow access to scoped paths for either the PWM chip itself, or one of it's internal
/// channels.
struct Paths {
  /// Holds paths scoped to the PWM chip.
  chip: ChipPaths,

  /// Holds paths scoped to the PWM channel on the given chip.
  channel: ChannelPaths
}

/// Internal helper struct to allow access to paths related to the PWM chip this `Driver` controls.
struct ChipPaths {
  /// The path to the read-only query for how many channels the PWM chip supports.
  max_channels: PathBuf,

  /// The path to the write-only mutation for the channel you want to export. Note: this will fail if called multiple
  /// times (i.e. already exported). So this should not be made public as it's arguably unsafe to call without meeting
  /// preconditions.
  export: PathBuf
}

/// Internal helper struct to allow access to paths related to the PWM channel this `Driver` controls.
struct ChannelPaths {
  /// The path to the read/write controller for the channel's `Polarity`.
  polarity: PathBuf,

  /// The path to the read/write controller for the channel's period. See the notes for `Driver` around the
  /// optimizations chosen. `Driver` is not optimized for frequent writes to this field.
  period: PathBuf,

  /// The path to the read/write controller for the channel's duty cycle. This `Driver` implementation is optimized for
  /// frequent dynamic updates to this field.
  duty_cycle: PathBuf,

  /// The path to the enable/disable controller for this channel.
  enable: PathBuf
}

impl Driver {
  /// Creates a new `Driver` to control the given pwmchip indexed `chip` and channel indexed `channel`. It will operate
  /// at the given `frequency` in Hz (e.g. 10_000 for 10kHz). No guarding is taken over the frequency, it is up to the
  /// caller to understand their hardware and the support it has.
  fn new(chip: u8, channel: u8, frequency: u16) -> Self {
    // PWM period time is set in nanoseconds, so convert incoming frequency to period.
    let period: u64 = 1_000_000_000u64 / frequency as u64;

    Driver {
      channel,
      period_string: period.to_string(),
      duty_cycle_map: Self::calculate_duty_cycle_map(period),
      default_duty_cycle_string: String::from("0"),
      paths: Paths {
        chip: ChipPaths {
          max_channels: PathBuf::from(format!("/sys/class/pwm/pwmchip{chip}/npwm")),
          export: PathBuf::from(format!("/sys/class/pwm/pwmchip{chip}/export"))
        },
        channel: ChannelPaths {
          polarity: PathBuf::from(format!("/sys/class/pwm/pwmchip{chip}/pwm{channel}/polarity")),
          period: PathBuf::from(format!("/sys/class/pwm/pwmchip{chip}/pwm{channel}/period")),
          duty_cycle: PathBuf::from(format!("/sys/class/pwm/pwmchip{chip}/pwm{channel}/duty_cycle")),
          enable: PathBuf::from(format!("/sys/class/pwm/pwmchip{chip}/pwm{channel}/enable"))
        }
      }
    }
  }

  /// Sanity check to make sure that the `pwmchip` indexed has support for the `channel` that is provided. Returns an
  /// error if the query isn't able to be made (permissions issue, or pwm not enabled on the device), if the query
  /// returns a response we can't interpret, or finally if the query returns a value saying that we've tried to allocate
  /// a channel higher than it supports.
  fn check_available_channels(&self) -> Result<(), &'static str> {
    let available_channels = std::fs::read_to_string(&self.paths.chip.max_channels)
      .map_err(|_io_err| "unable to detect pwm chip: is it enabled on your hardware?")
      .and_then(|result|
        result.
          trim()
          .parse::<u8>()
          .map_err(|_parse_err|
            "unable to parse pwm channel count: perhaps it is configured incorrectly?"
          )
      )?;

    if self.channel > available_channels {
      Err("there aren't enough channels on the specified chip to support the pwm interface")
    }

    else {
      Ok(())
    }
  }

  /// Exports the channel desired from the pwmchip, if necessary. Given the query call before this one, if this fails
  /// there is likely a hardware problem.
  fn ensure_export_channel(&self) -> Result<(), &'static str> {
    // The channel already exists. It's either been exported externally (i.e. mapped to an existing external export) or
    // was exported by us previously (e.g. application restart). No need to panic here.
    if std::fs::metadata(&self.paths.channel.enable).is_ok() {
      return Ok(());
    }

    // This operation doesn't occur frequently, so an allocation here (which can sometimes be avoided if the channel has
    // already been exported) is acceptable.
    let channel_string = self.channel.to_string();

    std::fs::write(&self.paths.chip.export, &channel_string)
      .map_err(|_io_err| "failed to export channel for the pwm interface")
  }

  /// Sets the `Polarity` of the channel. If this method fails, there are two major possibilities. One, there is a lack
  /// of support for this to be called (but at least the target OrangePi 3 LTS supports this), or two, there was a
  /// hardware failure.
  pub fn set_polarity(&self, polarity: Polarity) -> Result<(), &'static str> {
    std::fs::write(
      &self.paths.channel.polarity,
      match polarity {
        Polarity::Normal => "normal",
        Polarity::Inverse => "inverse"
      }
    ).map_err(|_io_err| "failed to update polarity for chip channel")
  }

  /// Sets the frequency of the channel. As you may have read numerous times already (if not, please read the
  /// documentation for `Driver`), or guessed from the lack of `pub` or parameters: this merely initializes the
  /// frequency (which is represented as a `period` under the hood). This `Driver` implementation is not optimized for
  /// real-time adjustment of frequency. It is totally possible to optimize for that, but our application calls for an
  /// adjustment to `duty_cycle` only, so it's much simpler to define this statically.
  fn set_frequency(&self) -> Result<(), &'static str> {
    std::fs::write(&self.paths.channel.period, &self.period_string)
      .map_err(|_io_err| "failed to update period for chip channel")
  }

  /// Adjusts the duty cycle of the signal pulse. Given how fast software can operate, it is recommended, but up to the
  /// caller, to add some delay in between subsequent calls to this method. At a minimum, you probably want to wait
  /// until at least one full period has finished, or you're unlikely to get smooth results scaling between duty cycle
  /// values.
  pub fn set_duty_cycle(&self, duty_cycle: u8) -> Result<(), &'static str> {
    let duty_cycle = if duty_cycle > 100 { 100 } else { duty_cycle } as usize;
    let duty_cycle = self.duty_cycle_map.get(duty_cycle).unwrap_or(&self.default_duty_cycle_string);

    std::fs::write(&self.paths.channel.duty_cycle, duty_cycle)
      .map_err(|_io_err| "failed to update duty cycle for chip channel")
  }

  /// Enables or disables the PWM channel. This does not invalidate the driver and can continue to be used and
  /// re-enabled after being disabled.
  pub fn set_enabled(&self, enabled: bool) -> Result<(), &'static str> {
    std::fs::write(&self.paths.channel.enable, if enabled { "1" } else { "0" })
      .map_err(|_io_err| "failed to update polarity for chip channel")
  }

  /// Internal helper to calculate string representations of every possible `duty_cycle` input value. These are
  /// effectively `String` representations of percentage slices of the input `period`, with 1% granularity.
  fn calculate_duty_cycle_map(period: u64) -> [String; 101] {
    const EMPTY_STRING: String = String::new();
    let period_pulse: u64 = period / 100u64;

    let mut map: [String; 101] = [ EMPTY_STRING; 101 ];

    for i in 0u64..=100 {
      map[i as usize] = (period_pulse * i).to_string()
    }

    map
  }
}

/// Simple `Drop` implementation that shuts down the `Driver` if it can. `Drop` cannot `Err`, so this is a best-attempt
/// sort of thing.
impl Drop for Driver {
  fn drop(&mut self) {
    self.set_duty_cycle(0).ok();
    self.set_enabled(false).ok();
  }
}

/// Initializes the PWM system on a given `chip` and `channel` to operate at the given `frequency`. To start, this will
/// operate at `Normal` `Polarity` and will start at a `duty_cycle` of `0` regardless of frequency setting.
pub fn init(chip: u8, channel: u8, frequency: u16) -> Result<Driver, &'static str> {
  let driver: Driver = Driver::new(chip, channel, frequency);
  driver.check_available_channels()?;
  driver.ensure_export_channel()?;
  driver.set_polarity(Polarity::Normal)?;
  driver.set_frequency()?;
  driver.set_duty_cycle(0)?;
  driver.set_enabled(true)?;

  Ok(driver)
}
