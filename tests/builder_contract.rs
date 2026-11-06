//! STM-02 public construction and structural error contracts.
use state_machine::{
    Condition,
    ConditionId,
    DefinitionBuildError,
    DefinitionBuilder,
    DispositionId,
    EventDisposition,
    EventId,
    MachineDefinition,
    MachineId,
    StateDefinition,
    StateId,
    Transition,
    TransitionBuildError,
    TransitionId,
};

#[path = "builder_contract/conditions.rs"]
mod conditions;
#[path = "builder_contract/ownership.rs"]
mod ownership;

fn state(name: &str) -> StateId {
    StateId::try_new(name).unwrap()
}
fn builder() -> DefinitionBuilder<u64, (), ()> {
    MachineDefinition::builder(MachineId::try_new("builder-contract").unwrap())
}
fn predicate() -> Condition<u64, (), ()> {
    Condition::builder(ConditionId::try_new("predicate").unwrap())
        .evaluate(|_: &u64, _: &(), _: &()| true)
        .build()
        .unwrap()
}
fn edge(name: &str, source: &str, target: &str) -> Transition<u64, (), ()> {
    Transition::builder(TransitionId::try_new(name).unwrap())
        .from(state(source))
        .on(EventId::try_new("tick").unwrap())
        .to(state(target))
        .build()
        .unwrap()
}
fn disposition(name: &str, source: &str) -> EventDisposition<u64, (), ()> {
    EventDisposition::builder(DispositionId::try_new(name).unwrap())
        .from(state(source))
        .on(EventId::try_new("ignore").unwrap())
        .when(predicate())
        .reason("explicit ignore")
        .build()
        .unwrap()
}

#[test]
fn missing_and_unknown_initial_states_are_distinguished() {
    assert!(matches!(
        builder().build(),
        Err(DefinitionBuildError::MissingInitialState)
    ));
    assert!(
        matches!(builder().initial(state("missing")).build(), Err(DefinitionBuildError::UnknownInitialState { state: actual }) if actual == state("missing"))
    );
}

#[test]
fn duplicate_graph_identities_are_rejected_with_the_offending_identity() {
    assert!(matches!(builder().initial(state("idle"))
        .state(StateDefinition::builder(state("idle")).build()).unwrap()
        .state(StateDefinition::builder(state("idle")).build()).unwrap().build(),
        Err(DefinitionBuildError::DuplicateState { state: actual }) if actual == state("idle")));
    assert!(matches!(builder().initial(state("idle"))
        .state(StateDefinition::builder(state("idle")).build()).unwrap()
        .transition(edge("same", "idle", "idle")).unwrap().transition(edge("same", "idle", "idle")).unwrap().build(),
        Err(DefinitionBuildError::DuplicateTransition { transition }) if transition == TransitionId::try_new("same").unwrap()));
    assert!(matches!(builder().initial(state("idle"))
        .state(StateDefinition::builder(state("idle")).build()).unwrap()
        .disposition(disposition("same", "idle")).unwrap().disposition(disposition("same", "idle")).unwrap().build(),
        Err(DefinitionBuildError::DuplicateDisposition { disposition }) if disposition == DispositionId::try_new("same").unwrap()));
}

#[test]
fn unknown_edge_and_disposition_endpoints_are_localized() {
    for (source, target) in [("missing", "idle"), ("idle", "missing")] {
        assert!(
            matches!(builder().initial(state("idle")).state(StateDefinition::builder(state("idle")).build()).unwrap()
            .transition(edge("edge", source, target)).unwrap().build(),
            Err(DefinitionBuildError::UnknownTransitionState { transition, state: actual }) if transition == TransitionId::try_new("edge").unwrap() && actual == state("missing"))
        );
    }
    assert!(
        matches!(builder().initial(state("idle")).state(StateDefinition::builder(state("idle")).build()).unwrap()
        .disposition(disposition("ignore", "missing")).unwrap().build(),
        Err(DefinitionBuildError::UnknownDispositionState { disposition, state: actual }) if disposition == DispositionId::try_new("ignore").unwrap() && actual == state("missing"))
    );
}

#[test]
fn edge_builder_rejects_missing_fields_and_internal_state_changes() {
    let edge = || Transition::<u64, (), ()>::builder(TransitionId::try_new("edge").unwrap());
    assert!(matches!(
        edge().build(),
        Err(TransitionBuildError::MissingSource)
    ));
    assert!(matches!(
        edge().from(state("idle")).build(),
        Err(TransitionBuildError::MissingEvent)
    ));
    assert!(matches!(
        edge()
            .from(state("idle"))
            .on(EventId::try_new("tick").unwrap())
            .build(),
        Err(TransitionBuildError::MissingDestination)
    ));
    assert!(matches!(
        edge()
            .from(state("idle"))
            .on(EventId::try_new("tick").unwrap())
            .to(state("other"))
            .internal()
            .build(),
        Err(TransitionBuildError::InternalTransitionMustBeSelf)
    ));
}
