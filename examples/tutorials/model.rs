//! Caller-owned identities, payloads and labels shared by the tutorials.
#![allow(
    dead_code,
    reason = "each tutorial uses a subset of the shared identity family"
)]
use status_machina::{
    CandidateData,
    DiagramLabels,
    MachineIds,
};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct Ids;
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum State {
    Draft,
    Accepted,
    Shipped,
    Cancelled,
    Idle,
    Issued,
}
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum Input {
    Submit,
    Ship,
    Cancel,
    InvoiceRequested,
}
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum Edge {
    Submit,
    Ship,
    Cancel,
    IssueInvoice,
}
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum Definition {
    Order,
    Invoice,
}
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct Instance(pub(crate) u64);
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum Guard {
    StockAvailable,
}

impl MachineIds for Ids {
    type Definition = Definition;
    type Instance = Instance;
    type State = State;
    type Event = Input;
    type Transition = Edge;
    type Action = u8;
    type Condition = Guard;
    type Effect = u8;
    type Disposition = u8;
    type Capability = u8;
    type Operation = u8;
    type Scenario = u8;
    type Assertion = u8;
    type Scope = u8;
    type Behavior = u16;
    type Implementation = u16;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Request {
    pub(crate) order: u64,
    pub(crate) quantity: u32,
}
pub(crate) struct Facts {
    pub(crate) stock_available: bool,
}
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Work {
    SendConfirmation { order: u64, quantity: u32 },
    DispatchParcel { order: u64 },
    SendInvoice { order: u64 },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct OrderData {
    pub(crate) order: u64,
    pub(crate) quantity: u32,
}
impl OrderData {
    pub(crate) const fn empty() -> Self {
        Self {
            order: 0,
            quantity: 0,
        }
    }
}
impl CandidateData for OrderData {
    fn snapshot(&self) -> Self {
        *self
    }
}

impl DiagramLabels<Ids> for Ids {
    fn state<'a>(&'a self, id: &'a State) -> &'a str {
        match id {
            State::Draft => "Draft",
            State::Accepted => "Accepted",
            State::Shipped => "Shipped",
            State::Cancelled => "Cancelled",
            State::Idle => "Idle",
            State::Issued => "Issued",
        }
    }
    fn transition<'a>(&'a self, id: &'a Edge) -> &'a str {
        match id {
            Edge::Submit => "Submit",
            Edge::Ship => "Ship",
            Edge::Cancel => "Cancel",
            Edge::IssueInvoice => "Issue invoice",
        }
    }
    fn event<'a>(&'a self, id: &'a Input) -> &'a str {
        match id {
            Input::Submit => "Submit",
            Input::Ship => "Ship",
            Input::Cancel => "Cancel",
            Input::InvoiceRequested => "Invoice requested",
        }
    }
    fn condition<'a>(&'a self, _: &'a Guard) -> &'a str {
        "Stock available"
    }
    fn disposition<'a>(&'a self, _: &'a u8) -> &'a str {
        "Unused"
    }
}
