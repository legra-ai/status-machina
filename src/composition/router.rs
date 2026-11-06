use std::any::TypeId;
use std::collections::HashMap;

use super::{
    CapabilitySnapshot,
    DEFAULT_MAX_BATCH,
    DEFAULT_MAX_ROUTES,
    HARD_MAX_BATCH,
    HARD_MAX_ROUTES,
    InstanceAddress,
    InstanceRegistry,
    Route,
};
use crate::{
    CapabilityStatus,
    Event,
    OutputEventIntent,
};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct RouteKey<I: crate::MachineIds = crate::DefaultIds> {
    source: InstanceAddress<I>,
    event: I::Event,
}

/// Bounded, explicit event composition. It never owns or dispatches instances.
pub struct Composition<E, I: crate::MachineIds = crate::DefaultIds, J: crate::MachineIds = I> {
    sources: InstanceRegistry<I>,
    routes: HashMap<RouteKey<I>, Route<I, J>>,
    max_batch: usize,
    payload: std::marker::PhantomData<fn(E)>,
}

impl<E, I: crate::MachineIds, J: crate::MachineIds> Composition<E, I, J> {
    /// Start building a composition over an explicit instance registry.
    #[must_use]
    pub fn builder(
        sources: InstanceRegistry<I>,
        destinations: InstanceRegistry<J>,
    ) -> CompositionBuilder<E, I, J> {
        CompositionBuilder {
            sources,
            destinations,
            routes: Vec::new(),
            max_routes: DEFAULT_MAX_ROUTES,
            max_batch: DEFAULT_MAX_BATCH,
            payload: std::marker::PhantomData,
        }
    }

    /// Route one bounded caller-owned batch without dispatching any
    /// destination.
    ///
    /// Each input produces exactly one ordered outcome. The caller retains the
    /// original vector if the source is unregistered or the batch exceeds its
    /// bound, because those errors return the vector intact.
    ///
    /// # Errors
    ///
    /// Returns an error with the original output vector for an unregistered
    /// source or an oversized batch.
    pub fn route_batch(
        &self,
        source: &InstanceAddress<I>,
        outputs: Vec<OutputEventIntent<E, I>>,
        capabilities: &CapabilitySnapshot<J>,
    ) -> Result<RoutedBatch<E, I, J>, RouteBatchError<E, I>> {
        if !self.sources.contains(source) {
            return Err(RouteBatchError {
                kind: RouteBatchErrorKind::UnregisteredSource,
                outputs,
            });
        }
        if outputs.len() > self.max_batch {
            return Err(RouteBatchError {
                kind: RouteBatchErrorKind::BatchLimitExceeded {
                    limit: self.max_batch,
                },
                outputs,
            });
        }

        // bounded: outputs.len() was checked against max_batch above.
        let mut items = Vec::with_capacity(outputs.len());
        for intent in outputs {
            items.push(self.route_one(source, intent, capabilities));
        }
        Ok(RoutedBatch { items })
    }

    fn route_one(
        &self,
        source: &InstanceAddress<I>,
        intent: OutputEventIntent<E, I>,
        capabilities: &CapabilitySnapshot<J>,
    ) -> RoutedItem<E, I, J> {
        let key = RouteKey {
            source: source.clone(),
            event: intent.id().clone(),
        };
        let Some(route) = self.routes.get(&key) else {
            return RoutedItem::Unmatched {
                source: source.clone(),
                intent,
            };
        };
        let destination = route.destination.clone();
        if let Some(capability) = &route.capability {
            let status = capabilities.status(&destination, capability);
            if status != CapabilityStatus::Available {
                return RoutedItem::Deferred {
                    source: source.clone(),
                    destination,
                    capability: capability.clone(),
                    status,
                    intent,
                };
            }
        }
        let (_, payload) = intent.into_parts();
        RoutedItem::Routed {
            source: source.clone(),
            destination,
            event: Event::new(route.input_event.clone(), payload),
        }
    }
}

/// Bounded builder for explicit, caller-driven event composition.
pub struct CompositionBuilder<E, I: crate::MachineIds = crate::DefaultIds, J: crate::MachineIds = I>
{
    sources: InstanceRegistry<I>,
    destinations: InstanceRegistry<J>,
    // bounded: max_routes is capped at HARD_MAX_ROUTES.
    routes: Vec<Route<I, J>>,
    max_routes: usize,
    max_batch: usize,
    payload: std::marker::PhantomData<fn(E)>,
}

impl<E, I: crate::MachineIds, J: crate::MachineIds> CompositionBuilder<E, I, J> {
    /// Set positive route and batch bounds within implementation ceilings.
    ///
    /// # Errors
    ///
    /// Returns an error when a limit is zero, exceeds its ceiling, or is below
    /// the number of routes already added.
    pub fn limits(
        mut self,
        max_routes: usize,
        max_batch: usize,
    ) -> Result<Self, CompositionBuildError> {
        if max_routes == 0
            || max_routes > HARD_MAX_ROUTES
            || max_batch == 0
            || max_batch > HARD_MAX_BATCH
        {
            return Err(CompositionBuildError::InvalidLimits);
        }
        if self.routes.len() > max_routes {
            return Err(CompositionBuildError::LimitBelowCurrentSize);
        }
        self.max_routes = max_routes;
        self.max_batch = max_batch;
        Ok(self)
    }

    /// Add a route, rejecting duplicate source/event pairs.
    ///
    /// # Errors
    ///
    /// Returns an error when the route key is duplicated or the route bound is
    /// reached.
    pub fn route(mut self, route: Route<I, J>) -> Result<Self, CompositionBuildError> {
        if self
            .routes
            .iter()
            .any(|existing| existing.source == route.source && existing.event == route.event)
        {
            return Err(CompositionBuildError::DuplicateRoute);
        }
        if self.routes.len() == self.max_routes {
            return Err(CompositionBuildError::RouteLimitExceeded {
                limit: self.max_routes,
            });
        }
        self.routes.push(route);
        Ok(self)
    }

    /// Validate every route endpoint and build the immutable composition.
    ///
    /// # Errors
    ///
    /// Returns an error when a source or destination is not registered.
    pub fn build(self) -> Result<Composition<E, I, J>, CompositionBuildError>
    where
        E: 'static,
    {
        let mut routes = HashMap::with_capacity(self.routes.len());
        for route in self.routes {
            if !self.sources.contains(&route.source) {
                return Err(CompositionBuildError::UnregisteredSource);
            }
            let source_signature = self
                .sources
                .signature(&route.source)
                .ok_or(CompositionBuildError::UnregisteredSource)?;
            if source_signature.output_payload != TypeId::of::<E>() {
                return Err(CompositionBuildError::SourcePayloadMismatch);
            }
            if !source_signature.output_events.contains(&route.event) {
                return Err(CompositionBuildError::UndeclaredSourceEvent);
            }
            if !self.destinations.contains(&route.destination) {
                return Err(CompositionBuildError::UnregisteredDestination);
            }
            let destination_signature = self
                .destinations
                .signature(&route.destination)
                .ok_or(CompositionBuildError::UnregisteredDestination)?;
            if destination_signature.input_payload != TypeId::of::<E>() {
                return Err(CompositionBuildError::DestinationPayloadMismatch);
            }
            if !destination_signature
                .input_events
                .contains(&route.input_event)
            {
                return Err(CompositionBuildError::UndeclaredDestinationEvent);
            }
            routes.insert(
                RouteKey {
                    source: route.source.clone(),
                    event: route.event.clone(),
                },
                route,
            );
        }
        Ok(Composition {
            sources: self.sources,
            routes,
            max_batch: self.max_batch,
            payload: std::marker::PhantomData,
        })
    }
}

/// An invalid or over-capacity composition definition.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum CompositionBuildError {
    /// One or both limits are outside supported bounds.
    #[error("composition limits are outside the supported range")]
    InvalidLimits,
    /// New limits are below the existing route count.
    #[error("composition limits are below the current size")]
    LimitBelowCurrentSize,
    /// A source/event route key was declared twice.
    #[error("duplicate source/event route")]
    DuplicateRoute,
    /// The configured route count was reached.
    #[error("composition route limit {limit} exceeded")]
    RouteLimitExceeded {
        /// Configured maximum route count.
        limit: usize,
    },
    /// A route source is not registered.
    #[error("route source address is not registered")]
    UnregisteredSource,
    /// A route destination is not registered.
    #[error("route destination address is not registered")]
    UnregisteredDestination,
    /// Source output payload type differs from the composition payload type.
    #[error("route source output payload type does not match composition type")]
    SourcePayloadMismatch,
    /// Destination input payload type differs from the composition payload
    /// type.
    #[error("route destination input payload type does not match composition type")]
    DestinationPayloadMismatch,
    /// Source event ID is not declared by its registered instance.
    #[error("route source output event is not declared by its instance")]
    UndeclaredSourceEvent,
    /// Destination event ID is not declared by its registered instance.
    #[error("route destination input event is not declared by its instance")]
    UndeclaredDestinationEvent,
}

/// Ordered outcomes for one caller-supplied composition batch.
#[derive(Debug)]
pub struct RoutedBatch<E, I: crate::MachineIds = crate::DefaultIds, J: crate::MachineIds = I> {
    items: Vec<RoutedItem<E, I, J>>,
}

impl<E, I: crate::MachineIds, J: crate::MachineIds> RoutedBatch<E, I, J> {
    /// Return one ordered outcome per input output event.
    #[must_use]
    pub fn items(&self) -> &[RoutedItem<E, I, J>] {
        &self.items
    }

    /// Consume the batch and return its ordered, owned routing outcomes.
    ///
    /// The caller can move routed events into destination dispatch and retain
    /// deferred or unmatched intents without cloning their payloads. No
    /// destination is dispatched by this method.
    #[must_use]
    pub fn into_items(self) -> Vec<RoutedItem<E, I, J>> {
        self.items
    }
}

/// One explicit result for a caller-owned output event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RoutedItem<E, I: crate::MachineIds = crate::DefaultIds, J: crate::MachineIds = I> {
    /// Caller may resolve this registered target and dispatch the typed event.
    Routed {
        /// Source instance address from the route key.
        source: InstanceAddress<I>,
        /// Registered destination instance address.
        destination: InstanceAddress<J>,
        /// Typed event data; it has not been dispatched.
        event: Event<E, J>,
    },
    /// No route was declared; the original intent is preserved for the caller.
    Unmatched {
        /// Source instance address supplied by the caller.
        source: InstanceAddress<I>,
        /// Original output intent, not discarded.
        intent: OutputEventIntent<E, I>,
    },
    /// Capability status is not Available; original intent remains
    /// caller-owned.
    Deferred {
        /// Source instance address supplied by the caller.
        source: InstanceAddress<I>,
        /// Registered destination that requires the capability.
        destination: InstanceAddress<J>,
        /// Capability policy required by the route.
        capability: J::Capability,
        /// Advisory status; never an authorization result.
        status: CapabilityStatus,
        /// Original output intent, not discarded.
        intent: OutputEventIntent<E, I>,
    },
}

/// The composition batch could not be routed; inputs remain recoverable.
#[derive(Debug)]
pub struct RouteBatchError<E, I: crate::MachineIds = crate::DefaultIds> {
    kind: RouteBatchErrorKind,
    outputs: Vec<OutputEventIntent<E, I>>,
}

impl<E, I: crate::MachineIds> RouteBatchError<E, I> {
    /// Return the reason the batch was refused.
    #[must_use]
    pub const fn kind(&self) -> RouteBatchErrorKind {
        self.kind
    }

    /// Recover every original output intent from the refused batch.
    #[must_use]
    pub fn into_outputs(self) -> Vec<OutputEventIntent<E, I>> {
        self.outputs
    }
}

/// Why the composition refused to start routing a batch.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum RouteBatchErrorKind {
    /// Source address was not part of the validated instance registry.
    #[error("batch source address is not registered")]
    UnregisteredSource,
    /// Batch exceeded the configured work bound.
    #[error("composition batch limit {limit} exceeded")]
    BatchLimitExceeded {
        /// Configured maximum batch size.
        limit: usize,
    },
}
