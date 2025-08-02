use alloc::vec::Vec;
use alloy_primitives::{Bytes, U256};
use alloy_rlp::Encodable as _;
use alloy_rpc_types_eth::Account;
use alloy_trie::Nibbles;
use msgpacker::Unpackable as _;
use valence_coprocessor::{DomainCircuit, StateProof};

use crate::{Ethereum, EthereumProvenAccount, EthereumStateProof, EthereumStorageProofArg};

impl DomainCircuit for Ethereum {
    type Output = EthereumProvenAccount;

    fn verify(proof: &StateProof) -> anyhow::Result<Self::Output> {
        let root = proof.state_root;
        let payload = proof.payload.clone();
        let proof = EthereumStateProof::unpack(&proof.proof)?.1;

        let state_root = From::from(&root);
        let key = alloy_primitives::keccak256(&proof.account);
        let key = Nibbles::unpack(key);

        let mut encoded_account = Vec::new();
        Account {
            nonce: proof.nonce,
            balance: U256::from(proof.balance),
            storage_root: proof.storage_root.into(),
            code_hash: proof.code_hash.into(),
        }
        .encode(&mut encoded_account);

        let account_proof: Vec<_> = proof
            .account_proof
            .iter()
            .map(|p| Bytes::copy_from_slice(p.as_slice()))
            .collect();

        alloy_trie::proof::verify_proof(
            state_root,
            key,
            Some(encoded_account),
            account_proof.iter(),
        )
        .map_err(|e| anyhow::anyhow!("account proof failed: {e}"))?;

        let root = proof.storage_root.into();

        for p in proof.storage_proofs.iter() {
            let key = alloy_primitives::keccak256(&p.key);
            let key = Nibbles::unpack(key);
            let value = p.value.as_ref().cloned();

            let proof: Vec<_> = p
                .proof
                .iter()
                .map(|p| Bytes::copy_from_slice(p.as_slice()))
                .collect();

            alloy_trie::proof::verify_proof(root, key, value, &proof)
                .map_err(|e| anyhow::anyhow!("storage proof failed: {e}"))?;
        }

        let EthereumStateProof {
            account,
            storage_proofs,
            ..
        } = proof;

        let storage = storage_proofs
            .into_iter()
            .map(|p| EthereumStorageProofArg {
                key: p.key,
                value: p.value,
            })
            .collect();

        Ok(EthereumProvenAccount {
            account,
            storage,
            payload,
        })
    }
}
