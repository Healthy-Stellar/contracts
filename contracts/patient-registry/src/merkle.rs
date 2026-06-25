//! Merkle tree utilities for patient record membership proofs.
//!
//! ## Tree construction
//! Leaves are the patient's record IDs in insertion order.
//! ```text
//! leaf  = sha256(0x00 || record_id_be_8)
//! node  = sha256(0x01 || min(left, right) || max(left, right))
//! ```
//! Sorting children before hashing means the verifier never needs position
//! bits — the proof is just `Vec<BytesN<32>>` of sibling hashes.
//!
//! Odd-length layers: the dangling node is paired with itself.

use soroban_sdk::{panic_with_error, Bytes, BytesN, Env, Vec};

const LEAF_TAG: u8 = 0x00;
const NODE_TAG: u8 = 0x01;

// ─── primitives ────────────────────────────────────────────────────────────

/// Hash a record ID as a Merkle leaf.
///
/// `leaf = sha256(0x00 || record_id_be_8)`
pub fn hash_leaf(env: &Env, record_id: u64) -> BytesN<32> {
    let mut pre = Bytes::new(env);
    pre.extend_from_array(&[LEAF_TAG]);
    pre.extend_from_array(&record_id.to_be_bytes());
    env.crypto().sha256(&pre).into()
}

/// Hash two child hashes into a parent node.
///
/// Children are sorted lexicographically before hashing so that no position
/// information needs to be stored alongside proof siblings:
///
/// `node = sha256(0x01 || min(a,b) || max(a,b))`
pub fn hash_pair(env: &Env, a: BytesN<32>, b: BytesN<32>) -> BytesN<32> {
    let (lo, hi) = if a.to_array() <= b.to_array() {
        (a, b)
    } else {
        (b, a)
    };
    let mut pre = Bytes::new(env);
    pre.extend_from_array(&[NODE_TAG]);
    pre.extend_from_array(&lo.to_array());
    pre.extend_from_array(&hi.to_array());
    env.crypto().sha256(&pre).into()
}

// ─── root computation ──────────────────────────────────────────────────────

/// Compute the Merkle root over an ordered slice of record IDs.
///
/// Empty set: returns `sha256("")` as a deterministic sentinel.
pub fn compute_merkle_root(env: &Env, record_ids: &Vec<u64>) -> BytesN<32> {
    let n = record_ids.len();
    if n == 0 {
        return env.crypto().sha256(&Bytes::new(env)).into();
    }

    // Build leaf layer
    let mut layer: Vec<BytesN<32>> = Vec::new(env);
    for id in record_ids.iter() {
        layer.push_back(hash_leaf(env, id));
    }

    // Reduce layer-by-layer until a single root remains
    while layer.len() > 1 {
        let len = layer.len();
        let mut next: Vec<BytesN<32>> = Vec::new(env);
        let mut i = 0u32;
        while i + 1 < len {
            next.push_back(hash_pair(
                env,
                layer.get(i).unwrap_or_else(|| panic_with_error!(env, crate::ContractError::NotFound)),
                layer.get(i + 1).unwrap_or_else(|| panic_with_error!(env, crate::ContractError::NotFound)),
            ));
            i += 2;
        }
        // Odd node: pair with itself
        if len % 2 == 1 {
            let last = layer.get(len - 1).unwrap_or_else(|| panic_with_error!(env, crate::ContractError::NotFound));
            next.push_back(hash_pair(env, last.clone(), last));
        }
        layer = next;
    }

    layer.get(0).unwrap_or_else(|| panic_with_error!(env, crate::ContractError::NotFound))
}

// ─── proof generation ───────────────────────────────────────────────────────

/// Generate a Merkle membership proof for the leaf at `target_index` within
/// `record_ids` (insertion order, matching `compute_merkle_root`).
///
/// Returns one sibling hash per tree level (leaf level → root), in the same
/// shape `verify_membership` / `verify_leaf_membership` expect. Returns an
/// empty `Vec` if `target_index` is out of bounds.
///
/// Mirrors `compute_merkle_root`'s layer-by-layer reduction (same pairing
/// and odd-node-self-pairing rules) while tracking which sibling pairs with
/// the target leaf at each level. Because `hash_pair` sorts its two inputs
/// before hashing, the proof never needs to record left/right position.
pub fn generate_proof(env: &Env, record_ids: &Vec<u64>, target_index: u32) -> Vec<BytesN<32>> {
    let mut proof: Vec<BytesN<32>> = Vec::new(env);
    let n = record_ids.len();
    if target_index >= n {
        return proof;
    }

    let mut layer: Vec<BytesN<32>> = Vec::new(env);
    for id in record_ids.iter() {
        layer.push_back(hash_leaf(env, id));
    }

    let mut idx = target_index;
    while layer.len() > 1 {
        let len = layer.len();
        let mut next: Vec<BytesN<32>> = Vec::new(env);
        let mut i = 0u32;
        while i + 1 < len {
            let left = layer.get(i).unwrap_or_else(|| panic_with_error!(env, crate::ContractError::NotFound));
            let right = layer.get(i + 1).unwrap_or_else(|| panic_with_error!(env, crate::ContractError::NotFound));
            if idx == i {
                proof.push_back(right.clone());
            } else if idx == i + 1 {
                proof.push_back(left.clone());
            }
            next.push_back(hash_pair(env, left, right));
            i += 2;
        }
        // Odd node: pair with itself; if it's our target, the "sibling" is itself.
        if len % 2 == 1 {
            let last = layer.get(len - 1).unwrap_or_else(|| panic_with_error!(env, crate::ContractError::NotFound));
            if idx == len - 1 {
                proof.push_back(last.clone());
            }
            next.push_back(hash_pair(env, last.clone(), last));
        }
        idx /= 2;
        layer = next;
    }

    proof
}

// ─── membership verification ───────────────────────────────────────────────

/// Verify that `record_id` belongs to the tree with the given `root`.
///
/// `proof` contains one sibling hash per tree level (leaf level → root).
/// Returns `true` iff the recomputed root matches `root`.
pub fn verify_membership(
    env: &Env,
    record_id: u64,
    proof: &Vec<BytesN<32>>,
    root: &BytesN<32>,
) -> bool {
    verify_leaf_membership(env, hash_leaf(env, record_id), proof, root)
}

/// Verify that a precomputed leaf hash belongs to the tree with the given
/// `root`, given a sibling proof. Same as `verify_membership` but for
/// callers that already have the leaf hash rather than the raw record ID.
pub fn verify_leaf_membership(
    env: &Env,
    leaf_hash: BytesN<32>,
    proof: &Vec<BytesN<32>>,
    root: &BytesN<32>,
) -> bool {
    let mut current = leaf_hash;
    for sibling in proof.iter() {
        current = hash_pair(env, current, sibling);
    }
    &current == root
}
