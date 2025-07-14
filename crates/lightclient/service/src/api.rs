use std::sync::Arc;

use poem::web::Data;
use poem_openapi::{payload::Json, OpenApi};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sp1_sdk::SP1VerifyingKey;
use tokio::sync::Mutex;
use valence_coprocessor::Base64;

pub struct Api;

#[derive(Clone, Serialize, Deserialize)]
pub struct Config {
    pub prover: String,
    pub coprocessor: String,
    pub chain: String,
    pub interval: u64,
}

impl Config {
    pub fn new(prover: String, coprocessor: String, chain: String, interval: u64) -> Self {
        Self {
            prover,
            coprocessor,
            chain,
            interval,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ProvingData {
    pub vk: SP1VerifyingKey,
    pub elf: &'static [u8],
}

impl ProvingData {
    pub fn new(vk: SP1VerifyingKey, elf: &'static [u8]) -> Self {
        Self { vk, elf }
    }
}

#[derive(Debug, Clone, Default)]
pub struct State {
    pub inner: Arc<Mutex<StateInner>>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateInner {
    pub finalized_block: u64,
    pub published_block: u64,
}

#[OpenApi]
impl Api {
    /// Service stats.
    #[oai(path = "/stats", method = "get")]
    pub async fn stats(
        &self,
        config: Data<&Config>,
        state: Data<&State>,
    ) -> poem::Result<Json<Value>> {
        let state = state.inner.lock().await.clone();

        Ok(Json(json!({"config": *config, "state": state})))
    }

    /// Circuit verifying key
    #[oai(path = "/vk", method = "get")]
    pub async fn vk(&self, proving: Data<&ProvingData>) -> poem::Result<Json<Value>> {
        Ok(Json(json!({"vk": proving.vk})))
    }

    /// Circuit ELF (base64)
    #[oai(path = "/elf", method = "get")]
    pub async fn elf(&self, proving: Data<&ProvingData>) -> poem::Result<Json<Value>> {
        let elf = Base64::encode(proving.elf);

        Ok(Json(json!({"elf": elf})))
    }
}
