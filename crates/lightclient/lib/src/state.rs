use alloc::vec::Vec;
use helios_consensus_core::{
    apply_finality_update, apply_update, consensus_spec::MainnetConsensusSpec,
    errors::ConsensusError, types::LightClientStore, verify_finality_update, verify_update,
};
use serde::{Deserialize, Serialize};

use crate::{Config, Input, Output};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct State {
    pub store: LightClientStore<MainnetConsensusSpec>,
}

impl Default for State {
    fn default() -> Self {
        let state = include_bytes!("../../lib/assets/state.json");

        serde_json::from_slice(state).unwrap()
    }
}

impl State {
    pub fn to_vec(&self) -> Vec<u8> {
        serde_cbor::to_vec(self).unwrap()
    }

    /// Filter irrelevant errors that are improperly setup as critical on helios
    pub fn filter_error(e: &eyre::Report) -> anyhow::Result<()> {
        let mut skip = false;

        for cause in e.chain() {
            if let Some(ce) = cause.downcast_ref::<ConsensusError>() {
                skip = matches!(
                    ce,
                    ConsensusError::InvalidTimestamp
                        | ConsensusError::InvalidPeriod
                        | ConsensusError::NotRelevant
                        | ConsensusError::CheckpointTooOld
                );

                if skip {
                    break;
                }
            }
        }

        if !skip {
            anyhow::bail!("{e}");
        }

        Ok(())
    }

    pub fn try_from_slice<B>(bytes: B) -> anyhow::Result<Self>
    where
        B: AsRef<[u8]>,
    {
        Ok(serde_cbor::from_slice(bytes.as_ref())?)
    }

    pub fn to_output(&self) -> anyhow::Result<Output> {
        let execution = self
            .store
            .finalized_header
            .execution()
            .map_err(|_| anyhow::anyhow!("failed to extract execution header from store"))?;

        Ok(Output {
            block_number: *execution.block_number(),
            state_root: *execution.state_root(),
        })
    }

    pub fn apply(&mut self, input: &Input) -> anyhow::Result<Output> {
        // code derived from
        // https://github.com/succinctlabs/sp1-helios/blob/51b1e4aaee2e3e614dd589b1fa83594aa7b528b6/program/src/light_client.rs

        let Config {
            genesis_root,
            forks,
            ..
        } = Config::default();

        let Input {
            updates,
            finality_update,
            expected_current_slot,
        } = input;

        let prev_head = self.store.finalized_header.beacon().slot;

        for u in updates.iter() {
            if let Err(e) =
                verify_update(u, *expected_current_slot, &self.store, genesis_root, &forks)
            {
                Self::filter_error(&e)?;
            }

            apply_update(&mut self.store, u);
        }

        if let Err(e) = verify_finality_update(
            finality_update,
            *expected_current_slot,
            &self.store,
            genesis_root,
            &forks,
        ) {
            Self::filter_error(&e)?;
        }

        apply_finality_update(&mut self.store, finality_update);

        anyhow::ensure!(
            self.store.finalized_header.beacon().slot >= prev_head,
            "New head is not greater than previous head."
        );
        anyhow::ensure!(
            self.store.finalized_header.beacon().slot.is_multiple_of(32),
            "New head is not a checkpoint slot."
        );

        self.to_output()
    }
}

#[test]
fn state_apply_works() {
    let state = include_bytes!("../assets/state.json");
    let input = include_bytes!("../assets/input.json");

    let mut state: State = serde_json::from_slice(state).unwrap();
    let input = serde_json::from_slice(input).unwrap();

    let output = state.apply(&input).unwrap();
    let execution = input
        .finality_update
        .finalized_header()
        .execution()
        .unwrap();

    assert_eq!(&output.block_number, execution.block_number());
    assert_eq!(&output.state_root, execution.state_root());
}
