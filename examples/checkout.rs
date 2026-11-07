//! Run a two-machine checkout using declared routes and explicit budgets.
#[path = "tutorials/checkout.rs"]
mod checkout;
#[path = "tutorials/model.rs"]
mod model;
#[path = "tutorials/order.rs"]
mod order;

fn main() {
    let summary = checkout::run();
    assert_eq!(summary.accepted_steps, 2);
    assert_eq!(summary.order_state, model::State::Accepted);
    assert_eq!(summary.invoice_state, model::State::Issued);
    println!("Two accepted transitions; caller owns {:?}", summary.work);
}
