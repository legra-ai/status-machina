//! Bounded owner-held wake intents composed into candidate machine data.

use std::time::SystemTime;

mod types;
pub use types::{
    RetryWaitError,
    WaitCorrelation,
    WakeIntent,
};

#[derive(Clone, Debug)]
struct Entry<P, I: crate::MachineIds = crate::DefaultIds> {
    intent: WakeIntent<P, I>,
    registered: bool,
}

/// Bounded active waits and pending registration intents for one instance.
///
/// Embed this value in caller-owned machine data. A local action schedules into
/// the candidate snapshot; dispatch then accepts the wait, pending intent and
/// machine state together, without an await. Propagate scheduling failures from
/// the action so insufficient capacity refuses the whole transition. Failed
/// external registration leaves the accepted intent available through
/// `pending`.
///
/// Keep this owner value for the lifetime of its instance. Cancel/consume
/// retain the generation counter; replacing the owner requires a fresh instance
/// ID. The number of entries is bounded, while payload size and clone cost
/// remain the caller's responsibility. This value performs no I/O or
/// scheduling.
#[derive(Clone, Debug)]
pub struct RetryWaits<P, I: crate::MachineIds = crate::DefaultIds> {
    instance: I::Instance,
    generation: u64,
    capacity: usize,
    // bounded: builder requires capacity in 1..=4096; schedule enforces it.
    entries: Vec<Entry<P, I>>,
}

impl<P, I: crate::MachineIds> RetryWaits<P, I> {
    /// Start configuring the active-wait bound for this instance incarnation.
    pub fn builder(instance: I::Instance) -> RetryWaitsBuilder<P, I> {
        RetryWaitsBuilder {
            instance,
            capacity: None,
            payload: std::marker::PhantomData,
        }
    }

    /// Accept a new wait or replace the same operation's existing wait.
    ///
    /// # Errors
    /// Returns capacity or generation exhaustion before changing any entry.
    pub fn schedule(
        &mut self,
        operation: I::Operation,
        deadline: SystemTime,
        payload: P,
    ) -> Result<WaitCorrelation<I>, RetryWaitError> {
        let previous = self
            .entries
            .iter()
            .position(|entry| entry.intent.correlation.operation == operation);
        if previous.is_none() && self.entries.len() == self.capacity {
            return Err(RetryWaitError::CapacityReached);
        }
        let generation = self
            .generation
            .checked_add(1)
            .ok_or(RetryWaitError::GenerationExhausted)?;
        let correlation = WaitCorrelation {
            instance: self.instance.clone(),
            operation,
            generation,
        };
        let entry = Entry {
            intent: WakeIntent {
                correlation: correlation.clone(),
                deadline,
                payload,
            },
            registered: false,
        };
        if let Some(index) = previous {
            self.entries[index] = entry;
        } else {
            self.entries.push(entry);
        }
        self.generation = generation;
        Ok(correlation)
    }

    /// Inspect accepted intents whose scheduler registration is not
    /// acknowledged.
    pub fn pending(&self) -> impl Iterator<Item = &WakeIntent<P, I>> {
        self.entries
            .iter()
            .filter(|entry| !entry.registered)
            .map(|entry| &entry.intent)
    }

    /// Inspect the currently active wait for an operation, registered or
    /// pending.
    pub fn get(&self, operation: &I::Operation) -> Option<&WakeIntent<P, I>> {
        self.entries
            .iter()
            .find(|entry| &entry.intent.correlation.operation == operation)
            .map(|entry| &entry.intent)
    }

    /// Record registration only for the exact current wait; late
    /// acknowledgements cannot mark a replacement registered or consume it.
    pub fn mark_registered(&mut self, correlation: &WaitCorrelation<I>) -> bool {
        let Some(entry) = self
            .entries
            .iter_mut()
            .find(|entry| &entry.intent.correlation == correlation)
        else {
            return false;
        };
        entry.registered = true;
        true
    }

    /// Whether a current wait matches this reminder and is due at supplied
    /// time.
    pub fn is_due(&self, correlation: &WaitCorrelation<I>, now: SystemTime) -> bool {
        self.entries
            .iter()
            .any(|entry| &entry.intent.correlation == correlation && entry.intent.deadline <= now)
    }

    /// Consume a matching due wake once, within the same candidate action that
    /// requests the next effect. Unknown, early and stale wakes return `None`.
    pub fn consume(
        &mut self,
        correlation: &WaitCorrelation<I>,
        now: SystemTime,
    ) -> Option<WakeIntent<P, I>> {
        let index = self.entries.iter().position(|entry| {
            &entry.intent.correlation == correlation && entry.intent.deadline <= now
        })?;
        Some(self.entries.swap_remove(index).intent)
    }

    /// End an operation's wait on cancellation or business completion. Removing
    /// the corresponding scheduler registration remains the caller's work.
    pub fn cancel(&mut self, operation: &I::Operation) -> Option<WakeIntent<P, I>> {
        let index = self
            .entries
            .iter()
            .position(|entry| &entry.intent.correlation.operation == operation)?;
        Some(self.entries.swap_remove(index).intent)
    }
}

/// Builder requiring an explicit bound on active waits and pending intents.
pub struct RetryWaitsBuilder<P, I: crate::MachineIds = crate::DefaultIds> {
    instance: I::Instance,
    capacity: Option<usize>,
    payload: std::marker::PhantomData<P>,
}

impl<P, I: crate::MachineIds> RetryWaitsBuilder<P, I> {
    /// Set the maximum number of simultaneous active waits (1..=4096).
    pub fn capacity(mut self, capacity: usize) -> Self {
        self.capacity = Some(capacity);
        self
    }

    /// Build an empty owner value; no instance, timer or registration is
    /// created.
    ///
    /// # Errors
    /// Returns an error for absent, zero or excessive capacity.
    pub fn build(self) -> Result<RetryWaits<P, I>, RetryWaitError> {
        let capacity = self
            .capacity
            .filter(|capacity| (1..=4096).contains(capacity))
            .ok_or(RetryWaitError::InvalidCapacity)?;
        Ok(RetryWaits {
            instance: self.instance.clone(),
            generation: 0,
            capacity,
            entries: Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        MachineInstanceId,
        OperationId,
    };

    #[test]
    fn generation_exhaustion_cannot_replace_an_existing_wait() {
        let id = MachineInstanceId::try_new(uuid::Uuid::from_u128(
            0x018f_0000_0000_7000_8000_0000_0000_0001,
        ))
        .unwrap();
        let operation = OperationId::try_new("fetch").unwrap();
        let mut waits = RetryWaits::<()>::builder(id).capacity(1).build().unwrap();
        let first = waits
            .schedule(operation.clone(), SystemTime::UNIX_EPOCH, ())
            .unwrap();
        waits.generation = u64::MAX;
        assert_eq!(
            waits.schedule(operation.clone(), SystemTime::UNIX_EPOCH, ()),
            Err(RetryWaitError::GenerationExhausted)
        );
        assert_eq!(waits.get(&operation).unwrap().correlation(), &first);
        assert_eq!(waits.generation, u64::MAX);
    }
}
