use std::net::SocketAddr;

use clap::Parser;
use poem::{listener::TcpListener, EndpointExt as _, Route};
use poem_openapi::OpenApiService;
use sp1_sdk::SP1VerifyingKey;
use tracing_subscriber::{fmt, layer::SubscriberExt as _, util::SubscriberInitExt as _, EnvFilter};

use crate::{
    api::{Api, Config, ProvingData, State},
    provider::Provider,
};

mod api;
mod provider;

const ELF: &[u8] = include_bytes!("../../elf/valence-coprocessor-ethereum-service-circuit");
const VK: &[u8] = include_bytes!("../../elf/valence-coprocessor-ethereum-service-vk");

#[derive(Parser)]
struct Cli {
    /// Bind to the provided socket
    #[arg(short, long, value_name = "SOCKET", default_value = "0.0.0.0:37283")]
    bind: SocketAddr,

    /// Socket to the Prover service backend.
    #[arg(
        short,
        long,
        value_name = "PROVER",
        default_value = "wss://prover.coprocessor.valence.zone"
    )]
    prover: String,

    /// Socket to the co-processor service.
    #[arg(
        long,
        value_name = "COPROCESSOR",
        default_value = "https://service.coprocessor.valence.zone"
    )]
    coprocessor: String,

    /// Block provider chain.
    #[arg(long, value_name = "CHAIN", default_value = "eth-mainnet")]
    chain: String,

    /// Proof interval.
    #[arg(short, long, value_name = "INTERVAL", default_value = "300000")]
    interval: u64,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let Cli {
        bind,
        prover,
        coprocessor,
        chain,
        interval,
    } = Cli::parse();

    let filter_layer = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let fmt_layer = fmt::layer().with_target(false);

    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .init();

    tracing::info!("Loading state data...");

    let vk: SP1VerifyingKey = postcard::from_bytes(VK)?;
    let proving = ProvingData::new(vk, ELF);
    let config = Config::new(prover, coprocessor, chain, interval);
    let state = State::default();

    tracing::info!("State loaded...");

    Provider::new(config.clone(), proving.clone(), state.clone()).run()?;

    tracing::info!("Block provider ready...");

    let api_service = OpenApiService::new(Api, env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"))
        .server(format!("{}/api", &bind));
    let ui = api_service.swagger_ui();
    let app = Route::new()
        .nest("/", ui)
        .nest("/api", api_service)
        .data(proving)
        .data(config)
        .data(state);

    tracing::info!("API loaded, listening on `{}`...", &bind);

    poem::Server::new(TcpListener::bind(&bind)).run(app).await?;

    Ok(())
}
