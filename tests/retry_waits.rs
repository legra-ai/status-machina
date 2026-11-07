//! Correlated waits survive unrelated revisions without retaining history.

use std::time::{
    Duration,
    SystemTime,
};

use status_machina::{
    MachineInstanceId,
    OperationId,
    RetryWaitError,
    RetryWaits,
};
use uuid::Uuid;

fn instance(seed: u128) -> MachineInstanceId {
    MachineInstanceId::try_new(Uuid::from_u128(
        0x018f_0000_0000_7000_8000_0000_0000_0000 + seed,
    ))
    .unwrap()
}

fn waits(capacity: usize) -> RetryWaits<u32> {
    RetryWaits::builder(instance(1))
        .capacity(capacity)
        .build()
        .unwrap()
}

fn operation(name: &str) -> OperationId {
    OperationId::try_new(name).unwrap()
}

#[test]
fn registration_is_distinct_from_consumption_and_duplicates_are_refused() {
    let mut waits = waits(1);
    let due = SystemTime::UNIX_EPOCH + Duration::from_secs(10);
    let correlation = waits.schedule(operation("fetch"), due, 7).unwrap();
    assert_eq!(waits.pending().count(), 1);
    assert!(waits.mark_registered(&correlation));
    assert_eq!(waits.pending().count(), 0);
    assert!(waits.get(&operation("fetch")).is_some());
    assert!(
        waits
            .consume(&correlation, SystemTime::UNIX_EPOCH)
            .is_none()
    );
    assert_eq!(*waits.consume(&correlation, due).unwrap().payload(), 7);
    assert!(waits.consume(&correlation, due).is_none());
    assert!(!waits.mark_registered(&correlation));
}

#[test]
fn replacement_and_cancel_recreate_never_reuse_a_wait_generation() {
    let mut waits = waits(1);
    let now = SystemTime::UNIX_EPOCH;
    let first = waits.schedule(operation("fetch"), now, 1).unwrap();
    let replacement = waits.schedule(operation("fetch"), now, 2).unwrap();
    assert_ne!(first, replacement);
    assert!(!waits.mark_registered(&first));
    assert_eq!(waits.pending().count(), 1);
    assert!(waits.consume(&first, now).is_none());
    waits.cancel(&operation("fetch")).unwrap();
    let recreated = waits.schedule(operation("fetch"), now, 3).unwrap();
    assert!(first.generation() < replacement.generation());
    assert!(replacement.generation() < recreated.generation());
    assert!(waits.consume(&replacement, now).is_none());
    assert_eq!(*waits.consume(&recreated, now).unwrap().payload(), 3);
}

#[test]
fn capacity_failure_preserves_existing_intents_and_generation() {
    let mut waits = waits(1);
    let now = SystemTime::UNIX_EPOCH;
    let first = waits.schedule(operation("fetch"), now, 1).unwrap();
    assert_eq!(
        waits.schedule(operation("publish"), now, 2),
        Err(RetryWaitError::CapacityReached)
    );
    assert_eq!(waits.pending().count(), 1);
    assert_eq!(
        waits.get(&operation("fetch")).unwrap().correlation(),
        &first
    );
    waits.cancel(&operation("fetch"));
    let next = waits.schedule(operation("publish"), now, 2).unwrap();
    assert_eq!(next.generation(), first.generation() + 1);
}

#[test]
fn another_instance_cannot_consume_a_current_wait() {
    let mut first = waits(1);
    let mut second = RetryWaits::builder(instance(2))
        .capacity(1)
        .build()
        .unwrap();
    let now = SystemTime::UNIX_EPOCH;
    let foreign = first.schedule(operation("fetch"), now, 1).unwrap();
    let current = second.schedule(operation("fetch"), now, 1).unwrap();
    assert!(second.consume(&foreign, now).is_none());
    assert!(second.consume(&current, now).is_some());
}

#[test]
fn wait_capacity_must_be_explicit_positive_and_bounded() {
    for capacity in [0, usize::MAX] {
        assert!(matches!(
            RetryWaits::<()>::builder(instance(1))
                .capacity(capacity)
                .build(),
            Err(RetryWaitError::InvalidCapacity)
        ));
    }
    assert!(matches!(
        RetryWaits::<()>::builder(instance(1)).build(),
        Err(RetryWaitError::InvalidCapacity)
    ));
}
