use super::super::{
    InstanceAddress,
    RoutedItem,
};
use crate::{
    DispatchError,
    DispatchOutcome,
    EffectIntent,
    Event,
    MachineIds,
};

/// An owned input addressed to exactly one registered instance.
#[derive(Debug)]
pub struct Delivery<E, I: MachineIds = crate::DefaultIds> {
    pub(super) destination: InstanceAddress<I>,
    pub(super) event: Event<E, I>,
}
impl<E, I: MachineIds> Delivery<E, I> {
    /// Construct an addressed input without cloning its payload.
    #[must_use]
    pub const fn new(destination: InstanceAddress<I>, event: Event<E, I>) -> Self {
        Self { destination, event }
    }
    /// Destination scope and instance identity.
    pub const fn destination(&self) -> &InstanceAddress<I> {
        &self.destination
    }
    /// Input kind and payload.
    pub const fn event(&self) -> &Event<E, I> {
        &self.event
    }
    /// Recover the input for explicit caller routing.
    pub fn into_parts(self) -> (InstanceAddress<I>, Event<E, I>) {
        (self.destination, self.event)
    }
}

/// Explicit work and retained-payload count limits for one synchronous call.
#[derive(Clone, Copy, Debug)]
pub struct PropagationLimits {
    pub(super) steps: usize,
    pub(super) intents: usize,
}
impl PropagationLimits {
    /// Build a positive budget. Both dimensions must be supplied.
    pub const fn builder() -> PropagationLimitsBuilder {
        PropagationLimitsBuilder {
            steps: None,
            intents: None,
        }
    }
}
/// Builder requiring explicit budgets, each at most 4096.
pub struct PropagationLimitsBuilder {
    steps: Option<usize>,
    intents: Option<usize>,
}
impl PropagationLimitsBuilder {
    /// Maximum dispatched events and retained step records.
    #[must_use]
    pub const fn max_steps(mut self, value: usize) -> Self {
        self.steps = Some(value);
        self
    }
    /// Maximum combined pending inputs, external effects and unrouted outputs.
    /// Payload byte size and callback cost remain the caller's bounded
    /// contract.
    #[must_use]
    pub const fn max_intents(mut self, value: usize) -> Self {
        self.intents = Some(value);
        self
    }
    /// Validate both explicit dimensions.
    ///
    /// # Errors
    /// Refuses missing, zero or over-ceiling limits.
    pub fn build(self) -> Result<PropagationLimits, PropagationLimitsError> {
        match (self.steps, self.intents) {
            (Some(steps @ 1..=4096), Some(intents @ 1..=4096)) => {
                Ok(PropagationLimits { steps, intents })
            }
            _ => Err(PropagationLimitsError),
        }
    }
}
/// A required propagation budget is absent or outside 1..=4096.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("propagation limits must both be explicit and within 1..=4096")]
pub struct PropagationLimitsError;

/// Why synchronous propagation returned to its caller.
#[derive(Debug, Eq, PartialEq)]
pub enum PropagationStatus<I: MachineIds = crate::DefaultIds> {
    /// No routed input remains. Deferred/unmatched outputs may still exist.
    Quiescent,
    /// Dispatch count reached its budget; remaining input is returned.
    StepLimit,
    /// Next acceptance would exceed the intent or configured route-batch bound.
    IntentLimit,
    /// An input addresses an instance outside the bound composition.
    UnboundDestination,
    /// Evaluation failed; this input and later work remain pending.
    DispatchFailed(DispatchError<I>),
}
/// Metadata for one completed dispatch; no user payload is copied into it.
#[derive(Debug)]
pub struct PropagationStep<I: MachineIds = crate::DefaultIds> {
    /// Instance that handled this input.
    pub destination: InstanceAddress<I>,
    /// Original dispatch metadata, with intent vectors empty.
    pub outcome: DispatchOutcome<(), (), I>,
}
/// An external intent returned with the instance that emitted it.
#[derive(Debug)]
pub struct AddressedEffect<X, I: MachineIds = crate::DefaultIds> {
    /// Instance owning this external request.
    pub source: InstanceAddress<I>,
    /// Accepted external request; no execution has occurred.
    pub intent: EffectIntent<X, I>,
}
/// Owned result of a bounded synchronous propagation call.
///
/// Fields are movable independently so non-Clone payloads can be executed or
/// resumed. Previous accepted transitions are never rolled back by later
/// failure.
#[derive(Debug)]
pub struct PropagationReport<E, X, I: MachineIds = crate::DefaultIds> {
    /// Quiescence, budget stop or failed evaluation.
    pub status: PropagationStatus<I>,
    /// Completed dispatches in deterministic FIFO order, bounded by
    /// `max_steps`.
    pub steps: Vec<PropagationStep<I>>,
    /// Accepted external work, bounded jointly by `max_intents`.
    pub effects: Vec<AddressedEffect<X, I>>,
    /// Unmatched/deferred outputs, bounded jointly by `max_intents`.
    pub unrouted: Vec<RoutedItem<E, I>>,
    /// Remaining inputs, including the input that could not be accepted.
    /// An initially oversized vector is returned intact without processing it.
    pub pending: Vec<Delivery<E, I>>,
}
impl<E, X, I: MachineIds> PropagationReport<E, X, I> {
    pub(super) fn new() -> Self {
        Self {
            status: PropagationStatus::Quiescent,
            steps: Vec::new(),
            effects: Vec::new(),
            unrouted: Vec::new(),
            pending: Vec::new(),
        }
    }
}
