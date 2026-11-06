//! Checked-in GitHub diagrams must match the executable examples.
#[path = "../examples/diagrams/model.rs"]
mod model;

#[test]
fn github_markdown_matches_generated_machine_diagrams() {
    assert_eq!(model::markdown(), include_str!("../docs/diagrams.md"));
}
