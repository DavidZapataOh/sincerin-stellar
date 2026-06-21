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

    // Docker options are consumed by value per guest, so build a small factory.
    let mk_docker = || {
        DockerOptionsBuilder::default()
            .root_dir(workspace_root.clone())
            // Pin the container tag explicitly for cross-machine determinism.
            .docker_container_tag("r0.1.88.0")
            .build()
            .expect("DockerOptions")
    };

    // ── DEPLOYED guest (`rollup-guest`, depth 3) — image_id cbeab7aa…0d46. ─────
    // Built with NO extra features and from byte-identical source, so it stays
    // bit-for-bit the binary the on-chain `settle_batch` contract binds. NOTHING
    // here (and no env var) changes this guest — its depth is hardcoded `3` in
    // methods/guest/src/main.rs. This is the guest the N=8 testnet demo settles.
    let deployed_options = GuestOptionsBuilder::default()
        .use_docker(mk_docker())
        .build()
        .expect("GuestOptions(deployed)");

    // ── PROVING-ONLY bench guest (`rollup-guest-bench`, depth 3/4/5). ──────────
    // NEVER deployed/settled — its image_id is allowed to differ per depth. We
    // could NOT put depth-switching in the deployed guest: risc0's image_id is a
    // commitment over the guest ELF, which embeds source-derived DWARF/cfg
    // metadata, so ANY `#[cfg]`/`cfg!()`/changed-initializer in the deployed
    // guest's source shifts its ELF and breaks the on-chain binding (measured:
    // pristine cbeab7aa… vs +one cfg attr c2b15d04…). So the depth feature lives
    // on THIS separate guest only. `ROLLUP_TREE_DEPTH` (host-side env var; read
    // here, NOT in any guest, so it never perturbs the deployed ELF) selects the
    // bench depth: unset/3 → depth 3, 4 → td4 (N=16), 5 → td5 (N=32).
    println!("cargo:rerun-if-env-changed=ROLLUP_TREE_DEPTH");
    let bench_features: Vec<String> = match std::env::var("ROLLUP_TREE_DEPTH").ok().as_deref() {
        None | Some("") | Some("3") => Vec::new(),
        Some("4") => vec!["td4".to_string()],
        Some("5") => vec!["td5".to_string()],
        Some(other) => panic!(
            "methods/build.rs: ROLLUP_TREE_DEPTH={other:?} unsupported \
             (only 3 [default], 4 [N=16 proving-only], 5 [N=32 proving-only])"
        ),
    };
    let bench_options = GuestOptionsBuilder::default()
        .use_docker(mk_docker())
        .features(bench_features)
        .build()
        .expect("GuestOptions(bench)");

    let mut options = HashMap::new();
    options.insert("rollup-guest", deployed_options);
    options.insert("rollup-guest-bench", bench_options);
    embed_methods_with_options(options);
}
