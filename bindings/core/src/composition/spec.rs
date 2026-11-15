//! Checked private composition transport. Addresses are indexes into the
//! composition's address table, followed by any call-local extra addresses.
use serde::Deserialize;

/// One native address of the composition table. `machine` indexes the
/// supplied machine slice when the address names that machine's identity.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Address {
    pub scope: u32,
    pub machine: Option<usize>,
}
/// A call-local address absent from the table. Its scope index is distinct
/// per distinct native address; `report` names a report whose instance the
/// native identity denotes.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Extra {
    pub scope: u32,
    pub report: Option<usize>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub address: u32,
    pub machine: usize,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Registry {
    pub max_instances: usize,
    pub entries: Vec<Entry>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Route {
    pub source: u32,
    pub event: u32,
    pub destination: u32,
    pub input_event: u32,
    pub capability: Option<u32>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Topology {
    pub addresses: Vec<Address>,
    pub sources: Registry,
    pub destinations: Registry,
    pub routes: Vec<Route>,
    pub max_routes: usize,
    pub max_batch: usize,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Output {
    pub event: u32,
    pub payload: u32,
}
/// Outputs to route; payload keys belong to the supplied scratch arena.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Batch {
    pub extra: Vec<Extra>,
    pub source: u32,
    pub outputs: Vec<Output>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Revision {
    pub address: u32,
    pub revision: String,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Observation {
    pub address: u32,
    pub report: usize,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub extra: Vec<Extra>,
    pub max_observations: usize,
    pub revisions: Vec<Revision>,
    pub reports: Vec<Observation>,
}
/// Bound addresses, parallel to the supplied machine slice.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub extra: Vec<Extra>,
    pub addresses: Vec<u32>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
    pub destination: u32,
    pub event: u32,
    pub payload: u32,
}
/// Propagation inputs; payload and facts keys belong to the scratch arena.
/// Absent limits are those the native value cannot represent.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Propagation {
    pub binding: Binding,
    pub inputs: Vec<Input>,
    pub facts: u32,
    pub max_steps: Option<usize>,
    pub max_intents: Option<usize>,
}
