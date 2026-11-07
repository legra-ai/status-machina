//! Print GitHub-ready Markdown generated from validated machine definitions.
#[path = "diagrams/model.rs"]
mod model;

fn main() {
    print!("{}", model::markdown());
}
