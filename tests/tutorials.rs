//! Public tutorials must execute the documented ownership and refusal
//! contracts.
#[path = "../examples/tutorials/checkout.rs"]
mod checkout;
#[path = "../examples/tutorials/model.rs"]
mod model;
#[path = "../examples/tutorials/order.rs"]
mod order;

use model::*;
use status_machina::{
    DispatchError,
    DispatchOutcome,
    Event,
};

#[test]
fn guarded_order_refuses_without_changing_data_or_releasing_work() {
    let definition = order::definition();
    let mut order = definition
        .instantiate(Instance(7), OrderData::empty())
        .unwrap();
    let request = Request {
        order: 7,
        quantity: 2,
    };
    assert!(
        order
            .dispatch(
                Event::new(Input::Submit, request),
                &Facts {
                    stock_available: false
                }
            )
            .unwrap()
            .is_refused()
    );
    assert_eq!(order.state(), &State::Draft);
    assert_eq!(order.data(), &OrderData::empty());
    assert_eq!(order.revision(), 0);
    let outcome = order
        .dispatch(
            Event::new(Input::Submit, request),
            &Facts {
                stock_available: true,
            },
        )
        .unwrap();
    let DispatchOutcome::Transitioned {
        effects, outputs, ..
    } = outcome
    else {
        panic!("submission must be accepted")
    };
    assert_eq!(order.state(), &State::Accepted);
    assert_eq!(order.data().quantity, 2);
    assert_eq!(
        effects[0].payload(),
        &Work::SendConfirmation {
            order: 7,
            quantity: 2
        }
    );
    assert_eq!(outputs[0].id(), &Input::InvoiceRequested);
    assert_eq!(outputs[0].payload(), &request);
}

#[test]
fn failed_candidate_invariant_does_not_accept_a_zero_quantity_order() {
    let definition = order::definition();
    let mut order = definition
        .instantiate(Instance(7), OrderData::empty())
        .unwrap();
    let result = order.dispatch(
        Event::new(
            Input::Submit,
            Request {
                order: 7,
                quantity: 0,
            },
        ),
        &Facts {
            stock_available: true,
        },
    );
    assert_eq!(
        result,
        Err(DispatchError::InvariantFailed {
            state: State::Accepted
        })
    );
    assert_eq!(order.state(), &State::Draft);
    assert_eq!(order.data(), &OrderData::empty());
    assert_eq!(order.revision(), 0);
}

#[test]
fn explicitly_composed_checkout_advances_two_independent_instances() {
    let report = checkout::run();
    assert_eq!(report.accepted_steps, 2);
    assert_eq!(report.order_state, State::Accepted);
    assert_eq!(report.invoice_state, State::Issued);
    assert_eq!(
        report.work,
        [
            Work::SendConfirmation {
                order: 7,
                quantity: 2
            },
            Work::SendInvoice { order: 7 }
        ]
    );
}
