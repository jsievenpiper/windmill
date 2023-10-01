use cxx::{ExternType, type_id, UniquePtr};

pub use ffi::{Buffer, Client};

/// A struct representing information about a DMX receive callback. Primarily this will be useful if you're aiming to
/// handle multiple universes within your application.
#[repr(C)]
pub struct Metadata {
  /// The universe the data buffer belongs to.
  pub universe: u32,

  /// The priority of the buffer that was sent.
  pub priority: u8
}

/// cxx binding implementation that allows instances of `Metadata` to be passed across the FFI boundary. This one has
/// been declared trivial because it is a simple copyable / movable structure packed in C representation. I've also run
/// this as a opaque type and it works fine either way.
unsafe impl ExternType for Metadata {
  type Id = type_id!("ola::client::DMXMetadata");
  type Kind = cxx::kind::Trivial;
}

/// `Bridge` is the structure on the Rust side of the house that holds the references to everything we need to process
/// DMX buffers in Rust. We're going to pass this thing over the boundary for C++ to hold on to (should be scary, but
/// it'll get wrapped into a `unique_ptr` so I'm reasonably content), and then that final pointer will actually get
/// passed back up to Rust for us to finally hold on to (as a `Client`). That makes the final object a bit opaque to us,
/// but prevents us from needing to rewrite their client wrapper type which houses the actual blocking receive loop.
pub struct Bridge<'a> {
  /// The universe this bridge is bound to. Will automatically filter out (and warn if it sees) messages for other
  /// universes. We're not expecting those to be patched into our program.
  universe: u32,

  /// A reference to a callback function to trigger when DMX packets are received.
  on_dmx_fn: &'a dyn Fn(&Metadata, &Buffer) -> ()
}

/// cxx binding representation for `Bridge` that allows it to be passed over the boundary. This type at one point was
/// absolutely not trivial, but honestly now that it's effectively a fat pointer and an int... it may actually be. But
/// whatever, it's working now.
unsafe impl<'a> ExternType for Bridge<'a> {
  type Id = type_id!("ola::dmx::Bridge");
  type Kind = cxx::kind::Opaque;
}

impl<'a> Bridge<'a> {
  /// Creates a new bridge that will listen for messages on the given universe and will call the referenced callback
  /// function with any new data.
  pub fn new(universe: u32, on_dmx_fn: &'a dyn Fn(&Metadata, &Buffer) -> ()) -> Self {
    Bridge {
      universe,
      on_dmx_fn
    }
  }

  /// Retrieves the universe the `Bridge` listens to. Since our type is opaque, we need methods that can be called on
  /// it in order to retrieve information on the other side of the boundary.
  pub fn get_universe(&self) -> u32 {
    self.universe
  }

  /// Initially called when DMX buffers come in. This method will forward to the wrapped reference that was provided
  /// when this `Bridge` was created, but first it will filter out any messages that may have erroneously arrived at our
  /// doorstep.
  pub fn on_dmx(&self, metadata: &Metadata, data: &Buffer) {
    // We're currently only patched and registered to read one universe, so we shouldn't see anything other than the one
    // we are on -- but just in case let's flag where we see updates not for us.
    if self.universe != metadata.universe {
      eprintln!("WARN: Received message for universe {}, but we're only listening on {}", metadata.universe, self.universe);
      return;
    }

    (self.on_dmx_fn)(metadata, data);
  }
}

/// Convenience implementation of `Into` that allows us to turn a `Bridge` into a `Client` without exposing the `ffi`
/// boundary on the public API.
impl<'a> Into<UniquePtr<Client<'a>>> for Bridge<'a> {
  fn into(self) -> UniquePtr<Client<'a>> {
    ffi::create(Box::new(self))
  }
}


#[cxx::bridge(namespace = "ola::dmx")]
mod ffi {
  unsafe extern "C++" {
    include!("ola/DmxBuffer.h");
    include!("ola/client/ClientWrapper.h");

    #[namespace = "ola::client"]
    #[cxx_name = "DMXMetadata"]
    type Metadata = crate::ola::dmx::Metadata;

    #[namespace = "ola"]
    #[cxx_name = "DmxBuffer"]
    type Buffer;

    #[cxx_name = "Get"]
    fn get(self: &Buffer, channel: u32) -> u8;
  }

  unsafe extern "C++" {
    include!("ola_smart_client.h");

    #[namespace = ""]
    type Client<'a>;

    fn setup(self: &Client) -> bool;
    fn run(self: &Client) -> ();

    #[namespace = ""]
    #[cxx_name = "create_client"]
    fn create<'a>(bridge: Box<Bridge<'a>>) -> UniquePtr<Client<'a>>;
  }

  extern "Rust" {
    type Bridge<'a>;

    fn get_universe(self: &Bridge<'_>) -> u32;
    fn on_dmx(self: &Bridge<'_>, metadata: &Metadata, data: &Buffer);
  }
}

