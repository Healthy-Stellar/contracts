/// Property-based fuzz tests for IPFS CID / envelope-URI validation in
/// imaging-radiology.  Mirrors the tests in health-records/src/cid_fuzz_tests.rs
/// to ensure the same validator is exercised from this contract's test suite.
///
/// Closes #401.
#[cfg(test)]
mod cid_fuzz_tests {
    use shared::privacy::{validate_envelope_uri_bytes, validate_key_version_id_bytes};

    const VALID_CID: &[u8] = b"bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi";
    const IPFS_PREFIX: &[u8] = b"enc+ipfs://";

    fn valid_ipfs_uri() -> Vec<u8> {
        let mut v = IPFS_PREFIX.to_vec();
        v.extend_from_slice(VALID_CID);
        v
    }

    #[test]
    fn valid_ipfs_uri_passes() {
        assert!(validate_envelope_uri_bytes(&valid_ipfs_uri()).is_ok());
    }

    #[test]
    fn wrong_prefix_rejected() {
        assert!(validate_envelope_uri_bytes(b"ipfs://bafyvalidcid").is_err());
    }

    #[test]
    fn null_byte_rejected() {
        let mut uri = valid_ipfs_uri();
        uri[15] = 0x00;
        assert!(validate_envelope_uri_bytes(&uri).is_err());
    }

    #[test]
    fn oversized_uri_rejected() {
        let oversized = vec![b'a'; 257];
        assert!(validate_envelope_uri_bytes(&oversized).is_err());
    }

    #[test]
    fn non_ascii_rejected() {
        let mut uri = valid_ipfs_uri();
        uri[12] = 0xC3;
        assert!(validate_envelope_uri_bytes(&uri).is_err());
    }

    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self) -> u8 {
            self.0 = self.0.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
            (self.0 >> 33) as u8
        }
        fn fill(&mut self, buf: &mut [u8]) {
            for b in buf.iter_mut() {
                *b = self.next();
            }
        }
    }

    #[test]
    fn fuzz_arbitrary_inputs_do_not_panic() {
        let mut rng = Lcg(0xFEED_FACE_DEAD_BEEF);
        let mut buf = [0u8; 300];

        for i in 0u64..10_000 {
            let len = (rng.next() as usize % 300).max(1);
            rng.fill(&mut buf[..len]);
            let input = &buf[..len];

            let _ = validate_envelope_uri_bytes(input);
            let _ = validate_key_version_id_bytes(input);

            if len < 16 {
                assert!(
                    validate_envelope_uri_bytes(input).is_err(),
                    "iteration {i}: short input ({len} bytes) should be rejected"
                );
            }

            if !input.starts_with(b"kv:") {
                assert!(
                    validate_key_version_id_bytes(input).is_err(),
                    "iteration {i}: input without kv: prefix should be rejected"
                );
            }
        }
    }
}
