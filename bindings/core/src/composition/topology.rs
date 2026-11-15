//! One checked composition: kernel registries, routes and limits over a native
//! address table.
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::sync::Arc;

use serde::de::DeserializeOwned;
use status_machina::{
    CapabilitySnapshot,
    Composition,
    InstanceAddress,
    InstanceRegistry,
    OutputEventIntent,
    Route,
};

use super::failure::{
    Failure,
    Fault,
};
use super::report::Item;
use super::spec;
use crate::error::Error;
use crate::ids::Ids;
use crate::runtime::Machine;
use crate::value::{
    Host,
    Value,
};

pub(super) type Address<H> = InstanceAddress<Ids<H>>;
pub(super) type KernelReport<H> = status_machina::CapabilityReport<Ids<H>>;
type NativeComposition<H> = Composition<Value<H>, Ids<H>>;
type NativeRoute<H> = Route<Ids<H>>;
const MAX_ADDRESSES: usize = 4096;
const MAX_TRANSPORT_BYTES: usize = 1_048_576;

/// A kernel capability snapshot, owned by the composition whose address table
/// lowered its observations.
pub struct Snapshot<H: Host> {
    pub(super) owner: uuid::Uuid,
    pub(super) snapshot: CapabilitySnapshot<Ids<H>>,
}

/// Immutable checked composition. It retains no machine instance.
pub struct Topology<H: Host> {
    pub(super) id: uuid::Uuid,
    pub(super) composition: NativeComposition<H>,
    // bounded: the table is capped at MAX_ADDRESSES entries.
    pub(super) addresses: Vec<Address<H>>,
    // bounded: registered addresses are capped by the two registry bounds.
    registered: HashMap<Address<H>, u32>,
    // bounded: one table index per registered address.
    pub(super) order: Vec<u32>,
    empty: CapabilitySnapshot<Ids<H>>,
}
impl<H: Host> Topology<H> {
    /// Register the supplied instances and validate every route through the
    /// kernel builders. Machines are read for their signatures only.
    ///
    /// # Errors
    /// Returns the kernel rejection with its registry entry or route position.
    pub fn try_new(source: &str, machines: &[&Machine<H>]) -> Result<Self, Fault<H>> {
        let spec: spec::Topology = read(source)?;
        if spec.addresses.len() > MAX_ADDRESSES {
            return Err(Error::construction("composition", "address table exceeds 4096").into());
        }
        // bounded: checked against MAX_ADDRESSES above.
        let mut addresses = Vec::with_capacity(spec.addresses.len());
        for address in &spec.addresses {
            let instance = match address.machine {
                Some(index) => machine(machines, index)?.get()?.instance_id(),
                None => uuid::Uuid::nil(),
            };
            addresses.push(InstanceAddress::new(address.scope, instance));
        }
        let sources = registry("sources", &spec.sources, &addresses, machines)?;
        let destinations = registry("destinations", &spec.destinations, &addresses, machines)?;
        // bounded: the 1 MiB transport caps the list; the kernel caps accepted
        // routes.
        let mut routes = Vec::with_capacity(spec.routes.len());
        for (position, route) in spec.routes.iter().enumerate() {
            routes.push(native_route(position, route, &addresses)?);
        }
        let composition = compose(&spec, &sources, &destinations, &routes)?;
        let mut registered = HashMap::new();
        let mut order = Vec::new();
        for entry in spec
            .sources
            .entries
            .iter()
            .chain(&spec.destinations.entries)
        {
            let address = table(&addresses, entry.address)?;
            if let Entry::Vacant(slot) = registered.entry(address) {
                slot.insert(entry.address);
                order.push(entry.address);
            }
        }
        Ok(Self {
            id: uuid::Uuid::now_v7(),
            composition,
            addresses,
            registered,
            order,
            empty: CapabilitySnapshot::empty(),
        })
    }

    /// Route one caller batch without dispatching any destination. Payload
    /// keys belong to `scratch`; items are in output order.
    ///
    /// # Errors
    /// Returns the kernel `RouteBatchError` kind, or a foreign snapshot.
    pub fn route_batch(
        &self,
        scratch: &Arc<H>,
        source: &str,
        snapshot: Option<&Snapshot<H>>,
    ) -> Result<Vec<Item>, Fault<H>> {
        let spec: spec::Batch = read(source)?;
        let capabilities = self.capabilities(snapshot)?;
        let source = self.resolve(spec.source, &spec.extra, &[])?;
        // bounded: the language reads at most max_batch + 1 outputs, and the
        // kernel refuses a batch above max_batch.
        let mut outputs = Vec::with_capacity(spec.outputs.len());
        for output in &spec.outputs {
            outputs.push(OutputEventIntent::new(
                output.event,
                claim(scratch, output.payload)?,
            ));
        }
        let batch = self
            .composition
            .route_batch(&source, outputs, capabilities)
            .map_err(|error| Failure::batch(error.kind()))?;
        // bounded: one item per accepted output.
        Ok(batch
            .into_items()
            .into_iter()
            .map(|item| Item::new(item, self))
            .collect())
    }

    /// Build a kernel capability snapshot: current revisions first, then
    /// reports, in caller order.
    ///
    /// # Errors
    /// Returns the kernel rejection with its list and entry position.
    pub fn snapshot(
        &self,
        source: &str,
        reports: &[&KernelReport<H>],
    ) -> Result<Snapshot<H>, Fault<H>> {
        let spec: spec::Snapshot = read(source)?;
        let mut builder = CapabilitySnapshot::builder()
            .max_observations(spec.max_observations)
            .map_err(|error| Failure::snapshot(None, None, error))?;
        for (entry, revision) in spec.revisions.iter().enumerate() {
            let address = self.resolve(revision.address, &spec.extra, reports)?;
            let value = revision
                .revision
                .parse()
                .map_err(|error| Error::construction("revision", error))?;
            builder = builder.current_revision(address, value).map_err(|error| {
                Failure::snapshot(Some("revisions"), Some(entry), error)
                    .with_address(revision.address)
            })?;
        }
        for (entry, observation) in spec.reports.iter().enumerate() {
            let address = self.resolve(observation.address, &spec.extra, reports)?;
            let report = report(reports, observation.report)?;
            builder = builder.report(address, report.clone()).map_err(|error| {
                Failure::snapshot(Some("reports"), Some(entry), error)
                    .with_address(observation.address)
            })?;
        }
        Ok(Snapshot {
            owner: self.id,
            snapshot: builder.build(),
        })
    }

    pub(super) fn capabilities<'a>(
        &'a self,
        snapshot: Option<&'a Snapshot<H>>,
    ) -> Result<&'a CapabilitySnapshot<Ids<H>>, Box<Failure>> {
        match snapshot {
            None => Ok(&self.empty),
            Some(snapshot) if snapshot.owner == self.id => Ok(&snapshot.snapshot),
            Some(_) => Err(Box::new(Failure::foreign_snapshot())),
        }
    }

    /// Resolve a table index or a call-local extra address.
    pub(super) fn resolve(
        &self,
        index: u32,
        extra: &[spec::Extra],
        reports: &[&KernelReport<H>],
    ) -> Result<Address<H>, Error<H>> {
        let position = index as usize;
        if let Some(address) = self.addresses.get(position) {
            return Ok(address.clone());
        }
        let extra = extra
            .get(position - self.addresses.len())
            .ok_or_else(|| Error::construction("composition", "unknown address index"))?;
        let instance = match extra.report {
            Some(index) => report(reports, index)?.instance(),
            None => uuid::Uuid::nil(),
        };
        Ok(InstanceAddress::new(extra.scope, instance))
    }

    /// Table index of a registered address.
    pub(super) fn index(&self, address: &Address<H>) -> Option<u32> {
        self.registered.get(address).copied()
    }
}

pub(super) fn read<T: DeserializeOwned, H: Host>(source: &str) -> Result<T, Error<H>> {
    if source.len() > MAX_TRANSPORT_BYTES {
        return Err(Error::construction(
            "composition",
            "transport exceeds 1 MiB",
        ));
    }
    serde_json::from_str(source).map_err(|error| Error::construction("composition", error))
}
pub(super) fn claim<H: Host>(host: &Arc<H>, key: u32) -> Result<Value<H>, Error<H>> {
    Value::claim(Arc::clone(host), key).map_err(Error::evaluation)
}
fn machine<'a, H: Host>(
    machines: &[&'a Machine<H>],
    index: usize,
) -> Result<&'a Machine<H>, Error<H>> {
    machines
        .get(index)
        .copied()
        .ok_or_else(|| Error::construction("composition", "unknown machine index"))
}
fn report<'a, H: Host>(
    reports: &[&'a KernelReport<H>],
    index: usize,
) -> Result<&'a KernelReport<H>, Error<H>> {
    reports
        .get(index)
        .copied()
        .ok_or_else(|| Error::construction("composition", "unknown capability report index"))
}
fn table<H: Host>(addresses: &[Address<H>], index: u32) -> Result<Address<H>, Error<H>> {
    addresses
        .get(index as usize)
        .cloned()
        .ok_or_else(|| Error::construction("composition", "unknown address index"))
}
fn registry<H: Host>(
    label: &'static str,
    spec: &spec::Registry,
    addresses: &[Address<H>],
    machines: &[&Machine<H>],
) -> Result<InstanceRegistry<Ids<H>>, Fault<H>> {
    let mut builder = InstanceRegistry::builder()
        .max_instances(spec.max_instances)
        .map_err(|error| Failure::registry(label, None, error))?;
    for (position, entry) in spec.entries.iter().enumerate() {
        let address = table(addresses, entry.address)?;
        let instance = machine(machines, entry.machine)?.get()?;
        builder = builder.instance(address, instance).map_err(|error| {
            Failure::registry(label, Some(position), error).with_address(entry.address)
        })?;
    }
    Ok(builder.build())
}
fn native_route<H: Host>(
    position: usize,
    spec: &spec::Route,
    addresses: &[Address<H>],
) -> Result<NativeRoute<H>, Fault<H>> {
    let mut builder = Route::builder(table(addresses, spec.source)?, spec.event)
        .to(table(addresses, spec.destination)?)
        .as_event(spec.input_event);
    if let Some(capability) = spec.capability {
        builder = builder.requires(capability);
    }
    Ok(builder
        .build()
        .map_err(|error| Failure::route(position, error))?)
}
/// Build through the kernel; when `build` rejects a route without naming it,
/// rebuild growing prefixes so the first route the kernel rejects is reported.
fn compose<H: Host>(
    spec: &spec::Topology,
    sources: &InstanceRegistry<Ids<H>>,
    destinations: &InstanceRegistry<Ids<H>>,
    routes: &[NativeRoute<H>],
) -> Result<NativeComposition<H>, Box<Failure>> {
    let attempt = |count: usize| -> Result<NativeComposition<H>, (Box<Failure>, bool)> {
        let mut builder = Composition::builder(sources.clone(), destinations.clone())
            .limits(spec.max_routes, spec.max_batch)
            .map_err(|error| (Box::new(Failure::definition(None, error)), false))?;
        for (position, route) in routes.iter().take(count).enumerate() {
            builder = builder
                .route(route.clone())
                .map_err(|error| (Box::new(Failure::definition(Some(position), error)), false))?;
        }
        builder
            .build()
            .map_err(|error| (Box::new(Failure::definition(None, error)), true))
    };
    match attempt(routes.len()) {
        Ok(composition) => Ok(composition),
        Err((failure, false)) => Err(failure),
        Err((failure, true)) => {
            let position = (1..=routes.len())
                .find(|count| attempt(*count).is_err())
                .map(|count| count - 1);
            let mut failure = failure;
            failure.route = position;
            Err(failure)
        }
    }
}
