//! Canonical journal codec — `{ merkle_root, [nullifier_i], [(recipient_i, amount_i)] }`.
//!
//! **This module OWNS the canonical layout.** s2/01 (`journal-interop`) only
//! *decodes* these bytes; it never redefines the layout. Realizes CONTEXT.md
//! **D3 / AC3.1** (the journal contains *exactly* these fields).
//!
//! The guest commits these raw bytes verbatim (`env::commit_slice`), so the
//! Soroban `settle_batch` contract reads `receipt.journal` as exactly this
//! buffer — no extra serde framing in the wire.
//!
//! ## Byte layout (length-prefixed, deterministic, unambiguous)
//! ```text
//!   offset  size      field
//!   0       32        root          (Fr, 32B LITTLE-ENDIAN)
//!   32      4         N             (u32 LITTLE-ENDIAN) — count of nullifiers = count of payouts
//!   36      N*32      nullifiers    (N × Fr, 32B LE each)
//!   36+N*32 N*48      payouts       (N × [ recipient(32B) ‖ amount(u128, 16B LE) ])
//! ```
//! Total length = `36 + N*32 + N*48 = 36 + N*80` bytes. The single `N`
//! prefix governs both the nullifier and payout arrays (they are always
//! equal-length: one nullifier and one payout per aggregated withdrawal), which
//! makes the layout self-describing and exactly decodable.
//!
//! Field elements use the project-wide **little-endian** convention
//! (`note::fr_to_le_bytes`, matching the PoC `scalar_to_bytes`). `amount` uses
//! native `u128::to_le_bytes`.

extern crate alloc;
use alloc::vec::Vec;

use crate::note;
use crate::poseidon2::Fr;

/// One payout: an opaque 32-byte recipient address and an in-claro `u128` amount.
///
/// Amounts are NOT hidden in this design (CONTEXT.md D3/AC3.3 — "confidential" =
/// unlinkable counterparties, amounts in the clear).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Payout {
    /// Opaque recipient address (e.g. a Stellar account / contract id), 32 bytes.
    pub recipient: [u8; 32],
    /// Transfer amount, in the clear.
    pub amount: u128,
}

/// Size in bytes of one encoded payout: `recipient(32) + amount(16)`.
pub const PAYOUT_SIZE: usize = 32 + 16;
/// Size in bytes of the fixed header: `root(32) + N(4)`.
pub const HEADER_SIZE: usize = 32 + 4;

/// Encode the journal `{ root, nullifiers, payouts }` into its canonical bytes.
///
/// # Panics
/// Panics if `nullifiers.len() != payouts.len()` — the two arrays are bound to
/// the same `N` (one nullifier and one payout per withdrawal); a mismatch is a
/// caller bug, not a runtime input condition.
pub fn encode(root: Fr, nullifiers: &[Fr], payouts: &[Payout]) -> Vec<u8> {
    assert_eq!(
        nullifiers.len(),
        payouts.len(),
        "journal: nullifier count ({}) must equal payout count ({})",
        nullifiers.len(),
        payouts.len()
    );
    let n = nullifiers.len();
    let mut out = Vec::with_capacity(HEADER_SIZE + n * (32 + PAYOUT_SIZE));

    // root (32B LE)
    out.extend_from_slice(&note::fr_to_le_bytes(&root));
    // N (u32 LE) — length prefix governing both arrays.
    out.extend_from_slice(&(n as u32).to_le_bytes());
    // nullifiers (N × 32B LE)
    for nf in nullifiers {
        out.extend_from_slice(&note::fr_to_le_bytes(nf));
    }
    // payouts (N × [recipient 32B ‖ amount u128 16B LE])
    for p in payouts {
        out.extend_from_slice(&p.recipient);
        out.extend_from_slice(&p.amount.to_le_bytes());
    }
    debug_assert_eq!(out.len(), HEADER_SIZE + n * (32 + PAYOUT_SIZE));
    out
}

/// The decoded journal — the mirror of [`encode`], for s2 and tests.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodedJournal {
    /// Merkle root the batch was proven against.
    pub root: Fr,
    /// One nullifier per aggregated withdrawal.
    pub nullifiers: Vec<Fr>,
    /// One payout per aggregated withdrawal (same order as `nullifiers`).
    pub payouts: Vec<Payout>,
}

/// Errors from [`decode`] — surfaced rather than panicking so the on-chain
/// decoder can reject a malformed journal explicitly.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JournalError {
    /// Buffer shorter than the fixed header (root + N).
    TooShortForHeader,
    /// Buffer length does not match `HEADER_SIZE + N*(32 + PAYOUT_SIZE)` for the
    /// declared `N`; the trailing carries `(expected_len, actual_len)`.
    LengthMismatch(usize, usize),
}

/// Decode canonical journal bytes back into `{ root, nullifiers, payouts }`.
///
/// This is the authoritative mirror of [`encode`]; s2/01 uses the same logic to
/// read the on-chain journal. Strict: the buffer length must match the declared
/// `N` exactly (no trailing bytes), so a malformed journal is rejected, not
/// silently truncated.
pub fn decode(bytes: &[u8]) -> Result<DecodedJournal, JournalError> {
    if bytes.len() < HEADER_SIZE {
        return Err(JournalError::TooShortForHeader);
    }
    let mut root_le = [0u8; 32];
    root_le.copy_from_slice(&bytes[0..32]);
    let root = note::fr_from_le_bytes(&root_le);

    let mut n_bytes = [0u8; 4];
    n_bytes.copy_from_slice(&bytes[32..36]);
    let n = u32::from_le_bytes(n_bytes) as usize;

    let expected = HEADER_SIZE + n * (32 + PAYOUT_SIZE);
    if bytes.len() != expected {
        return Err(JournalError::LengthMismatch(expected, bytes.len()));
    }

    let mut nullifiers = Vec::with_capacity(n);
    let mut off = HEADER_SIZE;
    for _ in 0..n {
        let mut nf = [0u8; 32];
        nf.copy_from_slice(&bytes[off..off + 32]);
        nullifiers.push(note::fr_from_le_bytes(&nf));
        off += 32;
    }

    let mut payouts = Vec::with_capacity(n);
    for _ in 0..n {
        let mut recipient = [0u8; 32];
        recipient.copy_from_slice(&bytes[off..off + 32]);
        off += 32;
        let mut amt = [0u8; 16];
        amt.copy_from_slice(&bytes[off..off + 16]);
        off += 16;
        payouts.push(Payout {
            recipient,
            amount: u128::from_le_bytes(amt),
        });
    }

    Ok(DecodedJournal {
        root,
        nullifiers,
        payouts,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fr(n: u64) -> Fr {
        Fr::from(n)
    }

    #[test]
    fn encode_decode_roundtrip_n2() {
        let root = fr(0xdead_beef);
        let nullifiers = [fr(11), fr(22)];
        let payouts = [
            Payout { recipient: [0xAA; 32], amount: 1_000_000 },
            Payout { recipient: [0xBB; 32], amount: 42 },
        ];
        let bytes = encode(root, &nullifiers, &payouts);
        assert_eq!(bytes.len(), HEADER_SIZE + 2 * (32 + PAYOUT_SIZE));

        let d = decode(&bytes).expect("decode");
        assert_eq!(d.root, root);
        assert_eq!(d.nullifiers, nullifiers);
        assert_eq!(&d.payouts[..], &payouts[..]);
    }

    #[test]
    fn layout_offsets_are_exact() {
        let root = fr(7);
        let nf = [fr(1)];
        let p = [Payout { recipient: [0xCD; 32], amount: 0x0102_0304_0506_0708 }];
        let b = encode(root, &nf, &p);
        // root | N=1 | nullifier(1) | recipient | amount
        assert_eq!(&b[0..32], &note::fr_to_le_bytes(&root));
        assert_eq!(&b[32..36], &1u32.to_le_bytes());
        assert_eq!(&b[36..68], &note::fr_to_le_bytes(&fr(1)));
        assert_eq!(&b[68..100], &[0xCDu8; 32]);
        assert_eq!(&b[100..116], &0x0102_0304_0506_0708u128.to_le_bytes());
        assert_eq!(b.len(), 116);
    }

    #[test]
    fn decode_rejects_short_header() {
        assert_eq!(decode(&[0u8; 10]), Err(JournalError::TooShortForHeader));
    }

    #[test]
    fn decode_rejects_length_mismatch() {
        // header claims N=2 but buffer is header-only.
        let mut b = Vec::new();
        b.extend_from_slice(&[0u8; 32]);
        b.extend_from_slice(&2u32.to_le_bytes());
        match decode(&b) {
            Err(JournalError::LengthMismatch(exp, act)) => {
                assert_eq!(exp, HEADER_SIZE + 2 * (32 + PAYOUT_SIZE));
                assert_eq!(act, HEADER_SIZE);
            }
            other => panic!("expected LengthMismatch, got {other:?}"),
        }
    }

    #[test]
    #[should_panic(expected = "must equal payout count")]
    fn encode_rejects_count_mismatch() {
        let _ = encode(fr(1), &[fr(1), fr(2)], &[Payout { recipient: [0; 32], amount: 1 }]);
    }
}
