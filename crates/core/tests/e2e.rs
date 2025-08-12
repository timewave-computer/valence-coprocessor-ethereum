use alloy_primitives::U256;
use alloy_rlp::{RlpDecodable, RlpEncodable};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use valence_coprocessor::{DomainCircuit as _, Hash};
use valence_coprocessor_ethereum::{
    controller::EthereumStorageLayoutBuilder, Ethereum, EthereumStateProofArgs,
    EthereumStorageProofArg,
};

#[test]
fn encode_and_verify_proofs_works() {
    let cases = [
        &include_bytes!("../../../assets/proof-short.json")[..],
        &include_bytes!("../../../assets/proof-long.json")[..],
    ];

    for data in cases {
        let data: Value = serde_json::from_slice(data).unwrap();

        let address = data["account"].as_str().unwrap().to_string();
        let account = address.strip_prefix("0x").unwrap();
        let account = hex::decode(account).unwrap();

        let block = data["block"].as_str().unwrap().strip_prefix("0x").unwrap();
        let block = U256::from_str_radix(block, 16).unwrap().to_le_bytes::<32>();
        let block = <[u8; 8]>::try_from(&block[..8]).unwrap();
        let block = u64::from_le_bytes(block);

        let root = data["root"].as_str().unwrap().strip_prefix("0x").unwrap();
        let root = hex::decode(root).unwrap();
        let root = Hash::try_from(root).unwrap();

        let proof = data["proof"].clone();

        let withdraw = data["withdraw"].clone();
        let withdraw: WithdrawRequest = serde_json::from_value(withdraw).unwrap();

        let storage = Vec::from(withdraw.clone());

        let payload = b"foo";
        let args = EthereumStateProofArgs {
            address: address.clone(),
            block,
            root,
            storage,
            payload: payload.to_vec(),
        };

        let proof = Ethereum::encode_proof(proof, args).unwrap();
        let proof = Ethereum::verify(&proof).unwrap();

        assert_eq!(proof.payload, payload);
        assert_eq!(proof.account, account);

        let value = [&withdraw.owner.into_array()[..], &withdraw.id.to_be_bytes()].concat();
        assert_eq!(Some(rlp::encode(&value).to_vec()), proof.storage[0].value);

        let value = withdraw.redemptionRate.to_be_bytes_trimmed_vec();
        assert_eq!(Some(rlp::encode(&value).to_vec()), proof.storage[1].value);

        let value = withdraw.sharesAmount.to_be_bytes_trimmed_vec();
        assert_eq!(Some(rlp::encode(&value).to_vec()), proof.storage[2].value);

        let value = ((withdraw.receiver.len() as u64) << 1) + 1;
        let value = U256::from(value).to_be_bytes_trimmed_vec();
        assert_eq!(Some(rlp::encode(&value).to_vec()), proof.storage[3].value);

        for (i, c) in withdraw.receiver.as_bytes().chunks(32).enumerate() {
            let mut value = c.to_vec();

            value.resize(32, 0);

            assert_eq!(
                Some(rlp::encode(&value).to_vec()),
                proof.storage[4 + i].value
            );
        }
    }
}

alloy_sol_types::sol! {
    #![sol(extra_derives(Debug, Serialize, Deserialize, RlpEncodable, RlpDecodable))]
    struct WithdrawRequest {
        uint64 id;
        address owner;
        uint256 redemptionRate;
        uint256 sharesAmount;
        string receiver;
    }
}

impl From<WithdrawRequest> for Vec<EthereumStorageProofArg> {
    fn from(withdraw: WithdrawRequest) -> Self {
        EthereumStorageLayoutBuilder::new_mapping(withdraw.id, 0x9)
            .add_combined_values([&withdraw.owner.into_array()[..], &withdraw.id.to_be_bytes()])
            .add_value(withdraw.redemptionRate.to_be_bytes_trimmed_vec())
            .add_value(withdraw.sharesAmount.to_be_bytes_trimmed_vec())
            .add_string_value(withdraw.receiver)
            .build()
    }
}
