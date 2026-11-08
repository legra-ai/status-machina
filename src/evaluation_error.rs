use std::error::Error;
use std::fmt;
use std::rc::Rc;

/// Evaluation stage at which application code failed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EvaluationPhase {
    /// Isolating machine data.
    Snapshot,
    /// Selecting a guarded transition.
    Guard,
    /// Checking an intentional event disposition.
    Disposition,
    /// Applying a local candidate action.
    Action,
    /// Checking a transition postcondition.
    Postcondition,
    /// Checking a state invariant.
    Invariant,
    /// Describing caller-owned external work.
    Effect,
    /// Constructing an output event.
    Output,
    /// Observing an advisory capability.
    Capability,
    /// Evaluating a replay safety assertion.
    Assertion,
}

/// An application failure retaining its original, potentially thread-bound
/// cause.
///
/// Causes need not be `Send` or `Sync`: evaluation runs synchronously and a
/// native exception may belong to one interpreter or Node environment. Cloning
/// retains the same cause. Equality compares the message and cause identity,
/// not the display representation of two unrelated exceptions.
#[derive(Clone, Debug)]
pub struct EvaluationError {
    message: String,
    cause: Option<Rc<dyn Error>>,
}
impl EvaluationError {
    /// Describe a failure without an underlying exception object.
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            cause: None,
        }
    }
    /// Retain an original exception without converting its identity to text.
    #[must_use]
    pub fn from_source(source: impl Error + 'static) -> Self {
        Self {
            message: source.to_string(),
            cause: Some(Rc::new(source)),
        }
    }
    /// Inspect the retained cause for native exception translation.
    #[must_use]
    pub fn cause(&self) -> Option<&(dyn Error + 'static)> {
        self.cause.as_deref()
    }
}
impl fmt::Display for EvaluationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.message.fmt(f)
    }
}
impl Error for EvaluationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.cause()
    }
}
impl PartialEq for EvaluationError {
    fn eq(&self, other: &Self) -> bool {
        self.message == other.message
            && match (&self.cause, &other.cause) {
                (None, None) => true,
                (Some(a), Some(b)) => Rc::ptr_eq(a, b),
                _ => false,
            }
    }
}
impl Eq for EvaluationError {}
