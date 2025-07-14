#![no_main]
sp1_zkvm::entrypoint!(main);

pub fn main() {
    let w = sp1_zkvm::io::read_vec();

    sp1_zkvm::io::commit_slice(&w);
}
