use alloc::{string::String, vec::Vec};
use alloy_primitives::B256;
use helios_consensus_core::{
    consensus_spec::MainnetConsensusSpec,
    types::{FinalityUpdate, Forks, Update},
};
use msgpacker::{MsgPacker, Packable as _, Unpackable as _};
use serde::{Deserialize, Serialize};
use sha2_v0_10_8::{Digest as _, Sha256};
use valence_coprocessor::{Base64, Blake3Hasher, Hash, Hasher as _, Proof, ValidatedBlock};
use zerocopy::TryFromBytes as _;

use crate::State;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, MsgPacker)]
pub struct ServiceState {
    pub latest_inner_proof: Proof,
    pub inner_vk: Vec<u8>,
    pub wrapper_vk: Vec<u8>,
    pub wrapper_vk_bytes32: String,
}

impl ServiceState {
    pub fn to_vec(&self) -> Vec<u8> {
        self.pack_to_vec()
    }

    pub fn try_from_slice<B>(bytes: B) -> anyhow::Result<Self>
    where
        B: AsRef<[u8]>,
    {
        Ok(Self::unpack(bytes.as_ref())?.1)
    }

    pub fn encode(&self) -> String {
        Base64::encode(self.to_vec())
    }

    pub fn decode<B>(base64: B) -> anyhow::Result<Self>
    where
        B: AsRef<str>,
    {
        Base64::decode(base64).and_then(Self::try_from_slice)
    }

    pub fn to_state(&self) -> anyhow::Result<State> {
        let inner = self.latest_inner_proof.decode()?.1;
        let inner = CircuitInner::try_from_slice(&inner)?;

        Ok(inner.state)
    }

    pub fn inner() -> Hash {
        Blake3Hasher::hash(CircuitInner::elf())
    }

    pub fn wrapper() -> Hash {
        Blake3Hasher::hash(CircuitInner::wrapper_elf())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, MsgPacker)]
pub struct ProvenState {
    pub inner: Proof,
    pub wrapper: Proof,
}

impl ProvenState {
    pub fn to_vec(&self) -> Vec<u8> {
        self.pack_to_vec()
    }

    pub fn try_from_slice<B>(bytes: B) -> anyhow::Result<Self>
    where
        B: AsRef<[u8]>,
    {
        Ok(Self::unpack(bytes.as_ref())?.1)
    }

    pub fn encode(&self) -> String {
        Base64::encode(self.to_vec())
    }

    pub fn decode<B>(base64: B) -> anyhow::Result<Self>
    where
        B: AsRef<str>,
    {
        Base64::decode(base64).and_then(Self::try_from_slice)
    }

    pub fn to_validated_block(&self) -> anyhow::Result<ValidatedBlock> {
        self.wrapper
            .decode()
            .and_then(|(_, a)| Ok(ValidatedBlock::unpack(&a)?.1))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub genesis_time: u64,
    pub genesis_root: B256,
    pub forks: Forks,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Output {
    pub block_number: u64,
    pub state_root: B256,
}

impl Default for Output {
    fn default() -> Self {
        State::default().to_output().unwrap()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Input {
    pub updates: Vec<Update<MainnetConsensusSpec>>,
    pub finality_update: FinalityUpdate<MainnetConsensusSpec>,

    /// This exists only to satisfy helios API, but the concept of time is not verifiable on a ZK
    /// circuit. Hence, it adds no security and is a trusted input.
    pub expected_current_slot: u64,
}

impl Default for Input {
    fn default() -> Self {
        let input = include_bytes!("../assets/input.json");

        serde_json::from_slice(input).unwrap()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(clippy::large_enum_variant)]
pub enum CircuitWitness {
    Genesis { vk: [u32; 8] },
    Update { public: Vec<u8>, input: Input },
}

impl Default for CircuitWitness {
    fn default() -> Self {
        Self::Genesis {
            vk: CircuitInner::vk_hash(),
        }
    }
}

impl CircuitWitness {
    pub fn update(public: Vec<u8>, input: Input) -> Self {
        Self::Update { public, input }
    }

    pub fn open(self) -> anyhow::Result<CircuitOpenWitness> {
        match self {
            CircuitWitness::Genesis { vk } => {
                let state = State::default();

                Ok(CircuitOpenWitness {
                    vk,
                    state,
                    args: None,
                })
            }

            CircuitWitness::Update { public, input } => {
                let digest = Sha256::digest(&public).into();
                let CircuitInner { vk, state } = serde_cbor::from_slice(&public)?;

                Ok(CircuitOpenWitness {
                    vk,
                    state,
                    args: Some((digest, input)),
                })
            }
        }
    }

    pub fn to_vec(&self) -> Vec<u8> {
        serde_cbor::to_vec(self).unwrap()
    }

    pub fn try_from_slice<B>(bytes: B) -> anyhow::Result<Self>
    where
        B: AsRef<[u8]>,
    {
        Ok(serde_cbor::from_slice(bytes.as_ref())?)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitOpenWitness {
    pub vk: [u32; 8],
    pub state: State,
    pub args: Option<(Hash, Input)>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CircuitInner {
    pub vk: [u32; 8],
    pub state: State,
}

impl From<State> for CircuitInner {
    fn from(state: State) -> Self {
        Self::new(state)
    }
}

impl From<CircuitInner> for State {
    fn from(circuit: CircuitInner) -> Self {
        circuit.state
    }
}

impl CircuitInner {
    pub fn new(state: State) -> Self {
        Self {
            vk: Self::vk_hash(),
            state,
        }
    }

    pub fn into_state<P>(public: P) -> anyhow::Result<State>
    where
        P: AsRef<[u8]>,
    {
        let inner: Self = serde_cbor::from_slice(public.as_ref())?;

        Ok(inner.state)
    }

    pub fn digest<P>(public: P) -> Hash
    where
        P: AsRef<[u8]>,
    {
        Sha256::digest(public.as_ref()).into()
    }

    pub fn try_from_slice(bytes: &[u8]) -> anyhow::Result<Self> {
        Ok(serde_cbor::from_slice(bytes)?)
    }

    pub fn to_vec(&self) -> Vec<u8> {
        serde_cbor::to_vec(self).unwrap()
    }

    pub const fn elf() -> &'static [u8] {
        include_bytes!("../../elf/inner.bin")
    }

    pub fn wrapper_elf() -> &'static [u8] {
        include_bytes!("../../elf/wrapper.bin")
    }

    pub fn vk_hash() -> [u32; 8] {
        let bytes = include_bytes!("../../elf/inner-vkh32.bin");

        <[u32; 8]>::try_read_from_bytes(bytes).unwrap()
    }
}

impl Default for Config {
    fn default() -> Self {
        serde_json::from_value(serde_json::json!({
            "genesis_time": 1606824023,
            "genesis_root": "0x4b363db94e286120d76eb905340fdd4e54bfe9f06bf33ff6cf5ad27f511bfe95",
            "forks": {
                "genesis": {
                    "epoch": 0,
                    "fork_version": "0x00000000"
                },
                "altair": {
                    "epoch": 74240,
                    "fork_version": "0x01000000"
                },
                "bellatrix": {
                    "epoch": 144896,
                    "fork_version": "0x02000000"
                },
                "capella": {
                    "epoch": 194048,
                    "fork_version": "0x03000000"
                },
                "deneb": {
                    "epoch": 269568,
                    "fork_version": "0x04000000"
                },
                "electra": {
                    "epoch": 364032,
                    "fork_version": "0x05000000"
                }
            }
        }))
        .unwrap()
    }
}

#[test]
fn circuit_elf_is_consistent() {
    use sp1_sdk::{HashableKey as _, Prover as _, ProverClient};

    let inner = CircuitInner::elf();
    let inner_vk = ProverClient::builder().cpu().build().setup(inner).1;
    let inner_vk = inner_vk.hash_u32();

    assert_eq!(inner_vk, CircuitInner::vk_hash());
}
