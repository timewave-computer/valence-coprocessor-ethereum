#![no_main]

use msgpacker::Packable as _;
use sp1_zkvm::lib::verify::verify_sp1_proof;
use valence_coprocessor::ValidatedBlock;
use valence_coprocessor_ethereum_lightclient::CircuitInner;

sp1_zkvm::entrypoint!(main);

pub fn main() {
    let inputs = sp1_zkvm::io::read_vec();

    let digest = CircuitInner::digest(&inputs);
    let inputs = CircuitInner::try_from_slice(&inputs).unwrap();

    let vk = inputs.vk;

    assert_eq!(vk, CircuitInner::vk_hash());

    verify_sp1_proof(&vk, &digest);

    let output = inputs.state.to_output().unwrap();
    let number = output.block_number;
    let root = *output.state_root;
    let payload = Default::default();

    let output = ValidatedBlock {
        number,
        root,
        payload,
    }
    .pack_to_vec();

    sp1_zkvm::io::commit_slice(&output);
}
