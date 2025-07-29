use msgpacker::Unpackable as _;
use sp1_verifier::{Groth16Verifier, GROTH16_VK_BYTES};
use valence_coprocessor::ValidatedBlock;

use crate::{ProvenState, ServiceState};

impl ServiceState {
    pub fn apply(&mut self, proof: ProvenState) -> anyhow::Result<ValidatedBlock> {
        let (wrapper, inputs) = proof.wrapper.decode()?;

        Groth16Verifier::verify(
            &wrapper,
            &inputs,
            &self.wrapper_vk_bytes32,
            &GROTH16_VK_BYTES,
        )?;

        // TODO verify the inner proof on the controller
        self.latest_inner_proof = proof.inner;

        Ok(ValidatedBlock::unpack(&inputs)?.1)
    }
}

#[test]
fn proven_state_apply_works() {
    use msgpacker::Unpackable as _;

    let service = include_bytes!("../assets/service-state.bin");
    let mut service = ServiceState::try_from_slice(service).unwrap();

    let proof = include_bytes!("../assets/proof.bin");
    let proof = ProvenState::unpack(proof).unwrap().1;

    service.apply(proof).unwrap();
}
