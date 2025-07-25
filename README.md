# Valence co-processor Ethereum domain

A Ethereum domain utility for the Valence co-processor.

## Controller witness generation

```rust,ignore
use valence_coprocessor::StateProof;
use valence_coprocessor_ethereum::{
    controller::{EthereumStateProofArgsBuilder, EthereumStorageLayoutBuilder},
    Ethereum,
};

alloy_sol_types::sol! {
    struct WithdrawRequest {
        uint64 id;
        address owner;
        uint256 redemptionRate;
        uint256 sharesAmount;
        string receiver;
    }
}

pub fn prove_withdraw(withdraw: WithdrawRequest) -> anyhow::Result<StateProof> {
    // initializes the storage layout for a mapping indexed by base slot 9
    let layout = EthereumStorageLayoutBuilder::new_mapping(withdraw.id, 0x9)

        // ethereum will combine multiple contiguous values into a slot, if they fit 32 bytes
        .add_combined_values([&withdraw.owner.into_array()[..], &withdraw.id.to_be_bytes()])

        // the remainder values are trivially inserted into individual slots
        .add_value(withdraw.redemptionRate.to_be_bytes_trimmed_vec())
        .add_value(withdraw.sharesAmount.to_be_bytes_trimmed_vec())

        // arbitrary length values, such as string, are also supported
        .add_value(withdraw.receiver)

        // finishes the layout
        .build();

    let address = "0xf2B85C389A771035a9Bd147D4BF87987A7F9cf98";
    let args = EthereumStateProofArgsBuilder::new(address.into())
        .with_storage(layout)
        .with_payload(b"foo".to_vec())
        .build()?;

    // computes the StateProof object that can be used as witness
    Ethereum::get_state_proof(&args)
}
```

## Circuit proof verification


```rust,ignore
use valence_coprocessor::{DomainCircuit, StateProof};
use valence_coprocessor_ethereum::{Ethereum, EthereumProvenAccount, EthereumStorageProofArg};

alloy_sol_types::sol! {
    struct WithdrawRequest {
        uint64 id;
        address owner;
        uint256 redemptionRate;
        uint256 sharesAmount;
        string receiver;
    }
}

pub fn verify_proof(proof: &StateProof) -> anyhow::Result<Vec<EthereumStorageProofArg>> {
    let EthereumProvenAccount {
        account,
        storage,
        payload,
    } = Ethereum::verify(&proof)?;

    // here goes the circuit user assertions

    anyhow::ensure!(hex::encode(account) == "f2b85c389a771035a9bd147d4bf87987a7f9cf98");
    anyhow::ensure!(storage[5].value == Some(b"some RLP encoding pre-image".to_vec()));

    // payload is *NOT* validated

    anyhow::ensure!(payload == b"foo");

    Ok(storage)
}
```
