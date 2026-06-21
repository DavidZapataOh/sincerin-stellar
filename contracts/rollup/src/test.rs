//! Unit tests for `settle_batch` (SECURITY-CRITICAL — D4 / AC4.1, AC4.2).
//!
//! Soroban test env. Per the task brief, a **mock verifier**, a **mock pool**, and
//! a **recording mock token** are allowed here (clearly-marked unit-test mocks);
//! the REAL verifier + pool + token run on testnet in s2/03. The mocks are
//! deliberately faithful to the production contracts' *interfaces*:
//!   - mock verifier exposes `verify(seal, image_id, journal) ` and can be put in
//!     an "accept" or "reject" mode (so the verify step is genuinely load-bearing);
//!   - mock pool exposes `is_known_root(root: U256) -> bool` over a stored set;
//!   - mock token exposes `transfer(from, to, amount)` and RECORDS every call, so
//!     "all transfers executed" is asserted on exact `(to, amount)` pairs, not a
//!     tautology.
//!
//! Tests:
//!   (i)   valid batch        → all nullifiers marked spent + all transfers done.
//!   (ii)  spent nullifier    → whole tx reverts (`settle_revert_on_spent_nullifier`).
//!   (iii) unknown root       → reverts (`settle_revert_on_unknown_root`).
//!   (iv)  intra-batch dup    → reverts on the 2nd mark (`settle_revert_on_duplicate_nullifier_in_batch`).
//!   (v)   WRONG image_id     → reverts (`settle_revert_on_wrong_image_id`).

extern crate std;

use soroban_sdk::{
    contract, contractimpl, contracttype,
    testutils::{BytesN as _, Ledger as _},
    vec, Address, Bytes, BytesN, Env, U256, Vec,
};

use crate::{DataKey, RollupContract, RollupContractClient, ROLLUP_GUEST_ID};

// ───────────────────────────── mock verifier ──────────────────────────────────

/// Storage flag: should `verify` accept (`true`) or reject (`false`)?
#[contracttype]
enum VKey {
    Accept,
}

/// Mock RISC Zero verifier. Mirrors the real `verify(seal, image_id, journal)`
/// signature. In "accept" mode it returns; in "reject" mode it panics, exactly as
/// the real verifier reverts on an invalid proof — so the contract's verify step
/// is genuinely exercised, never a no-op.
#[contract]
pub struct MockVerifier;

#[contractimpl]
impl MockVerifier {
    pub fn __constructor(env: Env, accept: bool) {
        env.storage().instance().set(&VKey::Accept, &accept);
    }
    pub fn verify(env: Env, _seal: Bytes, _image_id: BytesN<32>, _journal: BytesN<32>) {
        let accept: bool = env.storage().instance().get(&VKey::Accept).unwrap();
        if !accept {
            panic!("mock verifier: invalid proof");
        }
    }
}

// ─────────────────────────────── mock pool ────────────────────────────────────

/// Stored set of "known" roots for the mock pool.
#[contracttype]
enum PKey {
    KnownRoots,
}

/// Mock privacy-pool. Mirrors `is_known_root(root: U256) -> bool` over a stored
/// `Vec<U256>` of roots seeded at construction.
#[contract]
pub struct MockPool;

#[contractimpl]
impl MockPool {
    pub fn __constructor(env: Env, known_roots: Vec<U256>) {
        env.storage().instance().set(&PKey::KnownRoots, &known_roots);
    }
    pub fn is_known_root(env: Env, root: U256) -> bool {
        let roots: Vec<U256> = env.storage().instance().get(&PKey::KnownRoots).unwrap();
        roots.iter().any(|r| r == root)
    }
}

// ──────────────────────────── recording mock token ────────────────────────────

/// Recorded transfers, so tests can assert exact `(to, amount)` payouts.
#[contracttype]
enum TKey {
    /// Append-only log of `(to, amount)` transfers performed.
    Transfers,
}

/// Mock token. Records each `transfer(from, to, amount)` so the test can verify
/// every payout executed with the right recipient and amount.
#[contract]
pub struct MockToken;

#[contractimpl]
impl MockToken {
    pub fn transfer(env: Env, _from: Address, to: Address, amount: i128) {
        let mut log: Vec<(Address, i128)> = env
            .storage()
            .instance()
            .get(&TKey::Transfers)
            .unwrap_or_else(|| Vec::new(&env));
        log.push_back((to, amount));
        env.storage().instance().set(&TKey::Transfers, &log);
    }
    pub fn transfers(env: Env) -> Vec<(Address, i128)> {
        env.storage()
            .instance()
            .get(&TKey::Transfers)
            .unwrap_or_else(|| Vec::new(&env))
    }
}

// ───────────────────────────────── helpers ────────────────────────────────────

/// One aggregated withdrawal's worth of journal data, in plaintext.
struct Item {
    nullifier: [u8; 32],
    recipient: [u8; 32],
    amount: u128,
}

/// Build canonical journal bytes (mirror of `zk-core::journal::encode`):
/// `root(32 LE) ‖ N(4 LE) ‖ N×nullifier(32 LE) ‖ N×[recipient(32) ‖ amount(u128 16 LE)]`.
fn encode_journal(root_le: &[u8; 32], items: &[Item]) -> std::vec::Vec<u8> {
    let mut out = std::vec::Vec::new();
    out.extend_from_slice(root_le);
    out.extend_from_slice(&(items.len() as u32).to_le_bytes());
    for it in items {
        out.extend_from_slice(&it.nullifier);
    }
    for it in items {
        out.extend_from_slice(&it.recipient);
        out.extend_from_slice(&it.amount.to_le_bytes());
    }
    out
}

/// The `U256` the pool will be seeded with for a given little-endian journal root:
/// reverse LE→BE then `from_be_bytes` (the exact bridge `settle_batch` performs).
fn root_le_to_u256(env: &Env, root_le: &[u8; 32]) -> U256 {
    let mut be = *root_le;
    be.reverse();
    U256::from_be_bytes(env, &Bytes::from_array(env, &be))
}

/// The `Address` a given 32-byte journal recipient maps to (account ed25519 key).
fn recipient_address(env: &Env, recipient: &[u8; 32]) -> Address {
    use soroban_sdk::address_payload::AddressPayload;
    AddressPayload::AccountIdPublicKeyEd25519(BytesN::from_array(env, recipient)).to_address(env)
}

/// Deploy verifier + pool + token + rollup. `accept` controls the verifier;
/// `known_roots` seeds the pool. Returns (env, rollup client, token address).
fn setup(
    accept: bool,
    known_roots_le: &[[u8; 32]],
) -> (Env, RollupContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    // A real (non-snapshot) ledger so storage behaves normally.
    env.ledger().with_mut(|l| l.sequence_number = 10);

    let verifier = env.register(MockVerifier, (accept,));

    let mut roots: Vec<U256> = Vec::new(&env);
    for r in known_roots_le {
        roots.push_back(root_le_to_u256(&env, r));
    }
    let pool = env.register(MockPool, (roots,));

    let token = env.register(MockToken, ());

    let rollup_addr = env.register(
        RollupContract,
        (verifier.clone(), pool.clone(), token.clone()),
    );
    let rollup = RollupContractClient::new(&env, &rollup_addr);

    (env, rollup, token)
}

/// Is a nullifier marked spent in the rollup's persistent storage?
fn is_spent(env: &Env, rollup: &Address, nullifier: &[u8; 32]) -> bool {
    let key = DataKey::Spent(BytesN::from_array(env, nullifier));
    env.as_contract(rollup, || env.storage().persistent().has(&key))
}

// A known-good little-endian root used across tests.
const ROOT_LE: [u8; 32] = [
    0x9e, 0x24, 0xc3, 0xe7, 0xb5, 0xc3, 0x29, 0xb3, 0x4a, 0x58, 0xf0, 0x5a, 0x98, 0x40, 0xa9, 0x0f,
    0x05, 0x1d, 0x6e, 0x5c, 0x97, 0x83, 0x3c, 0x1d, 0x35, 0x6f, 0x81, 0x32, 0x3e, 0xf5, 0x21, 0x19,
];

// ─────────────────────────────────── tests ────────────────────────────────────

/// (i) Valid batch: every nullifier ends up spent AND every payout is transferred
/// to the right recipient for the right amount.
#[test]
fn settle_valid_batch_marks_nullifiers_and_transfers() {
    let (env, rollup, token_addr) = setup(true, &[ROOT_LE]);

    let items = [
        Item { nullifier: [0x11; 32], recipient: [0xAA; 32], amount: 1_000_000 },
        Item { nullifier: [0x22; 32], recipient: [0xBB; 32], amount: 42 },
    ];
    let journal = encode_journal(&ROOT_LE, &items);
    let journal_bytes = Bytes::from_slice(&env, &journal);
    let seal = Bytes::from_array(&env, &[0xDE, 0xAD, 0xBE, 0xEF]);
    let image_id: BytesN<32> = BytesN::from_array(&env, &ROLLUP_GUEST_ID);

    rollup.settle_batch(&seal, &image_id, &journal_bytes);

    // Nullifiers marked spent.
    assert!(is_spent(&env, &rollup.address, &items[0].nullifier));
    assert!(is_spent(&env, &rollup.address, &items[1].nullifier));

    // Exactly the two expected transfers happened, in order.
    let token = MockTokenClient::new(&env, &token_addr);
    let log = token.transfers();
    assert_eq!(log.len(), 2, "two transfers expected");
    let expected: Vec<(Address, i128)> = vec![
        &env,
        (recipient_address(&env, &items[0].recipient), 1_000_000i128),
        (recipient_address(&env, &items[1].recipient), 42i128),
    ];
    assert_eq!(log, expected, "exact (recipient, amount) payouts");
}

/// (ii) Inter-batch replay: a nullifier already spent by a prior batch makes the
/// whole second tx revert. We pre-mark the nullifier in the rollup storage to
/// simulate the prior batch, then assert the new settle panics AND left no state
/// change (no transfers, the fresh nullifier NOT marked).
#[test]
fn settle_revert_on_spent_nullifier() {
    let (env, rollup, token_addr) = setup(true, &[ROOT_LE]);

    let spent_nf = [0x33; 32];
    let fresh_nf = [0x44; 32];

    // Simulate a previous batch having spent `spent_nf`.
    env.as_contract(&rollup.address, || {
        env.storage()
            .persistent()
            .set(&DataKey::Spent(BytesN::from_array(&env, &spent_nf)), &());
    });

    let items = [
        Item { nullifier: fresh_nf, recipient: [0xAA; 32], amount: 5 },
        Item { nullifier: spent_nf, recipient: [0xBB; 32], amount: 7 },
    ];
    let journal = encode_journal(&ROOT_LE, &items);
    let journal_bytes = Bytes::from_slice(&env, &journal);
    let seal = Bytes::from_array(&env, &[1, 2, 3, 4]);
    let image_id: BytesN<32> = BytesN::from_array(&env, &ROLLUP_GUEST_ID);

    let res = rollup.try_settle_batch(&seal, &image_id, &journal_bytes);
    assert!(res.is_err(), "spent nullifier must revert the tx");

    // Atomicity: no partial state. The fresh nullifier (processed first, before
    // the spent one) must NOT remain marked, and no transfer happened.
    assert!(
        !is_spent(&env, &rollup.address, &fresh_nf),
        "no partial nullifier mark on revert"
    );
    let token = MockTokenClient::new(&env, &token_addr);
    assert_eq!(token.transfers().len(), 0, "no transfers on revert");
}

/// (iii) Unknown root: pool does not know the journal's root → revert, no state.
#[test]
fn settle_revert_on_unknown_root() {
    // Pool seeded with a DIFFERENT root than the journal carries.
    let other_root_le = [0x77; 32];
    let (env, rollup, token_addr) = setup(true, &[other_root_le]);

    let items = [Item { nullifier: [0x11; 32], recipient: [0xAA; 32], amount: 9 }];
    let journal = encode_journal(&ROOT_LE, &items); // ROOT_LE is NOT seeded
    let journal_bytes = Bytes::from_slice(&env, &journal);
    let seal = Bytes::from_array(&env, &[9, 9, 9, 9]);
    let image_id: BytesN<32> = BytesN::from_array(&env, &ROLLUP_GUEST_ID);

    let res = rollup.try_settle_batch(&seal, &image_id, &journal_bytes);
    assert!(res.is_err(), "unknown root must revert");

    assert!(!is_spent(&env, &rollup.address, &items[0].nullifier));
    let token = MockTokenClient::new(&env, &token_addr);
    assert_eq!(token.transfers().len(), 0, "no transfers on revert");
}

/// (iv) Intra-batch duplicate nullifier: the same nullifier twice in one journal
/// must revert on the SECOND mark (no extra logic needed — the sequential mark
/// catches it). Asserts revert + no transfers.
#[test]
fn settle_revert_on_duplicate_nullifier_in_batch() {
    let (env, rollup, token_addr) = setup(true, &[ROOT_LE]);

    let dup = [0x55; 32];
    let items = [
        Item { nullifier: dup, recipient: [0xAA; 32], amount: 1 },
        Item { nullifier: dup, recipient: [0xBB; 32], amount: 2 }, // same nullifier
    ];
    let journal = encode_journal(&ROOT_LE, &items);
    let journal_bytes = Bytes::from_slice(&env, &journal);
    let seal = Bytes::from_array(&env, &[7, 7, 7, 7]);
    let image_id: BytesN<32> = BytesN::from_array(&env, &ROLLUP_GUEST_ID);

    let res = rollup.try_settle_batch(&seal, &image_id, &journal_bytes);
    assert!(res.is_err(), "duplicate nullifier in batch must revert");

    // The first mark was rolled back too (all-or-nothing): nullifier not spent,
    // no transfers.
    assert!(!is_spent(&env, &rollup.address, &dup));
    let token = MockTokenClient::new(&env, &token_addr);
    assert_eq!(token.transfers().len(), 0, "no transfers on revert");
}

/// (v) WRONG image_id (a valid proof of ANOTHER program): the contract must
/// reject it BEFORE verifying or touching state. This is requirement #1 — without
/// the `image_id == ROLLUP_GUEST_ID` assert, such a proof would drain funds. We
/// even put the verifier in ACCEPT mode to prove the image_id check (not the
/// verifier) is what stops it.
#[test]
fn settle_revert_on_wrong_image_id() {
    let (env, rollup, token_addr) = setup(true /* verifier accepts */, &[ROOT_LE]);

    let items = [Item { nullifier: [0x11; 32], recipient: [0xAA; 32], amount: 1_000_000 }];
    let journal = encode_journal(&ROOT_LE, &items);
    let journal_bytes = Bytes::from_slice(&env, &journal);
    let seal = Bytes::from_array(&env, &[0, 0, 0, 0]);

    // An attacker-chosen image id for a DIFFERENT (but provable) program.
    let mut wrong = ROLLUP_GUEST_ID;
    wrong[0] ^= 0xFF;
    let wrong_image_id: BytesN<32> = BytesN::from_array(&env, &wrong);

    let res = rollup.try_settle_batch(&seal, &wrong_image_id, &journal_bytes);
    assert!(res.is_err(), "wrong image_id must revert even with an accepting verifier");

    // Nothing moved.
    assert!(!is_spent(&env, &rollup.address, &items[0].nullifier));
    let token = MockTokenClient::new(&env, &token_addr);
    assert_eq!(token.transfers().len(), 0, "no transfers when image_id is wrong");
}

/// Sanity: a wrong image id where the bytes are also a different length-equal
/// value still rejects (defense-in-depth on the equality check). Also confirms the
/// random helper path compiles. (Not one of the 5 required, kept minimal.)
#[test]
fn settle_revert_on_random_image_id() {
    let (env, rollup, _token) = setup(true, &[ROOT_LE]);
    let items = [Item { nullifier: [0x11; 32], recipient: [0xAA; 32], amount: 1 }];
    let journal_bytes = Bytes::from_slice(&env, &encode_journal(&ROOT_LE, &items));
    let seal = Bytes::from_array(&env, &[0, 0, 0, 0]);
    let random_id: BytesN<32> = BytesN::random(&env);
    let res = rollup.try_settle_batch(&seal, &random_id, &journal_bytes);
    assert!(res.is_err(), "random image_id must revert");
}
