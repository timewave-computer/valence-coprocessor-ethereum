#![no_std]

use msgpacker::Packable as _;
use serde_json::Value;
use valence_coprocessor::{Base64, DomainController, Hash, Proof, StateProof, ValidatedBlock};
use valence_coprocessor_ethereum::Ethereum;

pub fn validate_block(args: Value) -> anyhow::Result<ValidatedBlock> {
    let proof = args
        .get("proof")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("failed to read proof from argument"))?
        .into();

    let inputs = args
        .get("inputs")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("failed to read inputs from argument"))?
        .into();

    // TODO verify proof

    let values = Base64::decode(&inputs)?;

    let number = <[u8; 8]>::try_from(&values[32..40])
        .map(u64::from_le_bytes)
        .map_err(|_| anyhow::anyhow!("failed to read block number from inputs"))?;

    let root = Hash::try_from(&values[..32])
        .map_err(|_| anyhow::anyhow!("failed to read state root from inputs"))?;

    let payload = Proof { proof, inputs }.pack_to_vec();

    Ok(ValidatedBlock {
        number,
        root,
        payload,
    })
}

pub fn get_state_proof(args: Value) -> anyhow::Result<StateProof> {
    Ethereum.state_proof(args)
}
