use cxx::{ExternType, type_id};
pub use ffi::init;

/// Represents a logging level that can be supplied to the OpenLightingArchitecture system. Pretty typical logging stuff
/// and nothing should be surprising here. We do pack this as a 32-bit unsigned integer to make it easy to pass across
/// the cxx boundary.
#[repr(u32)]
pub enum LogLevel {
  /// Don't log anything.
  None,

  /// Log only catastrophic events.
  Fatal,

  /// Log catastrophic events and things that are probably wrong.
  Warn,

  /// Log catastrophic events, things that are probably wrong, and interesting things.
  Info,

  /// Log catastrophic events, things that are probably wrong, interesting things, and noisy developer things.
  Debug
}

/// cxx binding implementation for `LogLevel` that allows it to be moved and copied across the ffi boundary.
unsafe impl ExternType for LogLevel {
  type Id = type_id!("ola::log_level");
  type Kind = cxx::kind::Trivial;
}

/// Represents a sink the send log output to. This is specific to OpenLightingArchitecture. Like `LogLevel`, ee do pack
/// this as a 32-bit unsigned integer to make it easy to pass across the cxx boundary.
#[repr(u32)]
pub enum LogOutput {
  /// Send logs to stderr.
  StdErr,

  /// Send logs to the system log.
  SysLog
}

/// cxx binding implementation for `LogOutput` that allows it to be moved and copied across the ffi boundary.
unsafe impl ExternType for LogOutput {
  type Id = type_id!("ola::log_output");
  type Kind = cxx::kind::Trivial;
}

#[cxx::bridge(namespace = "ola")]
mod ffi {
  unsafe extern "C++" {
    include!("ola/Logging.h");

    type log_level = crate::ola::logging::LogLevel;
    type log_output = crate::ola::logging::LogOutput;

    #[cxx_name = "InitLogging"]
    fn init(level: log_level, sink: log_output) -> bool;
  }
}
