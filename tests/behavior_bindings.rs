//! Story and implementation identity are typed values, separate from graph use
//! sites.
use std::sync::Arc;
use std::sync::atomic::{
    AtomicUsize,
    Ordering,
};

use status_machina::{
    BehaviorRef,
    BehaviorRegistry,
    BindingError,
    Event,
    MachineDefinition,
    MachineIds,
    StateDefinition,
    Transition,
};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct Ids;
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum Story {
    Increment,
    Allowed,
    Notify,
    Emit,
    Valid,
    Available,
    Safe,
    Missing,
}
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum Implementation {
    RulesOne,
    RulesTwo,
}
impl MachineIds for Ids {
    type Definition = u8;
    type Instance = u8;
    type State = u8;
    type Event = u8;
    type Transition = u8;
    type Action = u8;
    type Condition = u8;
    type Effect = u8;
    type Disposition = u8;
    type Capability = u8;
    type Operation = u8;
    type Scenario = u8;
    type Assertion = u8;
    type Scope = u8;
    type Behavior = Story;
    type Implementation = Implementation;
}
fn reference(story: Story) -> BehaviorRef<Ids> {
    BehaviorRef::new(story, Implementation::RulesOne)
}

#[test]
fn typed_story_binding_is_retained_at_each_use_site_without_executing_during_binding() {
    let calls = Arc::new(AtomicUsize::new(0));
    let action_calls = Arc::clone(&calls);
    let registry = BehaviorRegistry::<u64, (), (), u64, u64, Ids>::builder()
        .action(reference(Story::Increment), move |data, _, _| {
            action_calls.fetch_add(1, Ordering::SeqCst);
            *data += 1;
            Ok(())
        })
        .unwrap()
        .condition(reference(Story::Allowed), |_, _, _| true)
        .unwrap()
        .effect(reference(Story::Notify), |data, _, _| *data)
        .unwrap()
        .output_event(reference(Story::Emit), |data, _, _| *data)
        .unwrap()
        .invariant(reference(Story::Valid), |data| *data <= 10)
        .unwrap()
        .build();
    let edge = Transition::builder_with_effects_and_output_events(1)
        .from(0)
        .on(1)
        .to(1)
        .when(
            registry
                .resolve_condition(2, &reference(Story::Allowed))
                .unwrap(),
        )
        .action(
            registry
                .resolve_action(3, &reference(Story::Increment))
                .unwrap(),
        )
        .unwrap()
        .action(
            registry
                .resolve_action(4, &reference(Story::Increment))
                .unwrap(),
        )
        .unwrap()
        .effect(
            registry
                .resolve_effect(5, &reference(Story::Notify))
                .unwrap(),
        )
        .unwrap()
        .output_event(
            registry
                .resolve_output_event(2, &reference(Story::Emit))
                .unwrap(),
        )
        .unwrap()
        .build()
        .unwrap();
    let definition =
        MachineDefinition::<u64, (), (), u64, u64, Ids>::builder_with_effects_and_output_events(1)
            .initial(0)
            .state(StateDefinition::builder(0).build())
            .unwrap()
            .state(
                StateDefinition::builder(1)
                    .bound_invariant(
                        registry
                            .resolve_invariant(&reference(Story::Valid))
                            .unwrap(),
                    )
                    .build(),
            )
            .unwrap()
            .transition(edge)
            .unwrap()
            .build()
            .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    // bounded: this fixture has exactly six registered behavior references.
    let bindings: Vec<_> = definition.bindings().cloned().collect();
    assert_eq!(bindings.len(), 6);
    assert_eq!(
        bindings
            .iter()
            .filter(|binding| **binding == reference(Story::Increment))
            .count(),
        2
    );
    assert!(bindings.contains(&reference(Story::Valid)));
    drop(registry);
    let mut instance = definition.instantiate(1, 0).unwrap();
    assert!(
        instance
            .dispatch(Event::new(1, ()), &())
            .unwrap()
            .is_transitioned()
    );
    assert_eq!(*instance.data(), 2);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[test]
fn unknown_story_implementation_and_role_fail_explicitly() {
    let registry = BehaviorRegistry::<u64, (), (), (), (), Ids>::builder()
        .action(reference(Story::Increment), |_, _, _| Ok(()))
        .unwrap()
        .build();
    assert!(matches!(
        registry.resolve_action(1, &reference(Story::Missing)),
        Err(BindingError::UnknownBehavior { .. })
    ));
    let other = BehaviorRef::new(Story::Increment, Implementation::RulesTwo);
    assert!(matches!(
        registry.resolve_action(1, &other),
        Err(BindingError::UnknownImplementation { .. })
    ));
    assert!(matches!(
        registry.resolve_condition(1, &reference(Story::Increment)),
        Err(BindingError::SignatureMismatch { .. })
    ));
}

#[test]
fn registry_rejects_duplicate_declarations_and_enforces_its_bound() {
    let builder = || {
        BehaviorRegistry::<u64, (), (), (), (), Ids>::builder()
            .max_bindings(1)
            .unwrap()
            .action(reference(Story::Increment), |_, _, _| Ok(()))
            .unwrap()
    };
    assert!(matches!(
        builder().action(reference(Story::Increment), |_, _, _| Ok(())),
        Err(BindingError::DuplicateImplementation { .. })
    ));
    assert!(matches!(
        builder().condition(reference(Story::Allowed), |_, _, _| true),
        Err(BindingError::LimitExceeded { limit: 1 })
    ));
    assert!(matches!(
        BehaviorRegistry::<u64, (), (), (), (), Ids>::builder().max_bindings(0),
        Err(BindingError::InvalidLimit)
    ));
    assert!(matches!(
        BehaviorRegistry::<u64, (), (), (), (), Ids>::builder().max_bindings(usize::MAX),
        Err(BindingError::InvalidLimit)
    ));
}

#[test]
fn multiple_implementations_of_one_story_keep_the_same_signature() {
    let alternative = BehaviorRef::new(Story::Increment, Implementation::RulesTwo);
    let registry = BehaviorRegistry::<u64, (), (), (), (), Ids>::builder()
        .action(reference(Story::Increment), |_, _, _| Ok(()))
        .unwrap()
        .action(alternative.clone(), |_, _, _| Ok(()))
        .unwrap()
        .build();
    assert_eq!(
        registry.resolve_action(1, &alternative).unwrap().binding(),
        Some(&alternative)
    );
    let invalid = BehaviorRegistry::<u64, (), (), (), (), Ids>::builder()
        .action(reference(Story::Increment), |_, _, _| Ok(()))
        .unwrap()
        .condition(alternative, |_, _, _| true);
    assert!(matches!(
        invalid,
        Err(BindingError::SignatureMismatch { .. })
    ));
}

#[test]
fn capability_and_assertion_references_are_typed_and_preserved() {
    let calls = Arc::new(AtomicUsize::new(0));
    let assertion_calls = Arc::clone(&calls);
    let registry = BehaviorRegistry::<u64, (), (), (), (), Ids>::builder()
        .capability(reference(Story::Available), |_, _, _| {
            status_machina::CapabilityStatus::Available
        })
        .unwrap()
        .assertion(reference(Story::Safe), move |_| {
            assertion_calls.fetch_add(1, Ordering::SeqCst);
            true
        })
        .unwrap()
        .build();
    let capability = registry
        .resolve_capability(1, &reference(Story::Available))
        .unwrap();
    let assertion = registry
        .resolve_assertion(2, &reference(Story::Safe))
        .unwrap();
    assert_eq!(capability.binding(), Some(&reference(Story::Available)));
    assert_eq!(assertion.binding(), Some(&reference(Story::Safe)));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let definition = MachineDefinition::<u64, (), (), (), (), Ids>::builder(1)
        .initial(0)
        .state(StateDefinition::builder(0).build())
        .unwrap()
        .build()
        .unwrap();
    let instance = definition.instantiate(1, 0).unwrap();
    assert_eq!(
        instance.evaluate_capability(&capability, &()).status(),
        status_machina::CapabilityStatus::Available
    );
    let scenarios = status_machina::ScenarioSet::<(), (), Ids>::builder(
        status_machina::ScenarioLimits::builder().build().unwrap(),
    )
    .scenario(1, [(Event::new(1, ()), ())])
    .unwrap()
    .build()
    .unwrap();
    status_machina::ScenarioReplay::run(&definition, 2, 0, scenarios, [assertion]).unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[path = "behavior_bindings/inspection.rs"]
mod inspection;
