use std::{
    env, fs,
    path::{Path, PathBuf},
};

use clap::{Parser, Subcommand};
use valence_coprocessor_client::Client as Coprocessor;
use valence_coprocessor_ethereum::Ethereum;
use valence_coprocessor_ethereum_lightclient::State;

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    pub cmd: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Bootstraps a new state, recording it into the assets folder.
    Bootstrap,

    /// Deploys the domain on the provided coprocessor.
    Deploy {
        /// Socket to the co-processor service.
        #[arg(
            long,
            value_name = "COPROCESSOR",
            default_value = "https://service.coprocessor.valence.zone"
        )]
        coprocessor: String,

        /// Socket to the co-processor service.
        #[arg(
            long,
            value_name = "NAME",
            default_value = Ethereum::ID
        )]
        name: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let Cli { cmd } = Cli::parse();

    let result = match cmd {
        Commands::Bootstrap => {
            if env::var("ANKR_API_KEY").is_err() {
                anyhow::bail!("Ankr API key is required");
            }

            let state = State::bootstrap().await?;
            let input = state
                .fetch_input()
                .await
                .ok_or_else(|| anyhow::anyhow!("failed to fetch input for bootstrapped state"))?;

            // sanity check
            state.clone().apply(&input)?;

            let state = serde_json::to_string_pretty(&state)?;
            let input = serde_json::to_string_pretty(&input)?;

            let path = env::var("CARGO_MANIFEST_PATH")?;
            let path = PathBuf::from(path)
                .parent()
                .and_then(Path::parent)
                .map(|p| p.join("lib").join("assets"))
                .ok_or_else(|| anyhow::anyhow!("failed to compute path"))?;

            fs::write(path.join("state.json"), state)?;
            fs::write(path.join("input.json"), input)?;

            serde_json::json!({
                "path": path.display().to_string(),
            })
        }

        Commands::Deploy { coprocessor, name } => {
            let path = env::var("CARGO_MANIFEST_PATH")?;
            let path = PathBuf::from(path)
                .parent()
                .and_then(Path::parent)
                .map(|p| p.join("elf"))
                .ok_or_else(|| anyhow::anyhow!("failed to compute path"))?;

            let controller = fs::read(path.join("controller.wasm"))?;
            let circuit = fs::read(path.join("wrapper.bin"))?;

            let id = Coprocessor::default()
                .with_coprocessor(coprocessor)
                .deploy_domain(name, controller, circuit)
                .await?;

            serde_json::json!({
                "id": id,
            })
        }
    };

    println!("{}", serde_json::to_string_pretty(&result)?);

    Ok(())
}
