/// Represents a state of the windmill. The windmill can either be `Off` (not spinning), moving `Forward` at some
/// desired rate, or moving in `Reverse` (again, at a desired rate). This is used for all representations of the
/// windmill. That is, an instance of this enum is not necessarily the current state. It can be something like a desired
/// or previous state as well. So careful reading should be taken when observing values of this enum.
///
/// Keep in mind this represents a real, physical device with mechanical and physical characteristics. The windmill
/// cannot react instantaneously in the way code can. Throwing the windmill from `Forward` to `Reverse` would probably
/// be bad news. This software doesn't actually determine those incoming desired states though, an operator does (I
/// think coincidentally that operator may end up being me, but hey, maybe it won't). To protect ourselves from humans
/// (yes, even myself), interpreting code that manages this state will manage three variations of this state at any
/// given time: the incoming state, the current state, and the desired state. These will be managed to attempt to
/// smoothly transition from one state to another without killing someone with a giant spinning wooden blade of death.
#[derive(Copy, Clone, Debug, PartialOrd, PartialEq)]
pub enum Windmill {
  /// The windmill is off. Or should be. Or wants to be.
  Off,

  /// The windmill is tired after just being braked. THe internal integer holds a number of cycles the windmill should
  /// wait for momentum to die down and to want to start spinning again.
  Cooldown(u8),

  /// The windmill should be or is moving forward. The internal integer represents the rate at which it should be
  /// moving.
  Forward(u8),

  /// The windmill should be or is moving backward. The internal integer represents the rate at which it should be
  /// moving backward.
  Reverse(u8)
}
