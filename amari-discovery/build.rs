// SPDX-License-Identifier: MIT OR Apache-2.0

fn main() {
    let target = std::env::var("TARGET").expect("Cargo always defines TARGET for build scripts");
    println!("cargo:rustc-env=AMARI_DISCOVERY_TARGET={target}");
}
