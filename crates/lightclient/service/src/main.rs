use std::time::Duration;

use clap::Parser;
use tracing_subscriber::{fmt, layer::SubscriberExt as _, util::SubscriberInitExt as _, EnvFilter};
use valence_coprocessor::DomainData;
use valence_coprocessor_client::Client as Coprocessor;
use valence_coprocessor_ethereum_lightclient::ServiceState;
use valence_coprocessor_prover::client::Client as Prover;

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

    let coprocessor = Coprocessor::default().with_coprocessor(coprocessor);
    let prover = Prover::new(prover);

    tracing::info!("Clients loaded...");

    loop {
        let service = match coprocessor.get_storage_file(&id, ServiceState::PATH).await {
            Ok(s) => {
                tracing::debug!("Loading service state...");

                ServiceState::try_from_slice(&s)
            }
            Err(e) => {
                tracing::warn!("Service state not available: {e}");
                tracing::info!("Initializing service state...");

                ServiceState::genesis(&prover)
            }
        };

        let mut service = match service {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("Service state corrupted: {e}");
                tracing::warn!("Forcing state initialization...");

                ServiceState::genesis(&prover)?
            }
        };

        tracing::debug!("Service state loaded...");

        let state = match service.to_state() {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("Inner state corrupted: {e}");
                tracing::warn!("Forcing state initialization...");

                service = ServiceState::genesis(&prover)?;
                service.to_state()?
            }
        };

        tracing::debug!("Loaded inner state...");

        let input = match state.fetch_input().await {
            Some(i) => i,
            None => {
                tracing::warn!("No state input available...");
                tokio::time::sleep(interval).await;
                continue;
            }
        };

        tracing::debug!("Loaded input...");

        let proof = match service.prove(&prover, input) {
            Ok(p) => p,
            Err(e) => {
                tracing::error!("Error computing service proof: {e}");
                tokio::time::sleep(interval).await;
                continue;
            }
        };

        tracing::debug!("Proof computed...");

        match proof.to_validated_block() {
            Ok(a) => tracing::info!(
                "Submitting block number {}, root {}",
                a.number,
                hex::encode(a.root)
            ),
            Err(e) => {
                tracing::error!("invalid wrapper proof: {e}");
                tokio::time::sleep(interval).await;
                continue;
            }
        };

        tracing::debug!("Proof parsed...");

        let service = service.encode();
        let proof = proof.encode();
        let args = serde_json::json!({
            "service": service,
            "proof": proof,
        });

        tracing::debug!(
            "Publishing block `{}`...",
            serde_json::to_string(&args).unwrap_or_default()
        );

        match coprocessor.add_domain_block(&domain, &args).await {
            Ok(b) => {
                tracing::info!(
                    "Block `{}`, root `{}` confirmed.",
                    b.number,
                    hex::encode(b.root)
                );
            }

            Err(e) => {
                tracing::error!("Error publishing block: {e}");
            }
        }

        tokio::time::sleep(interval).await;
    }
}
