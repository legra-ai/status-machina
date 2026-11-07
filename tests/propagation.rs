//! Caller-driven composition replaces application-specific routing loops.
#[path = "propagation/support.rs"]
mod support;
use status_machina::{
    CapabilitySnapshot,
    Delivery,
    PropagationLimits,
    PropagationStatus,
};
use support::*;

#[test]
fn bounded_cycle_runs_inline_and_returns_owned_continuation() {
    let definition = definition();
    let mut first = definition.instantiate(id(1), 0).unwrap();
    let mut second = definition.instantiate(id(2), 0).unwrap();
    let routes = routes(&first, &second);
    let caps = CapabilitySnapshot::builder().build();
    let limits = PropagationLimits::builder()
        .max_steps(3)
        .max_intents(16)
        .build()
        .unwrap();
    let mut bound = routes
        .bind()
        .instance(address(1), &mut first)
        .unwrap()
        .instance(address(2), &mut second)
        .unwrap()
        .build()
        .unwrap();
    let report = bound.propagate(
        vec![Delivery::new(address(1), event(7))],
        &true,
        &caps,
        limits,
    );
    assert_eq!(report.status, PropagationStatus::StepLimit);
    assert_eq!(report.steps.len(), 3);
    assert_eq!(
        report
            .effects
            .iter()
            .map(|entry| *entry.intent.payload())
            .collect::<Vec<_>>(),
        [7, 8, 9]
    );
    assert_eq!(report.pending.len(), 1);
    assert_eq!(report.pending[0].event().payload().0, 10);
    let resumed = bound.propagate(report.pending, &true, &caps, limits);
    assert_eq!(resumed.steps.len(), 3);
    drop(bound);
    assert_eq!(first.data(), &3);
    assert_eq!(second.data(), &3);
}

#[test]
fn intent_capacity_refusal_keeps_instance_and_nonclone_input_unchanged() {
    let definition = definition();
    let mut first = definition.instantiate(id(1), 0).unwrap();
    let mut second = definition.instantiate(id(2), 0).unwrap();
    let routes = routes(&first, &second);
    let mut bound = routes
        .bind()
        .instance(address(1), &mut first)
        .unwrap()
        .instance(address(2), &mut second)
        .unwrap()
        .build()
        .unwrap();
    let limits = PropagationLimits::builder()
        .max_steps(3)
        .max_intents(1)
        .build()
        .unwrap();
    let report = bound.propagate(
        vec![Delivery::new(address(1), event(7))],
        &true,
        &CapabilitySnapshot::builder().build(),
        limits,
    );
    assert_eq!(report.status, PropagationStatus::IntentLimit);
    assert!(report.effects.is_empty());
    assert_eq!(report.pending[0].event().payload().0, 7);
    drop(bound);
    assert_eq!(first.revision(), 0);
    assert_eq!(second.revision(), 0);
}

#[path = "propagation/contracts.rs"]
mod contracts;
#[path = "propagation/waiting.rs"]
mod waiting;

#[path = "propagation/boundaries.rs"]
mod boundaries;
