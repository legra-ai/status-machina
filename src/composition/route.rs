use super::InstanceAddress;

/// One explicit mapping from a source output event to a destination instance.
#[derive(Clone, Debug)]
pub struct Route<I: crate::MachineIds = crate::DefaultIds, J: crate::MachineIds = I> {
    pub(super) source: InstanceAddress<I>,
    pub(super) event: I::Event,
    pub(super) input_event: J::Event,
    pub(super) destination: InstanceAddress<J>,
    pub(super) capability: Option<J::Capability>,
}

impl<I: crate::MachineIds, J: crate::MachineIds> Route<I, J> {
    /// Start building a route for one source address and output event.
    #[must_use]
    pub fn builder(source: InstanceAddress<I>, event: I::Event) -> RouteBuilder<I, J> {
        RouteBuilder {
            source,
            event,
            destination: None,
            input_event: None,
            capability: None,
        }
    }
}

/// Builder for one source-event route.
pub struct RouteBuilder<I: crate::MachineIds = crate::DefaultIds, J: crate::MachineIds = I> {
    source: InstanceAddress<I>,
    event: I::Event,
    destination: Option<InstanceAddress<J>>,
    input_event: Option<J::Event>,
    capability: Option<J::Capability>,
}

impl<I: crate::MachineIds, J: crate::MachineIds> RouteBuilder<I, J> {
    /// Set the registered destination address.
    #[must_use]
    pub fn to(mut self, destination: InstanceAddress<J>) -> Self {
        self.destination = Some(destination);
        self
    }

    /// Name the destination's input event for this source output.
    #[must_use]
    pub fn as_event(mut self, input_event: J::Event) -> Self {
        self.input_event = Some(input_event);
        self
    }

    /// Require an advisory capability observation before returning the route.
    #[must_use]
    pub fn requires(mut self, capability: J::Capability) -> Self {
        self.capability = Some(capability);
        self
    }

    /// Validate and build the route.
    ///
    /// # Errors
    ///
    /// Returns an error when a destination or input event was omitted.
    pub fn build(self) -> Result<Route<I, J>, RouteBuildError> {
        Ok(Route {
            source: self.source,
            event: self.event,
            input_event: self.input_event.ok_or(RouteBuildError::MissingInputEvent)?,
            destination: self
                .destination
                .ok_or(RouteBuildError::MissingDestination)?,
            capability: self.capability,
        })
    }
}

/// A route builder omitted its destination address.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum RouteBuildError {
    /// No destination address was supplied.
    #[error("route destination is missing")]
    MissingDestination,
    /// The destination input event was not supplied.
    #[error("route destination input event is missing")]
    MissingInputEvent,
}
