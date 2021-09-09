use serde::Deserialize;
use serde::Serialize;
use web3::Web3;
use web3::transports::WebSocket;
use web3::ethabi::{Address};
use web3::types::Bytes;
use web3::contract::Contract;
use web3::types::CallRequest;
use web3::ethabi::ethereum_types::{H160, U256};

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ResponseApi {
    pub planetNo: String,
    pub coordinate: String,
    pub description: String,
    pub external_url: String,
    pub image: String,
    pub name: String,
    pub attributes: Attributes,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Attributes {
    #[serde(rename(deserialize = "0"))]
    pub attribute_0: Attribute,
    #[serde(rename(deserialize = "1"))]
    pub attribute_1: Attribute,
    #[serde(rename(deserialize = "2"))]
    pub attribute_2: Attribute,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Attribute {
    pub trait_type: String,
    pub value: u32,
}


pub async fn instantiate_contract(web3: &Web3<WebSocket>, contract_address: &Address, abi_path: &str) -> Contract<WebSocket> {
    let vec = std::fs::read(abi_path).unwrap();
    Contract::from_json(
        web3.eth(),
        *contract_address,
        vec.as_slice(),
    ).unwrap()
}

pub async fn get_web3(avalanche_go_url: &str) -> Web3<WebSocket> {
    let ws = web3::transports::WebSocket::new(avalanche_go_url)
        .await
        .unwrap();
    web3::Web3::new(ws)
}

pub async fn get_gas_usage_estimation(wallet_address: H160, mut gas_price: U256, web3: &Web3<WebSocket>, game_contract: &Contract<WebSocket>, bytes: Bytes) -> U256 {
    let estimated_gas_price = web3.eth().estimate_gas(
        CallRequest {
            from: Some(wallet_address),
            to: Some(game_contract.address()),
            gas: None,
            gas_price: Some(gas_price),
            value: None,
            data: Some(bytes),
            transaction_type: None,
            access_list: None,
        },
        None).await.unwrap();

    estimated_gas_price
}