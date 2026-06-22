# Sequencer — single-operator MVP (s2/05)

The sequencer is the off-chain operator that turns a stream of withdrawal
intents into one aggregated, on-chain settle. It **reserves notes** (so two
batches can never include the same note), assembles a batch at `N` (or a
timeout), **proves the batch ASYNCHRONOUSLY** (the user never blocks on the
multi-hour prove), pays and sends the on-chain `settle_batch`, and on a
**collision** drops the spent note, re-queues it, and rebuilds the `N−1` batch.

Implements CONTEXT.md **D4 / AC4.3, AC4.4**.

---

## Trust boundary (AC4.4) — read this first

> **The operator SEES the note↔recipient mapping. Unlinkability is ON-CHAIN /
> public, NOT against the operator.**

This rollup uses **Diseño B**: the guest re-executes validity natively in Rust,
so the sequencer receives each note's spending **secret** + Merkle path to build
the witness. That means the single operator necessarily learns which deposit
pays which recipient for the notes it batches.

What "Confidential" means here (consistent with the Privacy-Pools model and
CONTEXT.md D3/AC3.3):

- **Unlinkable counterparties** — to any *on-chain* observer, the deposit↔
  withdrawal link is broken by the anonymity set. That is the public privacy
  guarantee.
- **NOT** privacy *against the operator*. The operator is a trusted party for
  linkability in this MVP. Decentralizing the sequencer (removing this trust) is
  explicitly **out of MVP scope** (CONTEXT.md "Fuera de scope").
- **Amounts are in the clear** — never claim "hidden amounts". The journal
  carries `(recipient, amount)` per withdrawal verbatim.

The operator is also the party that pays the settle fee and is trusted to
include reserved notes fairly (mitigating batch griefing, CONTEXT.md riesgo #3).

## ZK latency — inherent, not a defect

A real Groth16 prove of the aggregated batch takes **hours** (the measured N=8
real prove took ~4h26m). This is **inherent to every ZK rollup**: validity is
proven off-chain and only the succinct receipt is verified on-chain. It is not a
bug — it is exactly the work the aggregation amortizes. A juez verifies the
**settle in seconds on testnet** because the expensive proving already happened
off-chain.

Consequence for the API: **`submit_withdrawal` never blocks on the prove.** It
validates + reserves + enqueues and returns a `request_id` immediately. The user
**polls `get_status(request_id)`** through the state machine:

```
Pending → Batched → Proving (~hours) → Settled{tx_hash} | Failed{reason}
```

On a collision a dropped note's request returns to `Pending` (see below).

---

## Architecture

### State machine + nullifier lock (pure, synchronous)

`Sequencer` holds the mempool (`requests`) and the **lock-by-nullifier**
(`reserved`: nullifier → the request holding it). The state machine, the lock,
batch assembly, and the collision rebuild are **pure synchronous logic**
(exhaustively unit-tested); only the prove is async.

- `submit_withdrawal(intent)` → validate → derive the note's **nullifier**
  (via `zk_core::note`, byte-identical to the guest/PoC) → if that nullifier is
  already reserved by a live request, **reject** (`AlreadyReserved`) so a note
  cannot enter two batches → otherwise reserve + enqueue (`Pending`) and return a
  `request_id`.
- `try_assemble_batch()` / `force_batch(max)` → transition `N` (or up to `max`,
  the timeout path) same-root `Pending` requests to `Batched` and return a
  `Batch`.
- `mark_proving` / `mark_settled{tx_hash}` / `mark_failed{reason}` → drive the
  rest of the lifecycle; settle/fail release the in-memory lock (the nullifiers
  now live in the on-chain `is_spent` set, the real anti-replay guard).

### Prover trait — the only seam to proving hardware

```rust
#[async_trait] trait Prover {
    async fn prove(&self, input: GuestInput) -> Result<ProvedBatch, ProverError>;
}
```

- **`LocalProver`** (production, built always): shells out to `host prove`
  (`RISC0_DEV_MODE=0`, REAL Groth16/BN254). Selected by `PROVER_BACKEND=local`.
- **`RemoteProver`** (s3, NOT built here): POST inputs to a GPU x86 prover, poll
  the receipt. `PROVER_BACKEND=remote` is reserved; the MVP binary rejects it
  with a clear message and **never** falls back to a fixture or dev-mode.
- **`FixtureProver`** (TEST-ONLY, `feature = "test-fixture"`): LOADS the real,
  pre-generated N=8 receipt from `out/bench/n8/{seal,image_id,journal}` —
  it never fabricates a `ProvedBatch`. It is **structurally unreachable** from
  the production binary (see locks below).

Local↔remote is pure config — the sequencer's state machine, lock, and collision
logic are identical regardless of backend, so s3 points at the GPU without
rewriting anything.

### Collision handler (security-critical — touches the fund flow)

`handle_collision(batch, is_spent_le)` takes a batch and the set of nullifiers
already `is_spent` **on-chain** (a real query, not a guess), and:

- **drops** every note whose nullifier is on-chain spent;
- **re-queues** each dropped request to `Pending` (no loss, no duplicate);
- **rebuilds** the surviving `Batch` over the remaining notes.

Invariants (SEC-reviewed, plan §5):

| field         | on collision | why |
|---------------|--------------|-----|
| `merkle_root` | **unchanged** | it is the **pool** root (root history `is_known_root`), not derived from the batch. Surviving membership proofs still verify. |
| surviving paths | **still valid** | same root ⇒ same authentication paths. |
| journal       | **changes**  | `N−1` nullifiers + `N−1` payouts ⇒ a **new** prove (new digest). |
| dropped note  | → `Pending`  | re-queued exactly once; the policy layer (out of MVP) decides retry vs reject. |

The rebuilt `N−1` `GuestInput` is the exact `zk_core::witness::GuestInput` the
real prover consumes (byte-compatible). The gate proves this rigorously by
running `host execute` on the rebuilt input (see lock 3).

---

## The 3 locks (SEC verifies these)

1. **FixtureProver loads the real file, and is unreachable from production.**
   It lives behind `#[cfg(feature = "test-fixture")]` and only ever calls
   `load_proved_batch(out/bench/n8)`. The production binary (`src/main.rs`)
   builds with **no features**, so the fixture module is not even compiled in;
   the demo driver (`src/bin/seq_demo.rs`) has `required-features =
   ["test-fixture"]`, so `cargo build` skips it entirely. Misuse is a compile
   error, not a runtime check.
2. **Honest gate label.** `scripts/seq_demo.sh` certifies **ORCHESTRATION**
   (state machine, lock, trigger, collision) **+ a real on-chain settle of a
   real proof** (the N=8 receipt, the real verifier `CBQF…`). It does **not**
   certify that the sequencer **generates** proofs — that is s3 (a real prove
   *through* the sequencer on GPU). The fixture only replaces the 4-hour prove
   with the already-generated real receipt.
3. **`GuestInput` byte-compatible (strict).** Both the happy-path input and the
   collision-rebuilt `N−1` input are built as `zk_core::witness::GuestInput` and
   serialized with the SAME inputs-file scheme `host prove`/`host execute` parse.
   The gate runs **`host execute` on the rebuilt input** — if the RISC Zero
   executor accepts it (valid journal, correct `N`, membership passes), the real
   prover would too. That is the strict proof; a laxer check is not accepted.

---

## Run

```bash
# Production CLI (selects LocalProver; FixtureProver is not compiled in):
cargo run -p sequencer --bin sequencer

# Gate — fresh deploy + real settle + collision rebuild (testnet):
bash scripts/seq_demo.sh && echo GATE_OK
```

The **on-chain settle of the rebuilt N−1 batch** needs a real re-prove, which on
a Mac does not complete for large N — that runs on GPU in **s3**, never in
dev-mode. This gate proves the plumbing (orchestration) with real artifacts; s3
proves the integration (a real prove through the sequencer).
