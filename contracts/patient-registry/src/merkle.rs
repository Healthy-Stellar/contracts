//! Merkle tree utilities for patient record membership proofs.
//!
//! ## Tree construction
//! Leaves are computed from record ID and record content hash:
//! ```text
//! leaf = sha256(0x00 || record_id_be_8 || sha256(xdr(RecordData)))
//! node = sha256(0x01 || left || right) (ordered pairing, no sorting)
//! ```
//! Odd-length layers: the dangling node is promoted to the next layer as-is.

use soroban_sdk::{panic_with_error, Bytes, BytesN, Env, Vec};

const LEAF_TAG: u8 = 0x00;
const NODE_TAG: u8 = 0x01;

// ─── primitives ────────────────────────────────────────────────────────────

/// Hash a record ID and its content hash as a Merkle leaf:
/// `leaf = sha256(0x00 || record_id_be_8 || content_hash)`
pub fn hash_leaf(env: &Env, record_id: u64, content_hash: &BytesN<32>) -> BytesN<32> {
    let mut pre = Bytes::new(env);
    pre.extend_from_array(&[LEAF_TAG]);
    pre.extend_from_array(&record_id.to_be_bytes());
    pre.append(&content_hash.clone().into());
    env.crypto().sha256(&pre).into()
}

/// Fallback for leaf hashing when only record_id is available (or empty content)
pub fn hash_leaf_id_only(env: &Env, record_id: u64) -> BytesN<32> {
    let empty_hash: BytesN<32> = env.crypto().sha256(&Bytes::new(env)).into();
    hash_leaf(env, record_id, &empty_hash)
}

/// Hash two child hashes into a parent node using strict ordered pairing (no sorting).
/// `node = sha256(0x01 || left || right)`
pub fn hash_pair(env: &Env, left: BytesN<32>, right: BytesN<32>) -> BytesN<32> {
    let mut pre = Bytes::new(env);
    pre.extend_from_array(&[NODE_TAG]);
    pre.extend_from_array(&left.to_array());
    pre.extend_from_array(&right.to_array());
    env.crypto().sha256(&pre).into()
}

// ─── root computation ──────────────────────────────────────────────────────

/// Compute the Merkle root over an ordered slice of leaf hashes.
/// Promotes odd dangling nodes without duplicating.
pub fn compute_merkle_root(env: &Env, leaves: &Vec<BytesN<32>>) -> BytesN<32> {
    let n = leaves.len();
    if n == 0 {
        return env.crypto().sha256(&Bytes::new(env)).into();
    }

    let mut layer = leaves.clone();

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
        // Odd node: promote to next layer without pairing with itself
        if len % 2 == 1 {
            let last = layer.get(len - 1).unwrap_or_else(|| panic_with_error!(env, crate::ContractError::NotFound));
            next.push_back(last);
        }
        layer = next;
    }

    layer.get(0).unwrap_or_else(|| panic_with_error!(env, crate::ContractError::NotFound))
}

// ─── proof generation ───────────────────────────────────────────────────────

/// Generate a Merkle membership proof for the leaf at `target_index` within `leaves`.
pub fn generate_proof(env: &Env, leaves: &Vec<BytesN<32>>, target_index: u32) -> Vec<BytesN<32>> {
    let mut proof: Vec<BytesN<32>> = Vec::new(env);
    let n = leaves.len();
    if target_index >= n {
        return proof;
    }

    let mut layer = leaves.clone();
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
        // Odd node: promoted as-is
        if len % 2 == 1 {
            let last = layer.get(len - 1).unwrap_or_else(|| panic_with_error!(env, crate::ContractError::NotFound));
            next.push_back(last);
        }
        idx /= 2;
        layer = next;
    }

    proof
}

// ─── membership verification ───────────────────────────────────────────────

/// Verify that `leaf_hash` at `leaf_index` within a tree of `total_leaves` belongs to `root`.
pub fn verify_leaf_membership_with_index(
    env: &Env,
    leaf_hash: BytesN<32>,
    leaf_index: u32,
    total_leaves: u32,
    proof: &Vec<BytesN<32>>,
    root: &BytesN<32>,
) -> bool {
    if total_leaves == 0 || leaf_index >= total_leaves {
        return false;
    }
    if total_leaves == 1 {
        return proof.len() == 0 && &leaf_hash == root;
    }

    let mut current = leaf_hash;
    let mut idx = leaf_index;
    let mut cur_len = total_leaves;
    let mut proof_idx = 0u32;

    while cur_len > 1 {
        if idx % 2 == 1 {
            // We are right child; sibling is on left
            if proof_idx >= proof.len() {
                return false;
            }
            let sibling = proof.get(proof_idx).unwrap();
            proof_idx += 1;
            current = hash_pair(env, sibling, current);
        } else if idx == cur_len - 1 && cur_len % 2 == 1 {
            // Odd dangling node: promoted as-is, no sibling at this level
        } else {
            // We are left child; sibling is on right
            if proof_idx >= proof.len() {
                return false;
            }
            let sibling = proof.get(proof_idx).unwrap();
            proof_idx += 1;
            current = hash_pair(env, current, sibling);
        }
        idx /= 2;
        cur_len = (cur_len + 1) / 2;
    }

    proof_idx == proof.len() && &current == root
}
