use msgpacker::Packable as _;
use serde_json::Value;
use valence_coprocessor::{DomainController, StateProof, ValidatedBlock};
use valence_coprocessor_ethereum::Ethereum;
use valence_coprocessor_ethereum_lightclient::{ProvenState, ServiceState};
use valence_coprocessor_wasm::abi;

pub fn validate_block_impl(args: Value) -> anyhow::Result<ValidatedBlock> {
    let mut service = args
        .get("service")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("No service state provided"))
        .and_then(ServiceState::decode)?;

    let proof = args
        .get("proof")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("No proof provided"))
        .and_then(ProvenState::decode)?;

    let payload = proof.wrapper.pack_to_vec();
    let mut block = service.apply(proof)?;

    block.payload = payload;

    Ok(block)
}

pub fn get_state_proof_impl(args: Value) -> anyhow::Result<StateProof> {
    Ethereum.state_proof(args)
}

#[no_mangle]
pub extern "C" fn validate_block() {
    let args = abi::args().unwrap();
    let validated = validate_block_impl(args).unwrap();
    let validated = serde_json::to_value(validated).unwrap();

    abi::ret(&validated).unwrap();
}

#[no_mangle]
pub extern "C" fn get_state_proof() {
    let args = abi::args().unwrap();
    let proof = get_state_proof_impl(args).unwrap();
    let proof = serde_json::to_value(proof).unwrap();

    abi::ret(&proof).unwrap();
}
