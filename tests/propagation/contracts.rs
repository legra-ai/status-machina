use status_machina::{
    CapabilityId,
    CapabilitySnapshot,
    Composition,
    CompositionBindingError,
    Delivery,
    EventId,
    InstanceRegistry,
    PropagationLimits,
    PropagationStatus,
    Route,
    RoutedItem,
};

use super::support::*;

fn limits(steps: usize, intents: usize) -> PropagationLimits {
    PropagationLimits::builder()
        .max_steps(steps)
        .max_intents(intents)
        .build()
        .unwrap()
}

#[test]
fn partial_failure_retains_accepted_effects_and_failed_input() {
    let source = definition();
    let destination = definition_with_failure(Some(8));
    let mut first = source.instantiate(id(1), 0).unwrap();
    let mut second = destination.instantiate(id(2), 0).unwrap();
    let routes = routes(&first, &second);
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
        &CapabilitySnapshot::builder().build(),
        limits(8, 32),
    );
    assert!(matches!(
        report.status,
        PropagationStatus::DispatchFailed(_)
    ));
    assert_eq!(report.steps.len(), 1);
    assert_eq!(report.effects.len(), 1);
    assert_eq!(*report.effects[0].intent.payload(), 7);
    assert_eq!(report.pending[0].event().payload().0, 8);
    drop(bound);
    assert_eq!(first.revision(), 1);
    assert_eq!(second.revision(), 0);
}

#[test]
fn failed_evaluation_can_resume_with_fresh_facts() {
    let definition = definition();
    let mut first = definition.instantiate(id(1), 0).unwrap();
    let mut second = definition.instantiate(id(2), 0).unwrap();
    let routes = routes(&first, &second);
    let caps = CapabilitySnapshot::builder().build();
    let mut bound = routes
        .bind()
        .instance(address(1), &mut first)
        .unwrap()
        .instance(address(2), &mut second)
        .unwrap()
        .build()
        .unwrap();
    let refused = bound.propagate(
        vec![Delivery::new(address(1), event(9))],
        &false,
        &caps,
        limits(1, 8),
    );
    assert!(matches!(
        refused.status,
        PropagationStatus::DispatchFailed(_)
    ));
    assert!(refused.effects.is_empty());
    let accepted = bound.propagate(refused.pending, &true, &caps, limits(1, 8));
    assert_eq!(accepted.steps.len(), 1);
    assert_eq!(*accepted.effects[0].intent.payload(), 9);
}

#[test]
fn deferred_and_unmatched_outputs_remain_owned_without_dispatch() {
    let definition = definition();
    let mut first = definition.instantiate(id(1), 0).unwrap();
    let mut second = definition.instantiate(id(2), 0).unwrap();
    let registry = InstanceRegistry::builder()
        .instance(address(1), &first)
        .unwrap()
        .instance(address(2), &second)
        .unwrap()
        .build();
    let routes = Composition::builder(registry.clone(), registry)
        .route(
            Route::builder(address(1), EventId::try_new("next-1").unwrap())
                .to(address(2))
                .as_event(EventId::try_new("tick").unwrap())
                .requires(CapabilityId::try_new("permission").unwrap())
                .build()
                .unwrap(),
        )
        .unwrap()
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
        vec![
            Delivery::new(address(1), event(1)),
            Delivery::new(address(2), event(10)),
        ],
        &true,
        &CapabilitySnapshot::builder().build(),
        limits(8, 8),
    );
    assert_eq!(report.status, PropagationStatus::Quiescent);
    assert_eq!(report.steps.len(), 2);
    assert!(
        matches!(&report.unrouted[0], RoutedItem::Deferred {intent, ..} if intent.payload().0 == 2)
    );
    assert!(
        matches!(&report.unrouted[1], RoutedItem::Unmatched {intent, ..} if intent.payload().0 == 11)
    );
}

#[test]
fn binding_rejects_missing_unknown_duplicate_and_wrong_identity() {
    let definition = definition();
    let mut first = definition.instantiate(id(1), 0).unwrap();
    let second = definition.instantiate(id(2), 0).unwrap();
    let mut impostor = definition.instantiate(id(1), 0).unwrap();
    let routes = routes(&first, &second);
    assert!(matches!(
        routes.bind::<bool, u32>().build(),
        Err(CompositionBindingError::MissingInstance)
    ));
    assert!(matches!(
        routes.bind().instance(address(9), &mut first),
        Err(CompositionBindingError::UnregisteredInstance)
    ));
    assert!(matches!(
        routes.bind().instance(address(2), &mut first),
        Err(CompositionBindingError::IdentityOrInventoryMismatch)
    ));
    assert!(matches!(
        routes
            .bind()
            .instance(address(1), &mut first)
            .unwrap()
            .instance(address(1), &mut impostor),
        Err(CompositionBindingError::DuplicateInstance)
    ));
}

#[test]
fn input_and_limit_validation_happens_before_callbacks() {
    assert!(PropagationLimits::builder().build().is_err());
    assert!(
        PropagationLimits::builder()
            .max_steps(0)
            .max_intents(1)
            .build()
            .is_err()
    );
    assert!(
        PropagationLimits::builder()
            .max_steps(4097)
            .max_intents(1)
            .build()
            .is_err()
    );
    assert!(
        PropagationLimits::builder()
            .max_steps(1)
            .max_intents(4097)
            .build()
            .is_err()
    );
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
    let caps = CapabilitySnapshot::builder().build();
    let large = bound.propagate(
        vec![
            Delivery::new(address(1), event(1)),
            Delivery::new(address(2), event(2)),
        ],
        &false,
        &caps,
        limits(1, 1),
    );
    assert_eq!(large.status, PropagationStatus::IntentLimit);
    assert_eq!(large.pending.len(), 2);
    let unknown = bound.propagate(
        vec![Delivery::new(address(9), event(3))],
        &false,
        &caps,
        limits(1, 1),
    );
    assert_eq!(unknown.status, PropagationStatus::UnboundDestination);
    assert_eq!(unknown.pending[0].event().payload().0, 3);
    drop(bound);
    assert_eq!(first.revision(), 0);
    assert_eq!(second.revision(), 0);
}
