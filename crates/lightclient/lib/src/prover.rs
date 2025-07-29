use sp1_sdk::HashableKey as _;
use valence_coprocessor_prover::{client::Client, types::ProofRequestBuilder};

use crate::{CircuitInner, CircuitWitness, Input, ProvenState, ServiceState};

impl ServiceState {
    pub fn genesis(prover: &Client) -> anyhow::Result<Self> {
        let inner = Self::inner();
        let wrapper = Self::wrapper();
        let genesis = CircuitWitness::default().to_vec();
        let latest_inner_proof = ProofRequestBuilder::new(inner)
            .with_witnesses(genesis)
            .with_type_compressed()
            .prove(prover, |_| Ok(CircuitInner::elf().to_vec()))?;

        let inner_vk = prover.get_sp1_verifying_key(inner, |_| Ok(CircuitInner::elf().to_vec()))?;
        let wrapper_vk =
            prover.get_sp1_verifying_key(wrapper, |_| Ok(CircuitInner::wrapper_elf().to_vec()))?;
        let wrapper_vk_bytes32 = wrapper_vk.bytes32();

        let inner_vk = serde_cbor::to_vec(&inner_vk)?;
        let wrapper_vk = serde_cbor::to_vec(&wrapper_vk)?;

        Ok(Self {
            latest_inner_proof,
            inner_vk,
            wrapper_vk,
            wrapper_vk_bytes32,
        })
    }

    pub fn prove(&self, prover: &Client, input: Input) -> anyhow::Result<ProvenState> {
        let inner = Self::inner();
        let wrapper = Self::wrapper();
        let proof = self.latest_inner_proof.clone();

        let args = proof.decode()?.1;
        let args = CircuitWitness::update(args, input).to_vec();

        let inner_vk = prover.get_sp1_verifying_key(inner, |_| Ok(CircuitInner::elf().to_vec()))?;

        let inner_proof = ProofRequestBuilder::new(inner)
            .with_witnesses(args)
            .with_type_compressed()
            .with_recursive_proof(proof, inner_vk.clone())?
            .prove(prover, |_| Ok(CircuitInner::elf().to_vec()))?;

        let _wrapper_vk =
            prover.get_sp1_verifying_key(wrapper, |_| Ok(CircuitInner::wrapper_elf().to_vec()))?;

        let args = inner_proof.decode()?.1;
        let wrapper = ProofRequestBuilder::new(wrapper)
            .with_witnesses(args)
            .with_recursive_proof(inner_proof.clone(), inner_vk)?
            .prove(prover, |_| Ok(CircuitInner::wrapper_elf().to_vec()))?;

        Ok(ProvenState {
            inner: inner_proof,
            wrapper,
        })
    }
}

#[test]
#[ignore = "depends on prover key"]
fn wrapper_proof_is_correct() {
    use msgpacker::Unpackable as _;
    use valence_coprocessor::ValidatedBlock;
    use valence_coprocessor_prover::client::Client;

    let prover = Client::new("ws://prover.timewave.computer:37282");

    let state = ServiceState::genesis(&prover).unwrap();

    let input = include_bytes!("../assets/input.json");
    let input = serde_json::from_slice(input).unwrap();

    let proof = state.prove(&prover, input).unwrap();
    let block = proof.wrapper.decode().unwrap().1;

    ValidatedBlock::unpack(&block).unwrap();
}
