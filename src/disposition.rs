use crate::Condition;

/// An explicitly harmless event outcome, such as a stale completion.
pub struct EventDisposition<D, E, F, I: crate::MachineIds = crate::DefaultIds> {
    pub(crate) id: I::Disposition,
    pub(crate) from: I::State,
    pub(crate) event: I::Event,
    pub(crate) condition: Condition<D, E, F, I>,
    pub(crate) reason: String,
}

impl<D, E, F, I: crate::MachineIds> EventDisposition<D, E, F, I> {
    /// Start building one event disposition.
    #[must_use]
    pub fn builder(id: I::Disposition) -> EventDispositionBuilder<D, E, F, I> {
        EventDispositionBuilder {
            id,
            from: None,
            event: None,
            condition: None,
            reason: None,
        }
    }

    /// Return the stable disposition identifier.
    #[must_use]
    pub const fn id(&self) -> &I::Disposition {
        &self.id
    }
}

/// Builder for a named event disposition.
pub struct EventDispositionBuilder<D, E, F, I: crate::MachineIds = crate::DefaultIds> {
    id: I::Disposition,
    from: Option<I::State>,
    event: Option<I::Event>,
    condition: Option<Condition<D, E, F, I>>,
    reason: Option<String>,
}

impl<D, E, F, I: crate::MachineIds> EventDispositionBuilder<D, E, F, I> {
    /// Set the state in which this disposition is valid.
    #[must_use]
    pub fn from(mut self, state: I::State) -> Self {
        self.from = Some(state);
        self
    }

    /// Set the event this disposition handles.
    #[must_use]
    pub fn on(mut self, event: I::Event) -> Self {
        self.event = Some(event);
        self
    }

    /// Set the condition under which the event is intentionally ignored.
    #[must_use]
    pub fn when(mut self, condition: Condition<D, E, F, I>) -> Self {
        self.condition = Some(condition);
        self
    }

    /// Explain why the event is intentionally ignored.
    #[must_use]
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }

    /// Validate and build the disposition.
    ///
    /// # Errors
    ///
    /// Returns an error when a required field is absent or the reason is empty
    /// or over 256 bytes.
    pub fn build(self) -> Result<EventDisposition<D, E, F, I>, DispositionBuildError> {
        let reason = self.reason.ok_or(DispositionBuildError::MissingReason)?;
        if reason.trim().is_empty() || reason.len() > 256 {
            return Err(DispositionBuildError::InvalidReason);
        }
        Ok(EventDisposition {
            id: self.id,
            from: self.from.ok_or(DispositionBuildError::MissingState)?,
            event: self.event.ok_or(DispositionBuildError::MissingEvent)?,
            condition: self
                .condition
                .ok_or(DispositionBuildError::MissingCondition)?,
            reason,
        })
    }
}

/// A disposition builder omitted a required or valid field.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum DispositionBuildError {
    /// No source state was supplied.
    #[error("disposition source state is missing")]
    MissingState,
    /// No triggering event was supplied.
    #[error("disposition event is missing")]
    MissingEvent,
    /// No condition was supplied.
    #[error("disposition condition is missing")]
    MissingCondition,
    /// No explanation was supplied.
    #[error("disposition reason is missing")]
    MissingReason,
    /// The explanation is empty or exceeds 256 bytes.
    #[error("disposition reason must contain 1 to 256 non-whitespace bytes")]
    InvalidReason,
}
