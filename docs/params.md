# Poseidon2-BN254 / Merkle parameters — PoC provenance (s1/02, BLOQUEANTE)

Realizes **CONTEXT.md D1 / AC1.2 (byte-identity) + AC1.3** (Poseidon2-BN254, same
params as the PoC). Every parameter, formula, domain separator and serialization
convention below was confirmed by **reading the PoC source**
(`stellar-private-payments/`, the Nethermind PoC) — not from secondary docs.
`zk-core` reproduces all of it byte-for-byte; the guard is
`crates/zk-core/tests/crosscheck_poc.rs` (gate: `cargo test -p zk-core --test
crosscheck_poc`).

All `file:line` references are in `stellar-private-payments/`.

---

## 1. Field

- BN254 scalar field. PoC type `FpBN256 = Fp256<MontBackend<FqConfig, 4>>`,
  modulus `21888242871839275222246405745257275088548364400416034343698204186575808495617`
  — `poseidon2/src/fields/bn256.rs:12-16`.
- `zk-core` uses `ark_bn254::Fr` (same modulus, same Montgomery backend, same
  `ark-ff` 0.6.0 crate the PoC uses → field arithmetic is byte-identical).
  `crates/zk-core/Cargo.toml` pins `ark-bn254 = 0.6.0`, `ark-ff = 0.6.0`.

## 2. Poseidon2 instances (HorizenLabs `zkhash`)

Three instances are used; **all** `d = 5, RF = 8, RP = 56` (64 rounds):

| t | params constant | constructor `file:line` |
|---|---|---|
| 2 | `POSEIDON2_BN256_PARAMS_2` | `poseidon2/src/poseidon2/poseidon2_instance_bn256.rs:282-284` → `Poseidon2Params::new(2, 5, 8, 56, …)` |
| 3 | `POSEIDON2_BN256_PARAMS_3` | `…:632-634` → `new(3, 5, 8, 56, …)` |
| 4 | `POSEIDON2_BN256_PARAMS_4` | `…:1056-1058` → `new(4, 5, 8, 56, …)` |

`Poseidon2Params::new(t, d, rounds_f, rounds_p, …)` — `poseidon2/src/poseidon2/poseidon2_params.rs:21-45`
(stores `rounds_f_div_2 = rounds_f/2`, `rounds = rounds_f + rounds_p`).

### Permutation structure (`poseidon2/src/poseidon2/poseidon2.rs:21-49`)

1. `matmul_external` (linear layer at the beginning) — `:28`.
2. `rounds_f_div_2` (= 4) **full** rounds: `add_rc(all lanes)`, `sbox(all lanes)`,
   `matmul_external` — `:30-34`.
3. `rounds_p` (= 56) **partial** rounds: `state[0] += round_constants[r][0]`,
   `sbox_p(state[0])`, `matmul_internal(diag)` — `:37-41`. **Only lane 0** gets a
   round constant and the sbox during partial rounds.
4. `rounds_f_div_2` (= 4) **full** rounds — `:43-47`.

- sbox d = 5 is `x^5` via `square; square; *x` — `sbox_p`, `:55-82` (the `5 =>`
  arm `:65-70`).
- `matmul_external`: t=2 circ(2,1), t=3 circ(2,1,1), t=4 cheap M4 (`matmul_m4`) —
  `:118-162` (M4 at `:84-116`).
- `matmul_internal`: t=2 `[[2,1],[1,3]]`, t=3 `[[2,1,1],[1,2,1],[1,1,3]]`,
  t=4 `sum + diag[i]*state[i]` — `:164-207`.

**`zk-core` port:** `crates/zk-core/src/poseidon2.rs` (`permutation`, `full_round`,
`matmul_external`, `matmul_m4`, `matmul_internal`, `sbox_d5`). KAT for t=3
`perm([0,1,2])` matches the upstream test (`poseidon2.rs:528-548`):
`perm[0] = 0x0bb61d24…4f4a3033` — checked by `poseidon2::tests::kat_t3_matches_upstream`.

### Constants

Internal-diagonal matrices and the full `64 × t` round-constant tables are the
**live upstream constants**, extracted byte-exact (see §6) into
`crates/zk-core/src/poseidon2_constants.rs`:
- `MAT_DIAG2_M_1 = [1, 2]` (`…bn256.rs:10-13`), `MAT_DIAG3_M_1 = [1, 1, 2]`
  (`:288-292`), `MAT_DIAG4_M_1 = [0x10dc…, 0x0c28…, 0x15ac…, 0x8b42…]` (`:638-…`).
- `RC2`/`RC3`/`RC4`: 64 rows each. Constants in the file are written as
  big-endian hex parsed by `from_hex` = `F::from_be_bytes_mod_order`
  (`poseidon2/src/fields/utils.rs:4-7`). `zk-core` stores them as 32-byte LE and
  rebuilds via `Fr::from_le_bytes_mod_order` — same field element. During RP
  rounds only lane 0 is read; the upstream table still stores all `t` lanes per
  row (lanes ≥ 1 are zero for partial rows), and the port keeps the full table.

## 3. Note crypto (domains + formulas)

All `Poseidon2_t*([…])[0]` (lane 0). Sources: `prover/src/crypto.rs`, mirrored by
`circuits/src/test/utils/{keypair,transaction}.rs` and used end-to-end in
`prover/src/notes.rs`.

| value | formula | domain | `file:line` |
|---|---|---|---|
| `pubkey` | `Poseidon2_t3([priv_key, 0, 3])[0]` | 3 | `crypto.rs:167-169` (`derive_public_key_internal`), uses `PARAMS_3` via `poseidon2_hash2_internal` `:37-45` |
| `commitment` | `Poseidon2_t4([amount, pubkey, blinding, 1])[0]` | 1 | `crypto.rs:100-108`, uses `PARAMS_4` via `poseidon2_hash3_internal` `:50-63` |
| `signature` | `Poseidon2_t4([priv_key, commitment, path_indices, 4])[0]` | 4 | `crypto.rs:111-122` |
| `nullifier` | `Poseidon2_t4([commitment, path_indices, signature, 2])[0]` | 2 | `crypto.rs:127-139` |

- Two-step nullifier (sign then nullify) wired in `prover/src/notes.rs:70-77`.
- `path_indices` = leaf index as field element: u64 packed into low 8 LE bytes of
  a 32-byte buffer — `prover/src/notes.rs:67-68`. Equals `Fr::from(leaf_index)`.

**`zk-core` port:** `crates/zk-core/src/note.rs` with named domain constants
`DOMAIN_PUBKEY=3`, `DOMAIN_COMMITMENT=1`, `DOMAIN_SIGNATURE=4`,
`DOMAIN_NULLIFIER=2`.

## 4. Merkle (binary, feed-forward, t = 2)

- Node = `Poseidon2_t2_perm([left, right])[0] + left` (feed-forward adds the
  **left** input) — `circuits/src/core/merkle.rs:18-23` (`perm[0].add(input[0])`).
  Identical compression in `crypto.rs:68-74`.
- Proof verification orientation: `is_right = (indices >> level) & 1`; if right,
  `compress(sibling, current)`, else `compress(current, sibling)` —
  `circuits/src/core/merkle.rs:132-144` (test `test_merkle_proof_verifies`).
- Leaf = note commitment. Zero (padding) leaf =
  `0x2530228819993503449741838331630db53abbef0f857575334eed36e0118f9ce` (BE) =
  `ZERO_LEAF_BYTES` — `crypto.rs:28-31`.

**`zk-core` port:** `crates/zk-core/src/merkle.rs` (`compress`,
`root_from_path`).

## 5. Serialization — 32-byte LITTLE-ENDIAN

- `scalar_to_bytes` / `prime_field_to_bytes` = `into_bigint().to_bytes_le()`
  padded to `FIELD_SIZE = 32` — `prover/src/serialization.rs:26-39,57-59`
  (`FIELD_SIZE` = `prover/src/types.rs:8`).
- `bytes_to_scalar` = `from_le_bytes_mod_order` — `serialization.rs:15-23,52-54`.
- **The s1/02 plan text once said "Fr = 32-byte big-endian" — that is WRONG.**
  The PoC is little-endian; byte-identity requires **LE**. `zk-core` uses LE
  (`note::fr_to_le_bytes` / `note::fr_from_le_bytes`).

## 6. Golden-vector provenance (NEVER invented)

Golden vectors in `golden/poc_vectors.json` were produced by running the PoC's
**own** Rust, not hand-computed:

- **Constants** (`poseidon2_constants.rs`): a temporary in-crate dumper inside the
  PoC `zkhash` crate read the `pub(crate)` `Poseidon2Params` fields and printed
  every constant as its 32-byte LE encoding; output transformed verbatim into the
  Rust `const` arrays. The dumper asserted the upstream KAT (t=3 `perm([0,1,2])[0]
  = 0x0bb61d24…4f4a3033`) before printing, proving the constants are live. The
  dumper + its one-line `lib.rs` hook were **reverted** after extraction (the
  PoC `zkhash` crate is unmodified); provenance is preserved by the KAT, which
  also runs inside `zk-core`.
- **Vectors** (`golden/poc_vectors.json`): generated by
  `stellar-private-payments/app/crates/core/prover/src/golden_s1_02.rs`
  (an ADDITIVE, `#[cfg(test)]`-only module; existing PoC logic untouched). It
  calls only confirmed-public PoC APIs — `crypto::{derive_public_key,
  compute_commitment, compute_signature, compute_nullifier}`,
  `serialization::{scalar_to_bytes, u64_to_field_bytes, bytes_to_scalar}`, and
  `circuits::core::merkle::{merkle_root, merkle_proof}` — to emit real
  `(amount, priv_key, blinding, leaf_index, pubkey, commitment, signature,
  nullifier, merkle_root, path_elements)` tuples as JSON.
  Run: `cargo test -p prover --lib golden_s1_02 -- --nocapture` (in the PoC),
  slice between `<<<GOLDEN_JSON_BEGIN>>>` / `<<<GOLDEN_JSON_END>>>`.

  **Fixed inputs:** tree depth 3 (8 leaves, mirrors N_demo=8); unused leaves =
  zero leaf. Four notes:
  | label | amount | leaf_index | priv_key / blinding |
  |---|---|---|---|
  | note0 | 1000000 | 0 | deterministic fixed LE (seed 1) |
  | note1 | 42 | 1 | deterministic fixed LE (seed 2) |
  | note2 | 7 | 3 | deterministic fixed LE (seed 3) |
  | note3 | 999999999 | 6 | deterministic fixed LE (seed 4) |

  Resulting Merkle root (LE): `0x9e24c3e7b5c329b34a58f05a9840a90f051d6e5c97833c1d356f81323ef52119`.

## 7. EDGE-input golden vectors (s1/02b security follow-up)

**Why:** the four §6 vectors all use field inputs whose LE high byte is 0, so they
are always `< r` and `from_le_bytes_mod_order` never reduces. A random 32-byte LE
scalar is `>= r` ~80% of the time and IS reduced. The **modular-reduction path**
inside `compute_commitment` / `compute_signature` / `compute_nullifier` (each input
goes through `bytes_to_scalar = from_le_bytes_mod_order`) was therefore untested.
These vectors test exactly that. A divergence here would mean the cryptographic
foundation cracks; the gate surfaces it byte-for-byte rather than papering over it.

**Provenance (NEVER invented):** generated by
`stellar-private-payments/app/crates/core/prover/src/golden_s1_02_edge.rs` — an
ADDITIVE, `#[cfg(test)]`-only sibling of `golden_s1_02.rs`; existing PoC logic
untouched, the `zkhash` / `poseidon2` crate unmodified. It calls only confirmed-
public PoC APIs (`crypto::{derive_public_key, compute_commitment, compute_signature,
compute_nullifier}`, the modulus const `crypto::BN256_MOD_BYTES`,
`serialization::u64_to_field_bytes`) and emits real
`(amount_le, priv_key_le, blinding_le, leaf_index, pubkey, commitment, signature,
nullifier)` tuples as JSON. Frozen into `golden/poc_vectors_edge.json`.
Run: `cargo test -p prover --lib golden_s1_02_edge -- --nocapture` (in the PoC),
slice between `<<<GOLDEN_EDGE_JSON_BEGIN>>>` / `<<<GOLDEN_EDGE_JSON_END>>>`.

**No Merkle tree:** edge vectors target only the scalar-reduction path, so they
omit path/root and the cross-check compares pubkey + commitment + signature +
nullifier (byte-exact).

**Edge inputs (3 vectors), with `r = BN254 scalar modulus`
(LE `0x010000f093f5e1439170b97948e833285d588181b64550b829a031e1724e6430`):**

| label | amount_le (raw) | priv_key_le (raw) | blinding_le (raw) | reduced? |
|---|---|---|---|---|
| `edge_near_modulus_below` | `r-1` | `r-1` | `r-1` | no — largest canonical element (`< r`) |
| `edge_over_modulus_reduce` | `0xFF·32` (2^256-1) | `r` (→ 0) | `r+7` (→ 7) | YES — every input `>= r` |
| `edge_random_seed_0xC0FFEE` | SplitMix64 draw | SplitMix64 draw | SplitMix64 draw | YES — uniform 256-bit, typically `>= r` |

The "randomized" vector uses a self-contained, no_std, hardcoded-seed PRNG
(SplitMix64, seed `0x00C0FFEE`) instead of `rand::StdRng` — deterministic and
algorithm-stable across toolchains (StdRng's stream is not a stability guarantee),
and adds no dependency to the `#![no_std]` PoC crate. Reproducibility is anchored
by the frozen JSON regardless: the SAME raw bytes feed both the PoC and zk-core.

**Cross-check (CRITICAL parse detail):** `crates/zk-core/tests/crosscheck_poc_edge.rs`
parses the edge INPUT scalars with `note::fr_from_le_bytes` =
`Fr::from_le_bytes_mod_order` (mod-order reduction, matching the PoC's
`bytes_to_scalar`) — NOT a strict canonical deserialize, which would REJECT the
`>= r` inputs. Outputs are canonical (`< r`) and compared byte-exact. The test also
(a) asserts at least one raw input is `>= r` so the reduction path is genuinely
exercised (not vacuous), and (b) pins that `r → 0` and `r+7 → 7` under
`fr_from_le_bytes`. Gate: `cargo test -p zk-core --test crosscheck_poc_edge`.

**Result (frozen edge outputs, LE):**
| label | commitment_le | nullifier_le |
|---|---|---|
| `edge_near_modulus_below` | `0x9baccff6f19a27d478ba40ee9642bf3e36485d91a601ca2f1e0a72f6b37b7f0f` | `0x7b95d5b3bd21eda3622fe81781caf6788eee9a3a835375e260e38328c1f1a12c` |
| `edge_over_modulus_reduce` | `0xe223f77be2099128e34777c895268fd9fe121e8ba15e12ee8b86de0236289915` | `0xcdbb98e0b8d1d6cfb946e2e37c68234215c385a478a41cb30a6a9e336141520b` |
| `edge_random_seed_0xC0FFEE` | `0x0312c4e2032830f59d615ee73eef96638cd48e24cb649bb347158c6beebf970c` | `0xa64b95ecf9a0682acaf6aebc2a3e5300e8ec0065140f574542e1dba3d3fec323` |

All three reproduce **byte-identical** in zk-core (pubkey + commitment + signature
+ nullifier). Verdict: byte-identity holds on the reduction path. (Negative control:
flipping one expected byte makes the gate fail loudly with `DIVERGENCE`.)
