//! External names are checked mappings, independent of runtime identities.
use status_machina::{
    IdentityMap,
    IdentityMapError,
};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum State {
    Pending,
    Complete,
}

#[test]
fn checked_mapping_round_trips_typed_values() {
    let mapping = IdentityMap::try_new(
        [(State::Pending, "pending"), (State::Complete, "complete")],
        2,
    )
    .unwrap();
    assert_eq!(mapping.encode(&State::Pending), Ok(&"pending"));
    assert_eq!(mapping.decode(&"complete"), Ok(&State::Complete));
}

#[test]
fn external_names_cannot_collapse_distinct_states() {
    assert!(matches!(
        IdentityMap::try_new([(State::Pending, "same"), (State::Complete, "same")], 2),
        Err(IdentityMapError::DuplicateRepresentation)
    ));
    assert!(matches!(
        IdentityMap::try_new([(State::Pending, "a"), (State::Pending, "b")], 2),
        Err(IdentityMapError::DuplicateIdentity)
    ));
}

#[test]
fn unknown_values_fail_in_both_directions() {
    let mapping = IdentityMap::try_new([(State::Pending, "pending")], 1).unwrap();
    assert_eq!(
        mapping.encode(&State::Complete),
        Err(IdentityMapError::UnknownIdentity)
    );
    assert_eq!(
        mapping.decode(&"missing"),
        Err(IdentityMapError::UnknownRepresentation)
    );
}

#[test]
fn mapping_stops_at_its_bound_even_for_an_infinite_iterator() {
    assert!(matches!(
        IdentityMap::try_new((0_u64..).map(|n| (n, n)), 2),
        Err(IdentityMapError::LimitExceeded { limit: 2 })
    ));
    assert!(matches!(
        IdentityMap::try_new([(1, 2)], 0),
        Err(IdentityMapError::InvalidLimit)
    ));
    assert!(matches!(
        IdentityMap::try_new([(1, 2)], usize::MAX),
        Err(IdentityMapError::InvalidLimit)
    ));
}
