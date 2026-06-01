use soroban_sdk::{contracttype, Address, Bytes, BytesN, String, Symbol};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EncryptedEnvelopeRef {
    pub content_hash: BytesN<32>,
    pub envelope_uri: String,
    pub key_version_id: String,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PolicyMetadata {
    pub retention_class: Symbol,
    pub access_policy_hash: BytesN<32>,
    pub purpose: Symbol,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum PrivacyError {
    InvalidContentHash,
    InvalidEnvelopeUri,
    InvalidKeyVersionId,
    InvalidPolicyMetadata,
    InvalidAddress,
}

pub fn validate_encrypted_ref(reference: &EncryptedEnvelopeRef) -> Result<(), PrivacyError> {
    validate_nonzero_hash(&reference.content_hash)?;
    validate_envelope_uri(&reference.envelope_uri)?;
    validate_key_version_id(&reference.key_version_id)?;
    Ok(())
}

pub fn validate_policy_metadata(policy: &PolicyMetadata) -> Result<(), PrivacyError> {
    validate_nonzero_hash(&policy.access_policy_hash)?;
    let _ = (&policy.retention_class, &policy.purpose);
    Ok(())
}

pub fn validate_nonzero_address(address: &Address) -> Result<(), PrivacyError> {
    let zero_addr = Address::from_array([0; 32]);
    if *address == zero_addr {
        return Err(PrivacyError::InvalidAddress);
    }
    Ok(())
}

pub fn validate_nonzero_hash(hash: &BytesN<32>) -> Result<(), PrivacyError> {
    let bytes = Bytes::from(hash.clone());
    let mut all_zero = true;
    for i in 0..bytes.len() {
        if bytes.get(i).unwrap_or(0) != 0 {
            all_zero = false;
            break;
        }
    }
    if all_zero {
        return Err(PrivacyError::InvalidContentHash);
    }
    Ok(())
}

pub fn validate_envelope_uri(uri: &String) -> Result<(), PrivacyError> {
    let len = uri.len() as usize;
    if !(16..=256).contains(&len) {
        return Err(PrivacyError::InvalidEnvelopeUri);
    }
    let mut buf = [0u8; 256];
    uri.copy_into_slice(&mut buf[..len]);
    validate_envelope_uri_bytes(&buf[..len]).map_err(|_| PrivacyError::InvalidEnvelopeUri)
}

pub fn validate_key_version_id(key_version_id: &String) -> Result<(), PrivacyError> {
    let len = key_version_id.len() as usize;
    if !(6..=64).contains(&len) {
        return Err(PrivacyError::InvalidKeyVersionId);
    }
    let mut buf = [0u8; 64];
    key_version_id.copy_into_slice(&mut buf[..len]);
    validate_key_version_id_bytes(&buf[..len]).map_err(|_| PrivacyError::InvalidKeyVersionId)
}

#[allow(clippy::result_unit_err)]
pub fn validate_envelope_uri_bytes(uri: &[u8]) -> Result<(), ()> {
    if !(16..=256).contains(&uri.len()) {
        return Err(());
    }
    if uri.iter().any(|b| !b.is_ascii() || b.is_ascii_whitespace()) {
        return Err(());
    }
    if let Some(rest) = uri.strip_prefix(b"enc+ipfs://") {
        return if rest.len() >= 4 { Ok(()) } else { Err(()) };
    }
    if let Some(rest) = uri.strip_prefix(b"enc+https://") {
        let slash = rest.iter().position(|b| *b == b'/').ok_or(())?;
        return if slash > 0 && slash + 1 < rest.len() {
            Ok(())
        } else {
            Err(())
        };
    }
    Err(())
}

#[allow(clippy::result_unit_err)]
pub fn validate_key_version_id_bytes(key_version_id: &[u8]) -> Result<(), ()> {
    if !(6..=64).contains(&key_version_id.len()) || !key_version_id.starts_with(b"kv:") {
        return Err(());
    }
    for &b in key_version_id {
        if !(b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.' || b == b':') {
            return Err(());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::Env;

    #[test]
    fn validates_envelope_uri_formats() {
        let env = Env::default();
        assert!(validate_envelope_uri(&String::from_str(&env, "enc+ipfs://bafyvalidcid")).is_ok());
        assert!(
            validate_envelope_uri(&String::from_str(&env, "enc+https://vault.example/ref")).is_ok()
        );
        assert_eq!(
            validate_envelope_uri(&String::from_str(&env, "ipfs://plaintext")),
            Err(PrivacyError::InvalidEnvelopeUri)
        );
        assert_eq!(
            validate_envelope_uri(&String::from_str(&env, "enc+https://bad host/ref")),
            Err(PrivacyError::InvalidEnvelopeUri)
        );
    }

    #[test]
    fn validates_key_version_ids() {
        let env = Env::default();
        assert!(validate_key_version_id(&String::from_str(&env, "kv:v01")).is_ok());
        assert!(validate_key_version_id(&String::from_str(&env, "kv:tenant_1.v2")).is_ok());
        assert_eq!(
            validate_key_version_id(&String::from_str(&env, "v1")),
            Err(PrivacyError::InvalidKeyVersionId)
        );
        assert_eq!(
            validate_key_version_id(&String::from_str(&env, "kv:bad key")),
            Err(PrivacyError::InvalidKeyVersionId)
        );
    }

    #[test]
    fn rejects_zero_hash() {
        let env = Env::default();
        let zero = BytesN::from_array(&env, &[0u8; 32]);
        assert_eq!(
            validate_nonzero_hash(&zero),
            Err(PrivacyError::InvalidContentHash)
        );
    }
}
