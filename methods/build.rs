//! build.rs for the `methods` crate.
//!
//! Compiles the guest ELF via `risc0-build` and emits the image ID.
//!
//! **Reproducible build (D5 / AC5.1).** The guest is compiled INSIDE the pinned
//! risc0 Docker image (`r0.1.88.0`), not on the host toolchain. A local build
//! embeds absolute source paths (e.g. the git-worktree path) into the ELF, which
//! changes the `image_id` per build location — and the on-chain `settle_batch`
//! binds a fixed `ROLLUP_GUEST_ID`, so a path-dependent id would (a) break that
//! binding and (b) violate "reproducible from a clean clone". Building in the
//! pinned container makes the `image_id` deterministic across machines and paths.
//! Docker is already required for the STARK→Groth16 wrap, so this adds no new dep.
use risc0_build::{embed_methods_with_options, DockerOptionsBuilder, GuestOptionsBuilder};
use std::collections::HashMap;

fn main() {
    // root_dir = workspace root (parent of `methods/`). It must contain the guest
    // (`methods/guest`) AND its path dep (`crates/zk-core`) AND a Cargo.lock — the
    // Docker build mounts this dir as the build context.
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR set by cargo");
    let workspace_root = std::path::Path::new(&manifest_dir)
        .parent()
        .expect("methods/ has a parent (the workspace root)")
        .to_path_buf();

    let docker_options = DockerOptionsBuilder::default()
        .root_dir(workspace_root)
        // Pin the container tag explicitly for cross-machine determinism.
        .docker_container_tag("r0.1.88.0")
        .build()
        .expect("DockerOptions");

    let guest_options = GuestOptionsBuilder::default()
        .use_docker(docker_options)
        .build()
        .expect("GuestOptions");

    let mut options = HashMap::new();
    options.insert("rollup-guest", guest_options);
    embed_methods_with_options(options);
}
