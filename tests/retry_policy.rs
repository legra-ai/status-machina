//! Retry-policy tests for caller-evaluated retry intent.

use std::time::Duration;

use status_machina::{
    RetryPolicy,
    RetryPolicyBuildError,
    RetrySchedule,
};

#[test]
fn fixed_retry_policy_returns_a_bounded_delay_without_reading_a_clock() {
    let policy = RetryPolicy::builder()
        .fixed_delay(Duration::from_secs(10))
        .max_attempts(3)
        .build()
        .unwrap();

    assert_eq!(
        policy.schedule(1, Duration::ZERO),
        RetrySchedule::At(Duration::from_secs(10))
    );
    assert_eq!(policy.schedule(3, Duration::ZERO), RetrySchedule::Exhausted);
}

#[test]
fn exponential_retry_policy_caps_delay_and_checks_elapsed_bound() {
    let policy = RetryPolicy::builder()
        .exponential_backoff(Duration::from_secs(5), Duration::from_secs(20))
        .max_attempts(5)
        .max_elapsed(Duration::from_secs(30))
        .build()
        .unwrap();

    assert_eq!(
        policy.schedule(1, Duration::ZERO),
        RetrySchedule::At(Duration::from_secs(5))
    );
    assert_eq!(
        policy.schedule(3, Duration::from_secs(10)),
        RetrySchedule::At(Duration::from_secs(20))
    );
    assert_eq!(
        policy.schedule(3, Duration::from_secs(20)),
        RetrySchedule::Exhausted
    );
}

#[test]
fn invalid_attempt_and_delay_bounds_are_rejected() {
    assert_eq!(
        RetryPolicy::builder()
            .fixed_delay(Duration::ZERO)
            .max_attempts(2)
            .build()
            .unwrap_err(),
        RetryPolicyBuildError::ZeroDelay,
    );
}

#[test]
fn exponential_retry_grows_past_a_u32_multiplier_and_reaches_its_cap() {
    let policy = RetryPolicy::builder()
        .exponential_backoff(Duration::from_nanos(1), Duration::from_secs(10))
        .max_attempts(u32::MAX)
        .build()
        .unwrap();

    for (attempt, delay) in [
        (33, Duration::from_nanos(4_294_967_296)),
        (34, Duration::from_nanos(8_589_934_592)),
        (35, Duration::from_secs(10)),
        (u32::MAX - 1, Duration::from_secs(10)),
    ] {
        assert_eq!(
            policy.schedule(attempt, Duration::ZERO),
            RetrySchedule::At(delay)
        );
    }
}

#[test]
fn exponential_retry_preserves_the_full_duration_range() {
    let policy = RetryPolicy::builder()
        .exponential_backoff(Duration::from_secs(1), Duration::MAX)
        .max_attempts(u32::MAX)
        .build()
        .unwrap();

    assert_eq!(
        policy.schedule(64, Duration::ZERO),
        RetrySchedule::At(Duration::from_secs(1_u64 << 63))
    );
    assert_eq!(
        policy.schedule(65, Duration::ZERO),
        RetrySchedule::At(Duration::MAX)
    );
    assert_eq!(
        policy.schedule(u32::MAX - 1, Duration::ZERO),
        RetrySchedule::At(Duration::MAX)
    );
}
