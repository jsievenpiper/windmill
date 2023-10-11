pub mod dmx;
pub mod logging;

use cxx::UniquePtr;
use tokio::sync::mpsc::{UnboundedSender, error::SendError};
use tokio_retry::Retry;
use tokio_retry::strategy::{ExponentialBackoff, jitter};
use crate::fixture::Windmill;
use crate::ola::dmx::{Buffer, Metadata};

/// Starts the OpenLightingArchitecture task with a small adapter to convert the DMX signals transmitted over something
/// like OSC or ArtNet and translates them to high-level `Windmill` commands. `speed_channel` and `direction_channel`
/// will dictate which channels the speed and direction values are read from, somewhat obviously. What is ever so
/// slightly less obvious is that these values are represented by their DMX channel numbers for ease of readability. But
/// the internal code is zero-indexed, which honestly in this situation I'm not sure if I dig or not. Either way,
/// decrement by one when actually indexing with these channel references.
pub fn start(
  sender: UnboundedSender<Windmill>,
  universe: u32,
  speed_channel: u32,
  direction_channel: u32
) -> Result<(), &'static str> {
  if !logging::init(logging::LogLevel::Info, logging::LogOutput::StdErr) {
    return Err("Failed to initialize Open Lighting Architecture logging system.");
  }

  let on_dmx = move |_: &Metadata, data: &Buffer| {
    let direction = data.get(direction_channel - 1);
    let speed = data.get(speed_channel - 1);

    if let Err(SendError(unsent_windmill)) = sender.send(
      match speed {
        0 => Windmill::Off,
        speed => match direction {
          0..=127 => Windmill::Forward(speed),
          128..=255 => Windmill::Reverse(speed)
        }
      }
    ) {
      eprintln!("Failed to send: {:?}", unsent_windmill)
    }
  };

  let client: UniquePtr<dmx::Client> = dmx::Bridge::new(universe, &on_dmx).into();

  if !client.setup() {
    return Err("Failed to initialize Open Lighting Architecture client.");
  }

  println!("... the wonderful wizard of Oz!");
  client.run();

  Err("Should never return!")
}

/// Ensures that OpenLightingArchitecture sees the correct interfaces before we attempt to connect a client. The
/// underlying `olad` systemd unit is supposed to start after networking (and it appears to), but it starts often before
/// an IP has been established and that seems to trip it up. Restarting it fixes things. Rather than introduce a delay,
/// this works around the bug, is generally a decent sanity thing, and allows us to run this completely headless in a
/// show scenario where we can't ssh into the windmill like I've been doing at home.
pub async fn ensure_patches_exist(for_universe: u32) -> Result<(), &'static str> {
  let retry = ExponentialBackoff::from_millis(5000)
    .map(jitter)
    .take(5);

  Retry::spawn(retry, || ensure_patches_exist_iteration(for_universe)).await
}

/// One iteration of our attempt to look for our DMX patches. Refactored out to readability of what is happening every
/// backoff loop.
async fn ensure_patches_exist_iteration(for_universe: u32) -> Result<(), &'static str> {
  let patched = patches_currently_exist(for_universe).await?;

    if patched {
      Ok(())
    }

    else {
      restart_olad().await?;
      Err("olad did not have correct patches")
    }
}

/// Checks to see, at the given moment, if we've discovered patches for our configured universe.
async fn patches_currently_exist(for_universe: u32) -> Result<bool, &'static str> {
  let interfaces = tokio::process::Command::new("ola_dev_info")
    .output()
    .await
    .map_err(|_| "failed to invoke ola_dev_info")
    .and_then(|result| String::from_utf8(result.stdout).map_err(|_| "failed to parse ola patch information"))?;

  let artnet_pattern = format!("ArtNet Universe 0:0:{for_universe}");
  let osc_pattern = format!("/dmx/universe/{for_universe}");
  Ok(interfaces.contains(&artnet_pattern) && interfaces.contains(&osc_pattern))
}

/// Restarts the systemd unit for `olad`. There's a small hardcoded sleep here just to give it some time to actually
/// restart without hitting the backoff limiter too much.
async fn restart_olad() -> Result<(), &'static str> {
  tokio::process::Command::new("systemctl")
    .arg("restart")
    .arg("olad")
    .output()
    .await
    .map(|_| ())
    .map_err(|_| "failed to restart olad")?;

  tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

  Ok(())
}
