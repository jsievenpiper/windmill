# windmill
## a relatively simple and _extremely_ cheap ArtNet / OSC / sACN receiver that powers a twelve foot prop windmill

This project pulls together [OpenLightingArchitecture](https://www.openlighting.org/) and the OrangePi fork of the
[wiringPi](https://github.com/orangepi-xunlong/wiringOP) project to create a $40 ArtNet / OSC / sACN receiver. In the
real world, it is networked into an ArtNet universe, and exposes two channels: one to control the speed of the DC motor
powering the windmill, and another to control the direction (used as a binary switch). It will debut (fingers crossed)
in a production of [The Wizard of Oz](https://en.wikipedia.org/wiki/The_Wizard_of_Oz_(2011_musical)), which is where the
silly useless print statements come from.

I have an obsession stuffing [Rust](https://www.rust-lang.org/) everywhere under the sun, mostly for the fun and
challenge. To do so, this project makes pretty good use of [cxx](https://cxx.rs/) to create easy ffi bindings into C++
and C libraries. I hope this project inspires you and makes it quick and easy to build your own custom DMX fixtures.

### Requirements
- An *Orange Pi*. Honestly you can probably use whatever SBC you have floating around, as long as it has the GPIO to do
  what you want. If you want to build an actual windmill or drive a motor, you'll want something that can drive an actual
  pwm signal in hardware.
- Everything else is optional, but you'll likely want to have on hand some relay drivers and other fun components to
  make your controller do something.
- Some base OS for your OrangePi. I used [Armbian](https://www.armbian.com/), which was new for me as an Arch user. It
  worked great, but I don't think there's anything too OS-specific here.

### Usage
This project makes a lot of assumptions in order to be compliant with the licensing requirements of both 
OpenLightingArchitecture and WiringPi.

1. Compile and install *shared* libraries for `wiringPi` and `ola`. This is going to be hardware specific a lot of the
  time unless you're trying to copy my project exactly (in which case, buy an OrangePi 3 LTS).
2. Configure your SBC to enable the PWM overlay if it isn't by default. e.g. for Armbian:
    ```shell
    # /boot/armbianEnv.txt
    overlays=pwm
    ```
3. Ensure `olad` is running, either via `systemd` or however you love to manage daemon processes.
4. Patch your `ola` interfaces via `ola_patch`.
5. `cargo build [--release]` to compile this program. Unfortunately, with this being a hardware project, this will only
  compile on the actual hardware, or on hardware that can also compile `wiringPi`. `ola` is easy to get compiled in a
  whole bunch of places.
6. Run the program `sudo ./target/[debug|release]/windmill`. `root` access is required for `wiringPi`. I assume you're
  not going to network this into critical infrastructure or connect it to the open internet (that'd be dumb). If you want
  to do that, find a rootless way to interface with your GPIO pins.
7. If nothing bows up, you should have a functional DMX windmill!
8. There's a simple `systemd` unit file in here as well that you can install that will start `windmill` automatically
  when the OrangePi starts. That should make it effectively headless!

### Things I Wish I Knew
- For whatever reason, the hardware PWM, at least as of writing with whatever version of `wiringOP` I built against
  defaulted the PWM control to "inverted", which is active-low. That means the default `duty_cycle` of `0` actually runs
  a solid high signal, which when directly mounted to your motor controller likes to make things go VERY FAST and VERY
  immediately. This is something that is probably totally fixable, but the quickest path to success for my goal of a
  headless windmill was to route the PWM signal through a spare relay I had on the normally-open channel, and have the
  application trigger that relay when it was initialized (if you read the code, you'll see). It works, and means I don't
  need to do any jank plugging / switching in the theatre.

### Reach Out!
If you find this useful or interesting, drop me a line. I really loved this project and love the idea of making the arts
more accessible to the world.
