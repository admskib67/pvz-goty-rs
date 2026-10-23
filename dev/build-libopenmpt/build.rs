// Records the target triple so the tool knows which folder to build into.
fn main() {
    println!("cargo:rustc-env=BUILD_TARGET={}", std::env::var("TARGET").unwrap());
}
