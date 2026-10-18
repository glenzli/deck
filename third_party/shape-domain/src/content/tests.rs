use std::str::FromStr;

use super::{ContentDigest, ContentRef};

#[test]
fn digest_is_canonical_hex_and_round_trips() {
    let digest = ContentDigest::from_bytes(b"shape");
    let encoded = digest.to_string();
    assert_eq!(encoded.len(), 64);
    assert_eq!(ContentDigest::from_str(&encoded).unwrap(), digest);
}

#[test]
fn media_type_is_explicit_and_portable() {
    let digest = ContentDigest::from_bytes(b"hello");
    assert!(ContentRef::new(digest, "text/plain; charset=utf-8", 5).is_ok());
    assert!(ContentRef::new(digest, "not-a-media-type", 5).is_err());
}
