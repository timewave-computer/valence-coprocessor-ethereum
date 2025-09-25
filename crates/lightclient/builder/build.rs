use std::{env, fs, path::PathBuf, process::Command};

use sp1_sdk::{HashableKey as _, Prover as _, ProverClient};
use valence_coprocessor::DomainData;
use zerocopy::IntoBytes as _;

fn main() {
    println!("cargo:rerun-if-env-changed=VALENCE_REBUILD");

    if env::var("VALENCE_REBUILD").is_err() {
        return;
    }

    let manifest = env::var("CARGO_MANIFEST_DIR").unwrap();
    let manifest = PathBuf::from(manifest).parent().unwrap().to_path_buf();
    let root = manifest.parent().unwrap().parent().unwrap();
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

    if env::var("VALENCE_REBUILD_SKIP_CIRCUIT").is_err() {
        // circuit

        sp1_build::build_program("../circuit");

        let inner = release.join("valence-coprocessor-ethereum-service-circuit");

        let inner_elf = fs::read(&inner).unwrap();

        let prover = ProverClient::builder().cpu().build();

        let (_, inner_vk) = prover.setup(&inner_elf);
        let inner_vk_b32 = inner_vk.bytes32();
        let inner_vk = inner_vk.vk.hash_u32();

        fs::write(out.join("inner.bin"), inner_elf).unwrap();
        fs::write(out.join("wrapper-bytes32"), inner_vk_b32).unwrap();
        fs::write(out.join("inner-vkh32.bin"), inner_vk.as_bytes()).unwrap();

        // wrapper

        sp1_build::build_program("../wrapper");

        let wrapper = release.join("valence-coprocessor-ethereum-service-circuit-wrapper");

        let wrapper_elf = fs::read(&wrapper).unwrap();

        let (_, wrapper_vk) = prover.setup(&wrapper_elf);
        let wrapper_vk_b32 = wrapper_vk.bytes32();
        let wrapper_vk = wrapper_vk.vk.hash_u32();

        fs::write(out.join("wrapper.bin"), wrapper_elf).unwrap();
        fs::write(out.join("wrapper-bytes32"), wrapper_vk_b32).unwrap();
        fs::write(out.join("wrapper-vkh32.bin"), wrapper_vk.as_bytes()).unwrap();
    }

    // controller

    if env::var("VALENCE_REBUILD_SKIP_CONTROLLER").is_err() {
        assert!(Command::new("cargo")
            .current_dir(root)
            .args([
                "build",
                "-p",
                "valence-coprocessor-ethereum-controller",
                "--target",
                "wasm32-unknown-unknown",
                "--release",
            ])
            .status()
            .unwrap()
            .success());

        let wasm = root
            .join("target")
            .join("wasm32-unknown-unknown")
            .join("release")
            .join("valence_coprocessor_ethereum_controller.wasm");

        let wasm = fs::read(&wasm).unwrap();

        fs::write(out.join("controller.wasm"), &wasm).unwrap();
    }

    // id

    let id = DomainData::identifier_from_parts("ethereum-electra-beta");
    let id = hex::encode(id);

    fs::write(out.join("id"), id).unwrap();
}
