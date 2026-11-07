//! Public API tests for caller-driven transition acceptance.

use status_machina::{
    ActionId,
    CandidateData,
    Condition,
    ConditionId,
    DispositionId,
    Effect,
    EffectId,
    Event,
    EventDisposition,
    EventId,
    LocalAction,
    MachineDefinition,
    MachineId,
    MachineInstanceId,
    StateDefinition,
    StateId,
    Transition,
    TransitionId,
};

#[derive(Clone, Debug, Eq, PartialEq)]
enum Payload {
    Open,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Data {
    opens: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Facts {
    may_open: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TraceData {
    actions: Vec<&'static str>,
}

fn open_machine(guarded: bool) -> MachineDefinition<Data, Payload, Facts> {
    let closed = StateId::try_new("closed").expect("valid state id");
    let open = StateId::try_new("open").expect("valid state id");
    let open_event = EventId::try_new("open").expect("valid event id");
    let action = LocalAction::builder(ActionId::try_new("increment-open-count").unwrap())
        .apply(|data: &mut Data, _payload: &Payload, _facts: &Facts| {
            data.opens += 1;
            Ok(())
        })
        .build()
        .unwrap();
    let mut transition = Transition::builder(TransitionId::try_new("open-door").unwrap())
        .from(closed.clone())
        .on(open_event)
        .to(open.clone())
        .action(action)
        .unwrap();
    if guarded {
        let condition = Condition::builder(ConditionId::try_new("permission-to-open").unwrap())
            .evaluate(|_data: &Data, _payload: &Payload, facts: &Facts| facts.may_open)
            .build()
            .unwrap();
        transition = transition.when(condition);
    }
    MachineDefinition::<_, _, _>::builder(MachineId::try_new("door").unwrap())
        .initial(closed.clone())
        .state(StateDefinition::builder(closed).build())
        .unwrap()
        .state(StateDefinition::builder(open).build())
        .unwrap()
        .transition(transition.build().unwrap())
        .unwrap()
        .build()
        .unwrap()
}

#[test]
fn dispatch_advances_only_when_caller_supplies_an_event() {
    let definition = open_machine(true);
    let mut instance = definition
        .instantiate(
            MachineInstanceId::try_new(uuid::Uuid::now_v7()).unwrap(),
            Data { opens: 0 },
        )
        .expect("valid initial data");

    assert_eq!(instance.state().as_str(), "closed");
    assert_eq!(instance.revision(), 0);

    let outcome = instance
        .dispatch(
            Event::new(EventId::try_new("open").unwrap(), Payload::Open),
            &Facts { may_open: false },
        )
        .unwrap();

    assert!(outcome.is_refused());
    assert_eq!(instance.state().as_str(), "closed");
    assert_eq!(instance.data().opens, 0);
    assert_eq!(instance.revision(), 0);
}

#[test]
fn dispatch_commits_state_data_and_revision_together() {
    let definition = open_machine(true);
    let mut instance = definition
        .instantiate(
            MachineInstanceId::try_new(uuid::Uuid::now_v7()).unwrap(),
            Data { opens: 0 },
        )
        .expect("valid initial data");

    let outcome = instance
        .dispatch(
            Event::new(EventId::try_new("open").unwrap(), Payload::Open),
            &Facts { may_open: true },
        )
        .unwrap();

    assert!(outcome.is_transitioned());
    assert_eq!(instance.state().as_str(), "open");
    assert_eq!(instance.data().opens, 1);
    assert_eq!(instance.revision(), 1);
}

#[test]
fn multiple_enabled_edges_are_rejected_without_mutation() {
    let closed = StateId::try_new("closed").unwrap();
    let open = StateId::try_new("open").unwrap();
    let event_id = EventId::try_new("open").unwrap();
    let first = Transition::builder(TransitionId::try_new("first").unwrap())
        .from(closed.clone())
        .on(event_id.clone())
        .to(open.clone())
        .build()
        .unwrap();
    let second = Transition::builder(TransitionId::try_new("second").unwrap())
        .from(closed.clone())
        .on(event_id.clone())
        .to(open.clone())
        .build()
        .unwrap();
    let definition = MachineDefinition::<_, _, _>::builder(MachineId::try_new("door").unwrap())
        .initial(closed.clone())
        .state(StateDefinition::builder(closed).build())
        .unwrap()
        .state(StateDefinition::builder(open).build())
        .unwrap()
        .transition(first)
        .unwrap()
        .transition(second)
        .unwrap()
        .build()
        .unwrap();
    let mut instance = definition
        .instantiate(
            MachineInstanceId::try_new(uuid::Uuid::now_v7()).unwrap(),
            Data { opens: 0 },
        )
        .expect("valid initial data");

    assert!(
        instance
            .dispatch(
                Event::new(event_id, Payload::Open),
                &Facts { may_open: true },
            )
            .is_err()
    );
    assert_eq!(instance.state().as_str(), "closed");
    assert_eq!(instance.data().opens, 0);
    assert_eq!(instance.revision(), 0);
}

#[test]
fn failed_candidate_action_leaves_instance_unchanged() {
    let closed = StateId::try_new("closed").unwrap();
    let open = StateId::try_new("open").unwrap();
    let event_id = EventId::try_new("open").unwrap();
    let action = LocalAction::builder(ActionId::try_new("fail-after-mutation").unwrap())
        .apply(|data: &mut Data, _payload: &Payload, _facts: &Facts| {
            data.opens = 99;
            Err(String::from("effect preparation failed"))
        })
        .build()
        .unwrap();
    let transition = Transition::builder(TransitionId::try_new("open-door").unwrap())
        .from(closed.clone())
        .on(event_id.clone())
        .to(open.clone())
        .action(action)
        .unwrap()
        .build()
        .unwrap();
    let definition = MachineDefinition::<_, _, _>::builder(MachineId::try_new("door").unwrap())
        .initial(closed.clone())
        .state(StateDefinition::builder(closed).build())
        .unwrap()
        .state(StateDefinition::builder(open).build())
        .unwrap()
        .transition(transition)
        .unwrap()
        .build()
        .unwrap();
    let mut instance = definition
        .instantiate(
            MachineInstanceId::try_new(uuid::Uuid::now_v7()).unwrap(),
            Data { opens: 0 },
        )
        .unwrap();

    assert!(
        instance
            .dispatch(
                Event::new(event_id, Payload::Open),
                &Facts { may_open: true }
            )
            .is_err()
    );
    assert_eq!(instance.state().as_str(), "closed");
    assert_eq!(instance.data().opens, 0);
    assert_eq!(instance.revision(), 0);
}

#[test]
fn invalid_initial_state_data_is_rejected_without_entry_action() {
    let initial = StateId::try_new("closed").unwrap();
    let definition =
        MachineDefinition::<Data, Payload, Facts>::builder(MachineId::try_new("door").unwrap())
            .initial(initial.clone())
            .state(
                StateDefinition::builder(initial)
                    .invariant(|data: &Data| data.opens == 0)
                    .build(),
            )
            .unwrap()
            .build()
            .unwrap();

    let result = definition.instantiate(
        MachineInstanceId::try_new(uuid::Uuid::now_v7()).unwrap(),
        Data { opens: 1 },
    );

    assert!(result.is_err());
}

#[test]
fn unreachable_states_are_rejected_when_definition_is_built() {
    let initial = StateId::try_new("closed").unwrap();
    let unreachable = StateId::try_new("open").unwrap();
    let result =
        MachineDefinition::<Data, Payload, Facts>::builder(MachineId::try_new("door").unwrap())
            .initial(initial.clone())
            .state(StateDefinition::builder(initial).build())
            .unwrap()
            .state(StateDefinition::builder(unreachable).build())
            .unwrap()
            .build();

    assert!(matches!(
        result,
        Err(status_machina::DefinitionBuildError::UnreachableState { .. })
    ));
}

#[test]
fn accepted_transition_returns_typed_effect_intents_to_its_caller() {
    let closed = StateId::try_new("closed").unwrap();
    let open = StateId::try_new("open").unwrap();
    let event_id = EventId::try_new("open").unwrap();
    let effect = Effect::builder(EffectId::try_new("notify-opened").unwrap())
        .describe(|data: &Data, _payload: &Payload, _facts: &Facts| {
            format!("door opened {} times", data.opens)
        })
        .build()
        .unwrap();
    let action = LocalAction::builder(ActionId::try_new("increment-open-count").unwrap())
        .apply(|data: &mut Data, _payload: &Payload, _facts: &Facts| {
            data.opens += 1;
            Ok(())
        })
        .build()
        .unwrap();
    let transition = Transition::<Data, Payload, Facts, String>::builder_with_effects(
        TransitionId::try_new("open-door").unwrap(),
    )
    .from(closed.clone())
    .on(event_id.clone())
    .to(open.clone())
    .action(action)
    .unwrap()
    .effect(effect)
    .unwrap()
    .build()
    .unwrap();
    let definition = MachineDefinition::<Data, Payload, Facts, String>::builder_with_effects(
        MachineId::try_new("door").unwrap(),
    )
    .initial(closed.clone())
    .state(StateDefinition::builder(closed).build())
    .unwrap()
    .state(StateDefinition::builder(open).build())
    .unwrap()
    .transition(transition)
    .unwrap()
    .build()
    .unwrap();
    let mut instance = definition
        .instantiate(
            MachineInstanceId::try_new(uuid::Uuid::now_v7()).unwrap(),
            Data { opens: 0 },
        )
        .unwrap();

    let outcome = instance
        .dispatch(
            Event::new(event_id, Payload::Open),
            &Facts { may_open: true },
        )
        .unwrap();

    let status_machina::DispatchOutcome::Transitioned { effects, .. } = outcome else {
        panic!("event should be accepted");
    };
    assert_eq!(effects.len(), 1);
    assert_eq!(effects[0].id().as_str(), "notify-opened");
    assert_eq!(effects[0].payload(), "door opened 1 times");
}

#[test]
fn external_transition_runs_exit_transition_and_entry_actions_in_order() {
    let closed = StateId::try_new("closed").unwrap();
    let open = StateId::try_new("open").unwrap();
    let event_id = EventId::try_new("open").unwrap();
    let exit = LocalAction::builder(ActionId::try_new("exit-closed").unwrap())
        .apply(|data: &mut TraceData, _payload: &Payload, _facts: &Facts| {
            data.actions.push("exit");
            Ok(())
        })
        .build()
        .unwrap();
    let transition_action = LocalAction::builder(ActionId::try_new("open-door").unwrap())
        .apply(|data: &mut TraceData, _payload: &Payload, _facts: &Facts| {
            data.actions.push("transition");
            Ok(())
        })
        .build()
        .unwrap();
    let entry = LocalAction::builder(ActionId::try_new("enter-open").unwrap())
        .apply(|data: &mut TraceData, _payload: &Payload, _facts: &Facts| {
            data.actions.push("entry");
            Ok(())
        })
        .build()
        .unwrap();
    let closed_state = StateDefinition::builder(closed.clone())
        .exit_action(exit)
        .unwrap()
        .build();
    let open_state = StateDefinition::builder(open.clone())
        .entry_action(entry)
        .unwrap()
        .build();
    let transition = Transition::builder(TransitionId::try_new("open-door").unwrap())
        .from(closed.clone())
        .on(event_id.clone())
        .to(open)
        .action(transition_action)
        .unwrap()
        .build()
        .unwrap();
    let definition = MachineDefinition::<TraceData, Payload, Facts>::builder(
        MachineId::try_new("door-trace").unwrap(),
    )
    .initial(closed)
    .state(closed_state)
    .unwrap()
    .state(open_state)
    .unwrap()
    .transition(transition)
    .unwrap()
    .build()
    .unwrap();
    let mut instance = definition
        .instantiate(
            MachineInstanceId::try_new(uuid::Uuid::now_v7()).unwrap(),
            TraceData {
                actions: Vec::new(),
            },
        )
        .unwrap();

    assert!(instance.data().actions.is_empty());
    instance
        .dispatch(
            Event::new(event_id, Payload::Open),
            &Facts { may_open: true },
        )
        .unwrap();

    assert_eq!(instance.data().actions, ["exit", "transition", "entry"]);
}

#[test]
fn internal_self_transition_skips_exit_and_entry_actions() {
    let state = StateId::try_new("waiting").unwrap();
    let event_id = EventId::try_new("tick").unwrap();
    let exit = LocalAction::builder(ActionId::try_new("exit").unwrap())
        .apply(|data: &mut TraceData, _payload: &Payload, _facts: &Facts| {
            data.actions.push("exit");
            Ok(())
        })
        .build()
        .unwrap();
    let entry = LocalAction::builder(ActionId::try_new("entry").unwrap())
        .apply(|data: &mut TraceData, _payload: &Payload, _facts: &Facts| {
            data.actions.push("entry");
            Ok(())
        })
        .build()
        .unwrap();
    let action = LocalAction::builder(ActionId::try_new("record-tick").unwrap())
        .apply(|data: &mut TraceData, _payload: &Payload, _facts: &Facts| {
            data.actions.push("transition");
            Ok(())
        })
        .build()
        .unwrap();
    let transition = Transition::builder(TransitionId::try_new("tick").unwrap())
        .from(state.clone())
        .on(event_id.clone())
        .to(state.clone())
        .internal()
        .action(action)
        .unwrap()
        .build()
        .unwrap();
    let definition = MachineDefinition::<TraceData, Payload, Facts>::builder(
        MachineId::try_new("waiter").unwrap(),
    )
    .initial(state.clone())
    .state(
        StateDefinition::builder(state)
            .exit_action(exit)
            .unwrap()
            .entry_action(entry)
            .unwrap()
            .build(),
    )
    .unwrap()
    .transition(transition)
    .unwrap()
    .build()
    .unwrap();
    let mut instance = definition
        .instantiate(
            MachineInstanceId::try_new(uuid::Uuid::now_v7()).unwrap(),
            TraceData {
                actions: Vec::new(),
            },
        )
        .unwrap();

    instance
        .dispatch(
            Event::new(event_id, Payload::Open),
            &Facts { may_open: true },
        )
        .unwrap();

    assert_eq!(instance.data().actions, ["transition"]);
}

#[test]
fn dispatch_refuses_a_stale_expected_revision_without_mutation() {
    let definition = open_machine(true);
    let mut instance = definition
        .instantiate(
            MachineInstanceId::try_new(uuid::Uuid::now_v7()).unwrap(),
            Data { opens: 0 },
        )
        .unwrap();

    let result = instance.dispatch_at(
        1,
        Event::new(EventId::try_new("open").unwrap(), Payload::Open),
        &Facts { may_open: true },
    );

    assert!(matches!(
        result,
        Err(status_machina::DispatchError::StaleRevision { .. })
    ));
    assert_eq!(instance.state().as_str(), "closed");
    assert_eq!(instance.data().opens, 0);
    assert_eq!(instance.revision(), 0);
}

#[test]
fn terminal_states_cannot_declare_outgoing_transitions() {
    let finished = StateId::try_new("finished").unwrap();
    let transition = Transition::builder(TransitionId::try_new("restart").unwrap())
        .from(finished.clone())
        .on(EventId::try_new("restart").unwrap())
        .to(finished.clone())
        .build()
        .unwrap();
    let result =
        MachineDefinition::<Data, Payload, Facts>::builder(MachineId::try_new("terminal").unwrap())
            .initial(finished.clone())
            .state(StateDefinition::builder(finished).terminal().build())
            .unwrap()
            .transition(transition)
            .unwrap()
            .build();

    assert!(matches!(
        result,
        Err(status_machina::DefinitionBuildError::TerminalHasOutgoingTransition { .. })
    ));
}

#[test]
fn failed_postcondition_discards_candidate_actions_and_effect_intents() {
    let closed = StateId::try_new("closed").unwrap();
    let open = StateId::try_new("open").unwrap();
    let event_id = EventId::try_new("open").unwrap();
    let action = LocalAction::builder(ActionId::try_new("increment-open-count").unwrap())
        .apply(|data: &mut Data, _payload: &Payload, _facts: &Facts| {
            data.opens += 1;
            Ok(())
        })
        .build()
        .unwrap();
    let postcondition = Condition::builder(ConditionId::try_new("must-remain-unopened").unwrap())
        .evaluate(|data: &Data, _payload: &Payload, _facts: &Facts| data.opens == 0)
        .build()
        .unwrap();
    let transition = Transition::builder(TransitionId::try_new("open-door").unwrap())
        .from(closed.clone())
        .on(event_id.clone())
        .to(open.clone())
        .action(action)
        .unwrap()
        .postcondition(postcondition)
        .unwrap()
        .build()
        .unwrap();
    let definition = MachineDefinition::<Data, Payload, Facts>::builder(
        MachineId::try_new("door-postcondition").unwrap(),
    )
    .initial(closed.clone())
    .state(StateDefinition::builder(closed).build())
    .unwrap()
    .state(StateDefinition::builder(open).build())
    .unwrap()
    .transition(transition)
    .unwrap()
    .build()
    .unwrap();
    let mut instance = definition
        .instantiate(
            MachineInstanceId::try_new(uuid::Uuid::now_v7()).unwrap(),
            Data { opens: 0 },
        )
        .unwrap();

    assert!(matches!(
        instance.dispatch(
            Event::new(event_id, Payload::Open),
            &Facts { may_open: true },
        ),
        Err(status_machina::DispatchError::PostconditionFailed { .. })
    ));
    assert_eq!(instance.state().as_str(), "closed");
    assert_eq!(instance.data().opens, 0);
    assert_eq!(instance.revision(), 0);
}

#[test]
fn nested_all_and_any_conditions_evaluate_with_bounded_composition() {
    let allowed = Condition::builder(ConditionId::try_new("allowed").unwrap())
        .evaluate(|_data: &Data, _event: &Payload, facts: &Facts| facts.may_open)
        .build()
        .unwrap();
    let payload_matches = Condition::builder(ConditionId::try_new("open-event").unwrap())
        .evaluate(|_data: &Data, event: &Payload, _facts: &Facts| *event == Payload::Open)
        .build()
        .unwrap();
    let any = Condition::any(ConditionId::try_new("either-check").unwrap())
        .condition(allowed)
        .unwrap()
        .condition(payload_matches)
        .unwrap()
        .build()
        .unwrap();
    let all = Condition::all(ConditionId::try_new("all-checks").unwrap())
        .condition(any)
        .unwrap()
        .condition(
            Condition::builder(ConditionId::try_new("one-open-so-far").unwrap())
                .evaluate(|data: &Data, _event: &Payload, _facts: &Facts| data.opens == 0)
                .build()
                .unwrap(),
        )
        .unwrap()
        .build()
        .unwrap();
    let event_id = EventId::try_new("open").unwrap();
    let closed = StateId::try_new("closed").unwrap();
    let open = StateId::try_new("open").unwrap();
    let transition = Transition::builder(TransitionId::try_new("open-door").unwrap())
        .from(closed.clone())
        .on(event_id.clone())
        .to(open.clone())
        .when(all)
        .build()
        .unwrap();
    let definition = MachineDefinition::<Data, Payload, Facts>::builder(
        MachineId::try_new("door-conditions").unwrap(),
    )
    .initial(closed.clone())
    .state(StateDefinition::builder(closed).build())
    .unwrap()
    .state(StateDefinition::builder(open).build())
    .unwrap()
    .transition(transition)
    .unwrap()
    .build()
    .unwrap();
    let mut instance = definition
        .instantiate(
            MachineInstanceId::try_new(uuid::Uuid::now_v7()).unwrap(),
            Data { opens: 0 },
        )
        .unwrap();

    let outcome = instance
        .dispatch(
            Event::new(event_id, Payload::Open),
            &Facts { may_open: true },
        )
        .unwrap();

    assert!(outcome.is_transitioned());
}

#[test]
fn not_condition_negates_its_single_child() {
    let waiting = StateId::try_new("waiting").unwrap();
    let ready = StateId::try_new("ready").unwrap();
    let event_id = EventId::try_new("retry-due").unwrap();
    let not_allowed = Condition::not(ConditionId::try_new("not-yet-allowed").unwrap())
        .condition(
            Condition::builder(ConditionId::try_new("already-allowed").unwrap())
                .evaluate(|_data: &Data, _event: &Payload, facts: &Facts| facts.may_open)
                .build()
                .unwrap(),
        )
        .unwrap()
        .build()
        .unwrap();
    let transition = Transition::builder(TransitionId::try_new("retry").unwrap())
        .from(waiting.clone())
        .on(event_id.clone())
        .to(ready.clone())
        .when(not_allowed)
        .build()
        .unwrap();
    let definition = MachineDefinition::<Data, Payload, Facts>::builder(
        MachineId::try_new("retry-machine").unwrap(),
    )
    .initial(waiting.clone())
    .state(StateDefinition::builder(waiting).build())
    .unwrap()
    .state(StateDefinition::builder(ready).build())
    .unwrap()
    .transition(transition)
    .unwrap()
    .build()
    .unwrap();
    let mut instance = definition
        .instantiate(
            MachineInstanceId::try_new(uuid::Uuid::now_v7()).unwrap(),
            Data { opens: 0 },
        )
        .unwrap();

    let outcome = instance
        .dispatch(
            Event::new(event_id, Payload::Open),
            &Facts { may_open: false },
        )
        .unwrap();

    assert!(outcome.is_transitioned());
}

#[test]
fn explicit_event_disposition_is_reported_without_mutating_the_instance() {
    let waiting = StateId::try_new("waiting").unwrap();
    let event_id = EventId::try_new("late-completion").unwrap();
    let condition = Condition::builder(ConditionId::try_new("is-stale").unwrap())
        .evaluate(|_data: &Data, _event: &Payload, _facts: &Facts| true)
        .build()
        .unwrap();
    let disposition = EventDisposition::builder(DispositionId::try_new("ignore-stale").unwrap())
        .from(waiting.clone())
        .on(event_id.clone())
        .when(condition)
        .reason("completion belongs to an older operation")
        .build()
        .unwrap();
    let definition =
        MachineDefinition::<Data, Payload, Facts>::builder(MachineId::try_new("waiter").unwrap())
            .initial(waiting.clone())
            .state(StateDefinition::builder(waiting).build())
            .unwrap()
            .disposition(disposition)
            .unwrap()
            .build()
            .unwrap();
    let mut instance = definition
        .instantiate(
            MachineInstanceId::try_new(uuid::Uuid::now_v7()).unwrap(),
            Data { opens: 1 },
        )
        .unwrap();

    let outcome = instance
        .dispatch(
            Event::new(event_id, Payload::Open),
            &Facts { may_open: false },
        )
        .unwrap();

    assert!(matches!(
        outcome,
        status_machina::DispatchOutcome::Ignored { .. }
    ));
    assert_eq!(instance.state().as_str(), "waiting");
    assert_eq!(instance.data().opens, 1);
    assert_eq!(instance.revision(), 0);
}

#[test]
fn a_matching_disposition_and_transition_are_ambiguous() {
    let waiting = StateId::try_new("waiting").unwrap();
    let ready = StateId::try_new("ready").unwrap();
    let event_id = EventId::try_new("complete").unwrap();
    let transition = Transition::builder(TransitionId::try_new("accept-completion").unwrap())
        .from(waiting.clone())
        .on(event_id.clone())
        .to(ready.clone())
        .build()
        .unwrap();
    let disposition =
        EventDisposition::builder(DispositionId::try_new("ignore-completion").unwrap())
            .from(waiting.clone())
            .on(event_id.clone())
            .when(
                Condition::builder(ConditionId::try_new("always").unwrap())
                    .evaluate(|_data: &Data, _event: &Payload, _facts: &Facts| true)
                    .build()
                    .unwrap(),
            )
            .reason("ambiguous completion")
            .build()
            .unwrap();
    let definition =
        MachineDefinition::<Data, Payload, Facts>::builder(MachineId::try_new("waiter").unwrap())
            .initial(waiting.clone())
            .state(StateDefinition::builder(waiting).build())
            .unwrap()
            .state(StateDefinition::builder(ready).build())
            .unwrap()
            .transition(transition)
            .unwrap()
            .disposition(disposition)
            .unwrap()
            .build()
            .unwrap();
    let mut instance = definition
        .instantiate(
            MachineInstanceId::try_new(uuid::Uuid::now_v7()).unwrap(),
            Data { opens: 0 },
        )
        .unwrap();

    assert!(matches!(
        instance.dispatch(
            Event::new(event_id, Payload::Open),
            &Facts { may_open: true },
        ),
        Err(status_machina::DispatchError::Ambiguous { .. })
    ));
    assert_eq!(instance.state().as_str(), "waiting");
    assert_eq!(instance.revision(), 0);
}

impl CandidateData for Data {
    fn snapshot(&self) -> Self {
        // This domain value owns its mutable fields; cloning shares no mutable
        // state.
        self.clone()
    }
}

impl CandidateData for TraceData {
    fn snapshot(&self) -> Self {
        // This domain value owns its mutable fields; cloning shares no mutable
        // state.
        self.clone()
    }
}
