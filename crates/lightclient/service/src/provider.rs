use std::env;

use alloy_primitives::{Bytes, U256};
use reqwest::Client;
use serde_json::Value;
use tokio::time;
use valence_coprocessor::{Base64, Blake3Hasher, Hasher as _};
use valence_coprocessor_ethereum::Ethereum;
use valence_coprocessor_prover::client::Client as Prover;

use crate::api::{Config, ProvingData, State};

pub struct Provider {
    pub config: Config,
    pub proving: ProvingData,
    pub state: State,
}

impl Provider {
    pub fn new(config: Config, proving: ProvingData, state: State) -> Self {
        Self {
            config,
            proving,
            state,
        }
    }

    pub async fn service(self, url: String, _prover: Prover) {
        let interval = time::Duration::from_millis(self.config.interval);
        let _circuit = Blake3Hasher::hash(self.proving.elf);
        let _elf = self.proving.elf.to_vec();
        let coprocessor = format!(
            "http://{}/api/registry/domain/{}",
            self.config.coprocessor,
            Ethereum::ID
        );

        loop {
            tracing::debug!("fetching block data...");

            let ret: Result<Value, _> = match Client::new()
                .post(&url)
                .json(&serde_json::json!({
                    "jsonrpc": "2.0",
                    "method": "eth_getBlockByNumber",
                    "params": [
                        "finalized",
                        false
                    ],
                    "id": 1
                }))
                .send()
                .await
            {
                Ok(r) => r.json().await,
                Err(e) => {
                    tracing::error!("error fetching block data from alchemy: {e}");
                    time::sleep(interval).await;
                    continue;
                }
            };

            let ret = match ret {
                Ok(r) => r.get("result").cloned(),
                Err(e) => {
                    tracing::error!("error reading block data from alchemy: {e}");
                    time::sleep(interval).await;
                    continue;
                }
            };

            let ret = match ret {
                Some(r) => {
                    let block = r
                        .get("number")
                        .cloned()
                        .and_then(|b| serde_json::from_value::<U256>(b).ok())
                        .map(|b| b.to::<u64>());

                    let root = r
                        .get("stateRoot")
                        .cloned()
                        .and_then(|r| serde_json::from_value::<Bytes>(r).ok())
                        .map(|r| r.to_vec());

                    block.zip(root)
                }
                None => {
                    tracing::error!("error reading result block data from alchemy");
                    time::sleep(interval).await;
                    continue;
                }
            };

            let (witness, block) = match ret {
                Some((b, r)) => {
                    tracing::debug!("received block {b}...");

                    self.state.inner.lock().await.finalized_block = b;

                    ([r.as_slice(), &b.to_le_bytes()].concat(), b)
                }
                None => {
                    tracing::error!("error parsing result block data from alchemy");
                    time::sleep(interval).await;
                    continue;
                }
            };

            /*
            let proof = match prover.get_sp1_proof(circuit, &witness, |_| Ok(elf.clone())) {
                Ok(p) => serde_json::json!({
                    "proof": p.proof,
                    "inputs": p.inputs,
                }),
                Err(e) => {
                    tracing::error!("error fetching proof from prover: {e}");
                    time::sleep(interval).await;
                    continue;
                }
            };
            */

            let proof = serde_json::json!({
                "proof": Base64::encode(&[]),
                "inputs": Base64::encode(&witness),
            });

            match Client::new().post(&coprocessor).json(&proof).send().await {
                Ok(_) => {
                    self.state.inner.lock().await.published_block = block;
                }
                Err(e) => {
                    tracing::error!("error publishing block data to coprocessor: {e}");
                    time::sleep(interval).await;
                    continue;
                }
            };

            tracing::debug!("block {block} submitted to the coprocessor...");

            time::sleep(interval).await;
        }
    }

    pub fn run(self) -> anyhow::Result<()> {
        let key = env::var("ALCHEMY_API_KEY")?;
        let url = format!("https://{}.g.alchemy.com/v2/{key}", self.config.chain);
        let prover = Prover::new(&self.config.prover)?;

        tokio::spawn(self.service(url, prover));

        Ok(())
    }
}
