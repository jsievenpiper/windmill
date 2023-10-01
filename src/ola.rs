pub mod dmx;
pub mod logging;

use cxx::UniquePtr;
use tokio::sync::mpsc::{UnboundedSender, error::SendError};
use crate::fixture::Windmill;
use crate::ola::dmx::{Buffer, Metadata};

/// Starts the OpenLightingArchitecture task with a small adapter to convert the DMX signals transmitted over something
/// like OSC or ArtNet and translates them to high-level `Windmill` commands.
pub fn start(sender: UnboundedSender<Windmill>) -> Result<(), &'static str> {
  if !logging::init(logging::LogLevel::Info, logging::LogOutput::StdErr) {
    return Err("Failed to initialize Open Lighting Architecture logging system.");
  }

  // These constants are represented by their DMX channel numbers for ease of readability. But the internal code is
  // zero-indexed, which honestly in this situation I'm not sure if I dig or not. Either way, decrement by one when
  // actually indexing with these channel references.
  const SPEED_CHANNEL: u32 = 49;
  const DIRECTION_CHANNEL: u32 = 50;

  let on_dmx = move |_: &Metadata, data: &Buffer| {
    let direction = data.get(DIRECTION_CHANNEL - 1);
    let speed = data.get(SPEED_CHANNEL - 1);

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

  let client: UniquePtr<dmx::Client> = dmx::Bridge::new(0, &on_dmx).into();

  if !client.setup() {
    return Err("Failed to initialize Open Lighting Architecture client.");
  }

  println!("... the wonderful wizard of Oz!");
  client.run();

  Err("Should never return!")
}
