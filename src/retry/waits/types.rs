use std::time::SystemTime;

/// Identity of one operation's particular wait, independent of machine
/// revision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WaitCorrelation<I: crate::MachineIds = crate::DefaultIds> {
    pub(super) instance: I::Instance,
    pub(super) operation: I::Operation,
    pub(super) generation: u64,
}

impl<I: crate::MachineIds> WaitCorrelation<I> {
    /// Instance incarnation that owns this wait.
    pub fn instance_id(&self) -> I::Instance {
        self.instance.clone()
    }

    /// Caller-assigned operation identity within the instance.
    pub const fn operation(&self) -> &I::Operation {
        &self.operation
    }

    /// Monotonic generation; removing a wait does not recycle it.
    pub const fn generation(&self) -> u64 {
        self.generation
    }
}

/// Accepted wake intent retained by the instance owner until
/// consumption/cancel.
///
/// `P` carries caller-owned scheduling metadata without a scheduler dependency.
/// The caller must bound payload size and clone cost, as for all machine data.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WakeIntent<P, I: crate::MachineIds = crate::DefaultIds> {
    pub(super) correlation: WaitCorrelation<I>,
    pub(super) deadline: SystemTime,
    pub(super) payload: P,
}

impl<P, I: crate::MachineIds> WakeIntent<P, I> {
    /// Identity that a reminder must return unchanged.
    pub const fn correlation(&self) -> &WaitCorrelation<I> {
        &self.correlation
    }

    /// Caller-supplied civil deadline; this value never reads a clock.
    pub const fn deadline(&self) -> SystemTime {
        self.deadline
    }

    /// Caller-owned metadata, for example an assigned schedule identity.
    pub const fn payload(&self) -> &P {
        &self.payload
    }
}

/// Refusal to allocate a wait; the existing wait set remains unchanged.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum RetryWaitError {
    /// Capacity is absent or outside the supported 1..=4096 range.
    #[error("retry wait capacity must be between 1 and 4096")]
    InvalidCapacity,
    /// All bounded active-wait slots are occupied.
    #[error("retry wait capacity is exhausted")]
    CapacityReached,
    /// The owner cannot assign another non-reusable generation.
    #[error("retry wait generation is exhausted")]
    GenerationExhausted,
}
