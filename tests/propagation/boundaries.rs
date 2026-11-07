use status_machina::{
    CapabilitySnapshot,
    Composition,
    CompositionBindingError,
    Delivery,
    InstanceRegistry,
    PropagationLimits,
    PropagationStatus,
};

use super::support::*;

#[test]
fn actual_binding_must_match_registered_signature() {
    let expected = definition();
    let changed = definition_with_outputs(None, 2);
    let first = expected.instantiate(id(1), 0).unwrap();
    let second = expected.instantiate(id(2), 0).unwrap();
    let composition = routes(&first, &second);
    let mut replacement = changed.instantiate(id(1), 0).unwrap();
    assert!(matches!(
        composition.bind().instance(address(1), &mut replacement),
        Err(CompositionBindingError::SignatureMismatch)
    ));
}

#[test]
fn routing_batch_capacity_refuses_before_acceptance_and_preserves_input() {
    let definition = definition_with_outputs(None, 2);
    let mut instance = definition.instantiate(id(1), 0).unwrap();
    let registry = InstanceRegistry::builder()
        .instance(address(1), &instance)
        .unwrap()
        .build();
    let composition = Composition::builder(registry.clone(), registry)
        .limits(1, 1)
        .unwrap()
        .build()
        .unwrap();
    let report = composition
        .bind()
        .instance(address(1), &mut instance)
        .unwrap()
        .build()
        .unwrap()
        .propagate(
            vec![Delivery::new(address(1), event(7))],
            &true,
            &CapabilitySnapshot::builder().build(),
            PropagationLimits::builder()
                .max_steps(8)
                .max_intents(8)
                .build()
                .unwrap(),
        );
    assert_eq!(report.status, PropagationStatus::IntentLimit);
    assert!(report.steps.is_empty());
    assert!(report.effects.is_empty());
    assert_eq!(report.pending[0].event().payload().0, 7);
    assert_eq!(instance.revision(), 0);
}

#[test]
fn initial_inputs_and_generated_outputs_keep_fifo_order() {
    let definition = definition();
    let mut first = definition.instantiate(id(1), 0).unwrap();
    let mut second = definition.instantiate(id(2), 0).unwrap();
    let composition = routes(&first, &second);
    let report = composition
        .bind()
        .instance(address(1), &mut first)
        .unwrap()
        .instance(address(2), &mut second)
        .unwrap()
        .build()
        .unwrap()
        .propagate(
            vec![
                Delivery::new(address(1), event(1)),
                Delivery::new(address(2), event(10)),
            ],
            &true,
            &CapabilitySnapshot::builder().build(),
            PropagationLimits::builder()
                .max_steps(4)
                .max_intents(8)
                .build()
                .unwrap(),
        );
    assert_eq!(report.status, PropagationStatus::StepLimit);
    assert_eq!(*report.effects[0].intent.payload(), 1);
    assert_eq!(*report.effects[1].intent.payload(), 10);
    assert_eq!(*report.effects[2].intent.payload(), 2);
    assert_eq!(*report.effects[3].intent.payload(), 11);
    assert_eq!(report.pending[0].event().payload().0, 3);
    assert_eq!(report.pending[1].event().payload().0, 12);
}
