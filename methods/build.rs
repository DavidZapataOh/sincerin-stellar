//! build.rs for the `methods` crate.
//!
//! Invokes `risc0-build` to compile the guest ELF and emit the image ID.
//! The guest must be compiled with the RISC Zero toolchain (via rzup).
fn main() {
    risc0_build::embed_methods();
}
