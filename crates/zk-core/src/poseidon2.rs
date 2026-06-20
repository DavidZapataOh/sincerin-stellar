//! Poseidon2 permutation over the BN254 scalar field.
//!
//! Byte-exact port of the PoC's `zkhash` implementation
//! (`stellar-private-payments/poseidon2/src/poseidon2/poseidon2.rs`), restricted
//! to the BN254 instances actually used by the pool: t = 2, 3, 4, all with
//! sbox degree d = 5, RF = 8 full rounds and RP = 56 partial rounds (64 rounds
//! total). Round constants and internal-diagonal matrices are the live upstream
//! constants, extracted byte-exact into [`crate::poseidon2_constants`].
//!
//! The permutation structure mirrors the reference exactly:
//!   1. `matmul_external` (linear layer at the beginning),
//!   2. RF/2 full rounds (`add_rc` all lanes, `sbox` all lanes, `matmul_external`),
//!   3. RP partial rounds (`add_rc` lane 0 only, `sbox` lane 0 only, `matmul_internal`),
//!   4. RF/2 full rounds.
//!
//! `hash(inputs)` returns lane 0 of the permutation of `inputs` — the PoC's
//! `permutation(input)[0]` convention used for commitment / signature /
//! nullifier / pubkey. Merkle compression adds the feed-forward term itself
//! (see [`crate::merkle`]).

use ark_ff::{AdditiveGroup, Field, PrimeField};

use crate::poseidon2_constants as k;

/// BN254 scalar field element. Identical modulus and Montgomery backend to the
/// PoC's `zkhash::fields::bn256::FpBN256`, so all field arithmetic — and hence
/// every hash output — is byte-identical.
pub type Fr = ark_bn254::Fr;

/// Sbox degree (x^5) for all BN254 instances.
const D: u64 = 5;
/// Full rounds (RF). Half are applied before the partial rounds, half after.
const ROUNDS_F: usize = 8;
/// Partial rounds (RP).
const ROUNDS_P: usize = 56;
/// Total rounds = RF + RP.
const ROUNDS: usize = ROUNDS_F + ROUNDS_P;
/// RF / 2.
const ROUNDS_F_DIV_2: usize = ROUNDS_F / 2;

/// Convert a stored 32-byte little-endian constant to a field element.
#[inline]
fn fr(le: &[u8; 32]) -> Fr {
    Fr::from_le_bytes_mod_order(le)
}

/// x -> x^5 (sbox for d = 5), matching `sbox_p` in the reference.
#[inline]
fn sbox_d5(x: Fr) -> Fr {
    debug_assert_eq!(D, 5);
    let x2 = x.square();
    let x4 = x2.square();
    x4 * x
}

/// External (full-round) MDS layer.
///
/// - t = 2: circ(2, 1)
/// - t = 3: circ(2, 1, 1)
/// - t = 4: cheap 4x4 MDS (`matmul_m4`)
#[inline]
fn matmul_external<const T: usize>(state: &mut [Fr; T]) {
    match T {
        2 => {
            let sum = state[0] + state[1];
            state[0] += sum;
            state[1] += sum;
        }
        3 => {
            let sum = state[0] + state[1] + state[2];
            state[0] += sum;
            state[1] += sum;
            state[2] += sum;
        }
        4 => {
            matmul_m4(state);
        }
        _ => unreachable!("unsupported Poseidon2 width t={T}"),
    }
}

/// Cheap 4x4 MDS matrix applied to a 4-element state, identical to the
/// reference `matmul_m4` (single 4-element block since t = 4).
#[inline]
fn matmul_m4<const T: usize>(state: &mut [Fr; T]) {
    debug_assert_eq!(T, 4);
    let mut t_0 = state[0];
    t_0 += state[1];
    let mut t_1 = state[2];
    t_1 += state[3];
    let mut t_2 = state[1];
    t_2.double_in_place();
    t_2 += t_1;
    let mut t_3 = state[3];
    t_3.double_in_place();
    t_3 += t_0;
    let mut t_4 = t_1;
    t_4.double_in_place();
    t_4.double_in_place();
    t_4 += t_3;
    let mut t_5 = t_0;
    t_5.double_in_place();
    t_5.double_in_place();
    t_5 += t_2;
    let mut t_6 = t_3;
    t_6 += t_5;
    let mut t_7 = t_2;
    t_7 += t_4;
    state[0] = t_6;
    state[1] = t_5;
    state[2] = t_7;
    state[3] = t_4;
}

/// Internal (partial-round) layer.
///
/// - t = 2: matrix [[2,1],[1,3]]
/// - t = 3: matrix [[2,1,1],[1,2,1],[1,1,3]]
/// - t = 4: `sum + diag[i] * state[i]`
#[inline]
fn matmul_internal<const T: usize>(state: &mut [Fr; T], diag: &[Fr; T]) {
    match T {
        2 => {
            let sum = state[0] + state[1];
            state[0] += sum;
            state[1].double_in_place();
            state[1] += sum;
        }
        3 => {
            let sum = state[0] + state[1] + state[2];
            state[0] += sum;
            state[1] += sum;
            state[2].double_in_place();
            state[2] += sum;
        }
        4 => {
            let mut sum = state[0];
            for el in state.iter().skip(1) {
                sum += el;
            }
            for i in 0..T {
                state[i] *= diag[i];
                state[i] += sum;
            }
        }
        _ => unreachable!("unsupported Poseidon2 width t={T}"),
    }
}

/// Generic Poseidon2 permutation over a width-`T` state. `round_constants` is
/// the full `ROUNDS x T` table; only lane 0 is read during the partial rounds
/// (matching the reference, which adds `round_constants[r][0]` to `state[0]`).
fn permutation<const T: usize>(
    input: [Fr; T],
    diag: &[Fr; T],
    round_constants: &[[Fr; T]; ROUNDS],
) -> [Fr; T] {
    let mut state = input;

    // Linear layer at the beginning.
    matmul_external::<T>(&mut state);

    // First RF/2 full rounds.
    for rc in round_constants.iter().take(ROUNDS_F_DIV_2) {
        full_round::<T>(&mut state, rc);
    }

    // RP partial rounds (lane 0 only): add_rc[0], sbox(lane 0), matmul_internal.
    let p_end = ROUNDS_F_DIV_2 + ROUNDS_P;
    for rc in round_constants.iter().take(p_end).skip(ROUNDS_F_DIV_2) {
        state[0] += rc[0];
        state[0] = sbox_d5(state[0]);
        matmul_internal::<T>(&mut state, diag);
    }

    // Final RF/2 full rounds.
    for rc in round_constants.iter().take(ROUNDS).skip(p_end) {
        full_round::<T>(&mut state, rc);
    }

    state
}

/// One full round: add the round constants to every lane, apply the sbox to
/// every lane, then the external MDS layer (matches the reference loop body).
#[inline]
fn full_round<const T: usize>(state: &mut [Fr; T], rc: &[Fr; T]) {
    for (s, c) in state.iter_mut().zip(rc.iter()) {
        *s += c;
    }
    for s in state.iter_mut() {
        *s = sbox_d5(*s);
    }
    matmul_external::<T>(state);
}

/// Build the width-`T` diagonal as field elements from the LE byte constants.
#[inline]
fn diag<const T: usize>(src: &[[u8; 32]; T]) -> [Fr; T] {
    core::array::from_fn(|i| fr(&src[i]))
}

/// Build the `ROUNDS x T` round-constant table as field elements.
#[inline]
fn rc_table<const T: usize>(src: &[[[u8; 32]; T]; ROUNDS]) -> [[Fr; T]; ROUNDS] {
    core::array::from_fn(|r| core::array::from_fn(|i| fr(&src[r][i])))
}

/// Full Poseidon2 permutation for the t = 2 instance.
pub fn permute_t2(input: [Fr; 2]) -> [Fr; 2] {
    let diag = diag::<2>(&k::MAT_DIAG2_M_1);
    let rc = rc_table::<2>(&k::RC2);
    permutation::<2>(input, &diag, &rc)
}

/// Full Poseidon2 permutation for the t = 3 instance.
pub fn permute_t3(input: [Fr; 3]) -> [Fr; 3] {
    let diag = diag::<3>(&k::MAT_DIAG3_M_1);
    let rc = rc_table::<3>(&k::RC3);
    permutation::<3>(input, &diag, &rc)
}

/// Full Poseidon2 permutation for the t = 4 instance.
pub fn permute_t4(input: [Fr; 4]) -> [Fr; 4] {
    let diag = diag::<4>(&k::MAT_DIAG4_M_1);
    let rc = rc_table::<4>(&k::RC4);
    permutation::<4>(input, &diag, &rc)
}

/// Poseidon2 hash: lane 0 of the permutation of `inputs`.
///
/// The width is selected from `inputs.len()` (must be 2, 3 or 4 — the instances
/// the pool uses). This is the PoC's `permutation(input)[0]` convention for
/// commitment / signature / nullifier / pubkey. (It does NOT add the Merkle
/// feed-forward term; [`crate::merkle`] does that explicitly.)
///
/// # Panics
/// Panics if `inputs.len()` is not 2, 3 or 4.
pub fn hash(inputs: &[Fr]) -> Fr {
    match inputs.len() {
        2 => permute_t2([inputs[0], inputs[1]])[0],
        3 => permute_t3([inputs[0], inputs[1], inputs[2]])[0],
        4 => permute_t4([inputs[0], inputs[1], inputs[2], inputs[3]])[0],
        n => panic!("Poseidon2 hash: unsupported input width {n} (expected 2, 3 or 4)"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ark_ff::{BigInteger, PrimeField};

    fn be_hex(f: &Fr) -> [u8; 32] {
        let mut le = [0u8; 32];
        let v = f.into_bigint().to_bytes_le();
        le[..v.len()].copy_from_slice(&v);
        le.reverse();
        le
    }

    /// Known-answer test from the upstream zkhash BN256 suite:
    /// t=3 permutation of [0,1,2] => perm[0..3] (big-endian hex).
    #[test]
    fn kat_t3_matches_upstream() {
        let out = permute_t3([Fr::from(0u64), Fr::from(1u64), Fr::from(2u64)]);
        let exp0 = hex_be("0bb61d24daca55eebcb1929a82650f328134334da98ea4f847f760054f4a3033");
        let exp1 = hex_be("303b6f7c86d043bfcbcc80214f26a30277a15d3f74ca654992defe7ff8d03570");
        let exp2 = hex_be("1ed25194542b12eef8617361c3ba7c52e660b145994427cc86296242cf766ec8");
        assert_eq!(be_hex(&out[0]), exp0);
        assert_eq!(be_hex(&out[1]), exp1);
        assert_eq!(be_hex(&out[2]), exp2);
    }

    fn hex_be(s: &str) -> [u8; 32] {
        let mut out = [0u8; 32];
        for (i, b) in out.iter_mut().enumerate() {
            *b = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).unwrap();
        }
        out
    }
}
