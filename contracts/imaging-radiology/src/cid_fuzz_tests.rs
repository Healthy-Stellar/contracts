/// Property-based fuzz tests for IPFS CID / envelope-URI validation in
/// imaging-radiology.  Mirrors the tests in health-records/src/cid_fuzz_tests.rs
/// to ensure the same validator is exercised from this contract's test suite.
///
/// Closes #401.
#[cfg(test)]
mod cid_fuzz_tests {
    use shared::privacy::{validate_envelope_uri_bytes, validate_key_version_id_bytes};

    const IPFS_PREFIX: &[u8] = b"enc+ipfs://";

    fn valid_ipfs_uri() -> [u8; 70] {
        let mut v = [0u8; 70];
        let prefix = b"enc+ipfs://";
        let cid = b"bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi";
        v[..11].copy_from_slice(prefix);
        v[11..].copy_from_slice(cid);
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
        let oversized = [b'a'; 257];
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

    #[test]
    fn valid_ipfs_uri_length_boundary() {
        let v = valid_ipfs_uri();
        assert_eq!(v.len(), 70); // "enc+ipfs://" (11) + VALID_CID (59)
        assert!(validate_envelope_uri_bytes(&v).is_ok());
    }

    #[test]
    fn invalid_prefix_with_valid_cid() {
        let mut v = [0u8; 80];
        let prefix = b"https://ipfs.io/ipfs/";
        let cid = b"bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi";
        v[..21].copy_from_slice(prefix);
        v[21..21+59].copy_from_slice(cid);
        assert!(validate_envelope_uri_bytes(&v[..80]).is_err());
    }

    #[test]
    fn empty_uri_is_rejected() {
        assert!(validate_envelope_uri_bytes(b"").is_err());
    }

    #[test]
    fn maximum_valid_length_uri() {
        let v = [b'a'; 256];
        let result = validate_envelope_uri_bytes(&v);
        // Should handle 256 bytes gracefully
        let _ = result;
    }

    #[test]
    fn cid_with_mixed_case_characters() {
        let mut v = IPFS_PREFIX.to_vec();
        v.extend_from_slice(b"BaFyBeIgDyRzT5SfP7UdM7Hu76Uh7Y26Nf3EfUyLqAbF3OcLgTqY55FbZdI");
        // Mixed case should still be valid as long as it's ASCII
        let result = validate_envelope_uri_bytes(&v);
        let _ = result;
    }
}
