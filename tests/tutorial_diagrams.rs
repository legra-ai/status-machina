//! Diagrams shipped to Rustdoc must describe the actual tutorial definitions.
#[allow(dead_code, reason = "diagram checks do not execute the checkout")]
#[path = "../examples/tutorials/checkout.rs"]
mod checkout;
#[path = "../examples/tutorials/model.rs"]
mod model;
#[path = "../examples/tutorials/order.rs"]
mod order;

use status_machina::InspectionLimits;

fn assert_svg_matches(svg: &str, name: &str, diagram: &str) {
    let escaped = diagram
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    assert!(svg.contains(&format!(
        "<metadata id=\"{name}-source\">{escaped}</metadata>"
    )));
    assert!(svg.contains("role=\"img\""));
    assert!(svg.contains(&format!(
        "aria-labelledby=\"{name}-title {name}-description\""
    )));
    assert!(!svg.contains("<script"));
    assert!(!svg.contains("foreignObject"));
}

#[test]
fn inline_rustdoc_images_match_the_executable_graphs() {
    let limits = InspectionLimits::builder().build().unwrap();
    assert_svg_matches(
        include_str!("../docs/tutorials/order.svg"),
        "order",
        &order::definition()
            .diagram_with_labels(&limits, &model::Ids)
            .unwrap(),
    );
    assert_svg_matches(
        include_str!("../docs/tutorials/invoice.svg"),
        "invoice",
        &checkout::definition()
            .diagram_with_labels(&limits, &model::Ids)
            .unwrap(),
    );
}
