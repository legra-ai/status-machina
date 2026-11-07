//! Definition inspection must describe structure without executing behavior.

use status_machina::{
    Condition,
    ConditionId,
    DispositionId,
    EventDisposition,
    EventId,
    InspectionError,
    InspectionLimits,
    MachineDefinition,
    MachineId,
    StateDefinition,
    StateId,
    Transition,
    TransitionId,
};

fn definition(reverse: bool) -> MachineDefinition<(), (), ()> {
    let ready = StateId::try_new("ready\"<script>").unwrap();
    let waiting = StateId::try_new("waiting").unwrap();
    let predicate = || {
        Condition::builder(ConditionId::try_new("authorized").unwrap())
            .evaluate(|_: &(), _: &(), _: &()| panic!("inspection ran a guard"))
            .build()
            .unwrap()
    };
    let mut builder =
        MachineDefinition::builder(MachineId::try_new("example").unwrap()).initial(waiting.clone());
    let states = if reverse {
        [waiting.clone(), ready.clone()]
    } else {
        [ready.clone(), waiting.clone()]
    };
    for state in states {
        builder = builder
            .state(StateDefinition::builder(state).build())
            .unwrap();
    }
    builder
        .transition(
            Transition::builder(TransitionId::try_new("connect").unwrap())
                .from(waiting.clone())
                .on(EventId::try_new("connected").unwrap())
                .to(ready.clone())
                .when(predicate())
                .build()
                .unwrap(),
        )
        .unwrap()
        .disposition(
            EventDisposition::builder(DispositionId::try_new("old-completion").unwrap())
                .from(ready)
                .on(EventId::try_new("connected").unwrap())
                .when(predicate())
                .reason("obsolete attempt")
                .build()
                .unwrap(),
        )
        .unwrap()
        .build()
        .unwrap()
}

#[test]
fn diagram_is_derived_canonical_and_does_not_execute_guards() {
    let limits = InspectionLimits::builder().build().unwrap();
    let diagram = definition(false).diagram(&limits).unwrap();
    assert_eq!(diagram, definition(true).diagram(&limits).unwrap());
    assert!(diagram.starts_with("stateDiagram-v2\n"));
    assert!(diagram.contains("ready#34;#60;script#62;"));
    assert!(!diagram.contains("<script>"));
    assert!(diagram.contains("connect / connected"));
    assert!(diagram.contains("guard: authorized"));
    assert!(diagram.contains("disposition: old-completion"));
    assert!(diagram.contains("obsolete attempt"));
    assert!(diagram.contains("[*] --> s1"));
}

#[test]
fn structural_coverage_reports_edges_without_claiming_execution_coverage() {
    let report = definition(false)
        .structural_coverage(&InspectionLimits::builder().build().unwrap())
        .unwrap();
    assert_eq!(report.states().len(), 2);
    assert_eq!(report.transitions().len(), 1);
    assert_eq!(report.dispositions().len(), 1);
    let waiting = report
        .states()
        .iter()
        .find(|state| state.id().as_str() == "waiting")
        .unwrap();
    assert!(waiting.is_initial());
    assert_eq!(waiting.outgoing(), 1);
    assert_eq!(waiting.incoming(), 0);
    let ready = &report.states()[0];
    assert_eq!(ready.incoming(), 1);
    assert_eq!(ready.dispositions(), 1);
}

#[test]
fn inspection_refuses_small_resource_budgets() {
    let definition = definition(false);
    let entries = InspectionLimits::builder().max_entries(3).build().unwrap();
    assert!(matches!(
        definition.structural_coverage(&entries),
        Err(InspectionError::EntryLimit { .. })
    ));
    let bytes = InspectionLimits::builder()
        .max_output_bytes(5)
        .build()
        .unwrap();
    assert!(matches!(
        definition.diagram(&bytes),
        Err(InspectionError::OutputLimit { .. })
    ));
    assert!(InspectionLimits::builder().max_entries(0).build().is_err());
    assert!(
        InspectionLimits::builder()
            .max_output_bytes(0)
            .build()
            .is_err()
    );
}

#[test]
fn self_transitions_and_terminal_states_are_not_lost_from_diagrams() {
    let start = StateId::try_new("start").unwrap();
    let done = StateId::try_new("done").unwrap();
    let machine = MachineDefinition::<(), (), ()>::builder(MachineId::try_new("loop").unwrap())
        .initial(start.clone())
        .state(StateDefinition::builder(start.clone()).build())
        .unwrap()
        .state(StateDefinition::builder(done.clone()).terminal().build())
        .unwrap()
        .transition(
            Transition::builder(TransitionId::try_new("tick").unwrap())
                .from(start.clone())
                .on(EventId::try_new("tick").unwrap())
                .to(start.clone())
                .internal()
                .build()
                .unwrap(),
        )
        .unwrap()
        .transition(
            Transition::builder(TransitionId::try_new("finish").unwrap())
                .from(start)
                .on(EventId::try_new("finish").unwrap())
                .to(done)
                .build()
                .unwrap(),
        )
        .unwrap()
        .build()
        .unwrap();
    let limits = InspectionLimits::builder().build().unwrap();
    let diagram = machine.diagram(&limits).unwrap();
    assert!(diagram.contains("s1 --> s1 : tick / tick (internal)"));
    assert!(diagram.contains("s0 --> [*]"));
    let report = machine.structural_coverage(&limits).unwrap();
    assert!(report.states()[0].is_terminal());
    assert_eq!(report.states()[1].incoming(), 1);
    assert_eq!(report.states()[1].outgoing(), 2);
    let exact = InspectionLimits::builder()
        .max_output_bytes(diagram.len())
        .build()
        .unwrap();
    assert_eq!(machine.diagram(&exact).unwrap(), diagram);
    let too_small = InspectionLimits::builder()
        .max_output_bytes(diagram.len() - 1)
        .build()
        .unwrap();
    assert!(machine.diagram(&too_small).is_err());
}
