//! s2/01 — Journal interop: guest bytes == contract bytes (SECURITY-CRITICAL).
//!
//! Proves that the Soroban `rollup::journal::decode` (no_std, soroban-sdk, raw
//! bytes) reconstructs the *exact* `{ root, nullifiers, payouts }` that the
//! RISC Zero guest committed — the canonical layout OWNED by
//! `zk-core::journal::encode` (CONTEXT.md D3 / AC3.1).
//!
//! A mismatch here breaks the funds logic (wrong recipient / wrong amount / wrong
//! nullifier ⇒ stolen or stuck funds), so the assertions below are byte-exact,
//! not a tautology:
//!   - The expected values are read straight out of the frozen real journal
//!     (`golden/journal_n2.bin`, produced by the guest in s1/04), AND
//!   - cross-checked against the authoritative host decoder
//!     `zk-core::journal::decode`, so the two independent decoders (wasm-side raw
//!     bytes vs host-side `Fr`) must agree on every byte.

use rollup::journal;
use soroban_sdk::{Bytes, Env};

/// The frozen, real guest journal — N=2, 196 bytes. Baked at compile time so the
/// test is hermetic (no runtime file IO). `CARGO_MANIFEST_DIR` = `contracts/rollup`.
const GOLDEN_N2: &[u8] = include_bytes!("../../../golden/journal_n2.bin");

/// The known plaintext of the golden journal (from s1/04's guest run).
/// Amounts are in the clear (CONTEXT.md D3/AC3.3).
const EXPECTED_N: u32 = 2;
const EXPECTED_AMOUNTS: [i128; 2] = [1_000_000, 42];

#[test]
fn golden_is_the_expected_size() {
    // root(32) + N(4) + N*(nullifier 32 + recipient 32 + amount 16) = 36 + 2*80.
    assert_eq!(GOLDEN_N2.len(), 36 + (EXPECTED_N as usize) * 80);
    assert_eq!(GOLDEN_N2.len(), 196);
}

#[test]
fn contract_decodes_guest_journal_exactly() {
    let env = Env::default();
    let bytes = Bytes::from_slice(&env, GOLDEN_N2);

    let decoded = journal::decode(&env, &bytes).expect("contract must decode the real guest journal");

    // ── N ──────────────────────────────────────────────────────────────────────
    assert_eq!(decoded.nullifiers.len(), EXPECTED_N, "nullifier count");
    assert_eq!(decoded.payouts.len(), EXPECTED_N, "payout count");

    // ── root: verbatim 32-byte LE chunk at offset 0 ─────────────────────────────
    let expected_root = &GOLDEN_N2[0..32];
    assert_eq!(decoded.root.to_array().as_slice(), expected_root, "root bytes");

    // ── nullifiers: verbatim 32-byte LE chunks at offset 36 + i*32 ──────────────
    for i in 0..EXPECTED_N as usize {
        let off = 36 + i * 32;
        let expected_nf = &GOLDEN_N2[off..off + 32];
        let got = decoded.nullifiers.get(i as u32).unwrap();
        assert_eq!(got.to_array().as_slice(), expected_nf, "nullifier[{i}] bytes");
    }

    // ── payouts: recipient (32B verbatim) ‖ amount (u128 LE → i128) ─────────────
    // Indexed loop on purpose: `i` indexes three parallel sources (the raw byte
    // buffer at computed offsets, `decoded.payouts`, and `EXPECTED_AMOUNTS`); the
    // explicit offset arithmetic is what makes the byte-equality auditable.
    let payouts_base = 36 + (EXPECTED_N as usize) * 32;
    #[allow(clippy::needless_range_loop)]
    for i in 0..EXPECTED_N as usize {
        let off = payouts_base + i * 48;
        let expected_recipient = &GOLDEN_N2[off..off + 32];
        let (recipient, amount) = decoded.payouts.get(i as u32).unwrap();
        assert_eq!(
            recipient.to_array().as_slice(),
            expected_recipient,
            "payout[{i}] recipient bytes",
        );
        assert_eq!(amount, EXPECTED_AMOUNTS[i], "payout[{i}] amount");

        // And the amount must equal the raw little-endian u128 in the file.
        let mut amt_le = [0u8; 16];
        amt_le.copy_from_slice(&GOLDEN_N2[off + 32..off + 48]);
        let raw = u128::from_le_bytes(amt_le);
        assert_eq!(raw, EXPECTED_AMOUNTS[i] as u128, "payout[{i}] raw LE amount");
    }
}

/// Independent cross-check: the contract's raw-byte decoder must agree, field by
/// field, with the AUTHORITATIVE host decoder `zk-core::journal::decode` (which
/// parses into `Fr`). This is what makes the test non-tautological — two
/// independently written decoders over the same bytes must produce equal results.
#[test]
fn contract_matches_zk_core_host_decoder() {
    use zk_core::note::fr_to_le_bytes;

    // Authoritative host-side decode (Fr-based).
    let host = zk_core::journal::decode(GOLDEN_N2).expect("zk-core decodes the golden");

    // Contract-side decode (raw soroban Bytes).
    let env = Env::default();
    let bytes = Bytes::from_slice(&env, GOLDEN_N2);
    let con = journal::decode(&env, &bytes).expect("contract decodes the golden");

    // root: host Fr re-encoded to 32B LE must equal the contract's verbatim chunk.
    assert_eq!(con.root.to_array(), fr_to_le_bytes(&host.root), "root mismatch host vs contract");

    assert_eq!(con.nullifiers.len() as usize, host.nullifiers.len());
    for (i, nf) in host.nullifiers.iter().enumerate() {
        let got = con.nullifiers.get(i as u32).unwrap();
        assert_eq!(got.to_array(), fr_to_le_bytes(nf), "nullifier[{i}] mismatch host vs contract");
    }

    assert_eq!(con.payouts.len() as usize, host.payouts.len());
    for (i, p) in host.payouts.iter().enumerate() {
        let (recipient, amount) = con.payouts.get(i as u32).unwrap();
        assert_eq!(recipient.to_array(), p.recipient, "recipient[{i}] mismatch host vs contract");
        // u128 amount in the host decoder, i128 in the contract (token transfer type).
        assert_eq!(amount as u128, p.amount, "amount[{i}] mismatch host vs contract");
    }
}

#[test]
fn rejects_buffer_shorter_than_header() {
    let env = Env::default();
    // 10 bytes: shorter than the 36-byte header.
    let short = Bytes::from_slice(&env, &[0u8; 10]);
    let err = journal::decode(&env, &short).unwrap_err();
    assert_eq!(err, journal::JournalError::TooShortForHeader);
}

#[test]
fn rejects_length_mismatch_trailing_bytes() {
    let env = Env::default();
    // The real 196-byte journal with one extra trailing byte ⇒ must be rejected.
    let mut buf = GOLDEN_N2.to_vec();
    buf.push(0xFF);
    let bytes = Bytes::from_slice(&env, &buf);
    let err = journal::decode(&env, &bytes).unwrap_err();
    assert_eq!(err, journal::JournalError::LengthMismatch);
}

#[test]
fn rejects_truncated_payload() {
    let env = Env::default();
    // Header claims N=2 but the body is one byte short ⇒ reject (no silent trunc).
    let buf = &GOLDEN_N2[..GOLDEN_N2.len() - 1];
    let bytes = Bytes::from_slice(&env, buf);
    let err = journal::decode(&env, &bytes).unwrap_err();
    assert_eq!(err, journal::JournalError::LengthMismatch);
}
