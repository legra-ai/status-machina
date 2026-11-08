//! Bounded metadata-only inspection through the Rust graph renderer.
use std::collections::{
    BTreeMap,
    BTreeSet,
};

use serde::{
    Deserialize,
    Serialize,
};
use status_machina::{
    DiagramLabels,
    InspectionError,
    InspectionLimits,
};

use crate::build::Template;
use crate::error::Error;
use crate::ids::Ids;
use crate::spec;
use crate::value::Host;

#[derive(Serialize)]
pub struct LabelInventory {
    pub states: Vec<u32>,
    pub transitions: Vec<u32>,
    pub events: Vec<u32>,
    pub conditions: Vec<u32>,
    pub dispositions: Vec<u32>,
}
impl LabelInventory {
    pub(crate) fn from_spec(spec: &spec::Definition) -> Self {
        // bounded: every inventory is derived from the capped private
        // specification.
        let unique = |values: Vec<u32>| {
            values
                .into_iter()
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect()
        };
        Self {
            states: unique(spec.states.iter().map(|v| v.id).collect()),
            transitions: unique(spec.transitions.iter().map(|v| v.id).collect()),
            events: unique(
                spec.transitions
                    .iter()
                    .map(|v| v.event)
                    .chain(spec.dispositions.iter().map(|v| v.event))
                    .collect(),
            ),
            conditions: unique(
                spec.transitions
                    .iter()
                    .filter_map(|v| v.guard.as_ref().map(|v| v.id))
                    .chain(spec.dispositions.iter().map(|v| v.condition.id))
                    .collect(),
            ),
            dispositions: unique(spec.dispositions.iter().map(|v| v.id).collect()),
        }
    }
    fn check_entries<H: Host>(&self, limit: usize) -> Result<(), Error<H>> {
        if self.states.len() + self.transitions.len() + self.dispositions.len() > limit {
            return Err(Error::inspection(InspectionError::EntryLimit { limit }));
        }
        Ok(())
    }
}
#[derive(Serialize)]
pub struct Coverage {
    pub states: Vec<StateCoverage>,
    pub transitions: Vec<u32>,
    pub dispositions: Vec<u32>,
}
#[derive(Serialize)]
pub struct StateCoverage {
    pub id: u32,
    pub initial: bool,
    pub terminal: bool,
    pub incoming: usize,
    pub outgoing: usize,
    pub dispositions: usize,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Label {
    id: u32,
    text: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LabelSpec {
    states: Vec<Label>,
    transitions: Vec<Label>,
    events: Vec<Label>,
    conditions: Vec<Label>,
    dispositions: Vec<Label>,
}
struct Labels {
    states: BTreeMap<u32, String>,
    transitions: BTreeMap<u32, String>,
    events: BTreeMap<u32, String>,
    conditions: BTreeMap<u32, String>,
    dispositions: BTreeMap<u32, String>,
}
impl Labels {
    fn parse<H: Host>(
        source: &str,
        inventory: &LabelInventory,
        budget: usize,
    ) -> Result<Self, Error<H>> {
        // bounded: up to 1 MiB label UTF-8 plus JSON escaping and bounded ID
        // metadata.
        if source.len() > 16 * 1024 * 1024 {
            return Err(Error::invalid_inspection("label transport exceeds 16 MiB"));
        }
        let spec: LabelSpec = serde_json::from_str(source).map_err(Error::invalid_inspection)?;
        let mut remaining = budget;
        Ok(Self {
            states: Self::role(spec.states, &inventory.states, &mut remaining, budget)?,
            transitions: Self::role(
                spec.transitions,
                &inventory.transitions,
                &mut remaining,
                budget,
            )?,
            events: Self::role(spec.events, &inventory.events, &mut remaining, budget)?,
            conditions: Self::role(
                spec.conditions,
                &inventory.conditions,
                &mut remaining,
                budget,
            )?,
            dispositions: Self::role(
                spec.dispositions,
                &inventory.dispositions,
                &mut remaining,
                budget,
            )?,
        })
    }
    fn role<H: Host>(
        labels: Vec<Label>,
        required: &[u32],
        remaining: &mut usize,
        budget: usize,
    ) -> Result<BTreeMap<u32, String>, Error<H>> {
        if labels.len() != required.len() {
            return Err(Error::invalid_inspection(
                "label inventory differs from the validated graph",
            ));
        }
        // bounded: label count equals the capped validated graph inventory.
        let mut result = BTreeMap::new();
        for label in labels {
            *remaining = remaining
                .checked_sub(label.text.len())
                .ok_or_else(|| Error::inspection(InspectionError::OutputLimit { limit: budget }))?;
            if result.insert(label.id, label.text).is_some() {
                return Err(Error::invalid_inspection("duplicate label identifier"));
            }
        }
        if required.iter().any(|id| !result.contains_key(id)) {
            return Err(Error::invalid_inspection("missing graph label"));
        }
        Ok(result)
    }
}
impl<H: Host> DiagramLabels<Ids<H>> for Labels {
    // All lookups were checked against the immutable graph inventory before
    // rendering.
    fn state<'a>(&'a self, id: &'a u32) -> &'a str {
        &self.states[id]
    }
    fn transition<'a>(&'a self, id: &'a u32) -> &'a str {
        &self.transitions[id]
    }
    fn event<'a>(&'a self, id: &'a u32) -> &'a str {
        &self.events[id]
    }
    fn condition<'a>(&'a self, id: &'a u32) -> &'a str {
        &self.conditions[id]
    }
    fn disposition<'a>(&'a self, id: &'a u32) -> &'a str {
        &self.dispositions[id]
    }
}
fn limits<H: Host>(entries: usize, bytes: usize) -> Result<InspectionLimits, Error<H>> {
    if entries > 1_000_000 || bytes > 1_048_576 {
        return Err(Error::invalid_inspection(
            "native inspection limits exceed 1000000 entries or 1 MiB output",
        ));
    }
    InspectionLimits::builder()
        .max_entries(entries)
        .max_output_bytes(bytes)
        .build()
        .map_err(Error::inspection)
}
impl<H: Host> Template<H> {
    pub fn structural_coverage(&self, entries: usize, bytes: usize) -> Result<Coverage, Error<H>> {
        let coverage = self
            .definition
            .structural_coverage(&limits(entries, bytes)?)
            .map_err(Error::inspection)?;
        Ok(Coverage {
            // bounded: the kernel checked the combined output inventory before allocation.
            states: coverage
                .states()
                .iter()
                .map(|s| StateCoverage {
                    id: *s.id(),
                    initial: s.is_initial(),
                    terminal: s.is_terminal(),
                    incoming: s.incoming(),
                    outgoing: s.outgoing(),
                    dispositions: s.dispositions(),
                })
                .collect(),
            transitions: coverage.transitions().to_vec(),
            dispositions: coverage.dispositions().to_vec(),
        })
    }
    pub fn diagram(&self, source: &str, entries: usize, bytes: usize) -> Result<String, Error<H>> {
        let limits = limits(entries, bytes)?;
        self.labels.check_entries(entries)?;
        let labels = Labels::parse(source, &self.labels, bytes)?;
        self.definition
            .diagram_with_labels(&limits, &labels)
            .map_err(Error::inspection)
    }
}
