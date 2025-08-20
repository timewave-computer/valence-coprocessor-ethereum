use alloc::collections::BTreeMap;
use msgpacker::{MsgPacker, Unpackable as _};
use serde::{Deserialize, Serialize};

use crate::ServiceState;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, MsgPacker)]
pub struct History {
    capacity: usize,
    minimum: usize,
    states: BTreeMap<u64, ServiceState>,
}

impl Default for History {
    fn default() -> Self {
        Self {
            capacity: 10,
            minimum: 2,
            states: BTreeMap::new(),
        }
    }
}

impl History {
    pub fn append(&mut self, state: ServiceState) -> anyhow::Result<()> {
        let number = state.to_state()?.to_output()?.block_number;

        if let Some((first, _)) = self.states.iter().next() {
            if *first > number {
                return Ok(());
            }
        }

        while self.len() >= self.capacity {
            self.states.pop_first();
        }

        self.states.insert(number, state);

        Ok(())
    }

    pub fn override_defaults(&mut self) {
        let other = Self::default();

        self.capacity = other.capacity;
        self.minimum = other.minimum;
    }

    pub fn first(&self) -> Option<ServiceState> {
        self.states.iter().next().map(|(_, v)| v.clone())
    }

    pub fn latest(&self) -> Option<ServiceState> {
        self.states.iter().next_back().map(|(_, v)| v.clone())
    }

    pub fn latest_block(&self) -> Option<u64> {
        self.states.iter().next_back().map(|(k, _)| *k)
    }

    pub fn len(&self) -> usize {
        self.states.len()
    }

    pub fn is_empty(&self) -> bool {
        self.states.is_empty()
    }

    pub fn discard_latest(&mut self) -> Option<ServiceState> {
        if self.len() <= self.minimum {
            return None;
        }

        self.states.pop_last().map(|(_, s)| s)
    }

    pub fn try_from_slice(buffer: &[u8]) -> anyhow::Result<Self> {
        Self::unpack(buffer)
            .map(|(_, h)| h)
            .map_err(|e| anyhow::anyhow!("failed to deserialize state: {e}"))
    }
}
