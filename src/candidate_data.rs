/// Machine data that can produce an isolated candidate for local evaluation.
///
/// # Contract
///
/// The snapshot must initially represent the same logical data as `self`.
/// Mutating or dropping either value must not change the other's observable
/// state. Shared immutable storage is allowed; shared mutable storage must be
/// detached before it can be changed. Snapshotting must be deterministic,
/// bounded and free of I/O or other external side effects.
///
/// The caller must keep all callbacks pure apart from mutations of the
/// candidate supplied to local actions. Neither this trait nor Rust's borrow
/// checker can prove the absence of aliases through interior mutability.
/// Violating these contracts invalidates dispatch's isolation guarantee.
/// Panics and external effects are not rolled back.
///
/// There is deliberately no blanket implementation for `Clone` or `Copy`:
/// those traits can preserve aliases to mutable storage. Implement this trait
/// on the caller's domain data after checking its ownership semantics.
///
/// ```compile_fail
/// use status_machina::CandidateData;
/// #[derive(Clone)]
/// struct CloneOnly;
/// fn requires_isolation<T: CandidateData>() {}
/// requires_isolation::<CloneOnly>();
/// ```
pub trait CandidateData: Sized {
    /// Produce an equivalent, independently mutable candidate.
    ///
    /// Payload size and snapshot cost must be bounded by the caller. The same
    /// boundary is used for dispatch and fresh scenario instances.
    #[must_use]
    fn snapshot(&self) -> Self;
}

macro_rules! scalar_data {
    ($($ty:ty),+ $(,)?) => {
        $(impl CandidateData for $ty {
            fn snapshot(&self) -> Self {
                *self
            }
        })+
    };
}

scalar_data!(
    (),
    bool,
    char,
    u8,
    u16,
    u32,
    u64,
    u128,
    usize,
    i8,
    i16,
    i32,
    i64,
    i128,
    isize,
    f32,
    f64
);

/// Data whose isolated snapshot can fail with an application exception.
///
/// The isolation, purity and bounded-cost contract is the same as
/// [`CandidateData`]. Infallible domain data implements [`CandidateData`] and
/// automatically satisfies this capability. Native domain data can implement
/// this trait directly when copying requires fallible application code.
pub trait FallibleCandidateData: Sized {
    /// Produce an equivalent independently mutable candidate, or stop
    /// evaluation.
    ///
    /// # Errors
    /// Returns the original snapshot failure without changing accepted data.
    fn try_snapshot(&self) -> Result<Self, crate::EvaluationError>;

    /// Finish isolation after application callbacks and before creating a
    /// decision.
    ///
    /// Native data overrides this when callbacks could retain aliases to their
    /// candidate argument. Acceptance itself never invokes application code.
    ///
    /// # Errors
    /// Returns an isolation failure, discarding the unaccepted candidate.
    fn prepare_accept(self) -> Result<Self, crate::EvaluationError> {
        Ok(self)
    }
}
impl<T: CandidateData> FallibleCandidateData for T {
    fn try_snapshot(&self) -> Result<Self, crate::EvaluationError> {
        Ok(self.snapshot())
    }
}
