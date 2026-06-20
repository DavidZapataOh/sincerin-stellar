//! Binary Merkle tree over BN254 with Poseidon2 feed-forward compression.
//!
//! Byte-exact port of the PoC Merkle hashing
//! (`stellar-private-payments/circuits/src/core/merkle.rs`). An internal node
//! is `node = Poseidon2_t2_perm([left, right])[0] + left` — i.e. the t=2
//! permutation's lane 0 plus the left input (feed-forward). The leaf is a note
//! commitment.

use crate::poseidon2::{self, Fr};

/// Poseidon2 feed-forward compression of two child nodes:
/// `Poseidon2_t2_perm([left, right])[0] + left`.
///
/// Matches `circuits::core::merkle::poseidon2_compression` exactly.
#[inline]
pub fn compress(left: Fr, right: Fr) -> Fr {
    let perm = poseidon2::permute_t2([left, right]);
    perm[0] + left
}

/// Recompute the Merkle root from `leaf`, its sibling `path` (one element per
/// level, leaf level first) and the path `index`.
///
/// Bit `level` of `index` selects orientation at that level (LSB = level 0,
/// closest to the leaf): if the bit is 1 the current node is the RIGHT child,
/// so the node is `compress(sibling, current)`; otherwise it is the LEFT child,
/// `compress(current, sibling)`. This is identical to the PoC's proof
/// verification (`circuits::core::merkle` test `test_merkle_proof_verifies`)
/// and to the on-chain tree.
pub fn root_from_path(leaf: Fr, path: &[Fr], index: u64) -> Fr {
    let mut current = leaf;
    for (level, sibling) in path.iter().enumerate() {
        let is_right = (index >> level) & 1 == 1;
        current = if is_right {
            compress(*sibling, current)
        } else {
            compress(current, *sibling)
        };
    }
    current
}
