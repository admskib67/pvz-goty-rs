//! Links the prebuilt static libopenmpt for the target from
//! `dev/libopenmpt-<os>/<target>/`. Those are built per OS with
//! `cargo run -p build-libopenmpt` and committed, so building the game needs
//! no C++ compiler and players need nothing installed.

use std::path::PathBuf;

fn main() {
    let target = std::env::var("TARGET").unwrap();
    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();
    let os_dir = match os.as_str() {
        "windows" => "libopenmpt-win",
        "macos" => "libopenmpt-darwin",
        "linux" => "libopenmpt-linux",
        other => panic!("libopenmpt isn't set up for target OS {other:?}"),
    };
    let dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap())
        .join("../../dev")
        .join(os_dir)
        .join(&target);
    let lib = if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") { "openmpt.lib" } else { "libopenmpt.a" };
    if !dir.join(lib).is_file() {
        panic!(
            "\n\nNo prebuilt libopenmpt for {target} in {}.\n\
             Build it on this machine with:\n\n    cargo run -p build-libopenmpt\n\n\
             then commit the new files under dev/.\n",
            dir.display()
        );
    }
    println!("cargo:rerun-if-changed={}", dir.display());
    println!("cargo:rustc-link-search=native={}", dir.display());
    println!("cargo:rustc-link-lib=static=openmpt");
    println!("cargo:rustc-link-lib=static=openmpt_codecs");
    // libopenmpt is C++: link the platform's C++ runtime.
    match os.as_str() {
        "macos" => println!("cargo:rustc-link-lib=c++"),
        "linux" => println!("cargo:rustc-link-lib=stdc++"),
        // UuidCreate lives in rpcrt4; MSVC links its C++ runtime automatically.
        "windows" => println!("cargo:rustc-link-lib=rpcrt4"),
        _ => {}
    }
}
