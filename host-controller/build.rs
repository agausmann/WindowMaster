fn main() {
    // Does not depend on any other files.
    println!("cargo:rerun-if-changed=build.rs");

    windows::build!();
}
