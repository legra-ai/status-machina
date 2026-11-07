use status_machina::{
    Condition,
    ConditionId,
    DispositionId,
    EventDisposition,
    EventId,
    InspectionLimits,
    MachineDefinition,
    MachineId,
    StateDefinition,
    StateId,
    Transition,
    TransitionId,
};

type Definition = MachineDefinition<(), (), ()>;

fn execution() -> Definition {
    let mut builder = Definition::builder(MachineId::try_new("execution").unwrap())
        .initial(StateId::try_new("Admitted").unwrap());
    for (name, terminal) in [
        ("Admitted", false),
        ("Queued", false),
        ("Running", false),
        ("Done", true),
        ("Failed", true),
        ("Cancelled", true),
    ] {
        let state = StateDefinition::builder(StateId::try_new(name).unwrap());
        builder = builder
            .state(if terminal {
                state.terminal().build()
            } else {
                state.build()
            })
            .unwrap();
    }
    for (id, from, event, to) in [
        ("AdmittedToQueued", "Admitted", "Queue", "Queued"),
        ("QueuedToRunning", "Queued", "Start", "Running"),
        ("RunningToDone", "Running", "Complete", "Done"),
        ("RunningToFailed", "Running", "Fail", "Failed"),
        ("RunningToCancelled", "Running", "Cancel", "Cancelled"),
        ("AdmittedToCancelled", "Admitted", "Cancel", "Cancelled"),
        ("QueuedToCancelled", "Queued", "Cancel", "Cancelled"),
    ] {
        builder = builder
            .transition(
                Transition::builder(TransitionId::try_new(id).unwrap())
                    .from(StateId::try_new(from).unwrap())
                    .on(EventId::try_new(event).unwrap())
                    .to(StateId::try_new(to).unwrap())
                    .build()
                    .unwrap(),
            )
            .unwrap();
    }
    builder.build().unwrap()
}

fn retry() -> Definition {
    let waiting = StateId::try_new("Waiting").unwrap();
    let done = StateId::try_new("Done").unwrap();
    Definition::builder(MachineId::try_new("retry").unwrap())
        .initial(waiting.clone())
        .state(StateDefinition::builder(waiting.clone()).build())
        .unwrap()
        .state(StateDefinition::builder(done.clone()).terminal().build())
        .unwrap()
        .transition(
            Transition::builder(TransitionId::try_new("Retry").unwrap())
                .from(waiting.clone())
                .on(EventId::try_new("RetryDue").unwrap())
                .to(waiting.clone())
                .internal()
                .when(
                    Condition::builder(ConditionId::try_new("AttemptCurrent").unwrap())
                        .evaluate(|_: &(), _: &(), _: &()| true)
                        .build()
                        .unwrap(),
                )
                .build()
                .unwrap(),
        )
        .unwrap()
        .transition(
            Transition::builder(TransitionId::try_new("AcceptCompletion").unwrap())
                .from(waiting)
                .on(EventId::try_new("Completed").unwrap())
                .to(done.clone())
                .build()
                .unwrap(),
        )
        .unwrap()
        .disposition(
            EventDisposition::builder(DispositionId::try_new("IgnoreDuplicate").unwrap())
                .from(done)
                .on(EventId::try_new("Completed").unwrap())
                .when(
                    Condition::builder(ConditionId::try_new("AlreadyAccepted").unwrap())
                        .evaluate(|_: &(), _: &(), _: &()| true)
                        .build()
                        .unwrap(),
                )
                .reason("completion already accepted")
                .build()
                .unwrap(),
        )
        .unwrap()
        .build()
        .unwrap()
}

/// Generate the complete checked-in Markdown without filesystem I/O.
pub(super) fn markdown() -> String {
    let limits = InspectionLimits::builder().build().unwrap();
    let execution = execution().diagram(&limits).unwrap();
    let retry = retry().diagram(&limits).unwrap();
    format!(
        "# Generated status-machina diagrams\n\n\
These Mermaid blocks are generated from validated Rust machine definitions.\n\
GitHub renders them directly when viewing this Markdown file.\n\n\
Regenerate from the repository root:\n\n\
```sh\n\
cargo run --quiet --example diagrams > docs/diagrams.md\n\
```\n\n\
The `generated_diagrams` test checks this file against the generator.\n\
[Generator source](../examples/diagrams/model.rs) uses the convenience identity\n\
family for readable labels; custom families can use enums and `DiagramLabels`.\n\
Layout is chosen by GitHub's Mermaid renderer. These examples do not require\n\
a particular Mermaid version or layout engine.\n\n\
## EKGF execution lifecycle\n\n\
Six states and seven permitted routes, based on the\n\
EKGF execution lifecycle model.\n\
Event identities are explicit example choices. Initial state is Admitted;\n\
Done, Failed and Cancelled are terminal.\n\n\
```mermaid\n{execution}```\n\n\
## Retry and duplicate completion\n\n\
A small illustrative machine shows a guarded internal self-transition, an\n\
accepted completion and an explicit harmless duplicate-completion disposition.\n\
Its unit payloads and constant predicates illustrate graph structure; a real\n\
consumer supplies attempt correlation and policy facts.\n\n\
```mermaid\n{retry}```\n\n\
"
    )
}
