//! Regression coverage for bounded finite-scenario replay.

use state_machine::{
    AssertionId,
    Effect,
    EffectId,
    Event,
    EventId,
    MachineDefinition,
    MachineId,
    MachineInstanceId,
    OutputEvent,
    ScenarioAssertion,
    ScenarioId,
    ScenarioLimits,
    ScenarioReplay,
    ScenarioResult,
    ScenarioSet,
    ScenarioSetBuildError,
    ScenarioStepKind,
    StateDefinition,
    StateId,
    Transition,
    TransitionId,
};

#[derive(Clone, Debug, Eq, PartialEq)]
struct Trigger;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Facts;

fn definition() -> MachineDefinition<(), Trigger, Facts> {
    let waiting = StateId::try_new("waiting").unwrap();
    let admitted = StateId::try_new("admitted").unwrap();
    let transition = Transition::builder(TransitionId::try_new("admit").unwrap())
        .from(waiting.clone())
        .on(EventId::try_new("request").unwrap())
        .to(admitted.clone())
        .build()
        .unwrap();

    MachineDefinition::builder(MachineId::try_new("admission").unwrap())
        .initial(waiting.clone())
        .state(StateDefinition::builder(waiting).build())
        .unwrap()
        .state(StateDefinition::builder(admitted).build())
        .unwrap()
        .transition(transition)
        .unwrap()
        .build()
        .unwrap()
}

fn definition_with_effect_and_output() -> MachineDefinition<(), Trigger, Facts, (), ()> {
    let waiting = StateId::try_new("waiting").unwrap();
    let done = StateId::try_new("done").unwrap();
    let effect = Effect::builder(EffectId::try_new("notify").unwrap())
        .describe(|_data: &(), _event: &Trigger, _facts: &Facts| ())
        .build()
        .unwrap();
    let output = OutputEvent::builder(EventId::try_new("follow-up").unwrap())
        .describe(|_data: &(), _event: &Trigger, _facts: &Facts| ())
        .build()
        .unwrap();
    let transition =
        Transition::<(), Trigger, Facts, (), ()>::builder_with_effects_and_output_events(
            TransitionId::try_new("finish").unwrap(),
        )
        .from(waiting.clone())
        .on(EventId::try_new("request").unwrap())
        .to(done.clone())
        .effect(effect)
        .unwrap()
        .output_event(output)
        .unwrap()
        .build()
        .unwrap();

    MachineDefinition::builder_with_effects_and_output_events(
        MachineId::try_new("effects-and-outputs").unwrap(),
    )
    .initial(waiting.clone())
    .state(StateDefinition::builder(waiting).build())
    .unwrap()
    .state(StateDefinition::builder(done).build())
    .unwrap()
    .transition(transition)
    .unwrap()
    .build()
    .unwrap()
}

fn instance_id() -> MachineInstanceId {
    MachineInstanceId::try_new(uuid::Uuid::from_u128(
        0x0000_0000_0000_7000_8000_0000_0000_0001,
    ))
    .unwrap()
}

fn scenario_set(limits: ScenarioLimits) -> ScenarioSet<Trigger, Facts> {
    ScenarioSet::<_, _, state_machine::DefaultIds>::builder(limits)
        .scenario(
            ScenarioId::try_new("admission-case").unwrap(),
            [(
                Event::new(EventId::try_new("request").unwrap(), Trigger),
                Facts,
            )],
        )
        .unwrap()
        .build()
        .unwrap()
}

#[test]
fn replay_reports_counterexample_with_only_bounded_step_metadata() {
    let assertion = ScenarioAssertion::<state_machine::DefaultIds>::builder(
        AssertionId::try_new("never-admit").unwrap(),
    )
    .evaluate(|observation| observation.after().as_str() != "admitted")
    .build()
    .unwrap();
    let report = ScenarioReplay::run(
        &definition(),
        instance_id(),
        (),
        scenario_set(ScenarioLimits::builder().build().unwrap()),
        [assertion],
    )
    .unwrap();

    let ScenarioResult::Counterexample(trace) = report.result() else {
        panic!("expected the independent assertion to report a counterexample");
    };
    assert_eq!(trace.assertion().as_str(), "never-admit");
    assert_eq!(trace.steps().len(), 1);
    assert_eq!(trace.steps()[0].input_event().as_str(), "request");
    assert_eq!(trace.steps()[0].after().as_str(), "admitted");
}

#[test]
fn replay_success_is_only_complete_within_supplied_scenarios() {
    let assertion = ScenarioAssertion::<state_machine::DefaultIds>::builder(
        AssertionId::try_new("transition-recorded").unwrap(),
    )
    .evaluate(|observation| {
        matches!(observation.kind(), ScenarioStepKind::Transitioned(_))
            || observation.after().as_str() == "waiting"
    })
    .build()
    .unwrap();
    let report = ScenarioReplay::run(
        &definition(),
        instance_id(),
        (),
        scenario_set(ScenarioLimits::builder().build().unwrap()),
        [assertion],
    )
    .unwrap();

    assert!(matches!(
        report.result(),
        ScenarioResult::CompleteWithinScenarios {
            scenarios: 1,
            nodes: 1
        }
    ));
}

#[test]
fn replay_stops_before_exceeding_trace_bound() {
    let limits = ScenarioLimits::builder()
        .max_steps_per_scenario(2)
        .max_nodes(2)
        .max_outputs(1)
        .max_trace_entries(1)
        .build()
        .unwrap();
    let scenarios = ScenarioSet::<_, _, state_machine::DefaultIds>::builder(limits)
        .scenario(
            ScenarioId::try_new("two-steps").unwrap(),
            [
                (
                    Event::new(EventId::try_new("request").unwrap(), Trigger),
                    Facts,
                ),
                (
                    Event::new(EventId::try_new("request").unwrap(), Trigger),
                    Facts,
                ),
            ],
        )
        .unwrap()
        .build()
        .unwrap();
    let report = ScenarioReplay::run(
        &definition(),
        instance_id(),
        (),
        scenarios,
        std::iter::empty(),
    )
    .unwrap();

    assert!(matches!(
        report.result(),
        ScenarioResult::LimitReached {
            scenario,
            step: 1,
            limit: state_machine::ScenarioLimit::TraceEntries,
        }
            if scenario.as_str() == "two-steps"
    ));
}

#[test]
fn scenario_set_bounds_aggregate_supplied_steps_by_the_node_limit() {
    let limits = ScenarioLimits::builder().max_nodes(1).build().unwrap();
    let first = ScenarioSet::<_, _, state_machine::DefaultIds>::builder(limits)
        .scenario(
            ScenarioId::try_new("first").unwrap(),
            [(
                Event::new(EventId::try_new("request").unwrap(), Trigger),
                Facts,
            )],
        )
        .unwrap();
    let over_limit = first.scenario(
        ScenarioId::try_new("second").unwrap(),
        [(
            Event::new(EventId::try_new("request").unwrap(), Trigger),
            Facts,
        )],
    );

    assert!(matches!(
        over_limit,
        Err(ScenarioSetBuildError::TotalNodeLimitExceeded { limit: 1 })
    ));
}

#[test]
fn scenario_set_enforces_scenario_and_per_scenario_step_caps() {
    let limits = ScenarioLimits::builder()
        .max_scenarios(1)
        .max_steps_per_scenario(1)
        .max_nodes(2)
        .build()
        .unwrap();
    let too_many_steps = ScenarioSet::<_, _, state_machine::DefaultIds>::builder(limits).scenario(
        ScenarioId::try_new("two-steps").unwrap(),
        [
            (
                Event::new(EventId::try_new("request").unwrap(), Trigger),
                Facts,
            ),
            (
                Event::new(EventId::try_new("request").unwrap(), Trigger),
                Facts,
            ),
        ],
    );
    assert!(matches!(
        too_many_steps,
        Err(ScenarioSetBuildError::StepLimitExceeded { limit: 1 })
    ));

    let one_scenario = ScenarioSet::<_, _, state_machine::DefaultIds>::builder(limits)
        .scenario(
            ScenarioId::try_new("first").unwrap(),
            [(
                Event::new(EventId::try_new("request").unwrap(), Trigger),
                Facts,
            )],
        )
        .unwrap();
    let too_many_scenarios = one_scenario.scenario(ScenarioId::try_new("second").unwrap(), []);
    assert!(matches!(
        too_many_scenarios,
        Err(ScenarioSetBuildError::ScenarioLimitExceeded { limit: 1 })
    ));
}

#[test]
fn output_budget_counts_effects_and_events_without_retaining_payloads() {
    let scenarios = scenario_set(ScenarioLimits::builder().max_outputs(2).build().unwrap());
    let assertion = ScenarioAssertion::<state_machine::DefaultIds>::builder(
        AssertionId::try_new("intent-ids").unwrap(),
    )
    .evaluate(|observation| {
        observation.output_events()[0].as_str() == "follow-up"
            && observation.effects()[0].as_str() == "notify"
    })
    .build()
    .unwrap();
    let complete = ScenarioReplay::run(
        &definition_with_effect_and_output(),
        instance_id(),
        (),
        scenarios,
        [assertion],
    )
    .unwrap();
    assert!(matches!(
        complete.result(),
        ScenarioResult::CompleteWithinScenarios { .. }
    ));

    let limited = ScenarioReplay::run(
        &definition_with_effect_and_output(),
        instance_id(),
        (),
        scenario_set(ScenarioLimits::builder().max_outputs(1).build().unwrap()),
        std::iter::empty(),
    )
    .unwrap();
    assert!(matches!(
        limited.result(),
        ScenarioResult::LimitReached {
            limit: state_machine::ScenarioLimit::Outputs,
            ..
        }
    ));
}
