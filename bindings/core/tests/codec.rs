//! Identity codecs keep the kernel `IdentityMap` contract, with the position
//! of the first offending entry for native error context.
use status_machina_bridge::codec::Codec;

#[test]
fn one_to_one_pairs_round_trip() {
    let codec = Codec::try_new(&[(0, 10), (1, 11), (2, 12)], 3).unwrap();
    assert_eq!(codec.encode(1).unwrap(), 11);
    assert_eq!(codec.decode(12).unwrap(), 2);
}

#[test]
fn duplicates_report_their_entry_position() {
    let error = Codec::try_new(&[(0, 10), (1, 11), (0, 12)], 8)
        .err()
        .unwrap();
    assert_eq!((error.reason, error.entry), ("duplicate_identity", Some(2)));
    let error = Codec::try_new(&[(0, 10), (1, 10)], 8).err().unwrap();
    assert_eq!(
        (error.reason, error.entry),
        ("duplicate_representation", Some(1))
    );
}

#[test]
fn limits_follow_the_kernel_ceiling() {
    for limit in [0, 4097] {
        let error = Codec::try_new(&[(0, 0)], limit).err().unwrap();
        assert_eq!((error.reason, error.entry), ("invalid_limit", None));
    }
    let error = Codec::try_new(&[(0, 0), (1, 1), (2, 2)], 2).err().unwrap();
    assert_eq!(
        (error.reason, error.limit, error.entry),
        ("limit_exceeded", Some(2), Some(2))
    );
    assert!(Codec::try_new(&[], 1).is_ok());
}

#[test]
fn unknown_lookups_are_never_substituted() {
    let codec = Codec::try_new(&[(0, 10)], 1).unwrap();
    assert_eq!(codec.encode(1).err().unwrap().reason, "unknown_identity");
    assert_eq!(
        codec.decode(0).err().unwrap().reason,
        "unknown_representation"
    );
    assert!(
        codec
            .encode(1)
            .err()
            .unwrap()
            .encode()
            .contains("\"code\":\"identity_map\"")
    );
}
