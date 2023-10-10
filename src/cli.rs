use clap::Parser;

/// Arguments that can be passed to the windmill to control it! These settings are most convenient when needing to live
/// alongside other hardware or dealing with unique console limitations.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
  /// The universe to listen on.
  #[arg(short, long, default_value_t = 5)]
  pub universe: u32,

  /// The channel to pick up speed signals from.
  #[arg(short, long, default_value_t = 10)]
  pub speed_channel: u32,

  /// The channel to pick up direction signals from.
  #[arg(short, long, default_value_t = 11)]
  pub direction_channel: u32
}
