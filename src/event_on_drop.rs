//! Caller-owned synchronous scope-exit reporting.

/// Delivers one typed event when an unfinished operation leaves its scope.
///
/// The handler is supplied by the caller and runs synchronously during drop. It
/// can borrow an exclusively owned machine or reach a surviving shared owner.
/// Correlation, checked dispatch, handling every returned effect/error, and
/// releasing locks before this guard drops remain the caller's responsibility.
/// No state is assigned directly and no runtime or background work is created.
///
/// On explicit completion, dispatch the completion event first, then
/// [`disarm`](Self::disarm). An armed guard also runs on early return,
/// unwinding, or cancellation that drops a polled future containing it.
/// Destructors are not guaranteed to run on process abort, termination, or
/// `std::mem::forget`. A future never polled has not constructed a guard inside
/// its async body.
///
/// # Handler failures
///
/// The handler returns `()`: it must explicitly handle dispatch errors and any
/// emitted effects. It must not start asynchronous cleanup or claim an external
/// operation has settled merely because its scope ended. A handler panic
/// follows normal Rust destructor semantics, including process abort if another
/// panic is already unwinding. This type does not catch, suppress, or retry
/// failures.
///
/// ```
/// use std::cell::Cell;
///
/// use status_machina::EventOnDrop;
///
/// let abandoned = Cell::new(false);
/// {
///     let _operation = EventOnDrop::new("abandoned", |event| {
///         assert_eq!(event, "abandoned");
///         abandoned.set(true);
///     });
/// }
/// assert!(abandoned.get());
/// ```
///
/// Fallible and async handlers must supply an explicit synchronous policy:
///
/// ```compile_fail
/// use status_machina::EventOnDrop;
/// let _guard = EventOnDrop::new((), |()| Err::<(), _>("unhandled failure"));
/// ```
///
/// ```compile_fail
/// use status_machina::EventOnDrop;
/// let _guard = EventOnDrop::new((), |()| async {});
/// ```
#[must_use = "bind the guard to the operation scope; an unused guard drops immediately"]
pub struct EventOnDrop<E, F: FnOnce(E)> {
    pending: Option<(E, F)>,
}

impl<E, F: FnOnce(E)> EventOnDrop<E, F> {
    /// Arm one event and its caller-owned synchronous delivery handler.
    ///
    /// Construction performs no dispatch. Neither value needs to be `Clone`,
    /// `Send`, `'static`, serialized, boxed, or registered globally.
    pub fn new(event: E, handler: F) -> Self {
        Self {
            pending: Some((event, handler)),
        }
    }

    /// Disarm delivery after explicit completion; repeated calls do nothing.
    ///
    /// Releases the event and handler without calling the handler. This does
    /// not report success or advance a machine: the caller does that first.
    pub fn disarm(&mut self) {
        self.pending = None;
    }

    /// Borrow the armed event to update its correlation after accepted
    /// progress.
    ///
    /// Returns `None` after disarming; a completed guard cannot be rearmed.
    /// Stale-event rejection belongs to the caller's checked dispatch handler.
    pub fn event_mut(&mut self) -> Option<&mut E> {
        self.pending.as_mut().map(|(event, _)| event)
    }
}

impl<E, F: FnOnce(E)> Drop for EventOnDrop<E, F> {
    fn drop(&mut self) {
        if let Some((event, handler)) = self.pending.take() {
            handler(event);
        }
    }
}
