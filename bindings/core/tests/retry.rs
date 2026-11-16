//! Retry policies and waits keep the kernel contract across the private
//! decimal transport, and report unrepresentable values as range failures.
use status_machina_bridge::retry::{
    Failure,
    Policy,
    Waits,
};

fn duration(seconds: &str) -> String {
    format!(r#"{{"seconds":"{seconds}","nanoseconds":0}}"#)
}
fn schedule(policy: &Policy, failed: &str, elapsed: &str) -> String {
    let request = format!(
        r#"{{"failed_attempts":"{failed}","elapsed":{}}}"#,
        duration(elapsed)
    );
    serde_json::to_string(&policy.schedule(&request).unwrap()).unwrap()
}
fn reason(failure: Failure) -> (&'static str, &'static str, Option<&'static str>) {
    (failure.family, failure.reason, failure.field)
}

#[test]
fn exponential_policy_caps_and_checks_its_window() {
    let policy = Policy::parse(&format!(
        r#"{{"delay":{{"kind":"exponential","initial":{},"maximum":{}}},"max_attempts":"5","max_elapsed":{}}}"#,
        duration("5"),
        duration("20"),
        duration("30")
    ))
    .unwrap();
    assert_eq!(
        schedule(&policy, "3", "10"),
        r#"{"kind":"at","delay":{"seconds":"20","nanoseconds":0}}"#
    );
    assert_eq!(schedule(&policy, "3", "20"), r#"{"kind":"exhausted"}"#);
    assert_eq!(schedule(&policy, "5", "0"), r#"{"kind":"exhausted"}"#);
}

#[test]
fn elapsed_overflow_exhausts_instead_of_wrapping() {
    let max = u64::MAX.to_string();
    let policy = Policy::parse(&format!(
        r#"{{"delay":{{"kind":"fixed","delay":{}}},"max_attempts":"3","max_elapsed":{}}}"#,
        duration("1"),
        duration(&max)
    ))
    .unwrap();
    assert_eq!(schedule(&policy, "1", &max), r#"{"kind":"exhausted"}"#);
}

#[test]
fn invalid_policies_and_unrepresentable_values_are_typed() {
    let fixed = |delay: &str, attempts: &str| {
        Policy::parse(&format!(
            r#"{{"delay":{{"kind":"fixed","delay":{}}},"max_attempts":"{attempts}","max_elapsed":null}}"#,
            duration(delay)
        ))
        .err()
        .unwrap()
    };
    assert_eq!(reason(fixed("0", "2")), ("policy", "zero_delay", None));
    assert_eq!(
        reason(fixed("1", "1")),
        ("policy", "too_few_attempts", None)
    );
    assert_eq!(
        reason(fixed("-1", "2")),
        ("range", "out_of_range", Some("delay"))
    );
    assert_eq!(
        reason(fixed("1", "4294967296")),
        ("range", "out_of_range", Some("max_attempts"))
    );
    let cap = Policy::parse(&format!(
        r#"{{"delay":{{"kind":"exponential","initial":{},"maximum":{}}},"max_attempts":"2","max_elapsed":null}}"#,
        duration("2"),
        duration("1")
    ))
    .err()
    .unwrap();
    assert_eq!(reason(cap), ("policy", "cap_below_initial", None));
}

#[test]
fn waits_consume_once_and_reject_stale_and_foreign_correlations() {
    let mut waits = Waits::try_new(Some(1)).unwrap();
    let at = r#"{"seconds":"10","nanoseconds":0}"#;
    let generation = waits
        .schedule(&format!(r#"{{"operation":0,"deadline":{at}}}"#))
        .unwrap();
    let observe = |owner: bool, operation: &str, generation: &str, now: &str| {
        format!(
            r#"{{"correlation":{{"owner":{owner},"operation":{operation},"generation":"{generation}"}},"now":{{"seconds":"{now}","nanoseconds":0}}}}"#
        )
    };
    assert!(!waits.is_due(&observe(true, "0", &generation, "9")).unwrap());
    assert!(
        waits
            .consume(&observe(false, "0", &generation, "10"))
            .unwrap()
            .is_none()
    );
    assert!(
        waits
            .consume(&observe(true, "null", &generation, "10"))
            .unwrap()
            .is_none()
    );
    assert!(
        waits
            .consume(&observe(true, "0", "0", "10"))
            .unwrap()
            .is_none()
    );
    let intent = waits
        .consume(&observe(true, "0", &generation, "10"))
        .unwrap()
        .unwrap();
    assert_eq!(
        (intent.operation, intent.generation),
        (0, generation.clone())
    );
    assert!(
        waits
            .consume(&observe(true, "0", &generation, "10"))
            .unwrap()
            .is_none()
    );
    let overflow = waits
        .consume(&observe(true, "0", "18446744073709551616", "10"))
        .err()
        .unwrap();
    assert_eq!(
        reason(overflow),
        ("range", "out_of_range", Some("generation"))
    );
}

#[test]
fn capacity_is_explicit_and_reached_capacity_changes_nothing() {
    for capacity in [None, Some(0), Some(4097)] {
        let failure = Waits::try_new(capacity).err().unwrap();
        assert_eq!(reason(failure), ("waits", "invalid_capacity", None));
    }
    let mut waits = Waits::try_new(Some(1)).unwrap();
    let deadline = r#"{"seconds":"-1","nanoseconds":500}"#;
    waits
        .schedule(&format!(r#"{{"operation":0,"deadline":{deadline}}}"#))
        .unwrap();
    let failure = waits
        .schedule(&format!(r#"{{"operation":1,"deadline":{deadline}}}"#))
        .err()
        .unwrap();
    assert_eq!(
        (failure.reason, failure.limit),
        ("capacity_reached", Some(1))
    );
    let pending = waits.pending();
    assert_eq!(pending.len(), 1);
    assert_eq!(
        (
            pending[0].deadline.seconds.as_str(),
            pending[0].deadline.nanoseconds
        ),
        ("-1", 500)
    );
    assert!(waits.cancel(None).is_none());
    assert!(waits.cancel(Some(0)).is_some());
    assert!(waits.get(Some(0)).is_none());
}
