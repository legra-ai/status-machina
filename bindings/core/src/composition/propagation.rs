//! Exclusive bound dispatch and bounded propagation through the kernel's
//! `BoundComposition`.
use std::sync::Arc;

use status_machina::{
    BoundComposition,
    CompositionBindingError,
    Delivery,
    Event,
    PropagationLimits,
};

use super::failure::{
    Failure,
    Fault,
};
use super::report::Report;
use super::spec;
use super::topology::{
    Snapshot,
    Topology,
    claim,
    read,
};
use crate::error::Error;
use crate::ids::Ids;
use crate::runtime::Machine;
use crate::value::{
    Host,
    Value,
};

type Bound<'a, H> = BoundComposition<'a, Value<H>, Value<H>, Value<H>, Ids<H>>;

/// The addressed report, plus the dispatch error that stopped it, if any.
pub struct Propagated<H: Host> {
    pub report: Report,
    pub failure: Option<Error<H>>,
}

/// Validate explicit propagation budgets with the kernel builder.
///
/// # Errors
/// Refuses a missing, zero or over-ceiling dimension.
pub fn limits(
    steps: Option<usize>,
    intents: Option<usize>,
) -> Result<PropagationLimits, Box<Failure>> {
    let mut builder = PropagationLimits::builder();
    if let Some(steps) = steps {
        builder = builder.max_steps(steps);
    }
    if let Some(intents) = intents {
        builder = builder.max_intents(intents);
    }
    builder
        .build()
        .map_err(|error| Box::new(Failure::limits(error)))
}

impl<H: Host> Topology<H> {
    /// Check exclusive bindings of the actual instances with the kernel.
    ///
    /// # Errors
    /// Returns the kernel `BindingError` with its binding entry position.
    pub fn bind(&self, machines: &mut [&mut Machine<H>], source: &str) -> Result<(), Fault<H>> {
        let spec: spec::Binding = read(source)?;
        self.bound(machines, &spec).map(drop)
    }

    /// Bind, then propagate the inputs in the caller's stack. Facts and
    /// capabilities stay fixed for the call; earlier accepted steps survive a
    /// later failure, which is returned with the report.
    ///
    /// # Errors
    /// Returns invalid limits, a foreign snapshot or a binding rejection
    /// before any instance is dispatched.
    pub fn propagate(
        &self,
        machines: &mut [&mut Machine<H>],
        scratch: &Arc<H>,
        source: &str,
        snapshot: Option<&Snapshot<H>>,
    ) -> Result<Propagated<H>, Fault<H>> {
        let spec: spec::Propagation = read(source)?;
        let limits = limits(spec.max_steps, spec.max_intents)?;
        let capabilities = self.capabilities(snapshot)?;
        let facts = claim(scratch, spec.facts)?;
        // bounded: the language reads at most max_intents + 1 inputs; the
        // kernel returns an oversized input vector untouched.
        let mut inputs = Vec::with_capacity(spec.inputs.len());
        for input in &spec.inputs {
            let destination = self.resolve(input.destination, &spec.binding.extra, &[])?;
            let payload = claim(scratch, input.payload)?;
            inputs.push(Delivery::new(destination, Event::new(input.event, payload)));
        }
        let count = inputs.len();
        let mut bound = self.bound(machines, &spec.binding)?;
        let report = bound.propagate(inputs, &facts, capabilities, limits);
        Ok(self.report(report, count))
    }

    fn bound<'a>(
        &'a self,
        machines: &'a mut [&mut Machine<H>],
        spec: &spec::Binding,
    ) -> Result<Bound<'a, H>, Fault<H>> {
        if spec.addresses.len() != machines.len() {
            return Err(Error::construction("binding", "addresses must pair with machines").into());
        }
        let mut builder = self.composition.bind::<Value<H>, Value<H>>();
        // bounded: one address per supplied machine.
        let mut bound = Vec::with_capacity(machines.len());
        for (entry, (machine, index)) in machines.iter_mut().zip(&spec.addresses).enumerate() {
            let address = self.resolve(*index, &spec.extra, &[])?;
            let instance = machine.get_mut()?;
            builder = builder
                .instance(address.clone(), instance)
                .map_err(|error| Failure::binding(Some(entry), error).with_address(*index))?;
            bound.push(address);
        }
        builder.build().map_err(|error| {
            let failure = Failure::binding(None, error);
            if error != CompositionBindingError::MissingInstance {
                return failure.into();
            }
            // The kernel names no address; report the first registered one,
            // in registration order, that was left unbound.
            match self
                .order
                .iter()
                .find(|index| !bound.contains(&self.addresses[**index as usize]))
            {
                Some(index) => failure.with_address(*index).into(),
                None => failure.into(),
            }
        })
    }
}
