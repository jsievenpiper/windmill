pub use ffi::{pin_mode, digital_write};

#[cxx::bridge]
mod ffi {
  unsafe extern "C++" {
    include!("wiringPi.h");

    #[cxx_name = "wiringPiSetup"]
    fn setup() -> i32;

    #[cxx_name = "pinMode"]
    fn pin_mode(pin: i32, mode: i32);

    #[cxx_name = "digitalWrite"]
    fn digital_write(pin: i32, value: i32);
  }
}

/// WiringPi magic number that sets a pin to INPUT (read) mode.
pub const PIN_MODE_INPUT: i32 = 0;

/// WiringPi magic number that sets a pin to OUTPUT (write) mode.
pub const PIN_MODE_OUTPUT: i32 = 1;

/// WiringPi and generally worldwide magic number for a low digital bit.
pub const DIGITAL_LOW: i32 = 0;

/// WiringPi and generally worldwide magic number for a high digital bit.
pub const DIGITAL_HIGH: i32 = 1;

/// Initializes the WiringPi library to interact with (most) of our GPIO pins. WiringPi, for whatever reason, cannot
/// drive the PWM pins via hardware, and we need way finer-grained timing than software like this can accomplish.
///
/// Thankfully I've found some low-cost ways to drive the PWM pin correctly so we'll just do that outside of WiringPi.
pub fn init() -> Result<(), &'static str> {
  if -1 == ffi::setup() {
    return Err("Could not initialize wiringpi");
  }

  Ok(())
}
