#![no_std]

use alloc::{string::String, vec::Vec};
use msgpacker::MsgPacker;
use serde::{Deserialize, Serialize};
use valence_coprocessor::Hash;

extern crate alloc;

pub struct Ethereum;

impl Ethereum {
    pub const ID: &str = "ethereum-electra-beta";
    pub const NETWORK: &str = "eth-mainnet";
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, MsgPacker)]
pub struct EthereumStateProof {
    pub state_root: Hash,
    pub account: Vec<u8>,
    pub nonce: u64,
    pub balance: u64,
    pub storage_root: Hash,
    pub code_hash: Hash,
    pub account_proof: Vec<Vec<u8>>,
    pub storage_proofs: Vec<EthereumStorageProof>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, MsgPacker)]
pub struct EthereumProvenAccount {
    /// Account address.
    pub account: Vec<u8>,

    /// RLP encoded proven storage values.
    pub storage: Vec<EthereumStorageProofArg>,

    /// User payload.
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, MsgPacker)]
pub struct EthereumStorageProofArg {
    /// The computed storage key for the storage slot.
    pub key: Vec<u8>,

    /// The RLP encoded slot value.
    pub value: Option<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, MsgPacker)]
pub struct EthereumStorageProof {
    /// The computed storage key for the storage slot.
    pub key: Vec<u8>,

    /// The RLP encoded slot value.
    pub value: Option<Vec<u8>>,

    /// The Merkle storage proof.
    pub proof: Vec<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EthereumStateProofArgs {
    /// Contract address.
    pub address: String,

    /// Block number.
    pub block: u64,

    /// Base64 encoded state root [Hash].
    pub root: Hash,

    /// List of storage entries to be proven.
    pub storage: Vec<EthereumStorageProofArg>,

    /// Payload to be forwarded to the circuit.
    pub payload: Vec<u8>,
}

#[cfg(feature = "circuit")]
pub mod circuit;

#[cfg(feature = "controller")]
pub mod controller;
