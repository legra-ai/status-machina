//! Inspection reports prohibitions as typed pairs, never as executable edges.
use status_machina::{
    DiagramLabels,
    InspectionError,
    InspectionLimits,
};

use super::{
    Definition,
    Edge,
    Ids,
    Session,
    Signal,
    forbidden,
    session_builder,
    with_legal_edges,
};

struct Labels;
impl DiagramLabels<Ids> for Labels {
    fn state<'a>(&'a self, id: &'a Session) -> &'a str {
        match id {
            Session::Open => "Open",
            Session::Sealing => "Sealing",
            Session::Done => "Done",
            Session::Failed => "Failed",
            Session::Cancelled => "Cancelled",
            Session::Ghost => "Ghost",
        }
    }
    fn transition<'a>(&'a self, id: &'a Edge) -> &'a str {
        match id {
            Edge::Seal => "seal",
            Edge::Finish => "finish",
            Edge::Fail => "fail",
            Edge::Cancel => "cancel",
            Edge::SealingCancel => "sealing cancel",
            Edge::Retry => "retry",
        }
    }
    fn event<'a>(&'a self, id: &'a Signal) -> &'a str {
        match id {
            Signal::Seal => "Seal",
            Signal::Finish => "Finish",
            Signal::Fail => "Fail",
            Signal::Cancel => "Cancel",
            Signal::Retry => "Retry",
            Signal::Stale => "Stale",
        }
    }
    fn condition<'a>(&'a self, _: &'a u8) -> &'a str {
        "condition"
    }
    fn disposition<'a>(&'a self, _: &'a u8) -> &'a str {
        "disposition"
    }
}

fn with(pairs: &[(Session, Session)]) -> Definition {
    let mut builder = with_legal_edges(session_builder());
    for (from, to) in pairs {
        builder = builder.forbid(forbidden(*from, *to)).unwrap();
    }
    builder.build().unwrap()
}

const PAIRS: [(Session, Session); 3] = [
    (Session::Sealing, Session::Cancelled),
    (Session::Open, Session::Done),
    (Session::Sealing, Session::Sealing),
];

#[test]
fn coverage_exposes_the_typed_pairs_independent_of_declaration_order() {
    let limits = InspectionLimits::builder().build().unwrap();
    let forward = with(&PAIRS).structural_coverage(&limits).unwrap();
    let mut reversed_pairs = PAIRS;
    reversed_pairs.reverse();
    let reversed = with(&reversed_pairs).structural_coverage(&limits).unwrap();
    assert_eq!(forward, reversed);
    let pairs: Vec<_> = forward
        .forbidden_edges()
        .iter()
        .map(|edge| (*edge.from(), *edge.to()))
        .collect();
    assert_eq!(
        pairs,
        [
            (Session::Open, Session::Done),
            (Session::Sealing, Session::Sealing),
            (Session::Sealing, Session::Cancelled),
        ]
    );
    // Prohibitions are not edges: incoming/outgoing counts are unchanged.
    let plain = with(&[]).structural_coverage(&limits).unwrap();
    assert_eq!(forward.states(), plain.states());
    assert_eq!(forward.transitions(), plain.transitions());
    assert!(plain.forbidden_edges().is_empty());
}

#[test]
fn coverage_counts_prohibitions_against_its_entry_budget() {
    // Five states, four transitions, three prohibitions.
    let definition = with(&PAIRS);
    let exact = InspectionLimits::builder().max_entries(12).build().unwrap();
    assert!(definition.structural_coverage(&exact).is_ok());
    let short = InspectionLimits::builder().max_entries(11).build().unwrap();
    assert_eq!(
        definition.structural_coverage(&short),
        Err(InspectionError::EntryLimit { limit: 11 })
    );
}

#[test]
fn diagrams_never_render_prohibitions_as_routes() {
    let limits = InspectionLimits::builder().build().unwrap();
    let constrained = with(&PAIRS).diagram_with_labels(&limits, &Labels).unwrap();
    let plain = with(&[]).diagram_with_labels(&limits, &Labels).unwrap();
    assert_eq!(constrained, plain);
    assert!(!constrained.contains("s1 --> s4"), "{constrained}");
    // Diagrams render only the nine states and edges, so prohibitions do not
    // count against a diagram's entry budget.
    let exact = InspectionLimits::builder().max_entries(9).build().unwrap();
    assert_eq!(
        with(&PAIRS).diagram_with_labels(&exact, &Labels).unwrap(),
        plain
    );
}
