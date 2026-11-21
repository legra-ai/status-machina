//! STM-16 explicit forbidden-edge constraints, through native builders and
//! data-only record binding alike.
use status_machina::{
    Condition,
    DefinitionBuildError,
    DefinitionBuilder,
    ForbiddenEdge,
    MachineDefinition,
    MachineIds,
    StateDefinition,
    Transition,
};

#[path = "forbidden_edges/behavior.rs"]
mod behavior;
#[path = "forbidden_edges/inspection.rs"]
mod inspection;
#[path = "forbidden_edges/records.rs"]
mod records;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct Ids;
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
enum Session {
    Open,
    Sealing,
    Done,
    Failed,
    Cancelled,
    Ghost,
}
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
enum Signal {
    Seal,
    Finish,
    Fail,
    Cancel,
    Retry,
    Stale,
}
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
enum Edge {
    Seal,
    Finish,
    Fail,
    Cancel,
    SealingCancel,
    Retry,
}
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum Story {
    Never,
    Always,
}
impl MachineIds for Ids {
    type Definition = u8;
    type Instance = u8;
    type State = Session;
    type Event = Signal;
    type Transition = Edge;
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
    type Implementation = u8;
}

type Definition = MachineDefinition<u32, (), (), (), (), Ids>;
type Builder = DefinitionBuilder<u32, (), (), (), (), Ids>;
type Error = DefinitionBuildError<Ids>;

const STATES: [Session; 5] = [
    Session::Open,
    Session::Sealing,
    Session::Done,
    Session::Failed,
    Session::Cancelled,
];
const EVENT_KINDS: [Signal; 6] = [
    Signal::Seal,
    Signal::Finish,
    Signal::Fail,
    Signal::Cancel,
    Signal::Retry,
    Signal::Stale,
];

fn forbidden(from: Session, to: Session) -> ForbiddenEdge<Ids> {
    ForbiddenEdge::new(from, to)
}

fn sealing_cancelled() -> ForbiddenEdge<Ids> {
    forbidden(Session::Sealing, Session::Cancelled)
}

fn never(id: u8) -> Condition<u32, (), (), Ids> {
    Condition::builder(id)
        .evaluate(|_: &u32, _: &(), _: &()| false)
        .build()
        .unwrap()
}

fn edge(id: Edge, from: Session, on: Signal, to: Session) -> Transition<u32, (), (), (), (), Ids> {
    Transition::builder_with_effects_and_output_events(id)
        .from(from)
        .on(on)
        .to(to)
        .build()
        .unwrap()
}

/// The legal session graph: `Open -> Sealing -> Done | Failed`, `Open ->
/// Cancelled`. Every state is reachable without a `Sealing -> Cancelled` edge.
fn legal_edges() -> Vec<Transition<u32, (), (), (), (), Ids>> {
    vec![
        edge(Edge::Seal, Session::Open, Signal::Seal, Session::Sealing),
        edge(
            Edge::Finish,
            Session::Sealing,
            Signal::Finish,
            Session::Done,
        ),
        edge(Edge::Fail, Session::Sealing, Signal::Fail, Session::Failed),
        edge(
            Edge::Cancel,
            Session::Open,
            Signal::Cancel,
            Session::Cancelled,
        ),
    ]
}

fn session_builder() -> Builder {
    let mut builder = MachineDefinition::builder_with_effects_and_output_events(1)
        .initial(Session::Open)
        .limits(16, 64)
        .unwrap();
    for state in STATES {
        let mut definition = StateDefinition::builder(state);
        if matches!(state, Session::Done | Session::Failed | Session::Cancelled) {
            definition = definition.terminal();
        }
        builder = builder.state(definition.build()).unwrap();
    }
    builder
}

fn with_legal_edges(mut builder: Builder) -> Builder {
    for transition in legal_edges() {
        builder = builder.transition(transition).unwrap();
    }
    builder
}

fn is_sealing_cancelled_conflict(result: Result<Definition, Error>, transition: Edge) -> bool {
    matches!(
        result,
        Err(DefinitionBuildError::ForbiddenTransition { transition: actual, from, to })
            if actual == transition && from == Session::Sealing && to == Session::Cancelled
    )
}

#[test]
fn sealing_to_cancelled_is_rejected_when_forbidden_after_the_edge() {
    let builder = with_legal_edges(session_builder())
        .transition(edge(
            Edge::SealingCancel,
            Session::Sealing,
            Signal::Cancel,
            Session::Cancelled,
        ))
        .unwrap()
        .forbid(sealing_cancelled())
        .unwrap();
    assert!(is_sealing_cancelled_conflict(
        builder.build(),
        Edge::SealingCancel
    ));
}

#[test]
fn sealing_to_cancelled_is_rejected_when_forbidden_before_the_edge() {
    let builder = with_legal_edges(session_builder().forbid(sealing_cancelled()).unwrap())
        .transition(edge(
            Edge::SealingCancel,
            Session::Sealing,
            Signal::Cancel,
            Session::Cancelled,
        ))
        .unwrap();
    assert!(is_sealing_cancelled_conflict(
        builder.build(),
        Edge::SealingCancel
    ));
}

#[test]
fn every_event_kind_conflicts_with_the_forbidden_pair() {
    for kind in EVENT_KINDS {
        let builder = with_legal_edges(session_builder())
            .forbid(sealing_cancelled())
            .unwrap()
            .transition(edge(
                Edge::SealingCancel,
                Session::Sealing,
                kind,
                Session::Cancelled,
            ))
            .unwrap();
        assert!(
            is_sealing_cancelled_conflict(builder.build(), Edge::SealingCancel),
            "{kind:?}"
        );
    }
}

#[test]
fn a_false_guard_does_not_make_the_contradiction_acceptable() {
    let guarded = Transition::builder_with_effects_and_output_events(Edge::SealingCancel)
        .from(Session::Sealing)
        .on(Signal::Cancel)
        .to(Session::Cancelled)
        .when(never(1))
        .build()
        .unwrap();
    let builder = with_legal_edges(session_builder())
        .transition(guarded)
        .unwrap()
        .forbid(sealing_cancelled())
        .unwrap();
    assert!(is_sealing_cancelled_conflict(
        builder.build(),
        Edge::SealingCancel
    ));
}

#[test]
fn legal_edges_beside_the_prohibition_build_and_expose_it() {
    let definition: Definition = with_legal_edges(session_builder())
        .forbid(sealing_cancelled())
        .unwrap()
        .build()
        .unwrap();
    assert_eq!(definition.forbidden_edges(), &[sealing_cancelled()]);
    assert_eq!(definition.forbidden_edges()[0].from(), &Session::Sealing);
    assert_eq!(definition.forbidden_edges()[0].to(), &Session::Cancelled);
}

#[test]
fn unknown_forbidden_endpoints_are_rejected_naming_the_pair_and_state() {
    for (from, to) in [
        (Session::Ghost, Session::Cancelled),
        (Session::Sealing, Session::Ghost),
    ] {
        let result = with_legal_edges(session_builder())
            .forbid(forbidden(from, to))
            .unwrap()
            .build();
        assert!(
            matches!(result, Err(DefinitionBuildError::UnknownForbiddenEdgeState {
                from: actual_from, to: actual_to, state
            }) if actual_from == from && actual_to == to && state == Session::Ghost),
            "{from:?} -> {to:?}"
        );
    }
}

#[test]
fn duplicate_prohibitions_are_rejected_even_without_a_conflicting_edge() {
    let result = with_legal_edges(session_builder())
        .forbid(sealing_cancelled())
        .unwrap()
        .forbid(forbidden(Session::Sealing, Session::Open))
        .unwrap()
        .forbid(sealing_cancelled())
        .unwrap()
        .build();
    assert!(matches!(
        result,
        Err(DefinitionBuildError::DuplicateForbiddenEdge { from, to })
            if from == Session::Sealing && to == Session::Cancelled
    ));
}

#[test]
fn reversed_pairs_are_distinct_prohibitions() {
    let definition = with_legal_edges(session_builder())
        .forbid(forbidden(Session::Sealing, Session::Open))
        .unwrap()
        .forbid(forbidden(Session::Open, Session::Done))
        .unwrap()
        .build()
        .unwrap();
    assert_eq!(definition.forbidden_edges().len(), 2);
    let reversed = with_legal_edges(session_builder())
        .forbid(forbidden(Session::Sealing, Session::Open))
        .unwrap()
        .transition(edge(
            Edge::Retry,
            Session::Open,
            Signal::Retry,
            Session::Sealing,
        ))
        .unwrap()
        .build();
    assert!(reversed.is_ok(), "Open -> Sealing is not Sealing -> Open");
}

#[test]
fn a_same_state_pair_forbids_internal_and_external_self_transitions() {
    let external = edge(
        Edge::Retry,
        Session::Sealing,
        Signal::Retry,
        Session::Sealing,
    );
    let internal = Transition::builder_with_effects_and_output_events(Edge::Retry)
        .from(Session::Sealing)
        .on(Signal::Retry)
        .to(Session::Sealing)
        .internal()
        .build()
        .unwrap();
    for transition in [external, internal] {
        let result = with_legal_edges(session_builder())
            .forbid(forbidden(Session::Sealing, Session::Sealing))
            .unwrap()
            .transition(transition)
            .unwrap()
            .build();
        assert!(matches!(
            result,
            Err(DefinitionBuildError::ForbiddenTransition { transition, from, to })
                if transition == Edge::Retry && from == Session::Sealing && to == Session::Sealing
        ));
    }
}

#[test]
fn terminal_states_still_reject_every_outgoing_edge() {
    // Forbidding or not, an edge leaving a terminal state keeps its stronger,
    // existing rejection.
    for prohibit in [false, true] {
        let mut builder = with_legal_edges(session_builder());
        if prohibit {
            builder = builder
                .forbid(forbidden(Session::Done, Session::Sealing))
                .unwrap();
        }
        let result = builder
            .transition(edge(
                Edge::Retry,
                Session::Done,
                Signal::Retry,
                Session::Sealing,
            ))
            .unwrap()
            .build();
        assert!(matches!(
            result,
            Err(DefinitionBuildError::TerminalHasOutgoingTransition { state }) if state == Session::Done
        ));
    }
    for target in STATES {
        let result = with_legal_edges(session_builder())
            .transition(edge(Edge::Retry, Session::Cancelled, Signal::Retry, target))
            .unwrap()
            .build();
        assert!(matches!(
            result,
            Err(DefinitionBuildError::TerminalHasOutgoingTransition { state }) if state == Session::Cancelled
        ));
    }
    // A redundant prohibition on a terminal source is consistent and accepted.
    let definition = with_legal_edges(session_builder())
        .forbid(forbidden(Session::Done, Session::Open))
        .unwrap()
        .build()
        .unwrap();
    assert_eq!(definition.forbidden_edges().len(), 1);
}

#[test]
fn the_constraint_bound_is_checked_before_the_excess_prohibition_is_stored() {
    // The bound counts declarations as they arrive; duplicates are a separate
    // check at build time.
    let mut builder = with_legal_edges(session_builder());
    for _ in 0..128 {
        builder = builder
            .forbid(forbidden(Session::Open, Session::Done))
            .unwrap();
    }
    let result = builder.forbid(sealing_cancelled());
    assert!(matches!(
        result,
        Err(DefinitionBuildError::ForbiddenEdgeLimitExceeded { limit: 128 })
    ));
}

#[test]
fn only_declared_pairs_are_stored_never_a_complement() {
    let definition: Definition = with_legal_edges(session_builder())
        .forbid(sealing_cancelled())
        .unwrap()
        .build()
        .unwrap();
    // Five states admit 25 ordered pairs; one prohibition stays one entry and
    // the undeclared remainder stays undeclared, not forbidden.
    assert_eq!(definition.forbidden_edges(), &[sealing_cancelled()]);
}
