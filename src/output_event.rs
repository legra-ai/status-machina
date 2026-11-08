use std::sync::Arc;

pub(crate) type OutputEventConstructor<D, E, F, O> =
    Arc<dyn Fn(&D, &E, &F) -> Result<O, crate::EvaluationError> + Send + Sync>;

/// A typed event description released to the caller after transition
/// acceptance.
pub struct OutputEvent<D, E, F, O, I: crate::MachineIds = crate::DefaultIds> {
    pub(crate) id: I::Event,
    pub(crate) construct: OutputEventConstructor<D, E, F, O>,
    binding: Option<crate::BehaviorRef<I>>,
}

impl<D, E, F, O, I: crate::MachineIds> OutputEvent<D, E, F, O, I> {
    /// Return the registered Story/implementation reference, if bound by a
    /// registry.
    pub const fn binding(&self) -> Option<&crate::BehaviorRef<I>> {
        self.binding.as_ref()
    }

    pub(crate) fn bound(
        id: I::Event,
        binding: crate::BehaviorRef<I>,
        construct: OutputEventConstructor<D, E, F, O>,
    ) -> Self {
        Self {
            id,
            binding: Some(binding),
            construct,
        }
    }

    /// Start building a named typed output event.
    #[must_use]
    pub fn builder(id: I::Event) -> OutputEventBuilder<D, E, F, O, I> {
        OutputEventBuilder {
            id,
            construct: None,
        }
    }
}

/// Builder for a caller-owned typed output event.
pub struct OutputEventBuilder<D, E, F, O, I: crate::MachineIds = crate::DefaultIds> {
    id: I::Event,
    construct: Option<OutputEventConstructor<D, E, F, O>>,
}

impl<D, E, F, O, I: crate::MachineIds> OutputEventBuilder<D, E, F, O, I> {
    /// Describe the bounded payload to return after the transition is accepted.
    #[must_use]
    pub fn describe(mut self, construct: impl Fn(&D, &E, &F) -> O + Send + Sync + 'static) -> Self {
        self.construct = Some(Arc::new(move |data, event, facts| {
            Ok(construct(data, event, facts))
        }));
        self
    }

    /// Bind an intent constructor that can return an application failure.
    #[must_use]
    pub fn try_describe(
        mut self,
        construct: impl Fn(&D, &E, &F) -> Result<O, crate::EvaluationError> + Send + Sync + 'static,
    ) -> Self {
        self.construct = Some(Arc::new(construct));
        self
    }

    /// Build the output event description.
    ///
    /// # Errors
    ///
    /// Returns an error when no typed payload constructor was supplied.
    pub fn build(self) -> Result<OutputEvent<D, E, F, O, I>, OutputEventBuildError> {
        Ok(OutputEvent {
            binding: None,
            id: self.id,
            construct: self
                .construct
                .ok_or(OutputEventBuildError::MissingConstructor)?,
        })
    }
}

/// An output-event builder omitted its payload constructor.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum OutputEventBuildError {
    /// No typed payload constructor was supplied.
    #[error("output-event payload constructor is missing")]
    MissingConstructor,
}

/// One typed output event returned after a transition is accepted.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OutputEventIntent<O, I: crate::MachineIds = crate::DefaultIds> {
    id: I::Event,
    payload: O,
}

impl<O, I: crate::MachineIds> OutputEventIntent<O, I> {
    pub(crate) const fn new(id: I::Event, payload: O) -> Self {
        Self { id, payload }
    }

    /// Return the output event identifier.
    #[must_use]
    pub const fn id(&self) -> &I::Event {
        &self.id
    }

    /// Return the caller-defined bounded payload, including any correlation
    /// data.
    #[must_use]
    pub const fn payload(&self) -> &O {
        &self.payload
    }

    /// Consume the intent and return its identifier and owned event payload.
    ///
    /// This transfers ownership without cloning the payload or dispatching it.
    #[must_use]
    pub fn into_parts(self) -> (I::Event, O) {
        (self.id, self.payload)
    }
}
