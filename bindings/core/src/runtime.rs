//! Owned native instances use the kernel's evaluation and acceptance path.
use std::sync::Arc;

use serde::Serialize;
use status_machina::{
    DispatchOutcome,
    Event,
    FallibleCandidateData,
    MachineDecision,
    MachineInstance,
};

use crate::build::Template;
use crate::error::Error;
use crate::ids::Ids;
use crate::value::{
    Data,
    Host,
    Value,
    ValueKey,
};

pub type Instance<H> =
    MachineInstance<'static, Data<H>, Value<H>, Value<H>, Value<H>, Value<H>, Ids<H>>;
pub type Decision<H> = MachineDecision<Data<H>, Value<H>, Value<H>, Ids<H>>;
pub struct Machine<H: Host> {
    pub instance: Option<Instance<H>>,
    pub host: Arc<H>,
}
impl<H: Host> Machine<H> {
    pub fn try_new(template: &Template<H>, host: Arc<H>, initial: u32) -> Result<Self, Error<H>> {
        let data = Data {
            value: Value::claim(Arc::clone(&host), initial).map_err(Error::evaluation)?,
        };
        let isolated = data.try_snapshot().map_err(Error::evaluation)?;
        let instance =
            Arc::clone(&template.definition).instantiate_shared(uuid::Uuid::now_v7(), isolated)?;
        Ok(Self {
            instance: Some(instance),
            host,
        })
    }
    fn get(&self) -> Result<&Instance<H>, Error<H>> {
        self.instance.as_ref().ok_or(Error::Disposed)
    }
    pub fn state(&self) -> Result<u32, Error<H>> {
        Ok(*self.get()?.state())
    }
    pub fn revision(&self) -> Result<u64, Error<H>> {
        Ok(self.get()?.revision())
    }
    pub fn snapshot(&self) -> Result<ValueKey, Error<H>> {
        Ok(self
            .get()?
            .data()
            .try_snapshot()
            .map_err(Error::evaluation)?
            .value
            .key())
    }
    pub fn dispatch(
        &mut self,
        kind: u32,
        payload: u32,
        facts: u32,
        revision: Option<u64>,
    ) -> Result<Outcome, Error<H>> {
        self.get()?;
        let event = Event::new(
            kind,
            Value::claim(Arc::clone(&self.host), payload).map_err(Error::evaluation)?,
        );
        let facts = Value::claim(Arc::clone(&self.host), facts).map_err(Error::evaluation)?;
        let instance = self.instance.as_mut().ok_or(Error::Disposed)?;
        let outcome =
            instance.dispatch_at(revision.unwrap_or(instance.revision()), event, &facts)?;
        Ok(outcome.into())
    }
    pub fn evaluate(
        &self,
        kind: u32,
        payload: u32,
        facts: u32,
        revision: Option<u64>,
    ) -> Result<Decision<H>, Error<H>> {
        let instance = self.get()?;
        let event = Event::new(
            kind,
            Value::claim(Arc::clone(&self.host), payload).map_err(Error::evaluation)?,
        );
        let facts = Value::claim(Arc::clone(&self.host), facts).map_err(Error::evaluation)?;
        Ok(instance.evaluate_at(revision.unwrap_or(instance.revision()), event, &facts)?)
    }
    pub fn accept(&mut self, decision: Decision<H>) -> Result<Outcome, Error<H>> {
        Ok(self
            .instance
            .as_mut()
            .ok_or(Error::Disposed)?
            .accept(decision)?
            .into())
    }
    pub fn dispose(&mut self) {
        self.instance.take();
    }
}
#[derive(Serialize)]
pub struct Record {
    pub transition: u32,
    pub from_state: u32,
    pub to_state: u32,
    pub previous_revision: String,
    pub revision: String,
}
#[derive(Serialize)]
pub struct Intent {
    pub id: u32,
    pub payload: ValueKey,
}
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Outcome {
    Refused {
        state: u32,
        event: u32,
    },
    Ignored {
        disposition: u32,
        state: u32,
        event: u32,
        reason: String,
    },
    Transitioned {
        record: Record,
        effects: Vec<Intent>,
        outputs: Vec<Intent>,
    },
}
impl<H: Host> From<DispatchOutcome<Value<H>, Value<H>, Ids<H>>> for Outcome {
    fn from(outcome: DispatchOutcome<Value<H>, Value<H>, Ids<H>>) -> Self {
        match outcome {
            DispatchOutcome::Refused { state, event } => Self::Refused { state, event },
            DispatchOutcome::Ignored {
                disposition,
                state,
                event,
                reason,
            } => Self::Ignored {
                disposition,
                state,
                event,
                reason,
            },
            DispatchOutcome::Transitioned {
                record,
                effects,
                outputs,
            } => Self::Transitioned {
                record: Record {
                    transition: *record.transition(),
                    from_state: *record.from(),
                    to_state: *record.to(),
                    previous_revision: record.previous_revision().to_string(),
                    revision: record.revision().to_string(),
                },
                // bounded: core construction caps effects at 48 and outputs at 16.
                effects: effects
                    .into_iter()
                    .map(|effect| {
                        let (id, payload) = effect.into_parts();
                        Intent {
                            id,
                            payload: payload.key(),
                        }
                    })
                    .collect(),
                outputs: outputs
                    .into_iter()
                    .map(|output| {
                        let (id, payload) = output.into_parts();
                        Intent {
                            id,
                            payload: payload.key(),
                        }
                    })
                    .collect(),
            },
        }
    }
}
