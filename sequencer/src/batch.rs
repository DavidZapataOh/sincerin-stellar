//! Batch assembly — turning reserved [`WithdrawalIntent`]s into the exact
//! `zk_core::witness::GuestInput` the real prover consumes (LOCK 3:
//! byte-compatible), plus the inputs-file JSON the host `prove`/`execute`
//! subcommands parse.
//!
//! The sequencer NEVER invents a second serialization. It constructs
//! `GuestInput`/`NoteWitness` directly (the same struct, same 32B-LE field
//! encoding) and, when handing off to the prover or the byte-compat oracle,
//! emits the SAME inputs-file JSON shape as `golden/*_inputs.json`. If
//! `host execute` accepts the JSON we emit, the real prover would accept the
//! same witness — that is the strict proof lock 3 demands.

use zk_core::note::{self, Note};
use zk_core::poseidon2::Fr;
use zk_core::witness::{GuestInput, NoteWitness};

use crate::types::WithdrawalIntent;

/// Build a `NoteWitness` (the prover wire type) from a user's intent.
pub fn note_witness_of(intent: &WithdrawalIntent) -> NoteWitness {
    NoteWitness {
        secret: intent.secret,
        blinding: intent.blinding,
        amount: intent.amount,
        recipient: intent.recipient,
        path: intent.path.clone(),
        index: intent.index,
    }
}

/// Build the `GuestInput` for a batch of intents. All intents MUST share one
/// `merkle_root` (they prove membership against the same pool root); this is the
/// caller's invariant — see [`crate::Sequencer`], which only batches same-root
/// intents.
///
/// The merkle_root is taken from the first intent; callers guarantee all match.
pub fn guest_input_of(intents: &[WithdrawalIntent]) -> GuestInput {
    let notes = intents.iter().map(note_witness_of).collect();
    let merkle_root = intents.first().map(|i| i.merkle_root).unwrap_or([0u8; 32]);
    GuestInput { notes, merkle_root }
}

/// Compute the nullifier (32B LE) of an intent — the lock key (lock-by-nullifier)
/// and the on-chain `is_spent` key. Derived via `zk_core::note` (byte-identical to
/// the PoC / the guest), so the sequencer's idea of "this note's nullifier" equals
/// what ends up in the journal and what `settle_batch` marks spent.
pub fn nullifier_of(intent: &WithdrawalIntent) -> [u8; 32] {
    let secret = note::fr_from_le_bytes(&intent.secret);
    let blinding = note::fr_from_le_bytes(&intent.blinding);
    let note = Note {
        amount: Fr::from(intent.amount),
        priv_key: secret,
        blinding,
        leaf_index: intent.index,
    };
    let nf = note::nullifier(&note, secret);
    note::fr_to_le_bytes(&nf)
}

/// Serialize a `GuestInput` to the inputs-file JSON the host `prove`/`execute`
/// subcommands parse (`host/src/main.rs` `InputsFile`): `merkle_root_le` + `notes`
/// with `secret_le`/`blinding_le`/`amount`/`recipient`/`index`/`path_le`, all
/// `0x…` 32-byte LITTLE-ENDIAN hex.
///
/// This is the bridge used by BOTH the `LocalProver` (to feed the real prover) and
/// the gate's byte-compat oracle (`host execute`): if `host execute` accepts this
/// JSON, the witness is byte-compatible with the real prover (LOCK 3).
pub fn inputs_file_json(input: &GuestInput) -> String {
    let mut s = String::new();
    s.push_str("{\n");
    s.push_str(
        "  \"description\": \"sequencer-built batch input (zk_core::witness::GuestInput, 32B-LE). host execute on this == prover acceptance (lock 3).\",\n",
    );
    s.push_str(&format!("  \"n\": {},\n", input.notes.len()));
    s.push_str(&format!(
        "  \"merkle_root_le\": \"0x{}\",\n",
        hex::encode(input.merkle_root)
    ));
    s.push_str("  \"notes\": [\n");
    for (i, w) in input.notes.iter().enumerate() {
        s.push_str("    {\n");
        s.push_str(&format!("      \"label\": \"note{i}\",\n"));
        s.push_str(&format!(
            "      \"secret_le\": \"0x{}\",\n",
            hex::encode(w.secret)
        ));
        s.push_str(&format!(
            "      \"blinding_le\": \"0x{}\",\n",
            hex::encode(w.blinding)
        ));
        s.push_str(&format!("      \"amount\": {},\n", w.amount));
        s.push_str(&format!(
            "      \"recipient\": \"0x{}\",\n",
            hex::encode(w.recipient)
        ));
        s.push_str(&format!("      \"index\": {},\n", w.index));
        s.push_str("      \"path_le\": [\n");
        for (k, sib) in w.path.iter().enumerate() {
            let comma = if k + 1 < w.path.len() { "," } else { "" };
            s.push_str(&format!("        \"0x{}\"{}\n", hex::encode(sib), comma));
        }
        s.push_str("      ]\n");
        let comma = if i + 1 < input.notes.len() { "," } else { "" };
        s.push_str(&format!("    }}{comma}\n"));
    }
    s.push_str("  ]\n");
    s.push_str("}\n");
    s
}
