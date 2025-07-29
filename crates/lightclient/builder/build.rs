use std::{env, fs, path::PathBuf, process::Command};

use sp1_sdk::{HashableKey as _, Prover as _, ProverClient};
use zerocopy::IntoBytes as _;

fn main() {
    if !Command::new("cargo")
        .args(["prove", "--version"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
    {
        return ();
    }

    sp1_build::build_program("../circuit");

    let manifest = env::var("CARGO_MANIFEST_DIR").unwrap();
    let manifest = PathBuf::from(manifest).parent().unwrap().to_path_buf();
    let out = manifest.join("elf");

    let release = manifest
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("target")
        .join("elf-compilation")
        .join("riscv32im-succinct-zkvm-elf")
        .join("release");

    let inner = release.join("valence-coprocessor-ethereum-service-circuit");

    let inner_elf = fs::read(&inner).unwrap();

    let prover = ProverClient::builder().cpu().build();

    let (_, inner_vk) = prover.setup(&inner_elf);
    let inner_vk = inner_vk.vk.hash_u32();

    fs::write(out.join("inner.bin"), inner_elf).unwrap();
    fs::write(out.join("inner-vkh32.bin"), inner_vk.as_bytes()).unwrap();

    sp1_build::build_program("../wrapper");

    let wrapper = release.join("valence-coprocessor-ethereum-service-circuit-wrapper");

    let wrapper_elf = fs::read(&wrapper).unwrap();

    fs::write(out.join("wrapper.bin"), wrapper_elf).unwrap();
}
