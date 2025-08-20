use std::time::Duration;

use clap::Parser;
use msgpacker::Packable as _;
use serde_json::Value;
use tracing_subscriber::{fmt, layer::SubscriberExt as _, util::SubscriberInitExt as _, EnvFilter};
use valence_coprocessor::DomainData;
use valence_coprocessor_ethereum_lightclient::{Config, History, ServiceState};
use valence_coprocessor_prover::client::Client as Prover;
use valence_domain_clients::{
    clients::coprocessor::CoprocessorClient as Coprocessor,
    coprocessor::base_client::CoprocessorBaseClient as _,
};

#[derive(Parser)]
struct Cli {
    /// Socket to the Prover service backend.
    #[arg(
        short,
        long,
        value_name = "PROVER",
        default_value = "ws://prover.timewave.computer:37282"
    )]
    prover: String,

    /// Socket to the co-processor service.
    #[arg(
        long,
        value_name = "COPROCESSOR",
        default_value = "https://service.coprocessor.valence.zone"
    )]
    coprocessor: String,

    /// Co-processor domain name.
    #[arg(long, value_name = "CHAIN", default_value = "ethereum-electra-alpha")]
    domain: String,

    /// Proof interval (ms).
    #[arg(short, long, value_name = "INTERVAL", default_value = "60000")]
    interval: u64,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let Cli {
        prover,
        coprocessor,
        domain,
        interval,
    } = Cli::parse();

    let filter_layer = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let fmt_layer = fmt::layer().with_target(false);

    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .init();

    tracing::info!("Loading state data...");

    let id = DomainData::identifier_from_parts(&domain);
    let id = hex::encode(id);
    let interval = Duration::from_millis(interval);

    tracing::info!("Controller set to `{id}`...");

    let coprocessor = Coprocessor::new(coprocessor);
    let prover = Prover::new(prover);

    tracing::info!("Clients loaded...");

    loop {
        let history = coprocessor
            .get_storage_raw(&id)
            .await
            .and_then(|h| h.ok_or_else(|| anyhow::anyhow!("no data available")))
            .and_then(|h| History::try_from_slice(&h));

        let mut history = match history {
            Ok(h) => h,
            _ => {
                tracing::warn!("Service state not available!");
                tracing::info!("Initializing service state...");

                let state = ServiceState::genesis(&prover)?;
                let mut h = History::default();

                h.append(state)?;

                h
            }
        };

        tracing::info!(
            "History loaded with `{}` entries; latest block on `{}`...",
            history.len(),
            history.latest_block().unwrap_or_default()
        );

        history.override_defaults();

        let service = history
            .latest()
            .ok_or_else(|| anyhow::anyhow!("no state available"))?;

        tracing::debug!("Service state loaded...");

        let state = match service.to_state() {
            Ok(s) => s,
            Err(e) => {
                history.discard_latest();

                tracing::error!("Inner state corrupted: {e}");
                tracing::error!(
                    "Discarding latest proof from series; len at {}...",
                    history.len()
                );

                tokio::time::sleep(interval).await;
                continue;
            }
        };

        tracing::debug!("Loaded inner state...");

        let latest = state
            .to_output()
            .map(|s| s.block_number)
            .unwrap_or_default();

        tracing::debug!("Loaded latest block `{latest}`...");

        let mut input = match state.fetch_input().await {
            Some(i) => i,
            None => {
                tracing::warn!("No state input available...");
                tokio::time::sleep(interval).await;
                continue;
            }
        };

        tracing::debug!("Loaded input...");

        let Config {
            genesis_root,
            forks,
            ..
        } = Config::default();

        let mut store = state.store.clone();
        let updates: Vec<_> = input.updates.drain(..).collect();

        for u in updates.into_iter() {
            match helios_consensus_core::verify_update(
                &u,
                input.expected_current_slot,
                &store,
                genesis_root,
                &forks,
            ) {
                Ok(_) => {
                    helios_consensus_core::apply_update(&mut store, &u);
                    input.updates.push(u);
                }
                Err(e) if e.to_string().contains("not relevant") => (),
                Err(e) => {
                    history.discard_latest();

                    tracing::error!("invalid update for state: {e}");
                    tracing::error!(
                        "Discarding latest proof from series; len at {}...",
                        history.len()
                    );

                    tokio::time::sleep(interval).await;
                    continue;
                }
            }
        }

        if let Err(e) = state.clone().apply(&input) {
            history.discard_latest();

            tracing::error!("invalid input for state: {e}");
            tracing::error!(
                "Discarding latest proof from series; len at {}...",
                history.len()
            );

            tokio::time::sleep(interval).await;
            continue;
        }

        tracing::debug!("Sanity check ok...");

        let proof = match service.prove(&prover, input) {
            Ok(p) => p,
            Err(e) => {
                history.discard_latest();

                tracing::error!("Error computing service proof: {e}");
                tracing::error!(
                    "Discarding latest proof from series; len at {}...",
                    history.len()
                );

                tokio::time::sleep(interval).await;
                continue;
            }
        };

        tracing::debug!("Proof computed...");

        let mut transition = service.clone();

        match transition.apply(proof.clone()).and_then(|block| {
            tracing::debug!("block proof for `{}` validated...", block.number);
            history.append(transition)
        }) {
            Ok(_) => tracing::debug!("transition appended to history..."),
            Err(e) => {
                history.discard_latest();

                tracing::error!("Service proof computed but invalid: {e}");
                tracing::error!(
                    "Discarding latest proof from series; len at {}...",
                    history.len()
                );

                tokio::time::sleep(interval).await;
                continue;
            }
        }

        let service = service.encode();
        let proof = proof.encode();
        let args = serde_json::json!({
            "service": service,
            "proof": proof,
        });

        tracing::debug!("Publishing block...",);

        match coprocessor.add_domain_block(&domain, &args).await {
            Ok(b) => {
                let number = b.get("number").and_then(Value::as_u64).unwrap_or_default();

                tracing::info!("Block `{}` confirmed.", number,);

                if let Some(l) = b.get("log").and_then(Value::as_array) {
                    for le in l {
                        if let Some(le) = le.as_str() {
                            tracing::debug!("log: {le}");
                        }
                    }
                }
            }

            Err(e) => {
                tracing::error!("Error publishing block: {e}");
            }
        }

        let file = history.pack_to_vec();

        tracing::debug!(
            "Publishing `{}` kbytes to co-processor...",
            file.len() / 1024
        );

        if let Err(e) = coprocessor.set_storage_raw(&id, &file).await {
            tracing::error!("error updating co-processor state: {e}");
        }

        tracing::debug!("State published");

        tokio::time::sleep(interval).await;
    }
}
