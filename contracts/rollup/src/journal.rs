//! On-chain journal decoder (no_std, soroban-sdk) — SECURITY-CRITICAL.
//!
//! Consumes the canonical journal layout **OWNED by `zk-core::journal::encode`**
//! (s1/03); this module never redefines it, it only *reads* it. Realizes
//! CONTEXT.md **D3 / AC3.1** on the contract side: the journal the guest
//! committed is parsed back into exactly `{ root, nullifiers, payouts }`.
//!
//! ## Byte layout (mirror of `zk-core::journal`, length-prefixed, LITTLE-ENDIAN)
//! ```text
//!   offset    size      field
//!   0         32        root          (32B LE, taken verbatim)
//!   32        4         N             (u32 LE) — count of nullifiers == count of payouts
//!   36        N*32      nullifiers    (N × 32B LE, verbatim)
//!   36+N*32   N*48      payouts       (N × [ recipient(32B verbatim) ‖ amount(u128, 16B LE) ])
//! ```
//! Total length = `36 + N*80` bytes (N=2 ⇒ 196).
//!
//! ## Why raw bytes, NOT `Fr`
//! The rollup contract does **no field arithmetic** — it verifies the receipt,
//! checks the root against the pool, marks nullifiers, and transfers. So this
//! decoder deliberately works on raw `soroban_sdk::Bytes` and keeps `ark-bn254`
//! out of the wasm. The 32-byte chunks (root, nullifiers, recipients) are taken
//! **verbatim** (little-endian as stored on the wire), byte-for-byte identical to
//! what the guest emitted via `zk-core::note::fr_to_le_bytes`.
//!
//! ## Recipient representation — DECISION (auditable; SEC-reviewed)
//! The journal recipient is an opaque `[u8; 32]`. This decoder returns it as a
//! **`BytesN<32>` taken verbatim** — it does NOT build an `Address` here. Reasons:
//!   1. **Exact, lossless binding.** The recipient surfaced to the caller is the
//!      *identical* 32 bytes the guest proved against — zero transformation,
//!      trivially auditable (journal bytes ⇒ payout recipient is the identity map).
//!   2. **No raw-key → Address constructor exists in no_std.** soroban-sdk's
//!      `Address::from_string_bytes` expects base-32 *strkey* ("G…/C…") ASCII
//!      bytes, NOT a raw 32-byte ed25519 public key; there is no API to mint an
//!      `Address` from a raw key without re-implementing strkey (checksum +
//!      base32) in the contract, and one cannot tell account (G) from contract
//!      (C) from 32 opaque bytes alone.
//! s2/02 (which owns `token.transfer`) performs the single, explicit
//! `BytesN<32> ⇒ Address` conversion at the transfer site, where the
//! account/contract discriminant and strkey encoding are decided once and
//! auditable. The binding (journal recipient bytes ⇒ the address that receives
//! funds) therefore stays exact end-to-end.
//!
//! ## Amount type
//! Amounts are `u128` on the wire (`zk-core::journal::Payout::amount`). Soroban
//! token transfers use `i128`, so the decoder converts and **rejects any amount
//! whose high bit is set** (would not fit a non-negative `i128`), surfacing
//! `JournalError::AmountOverflow` rather than silently wrapping into a negative
//! transfer.

use soroban_sdk::{Bytes, BytesN, Env, Vec};

/// Size in bytes of the fixed header: `root(32) + N(4)`. Mirrors
/// `zk_core::journal::HEADER_SIZE`.
pub const HEADER_SIZE: u32 = 32 + 4;
/// Size in bytes of one encoded payout: `recipient(32) + amount(16)`. Mirrors
/// `zk_core::journal::PAYOUT_SIZE`.
pub const PAYOUT_SIZE: u32 = 32 + 16;
/// Size in bytes of one nullifier (a field element on the wire).
pub const NULLIFIER_SIZE: u32 = 32;

/// The journal decoded into soroban-sdk types — the on-chain mirror of
/// `zk_core::journal::DecodedJournal`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodedJournal {
    /// Merkle root the batch was proven against (32B LE verbatim).
    pub root: BytesN<32>,
    /// One nullifier per aggregated withdrawal (32B LE verbatim, in order).
    pub nullifiers: Vec<BytesN<32>>,
    /// One payout per aggregated withdrawal, same order as `nullifiers`:
    /// `(recipient bytes verbatim, amount as non-negative i128)`.
    pub payouts: Vec<(BytesN<32>, i128)>,
}

/// Errors from [`decode`]. Surfaced (not panicked) so `settle_batch` can reject a
/// malformed journal explicitly. Strict by design — a wrong length or an
/// out-of-range amount must fail, never be silently coerced.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JournalError {
    /// Buffer shorter than the fixed header (`root + N`).
    TooShortForHeader,
    /// Buffer length does not equal `HEADER_SIZE + N*(NULLIFIER_SIZE + PAYOUT_SIZE)`
    /// for the declared `N` (too short, or trailing bytes).
    LengthMismatch,
    /// A payout amount has its high bit set (does not fit a non-negative `i128`).
    AmountOverflow,
}

/// Decode canonical journal bytes into `{ root, nullifiers, payouts }`.
///
/// Strict mirror of `zk_core::journal::decode`:
///   - rejects a buffer shorter than the 36-byte header,
///   - rejects any buffer whose length != `36 + N*80` (no trailing bytes, no
///     truncation),
///   - takes every 32-byte chunk (root, nullifier, recipient) **verbatim**,
///   - reads `amount` as `u128` little-endian and converts to a non-negative
///     `i128` (rejecting the high-bit-set case).
pub fn decode(env: &Env, bytes: &Bytes) -> Result<DecodedJournal, JournalError> {
    let len = bytes.len();
    if len < HEADER_SIZE {
        return Err(JournalError::TooShortForHeader);
    }

    // N (u32 LE) at offset 32..36.
    let n = u32::from_le_bytes([
        bytes.get_unchecked(32),
        bytes.get_unchecked(33),
        bytes.get_unchecked(34),
        bytes.get_unchecked(35),
    ]);

    // Exact length: header + N*(nullifier + payout). Use u64 to avoid any u32
    // overflow in the multiplication before the equality check.
    let expected = HEADER_SIZE as u64 + n as u64 * (NULLIFIER_SIZE as u64 + PAYOUT_SIZE as u64);
    if len as u64 != expected {
        return Err(JournalError::LengthMismatch);
    }

    // root: verbatim 32B at offset 0..32.
    let root = read_bytes32(env, bytes, 0);

    // nullifiers: N × 32B starting at HEADER_SIZE.
    let mut nullifiers = Vec::new(env);
    let mut off = HEADER_SIZE;
    for _ in 0..n {
        nullifiers.push_back(read_bytes32(env, bytes, off));
        off += NULLIFIER_SIZE;
    }

    // payouts: N × [recipient 32B verbatim ‖ amount u128 16B LE].
    let mut payouts = Vec::new(env);
    for _ in 0..n {
        let recipient = read_bytes32(env, bytes, off);
        off += 32;
        let amount = read_amount_le(bytes, off)?;
        off += 16;
        payouts.push_back((recipient, amount));
    }

    Ok(DecodedJournal {
        root,
        nullifiers,
        payouts,
    })
}

/// Read a verbatim 32-byte chunk at `offset` into a `BytesN<32>`.
///
/// Caller guarantees `offset + 32 <= bytes.len()` (the length was validated
/// against the declared `N` above).
fn read_bytes32(env: &Env, bytes: &Bytes, offset: u32) -> BytesN<32> {
    let mut buf = [0u8; 32];
    bytes.slice(offset..offset + 32).copy_into_slice(&mut buf);
    BytesN::from_array(env, &buf)
}

/// Read a `u128` little-endian amount at `offset` and convert to a non-negative
/// `i128`. Rejects amounts with the high bit set (would not fit `i128 >= 0`).
fn read_amount_le(bytes: &Bytes, offset: u32) -> Result<i128, JournalError> {
    let mut buf = [0u8; 16];
    bytes.slice(offset..offset + 16).copy_into_slice(&mut buf);
    let amount = u128::from_le_bytes(buf);
    // Token transfers use i128; reject anything that does not fit a non-negative
    // i128 rather than wrapping into a negative (would invert a transfer).
    if amount > i128::MAX as u128 {
        return Err(JournalError::AmountOverflow);
    }
    Ok(amount as i128)
}
