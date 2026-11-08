//! Replay must distinguish raised exceptions from failed safety assertions.
use status_machina::{
    AssertionId,
    DefaultIds,
    EvaluationError,
    EvaluationPhase,
    Event,
    EventId,
    MachineDefinition,
    MachineId,
    MachineInstanceId,
    ScenarioAssertion,
    ScenarioId,
    ScenarioLimits,
    ScenarioReplay,
    ScenarioReplayBuildError,
    ScenarioSet,
    StateDefinition,
    StateId,
    Transition,
    TransitionId,
};

#[test]
fn assertion_exception_stops_replay_without_fabricating_counterexample() {
    let ready = StateId::try_new("ready").unwrap();
    let done = StateId::try_new("done").unwrap();
    let event = EventId::try_new("go").unwrap();
    let definition = MachineDefinition::<(), (), ()>::builder(MachineId::try_new("test").unwrap())
        .initial(ready.clone())
        .state(StateDefinition::builder(ready.clone()).build())
        .unwrap()
        .state(StateDefinition::builder(done.clone()).build())
        .unwrap()
        .transition(
            Transition::builder(TransitionId::try_new("go").unwrap())
                .from(ready)
                .to(done)
                .on(event.clone())
                .build()
                .unwrap(),
        )
        .unwrap()
        .build()
        .unwrap();
    let cases =
        ScenarioSet::<(), (), DefaultIds>::builder(ScenarioLimits::builder().build().unwrap())
            .scenario(
                ScenarioId::try_new("case").unwrap(),
                [(Event::new(event, ()), ())],
            )
            .unwrap()
            .build()
            .unwrap();
    let assertion =
        ScenarioAssertion::<DefaultIds>::builder(AssertionId::try_new("fails").unwrap())
            .try_evaluate(|_| Err(EvaluationError::new("assertion exception")))
            .build()
            .unwrap();
    let result = ScenarioReplay::run(
        &definition,
        MachineInstanceId::try_new(uuid::Uuid::now_v7()).unwrap(),
        (),
        cases,
        [assertion],
    );
    assert!(matches!(
        result,
        Err(ScenarioReplayBuildError::EvaluationFailed {
            phase: EvaluationPhase::Assertion,
            ..
        })
    ));
}
