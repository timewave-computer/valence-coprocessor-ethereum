#![no_main]

use sp1_zkvm::lib::verify::verify_sp1_proof;
use valence_coprocessor_ethereum_lightclient::{CircuitInner, CircuitOpenWitness, CircuitWitness};

sp1_zkvm::entrypoint!(main);

pub fn main() {
    let inputs = sp1_zkvm::io::read_vec();
    let inputs = CircuitWitness::try_from_slice(&inputs).unwrap();

    let CircuitOpenWitness {
        vk,
        mut state,
        args,
    } = inputs.open().unwrap();

    if let Some((digest, input)) = args {
        verify_sp1_proof(&vk, &digest);

        state.apply(&input).unwrap();
    }

    let output = CircuitInner { vk, state }.to_vec();

    sp1_zkvm::io::commit_slice(&output);
}
