use alloc::{string::String, vec::Vec};
use alloy_primitives::U256;
use alloy_rpc_types_eth::EIP1186AccountProofResponse;
use alloy_serde::JsonStorageKey;
use msgpacker::Packable as _;
use serde_json::{json, Value};
use valence_coprocessor::{DomainController, DomainData, Hash, StateProof, ValidatedDomainBlock};
use valence_coprocessor_wasm::abi;

use crate::{
    Ethereum, EthereumStateProof, EthereumStateProofArgs, EthereumStorageProof,
    EthereumStorageProofArg,
};

impl Ethereum {
    pub fn get_latest_block() -> anyhow::Result<ValidatedDomainBlock> {
        abi::get_latest_block(Self::ID)?.ok_or_else(|| anyhow::anyhow!("no valid domain block"))
    }

    pub fn get_state_proof(args: &Value) -> anyhow::Result<StateProof> {
        abi::get_state_proof(Self::ID, args)
    }

    pub fn encode_proof(proof: Value, args: EthereumStateProofArgs) -> anyhow::Result<StateProof> {
        let proof: EIP1186AccountProofResponse = serde_json::from_value(proof)?;
        let account = proof.address.to_vec();
        let nonce = proof.nonce;
        let balance = proof.balance.to();
        let account_proof = proof.account_proof.iter().map(|b| b.to_vec()).collect();

        let storage_root = proof.storage_hash.as_slice();
        let storage_root =
            Hash::try_from(storage_root).map_err(|_| anyhow::anyhow!("invalid storage root"))?;

        let code_hash = proof.code_hash.as_slice();
        let code_hash =
            Hash::try_from(code_hash).map_err(|_| anyhow::anyhow!("invalid code hash"))?;

        let EthereumStateProofArgs {
            storage,
            payload,
            root,
            block,
            ..
        } = args;

        let storage_proofs = proof
            .storage_proof
            .iter()
            .zip(storage)
            .map(|(p, arg)| {
                let key = match p.key {
                    JsonStorageKey::Hash(b) => b.to_vec(),
                    JsonStorageKey::Number(n) => n.to_be_bytes::<32>().to_vec(),
                };
                let value = arg.value.filter(|v| !v.is_empty());
                let proof = p.proof.iter().map(|b| b.to_vec()).collect();

                EthereumStorageProof { key, value, proof }
            })
            .collect();

        let proof = EthereumStateProof {
            state_root: args.root,
            account,
            nonce,
            balance,
            storage_root,
            code_hash,
            account_proof,
            storage_proofs,
        }
        .pack_to_vec();

        let domain = DomainData::identifier_from_parts(Self::ID);
        let state_root = root;

        Ok(StateProof {
            domain,
            state_root,
            payload,
            proof,
            number: block,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EthereumStorageLayoutBuilder {
    storage: Vec<EthereumStorageProofArg>,
    base: U256,
}

impl EthereumStorageLayoutBuilder {
    pub fn new(base_slot: u64) -> Self {
        Self {
            storage: Vec::with_capacity(10),
            base: U256::from(base_slot),
        }
    }

    pub fn new_mapping(mapping_id: u64, base_slot: u64) -> Self {
        let id = U256::from(mapping_id).to_be_bytes::<32>();
        let slot = U256::from(base_slot).to_be_bytes::<32>();
        let slot = [id, slot].concat();
        let slot = alloy_primitives::keccak256(slot);

        Self {
            storage: Vec::with_capacity(10),
            base: U256::from_be_slice(slot.as_slice()),
        }
    }

    fn next_slot_entry(&mut self) -> Vec<u8> {
        let key = self.base.to_be_bytes::<32>();
        self.base += U256::ONE;
        key.to_vec()
    }

    /// Adds combined values into a single slot.
    pub fn add_combined_values<I, R>(self, items: I) -> Self
    where
        I: IntoIterator<Item = R>,
        R: AsRef<[u8]>,
    {
        let mut value = Vec::with_capacity(32);

        for i in items {
            value.extend(i.as_ref());
        }

        self.add_value(value)
    }

    /// Adds a single entry
    ///
    /// Only for types that fit into a single slot and don't have a special encoding (e.g. strings, bytes...)
    pub fn add_value<T>(mut self, value: T) -> Self
    where
        T: AsRef<[u8]>,
    {
        let value = value.as_ref();
        let key = self.next_slot_entry();

        let value = Some(rlp::encode(&value).to_vec());
        self.storage.push(EthereumStorageProofArg { key, value });
        self
    }

    /// Adds a single string entry.
    ///
    /// Will consume multiple slots if the length is greater or equal than 32.
    pub fn add_string_value<T>(mut self, value: T) -> Self
    where
        T: AsRef<[u8]>,
    {
        let value = value.as_ref();
        let len = value.len() as u64;

        if len < 32 {
            let key = self.next_slot_entry();
            let mut slot_value = [0u8; 32];
            slot_value[..len as usize].copy_from_slice(value); // Left-align the data
            slot_value[31] = (len * 2) as u8; // Length * 2 in rightmost byte

            let value = Some(rlp::encode(&slot_value.to_vec()).to_vec());
            self.storage.push(EthereumStorageProofArg { key, value });
        } else {
            let key = self.base;
            self.base += U256::ONE;

            let base_slot = (len << 1) + 1;
            let base_slot = U256::from(base_slot).to_be_bytes_trimmed_vec();

            self.storage.push(EthereumStorageProofArg {
                key: key.to_be_bytes::<32>().to_vec(),
                value: Some(rlp::encode(&base_slot).to_vec()),
            });

            let base_slot = alloy_primitives::keccak256(key.to_be_bytes::<32>());
            let base_slot = U256::from_be_slice(base_slot.as_slice());

            for (i, c) in value.chunks(32).enumerate() {
                let i = U256::from(i);
                let mut value = c.to_vec();

                value.resize(32, 0);

                self.storage.push(EthereumStorageProofArg {
                    key: (base_slot + i).to_be_bytes::<32>().to_vec(),
                    value: Some(rlp::encode(&value).to_vec()),
                });
            }
        }

        self
    }

    /// Adds an empty slot for proof of non-membership.
    pub fn add_empty_slot(mut self) -> Self {
        let key = self.next_slot_entry();
        let value = None;

        self.storage.push(EthereumStorageProofArg { key, value });

        self
    }

    pub fn build(self) -> Vec<EthereumStorageProofArg> {
        self.storage
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EthereumStateProofArgsBuilder {
    address: String,
    block: Option<(u64, Hash)>,
    storage: Vec<EthereumStorageProofArg>,
    payload: Vec<u8>,
}

impl EthereumStateProofArgsBuilder {
    pub fn new(address: String) -> Self {
        Self {
            address,
            block: None,
            storage: Default::default(),
            payload: Default::default(),
        }
    }

    pub fn with_block(mut self, number: u64, root: Hash) -> Self {
        self.block.replace((number, root));
        self
    }

    pub fn with_storage(mut self, storage: Vec<EthereumStorageProofArg>) -> Self {
        self.storage = storage;
        self
    }

    pub fn with_payload(mut self, payload: Vec<u8>) -> Self {
        self.payload = payload;
        self
    }

    pub fn build(self) -> anyhow::Result<Value> {
        let (block, root) = match self.block {
            Some(x) => x,
            None => Ethereum::get_latest_block().map(|b| (b.number, b.root))?,
        };

        Ok(serde_json::to_value(EthereumStateProofArgs {
            address: self.address,
            block,
            root,
            storage: self.storage,
            payload: self.payload,
        })?)
    }
}

impl DomainController for Ethereum {
    const ID: &str = Self::ID;

    fn state_proof(&self, args: Value) -> anyhow::Result<StateProof> {
        let args: EthereumStateProofArgs = serde_json::from_value(args)?;
        let block = U256::from(args.block);

        let storage_keys: Vec<_> = args
            .storage
            .iter()
            .map(|s| U256::from_be_slice(s.key.as_slice()))
            .collect();

        let proof = abi::alchemy(
            Self::NETWORK,
            "eth_getProof",
            &json!([args.address, storage_keys, block]),
        )?;

        Ethereum::encode_proof(proof, args)
    }
}
