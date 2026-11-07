//! Print canonical graphs used by the public executable tutorials.
#[allow(dead_code, reason = "diagram generation does not execute the checkout")]
#[path = "tutorials/checkout.rs"]
mod checkout;
#[path = "tutorials/model.rs"]
mod model;
#[path = "tutorials/order.rs"]
mod order;

use status_machina::InspectionLimits;

fn main() {
    let limits = InspectionLimits::builder().build().unwrap();
    let order = order::definition()
        .diagram_with_labels(&limits, &model::Ids)
        .unwrap();
    let invoice = checkout::definition()
        .diagram_with_labels(&limits, &model::Ids)
        .unwrap();
    print!(
        "# Executable tutorial diagrams\n\nThese graphs are generated from the same definitions used by the runnable\n[typed order](../../examples/typed_order.rs) and\n[composed checkout](../../examples/checkout.rs) tutorials.\nSee the [tutorial guide](README.md) for the execution and ownership contracts.\n\n## Guarded order\n\n```mermaid\n{order}```\n\n## Invoice\n\n```mermaid\n{invoice}```\n"
    );
}
