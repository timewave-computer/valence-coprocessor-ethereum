use std::{env, sync::LazyLock};

use helios_consensus_core::{
    apply_bootstrap,
    consensus_spec::{ConsensusSpec, MainnetConsensusSpec},
    types::{FinalityUpdate, LightClientStore, Update},
};
use serde::de::DeserializeOwned;
use serde_json::Value;
use tree_hash::TreeHash as _;

use crate::{Input, State};

static ANKR_URI: LazyLock<String> = LazyLock::new(|| {
    let key = env::var("ANKR_API_KEY").unwrap();

    format!("https://rpc.ankr.com/premium-http/eth_beacon/{key}/eth/v1/beacon")
});

impl State {
    async fn fetch_raw<U>(uri: U) -> anyhow::Result<Value>
    where
        U: AsRef<str>,
    {
        let uri = format!("{}{}", ANKR_URI.as_str(), uri.as_ref());

        Ok(reqwest::Client::new().get(uri).send().await?.json().await?)
    }

    async fn fetch<U, T>(uri: U) -> anyhow::Result<T>
    where
        U: AsRef<str>,
        T: DeserializeOwned,
    {
        let ret = Self::fetch_raw(uri)
            .await?
            .get("data")
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("no data available on response"))?;

        Ok(serde_json::from_value(ret)?)
    }

    /// Bootstraps a new state.
    pub async fn bootstrap() -> anyhow::Result<Self> {
        let mut store = LightClientStore::default();

        let finality_update: FinalityUpdate<MainnetConsensusSpec> =
            Self::fetch("/light_client/finality_update").await?;
        let root = finality_update.finalized_header().beacon().tree_hash_root();
        let root = hex::encode(root);

        let bootstrap = format!("/light_client/bootstrap/0x{root}");
        let bootstrap = Self::fetch(bootstrap).await?;

        apply_bootstrap(&mut store, &bootstrap);

        Ok(Self { store })
    }

    /// Fetch a state transition input.
    pub async fn fetch_input(&self) -> Option<Input> {
        async fn _fetch(state: &State) -> anyhow::Result<Input> {
            let finality_update: FinalityUpdate<MainnetConsensusSpec> =
                State::fetch("/light_client/finality_update").await?;

            let slot = finality_update.finalized_header().beacon().slot;
            let period = slot / MainnetConsensusSpec::slots_per_sync_committee_period();

            let current_slot = state.store.finalized_header.beacon().slot;
            let current_period =
                current_slot / MainnetConsensusSpec::slots_per_sync_committee_period();

            let count = period.saturating_sub(current_period).max(1);

            let updates =
                format!("/light_client/updates?start_period={current_period}&count={count}",);

            let updates: Vec<Update<MainnetConsensusSpec>> = State::fetch_raw(updates)
                .await?
                .as_array()
                .ok_or_else(|| anyhow::anyhow!("unexpected updates type"))?
                .iter()
                .filter_map(|u| {
                    let u = u.get("data").cloned()?;

                    serde_json::from_value(u).ok()
                })
                .collect();

            let expected_current_slot = updates
                .iter()
                .map(|u| u.signature_slot())
                .max()
                .copied()
                .unwrap_or_default()
                .max(*finality_update.signature_slot());

            Ok(Input {
                updates,
                finality_update,
                expected_current_slot,
            })
        }

        // the API is unstable so return `None` when fail
        _fetch(self).await.ok()
    }
}

#[tokio::test]
#[ignore = "depends on ankr api key"]
async fn state_bootstrap_works() {
    State::bootstrap().await.unwrap();
}

#[tokio::test]
#[ignore = "depends on ankr api key"]
async fn state_fetch_input_works() {
    let state = include_bytes!("../assets/state.json");
    let state: State = serde_json::from_slice(state).unwrap();

    state.fetch_input().await.unwrap();
}
