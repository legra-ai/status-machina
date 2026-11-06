//! Neutral graph records bind through the same builders as native definitions.
use state_machine::{
    BehaviorRef,
    BehaviorRegistry,
    BehaviorUse,
    ConditionRecord,
    DefinitionBuildError,
    DefinitionRecord,
    Event,
    MachineDefinition,
    MachineIds,
    RecordBuildError,
    StateDefinition,
    StateRecord,
    Transition,
    TransitionDefinitionRecord,
    TransitionKind,
};

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
struct Ids;
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
enum State {
    Running,
    Done,
}
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
enum Kind {
    Complete,
}
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
enum Story {
    Allowed,
    Complete,
}
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
enum Implementation {
    Rules,
}
impl MachineIds for Ids {
    type Definition = u8;
    type Instance = u8;
    type State = State;
    type Event = Kind;
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
    BehaviorRef::new(story, Implementation::Rules)
}
fn registry() -> BehaviorRegistry<u64, (), (), (), (), Ids> {
    BehaviorRegistry::builder()
        .condition(reference(Story::Allowed), |data, _, _| *data == 0)
        .unwrap()
        .action(reference(Story::Complete), |data, _, _| {
            *data += 1;
            Ok(())
        })
        .unwrap()
        .build()
}

#[test]
fn record_and_rust_graphs_have_identical_execution_and_binding_identities() {
    let registry = registry();
    let guard = ConditionRecord::Predicate(BehaviorUse::new(1, reference(Story::Allowed)));
    let actions = [BehaviorUse::new(1, reference(Story::Complete))];
    let states = [
        StateRecord::new(State::Running),
        StateRecord::new(State::Done).terminal(),
    ];
    let mut edge = TransitionDefinitionRecord::new(1, State::Running, Kind::Complete, State::Done);
    edge.guard = Some(&guard);
    edge.actions = &actions;
    let edges = [edge];
    let records = DefinitionRecord::new(1, State::Running, &states, &edges);
    let from_records = records.bind(&registry).unwrap();
    let native_edge = Transition::builder(1)
        .from(State::Running)
        .on(Kind::Complete)
        .to(State::Done)
        .when(
            registry
                .resolve_condition(1, &reference(Story::Allowed))
                .unwrap(),
        )
        .action(
            registry
                .resolve_action(1, &reference(Story::Complete))
                .unwrap(),
        )
        .unwrap()
        .build()
        .unwrap();
    let native = MachineDefinition::<u64, (), (), (), (), Ids>::builder(1)
        .initial(State::Running)
        .state(StateDefinition::builder(State::Running).build())
        .unwrap()
        .state(StateDefinition::builder(State::Done).terminal().build())
        .unwrap()
        .transition(native_edge)
        .unwrap()
        .build()
        .unwrap();
    assert!(from_records.bindings().eq(native.bindings()));
    let mut left = from_records.instantiate(1, 0).unwrap();
    let mut right = native.instantiate(2, 0).unwrap();
    assert!(
        left.dispatch(Event::new(Kind::Complete, ()), &())
            .unwrap()
            .is_transitioned()
    );
    assert!(
        right
            .dispatch(Event::new(Kind::Complete, ()), &())
            .unwrap()
            .is_transitioned()
    );
    assert_eq!(left.state(), right.state());
    assert_eq!(left.data(), right.data());
}

#[test]
fn records_reject_missing_bindings_and_invalid_graphs_before_instantiation() {
    let registry = registry();
    let unknown = BehaviorUse::new(1, BehaviorRef::new(Story::Complete, Implementation::Rules));
    let guards = ConditionRecord::Predicate(unknown);
    let states = [StateRecord::new(State::Running)];
    let mut edge = TransitionDefinitionRecord::new(1, State::Running, Kind::Complete, State::Done);
    edge.guard = Some(&guards);
    let edges = [edge];
    let result = DefinitionRecord::new(1, State::Running, &states, &edges).bind(&registry);
    assert!(matches!(result, Err(RecordBuildError::Binding(_))));
    let edges = [TransitionDefinitionRecord::new(
        1,
        State::Running,
        Kind::Complete,
        State::Done,
    )];
    assert!(matches!(
        DefinitionRecord::new(1, State::Running, &states, &edges).bind(&registry),
        Err(RecordBuildError::Definition(
            DefinitionBuildError::UnknownTransitionState { .. }
        ))
    ));
    let mut internal =
        TransitionDefinitionRecord::new(1, State::Running, Kind::Complete, State::Done);
    internal.kind = TransitionKind::Internal;
    assert!(matches!(
        DefinitionRecord::new(1, State::Running, &states, &[internal]).bind(&registry),
        Err(RecordBuildError::Transition { .. })
    ));
}

#[path = "records/limits.rs"]
mod limits;

#[path = "records/behaviors.rs"]
mod behaviors;
