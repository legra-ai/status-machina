//! Run an enum-identity order with guarded acceptance and caller-owned work.
#[path = "tutorials/model.rs"]
mod model;
#[path = "tutorials/order.rs"]
mod order;

use model::*;
use status_machina::{
    DispatchOutcome,
    Event,
};

fn main() {
    let definition = order::definition();
    let mut order = definition
        .instantiate(Instance(7), OrderData::empty())
        .unwrap();
    let event = Request {
        order: 7,
        quantity: 2,
    };
    let refused = order
        .dispatch(
            Event::new(Input::Submit, event),
            &Facts {
                stock_available: false,
            },
        )
        .unwrap();
    assert!(refused.is_refused());
    assert_eq!(order.revision(), 0);
    let accepted = order
        .dispatch(
            Event::new(Input::Submit, event),
            &Facts {
                stock_available: true,
            },
        )
        .unwrap();
    let DispatchOutcome::Transitioned {
        effects, outputs, ..
    } = accepted
    else {
        panic!("stock permits submission")
    };
    assert_eq!(order.state(), &State::Accepted);
    println!("Accepted order at revision {}", order.revision());
    for effect in effects {
        println!("Caller owns: {:?}", effect.into_parts().1);
    }
    for output in outputs {
        println!("Caller may route: {:?}", output.into_parts());
    }
}
