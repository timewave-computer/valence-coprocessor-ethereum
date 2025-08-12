#![no_std]

use msgpacker::Packable as _;
use serde_json::Value;
use valence_coprocessor::{DomainController, StateProof, ValidatedBlock};
use valence_coprocessor_ethereum::Ethereum;
use valence_coprocessor_ethereum_lightclient::{ProvenState, ServiceState};
use valence_coprocessor_wasm::abi;

pub fn validate_block(args: Value) -> anyhow::Result<ValidatedBlock> {
    let mut service = args
        .get("service")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("No service state provided"))
        .and_then(ServiceState::decode)?;

    let block_number = service.to_state()?.to_output()?.block_number;

    let proof = args
        .get("proof")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("No proof provided"))
        .and_then(ProvenState::decode)?;

    let payload = proof.wrapper.pack_to_vec();
    let mut block = service.apply(proof)?;

    block.payload = payload;

    let replace = abi::get_storage_file(ServiceState::PATH)
        .map(Option::unwrap_or_default)
        .and_then(ServiceState::try_from_slice)
        .and_then(|s| s.to_state())
        .and_then(|s| s.to_output())
        .map(|o| o.block_number)
        .ok()
        .filter(|n| *n >= block_number)
        .is_none();

    if replace {
        if let Err(e) = abi::set_storage_file(ServiceState::PATH, &service.to_vec()) {
            abi::log!("failed to override storage: {e}").ok();
        }
    }

    Ok(block)
}

pub fn get_state_proof(args: Value) -> anyhow::Result<StateProof> {
    Ethereum.state_proof(args)
}
