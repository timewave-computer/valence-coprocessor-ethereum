#![no_std]

use serde_json::Value;
use valence_coprocessor::{DomainController, StateProof, ValidatedBlock};
use valence_coprocessor_ethereum::{Ethereum, ValidateBlockRequest};

pub fn validate_block(args: Value) -> anyhow::Result<ValidatedBlock> {
    let req: ValidateBlockRequest = serde_json::from_value(args)?;

    Ok(req.into())
}

pub fn get_state_proof(args: Value) -> anyhow::Result<StateProof> {
    Ethereum.state_proof(args)
}
