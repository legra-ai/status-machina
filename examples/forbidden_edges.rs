//! Default refusal, a selective forbidden edge and terminal states, side by
//! side.
//!
//! - Default refusal: dispatch refuses any event without a declared edge from
//!   the current state, without changing state, data or revision.
//! - Selective prohibition: `Sealing -> Cancelled` is explicitly forbidden, so
//!   a later definition edit that adds the edge fails construction.
//! - Terminal states: construction already rejects every outgoing edge of
//!   `Done`, `Failed` and `Cancelled`; they need no prohibitions.
use status_machina::{
    DefinitionBuildError,
    DefinitionBuilder,
    DispatchOutcome,
    Event,
    EventId,
    ForbiddenEdge,
    MachineDefinition,
    MachineId,
    MachineInstanceId,
    StateDefinition,
    StateId,
    Transition,
    TransitionId,
};

fn state(name: &str) -> StateId {
    StateId::try_new(name).unwrap()
}

fn event(name: &str) -> EventId {
    EventId::try_new(name).unwrap()
}

fn edge(name: &str, from: &str, on: &str, to: &str) -> Transition<(), (), ()> {
    Transition::builder(TransitionId::try_new(name).unwrap())
        .from(state(from))
        .on(event(on))
        .to(state(to))
        .build()
        .unwrap()
}

/// `Open -> Sealing -> Done | Failed`, `Open -> Cancelled`, and the explicit
/// prohibition `Sealing -> Cancelled`.
fn session() -> DefinitionBuilder<(), (), ()> {
    let mut builder = MachineDefinition::builder(MachineId::try_new("session").unwrap())
        .initial(state("open"))
        .forbid(ForbiddenEdge::new(state("sealing"), state("cancelled")))
        .unwrap();
    for (name, terminal) in [
        ("open", false),
        ("sealing", false),
        ("done", true),
        ("failed", true),
        ("cancelled", true),
    ] {
        let mut definition = StateDefinition::builder(state(name));
        if terminal {
            definition = definition.terminal();
        }
        builder = builder.state(definition.build()).unwrap();
    }
    for transition in [
        edge("seal", "open", "seal", "sealing"),
        edge("finish", "sealing", "finish", "done"),
        edge("fail", "sealing", "fail", "failed"),
        edge("cancel", "open", "cancel", "cancelled"),
    ] {
        builder = builder.transition(transition).unwrap();
    }
    builder
}

fn main() {
    let definition = session().build().unwrap();
    let mut instance = definition
        .instantiate(
            MachineInstanceId::try_new(uuid::Uuid::now_v7()).unwrap(),
            (),
        )
        .unwrap();
    instance
        .dispatch(Event::new(event("seal"), ()), &())
        .unwrap();

    // Default refusal: no `cancel` edge leaves `sealing`.
    let outcome = instance
        .dispatch(Event::new(event("cancel"), ()), &())
        .unwrap();
    assert!(matches!(outcome, DispatchOutcome::Refused { .. }));
    assert_eq!(instance.state(), &state("sealing"));
    assert_eq!(instance.revision(), 1);
    println!("default refusal: cancel while sealing is refused");

    // Selective prohibition: adding the edge is a contradictory definition.
    let edited = session()
        .transition(edge("abandon", "sealing", "cancel", "cancelled"))
        .unwrap()
        .build();
    let Err(DefinitionBuildError::ForbiddenTransition {
        transition,
        from,
        to,
    }) = edited
    else {
        panic!("the forbidden edge must be rejected");
    };
    println!("selective prohibition: {transition} declares forbidden {from} -> {to}");

    // Terminal states: an outgoing edge is rejected without any prohibition.
    let reopened = session()
        .transition(edge("reopen", "done", "seal", "sealing"))
        .unwrap()
        .build();
    let Err(DefinitionBuildError::TerminalHasOutgoingTransition { state }) = reopened else {
        panic!("terminal states keep rejecting outgoing edges");
    };
    println!("terminal state: {state} has no outgoing edges");

    // Inspection lists the prohibition separately from executable edges.
    for forbidden in definition.forbidden_edges() {
        println!("forbidden: {} -> {}", forbidden.from(), forbidden.to());
    }
}
