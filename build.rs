fn main() {
  bind_ola();
  build_and_bind_wiringpi();
}

/// OpenLightingArchitecture is assumed in this case to be installed as a library that is built outside the context of
/// this project. So we don't need to stress too much about it itself (other than having it on the link path). It is
/// however, not easily compatible with `cxx`'s way of thinking of the world, so I'm wrapping it in a "smart client",
/// which is really just a wrapper that returns C++ smart pointers where appropriate to make `cxx` happy. I am now very
/// tempted to build a rust-native OLA client.
fn bind_ola() {
  cxx_build::bridges(vec![
    "src/ola/logging.rs",
    "src/ola/dmx.rs"
  ])
    .include("./include/ola-smart-client")
    .file("./src/ola_smart_client.cpp")
    .std("c++11")
    .cpp(true)
    .compile("ola-smart-client");

  println!("cargo:rustc-link-lib=ola");
  println!("cargo:rustc-link-lib=olacommon");
  println!("cargo:rustc-link-lib=protobuf");
  println!("cargo:rerun-if-changed=src/ola.rs");
}

/// Similar to OpenLightingArchitecture above, wiringPi and its other dependencies are assumed to be built outside the
/// context of this project. This allows us to link dynamically to this library and maintain the licensing restrictions
/// for using this library (and allow me to actually make this example more permissive).
fn build_and_bind_wiringpi() {
  cxx_build::bridge("src/wiringpi.rs")
    .compile("wiring-pi");

  // -lm -lpthread -lrt -lcrypt
  println!("cargo:rustc-link-lib=m");
  println!("cargo:rustc-link-lib=pthread");
  println!("cargo:rustc-link-lib=rt");
  println!("cargo:rustc-link-lib=crypt");
  println!("cargo:rustc-link-lib=wiringPi");
  println!("cargo:rerun-if-changed=src/wiringpi.rs");
}
