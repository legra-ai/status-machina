use status_machina::{
    Condition,
    DefinitionBuildError,
    EventDisposition,
};

use super::*;

#[test]
fn inspection_visits_nested_conditions_and_every_state_use_site_without_execution() {
    let registry = BehaviorRegistry::<u64, (), (), u64, (), Ids>::builder()
        .action(reference(Story::Increment), |_, _, _| {
            panic!("construction executed action")
        })
        .unwrap()
        .condition(reference(Story::Allowed), |_, _, _| {
            panic!("construction executed predicate")
        })
        .unwrap()
        .effect(reference(Story::Notify), |_, _, _| {
            panic!("construction executed effect")
        })
        .unwrap()
        .invariant(reference(Story::Valid), |_| {
            panic!("construction executed invariant")
        })
        .unwrap()
        .build();
    let predicate = |id| {
        registry
            .resolve_condition(id, &reference(Story::Allowed))
            .unwrap()
    };
    let nested = Condition::all(10)
        .condition(predicate(11))
        .unwrap()
        .condition(
            Condition::any(12)
                .condition(
                    Condition::not(13)
                        .condition(predicate(14))
                        .unwrap()
                        .build()
                        .unwrap(),
                )
                .unwrap()
                .condition(predicate(15))
                .unwrap()
                .build()
                .unwrap(),
        )
        .unwrap()
        .build()
        .unwrap();
    let state = StateDefinition::builder(0)
        .bound_invariant(
            registry
                .resolve_invariant(&reference(Story::Valid))
                .unwrap(),
        )
        .entry_action(
            registry
                .resolve_action(1, &reference(Story::Increment))
                .unwrap(),
        )
        .unwrap()
        .exit_action(
            registry
                .resolve_action(2, &reference(Story::Increment))
                .unwrap(),
        )
        .unwrap()
        .entry_effect(
            registry
                .resolve_effect(1, &reference(Story::Notify))
                .unwrap(),
        )
        .unwrap()
        .exit_effect(
            registry
                .resolve_effect(2, &reference(Story::Notify))
                .unwrap(),
        )
        .unwrap()
        .build();
    let edge = Transition::builder_with_effects(1)
        .from(0)
        .on(1)
        .to(0)
        .when(nested)
        .postcondition(predicate(16))
        .unwrap()
        .build()
        .unwrap();
    let disposition = EventDisposition::builder(1)
        .from(0)
        .on(2)
        .when(predicate(17))
        .reason("obsolete")
        .build()
        .unwrap();
    let definition = MachineDefinition::<u64, (), (), u64, (), Ids>::builder_with_effects(1)
        .initial(0)
        .state(state)
        .unwrap()
        .transition(edge)
        .unwrap()
        .disposition(disposition)
        .unwrap()
        .build()
        .unwrap();
    let expected = [
        Story::Valid,
        Story::Increment,
        Story::Increment,
        Story::Notify,
        Story::Notify,
        Story::Allowed,
        Story::Allowed,
        Story::Allowed,
        Story::Allowed,
        Story::Allowed,
    ];
    assert!(
        definition
            .bindings()
            .map(BehaviorRef::behavior)
            .eq(expected.iter())
    );
}

#[test]
fn bound_invariant_is_enforced_and_inline_replacement_clears_its_reference() {
    let registry = BehaviorRegistry::<u64, (), (), (), (), Ids>::builder()
        .invariant(reference(Story::Valid), |data| *data <= 10)
        .unwrap()
        .build();
    let bound_state = || {
        StateDefinition::builder(0).bound_invariant(
            registry
                .resolve_invariant(&reference(Story::Valid))
                .unwrap(),
        )
    };
    let definition = MachineDefinition::<u64, (), (), (), (), Ids>::builder(1)
        .initial(0)
        .state(bound_state().build())
        .unwrap()
        .build()
        .unwrap();
    assert!(matches!(
        definition.instantiate(1, 11),
        Err(DefinitionBuildError::InitialInvariantFailed { .. })
    ));
    let inline = MachineDefinition::<u64, (), (), (), (), Ids>::builder(1)
        .initial(0)
        .state(bound_state().invariant(|_| true).build())
        .unwrap()
        .build()
        .unwrap();
    assert_eq!(inline.bindings().count(), 0);
    assert!(inline.instantiate(1, 11).is_ok());
}

#[test]
fn registry_limit_counts_implementation_pairs_and_cannot_shrink_below_existing_size() {
    let builder = || {
        BehaviorRegistry::<u64, (), (), (), (), Ids>::builder()
            .action(reference(Story::Increment), |_, _, _| Ok(()))
            .unwrap()
            .action(
                BehaviorRef::new(Story::Increment, Implementation::RulesTwo),
                |_, _, _| Ok(()),
            )
            .unwrap()
    };
    assert!(matches!(
        builder().max_bindings(1),
        Err(BindingError::LimitBelowCurrentSize)
    ));
    assert!(builder().max_bindings(2).is_ok());
    assert!(matches!(
        builder()
            .max_bindings(2)
            .unwrap()
            .condition(reference(Story::Allowed), |_, _, _| true),
        Err(BindingError::LimitExceeded { limit: 2 })
    ));
}
