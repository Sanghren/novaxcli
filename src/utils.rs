use serde::Deserialize;
use serde::Serialize;
use web3::{Web3, Error};
use web3::transports::WebSocket;
use web3::ethabi::{Address, Token};
use web3::types::{Bytes, BlockNumber};
use web3::contract::{Contract, Options};
use web3::types::CallRequest;
use web3::ethabi::ethereum_types::{H160, U256};
use std::time::Duration;
use std::thread;

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

pub async fn get_gas_usage_estimation(wallet_address: H160, mut gas_price: U256, web3: &Web3<WebSocket>, game_contract: &Contract<WebSocket>, bytes: &Bytes) -> U256 {
    let mut estimated_gas_price: U256 = U256::from(0);
    let mut iteration = 0;
    while iteration < 10 {
        match web3.eth().estimate_gas(
            CallRequest {
                from: Some(wallet_address),
                to: Some(game_contract.address()),
                gas: None,
                gas_price: Some(gas_price),
                value: None,
                data: Some(bytes.clone()),
                transaction_type: None,
                access_list: None,
            },
            None).await {
            Ok(gas_usage) => { estimated_gas_price = gas_usage; break; },
            Err(err) => { println!("Iteration {} / 10 -- Error while estimating gas usage for this call on contract {:?} -- Error message : {:?}", iteration,game_contract.address(), err); iteration = iteration + 1; thread::sleep(Duration::new(5,0)) },
        }
    }

    if iteration == 10 && estimated_gas_price == U256::from(0) {
        panic!("Failed to estimate gas usage . Probably this is due to your gas price being too low for the current network base fee. Try later or increase gas price !")
    }
    estimated_gas_price
}

pub async fn get_current_nonce(wallet_address: H160, web3: &Web3<WebSocket>) -> u64 {
    let nonce = web3.eth().transaction_count(wallet_address, Option::from(BlockNumber::Pending)).await.unwrap();
    let u64_nonce = nonce.as_u64();
    u64_nonce
}

pub async fn fetch_current_resources(wallet_address: H160, iron_contract: &Contract<WebSocket>, solar_contract: &Contract<WebSocket>, crystal_contract: &Contract<WebSocket>, upgrade_cost: &Vec<U256>) -> (U256, U256, U256, f64, f64, f64, f64, f64, f64) {
    let wallet_iron_amount_future = iron_contract.query("balanceOf", Token::Address(wallet_address), None, Options::default(), None);
    let wallet_iron_amount: U256 = wallet_iron_amount_future.await.unwrap();
    let solar_amount_future = solar_contract.query("balanceOf", Token::Address(wallet_address), None, Options::default(), None);
    let solar_amount: U256 = solar_amount_future.await.unwrap();
    let crystal_amount_future = crystal_contract.query("balanceOf", Token::Address(wallet_address), None, Options::default(), None);
    let crystal_amount: U256 = crystal_amount_future.await.unwrap();

    let iron_amount_decimals = (wallet_iron_amount.as_u128() as f64
        / (10_u64.pow(18 as u32)) as f64)
        as f64;
    let solar_amount_decimals = (solar_amount.as_u128() as f64
        / (10_u64.pow(18 as u32)) as f64)
        as f64;
    let crystal_amount_decimals = (crystal_amount.as_u128() as f64
        / (10_u64.pow(18 as u32)) as f64)
        as f64;

    let upgrade_iron_amount_decimals = (upgrade_cost.get(1).unwrap().as_u128() as f64
        / (10_u64.pow(18 as u32)) as f64)
        as f64;
    let upgrade_solar_amount_decimals = (upgrade_cost.get(0).unwrap().as_u128() as f64
        / (10_u64.pow(18 as u32)) as f64)
        as f64;
    let upgrade_crystal_amount_decimals = (upgrade_cost.get(2).unwrap().as_u128() as f64
        / (10_u64.pow(18 as u32)) as f64)
        as f64;
    (wallet_iron_amount, solar_amount, crystal_amount, iron_amount_decimals, solar_amount_decimals, crystal_amount_decimals, upgrade_iron_amount_decimals, upgrade_solar_amount_decimals, upgrade_crystal_amount_decimals)
}
