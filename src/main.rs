use std::{env, time, thread};
use web3::Web3;
use web3::transports::WebSocket;
use web3::ethabi::{Address, Token};
use std::error::Error;
use std::str::FromStr;
use web3::ethabi::ethereum_types::{U256, U64, H160};
use std::ops::{Add, Mul};
use serde::Deserialize;
use serde::Serialize;
use web3::types::{TransactionParameters, BlockNumber};
use web3::{
    contract::{Contract, Options},
    futures::StreamExt,
    types::FilterBuilder,
    types::Bytes,
};
use secp256k1::SecretKey;
use hex_literal::hex;
use web3::types::CallRequest;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();

    // START SETUP CONFIG FROM CMD ARG
    let mut harvest_mode = false;
    let mut upgrade_mode = false;
    let mut fetch_info_mode = false;
    let mut threshold = 2;

    if args.len() < 2 {
        panic!("Invalid number of arguments. You must pass 1 - pkey, 2 - gas price (in nAvax) , 3 - the command (harvestAll, fetchInfo, upgradeAll)");
    }
    let wallet_address = Address::from_str(args.get(1).unwrap()).unwrap();
    let byte_pkey = hex::decode(args.get(2).unwrap())?;

    let mut _ppkey = SecretKey::from_slice(byte_pkey.as_slice()).unwrap();
    let mut gas_price = U256::from(args.get(3).unwrap().parse::<i64>()?);

    if args.len() > 1 {
        let cmd = args.get(4).unwrap();
        if cmd.eq_ignore_ascii_case("harvestAll") {
            harvest_mode = true;
        } else if cmd.eq_ignore_ascii_case("upgradeMode") {
            upgrade_mode = true;
            threshold = u32::from_str(args.get(5).unwrap())?;
        } else if cmd.eq_ignore_ascii_case("fetchInfo") {
            fetch_info_mode = true;
        }
    }
    // END SETUP CONFIG FROM CMD ARG

    // Get Web3 Instance
    let web3 = get_web3("wss://api.avax.network/ext/bc/C/ws").await;

    // Instantiate the contracts
    let planet_contract = instantiate_contract(&web3, &Address::from_str("0x0C3b29321611736341609022C23E981AC56E7f96").unwrap(), "abi/novax_planet.abi").await;
    let game_contract = instantiate_contract(&web3, &Address::from_str("0x08776C5830c80e2A0Acd7596BdDfEB3cB19cB5Fd").unwrap(), "abi/novax_game.abi").await;
    let iron_contract = instantiate_contract(&web3, &Address::from_str("0x4C1057455747e3eE5871D374FdD77A304cE10989").unwrap(), "abi/erc20.abi").await;
    let solar_contract = instantiate_contract(&web3, &Address::from_str("0xE6eE049183B474ecf7704da3F6F555a1dCAF240F").unwrap(), "abi/erc20.abi").await;
    let crystal_contract = instantiate_contract(&web3, &Address::from_str("0x70b4aE8eb7bd572Fc0eb244Cd8021066b3Ce7EE4").unwrap(), "abi/erc20.abi").await;

    // We fetch the planetes owned by the address we passed as first argument
    let planets_for_address_future =
        planet_contract.query("tokensOfOwner", Token::Address(wallet_address), None, Options::default(), None);

    let planets_for_address: Vec<U256> = planets_for_address_future.await.unwrap();

    if fetch_info_mode {
        fetch_info(planet_contract, game_contract, iron_contract, solar_contract, crystal_contract, planets_for_address, wallet_address).await;
    } else if harvest_mode {
        let nonce = web3.eth().transaction_count(wallet_address, Option::from(BlockNumber::Pending)).await.unwrap();
        let u64_nonce = nonce.as_u64();
        let mut tokens_array_planets_id: Vec<Token> = Vec::new();

        for planet_id in planets_for_address {
            tokens_array_planets_id.push(Token::Uint(planet_id));
        }

        let harvest_all = game_contract.abi().functions.get("harvestAll").unwrap().get(0).unwrap().encode_input([Token::Array(tokens_array_planets_id)].as_ref()).unwrap();

        let vec = harvest_all.clone();
        let bytes = Bytes::from(vec);
        let estimated_gas_usage = get_gas_usage_estimation(wallet_address, gas_price, &web3, &game_contract, bytes).await;

        let transaction = TransactionParameters {
            nonce: Some(U256::from(u64_nonce)),
            to: Some(game_contract.address()),
            value: Default::default(),
            gas_price: Some(gas_price),
            gas: estimated_gas_usage,
            data: Bytes::from(harvest_all.clone()),
            chain_id: Some(43114_u64),
            transaction_type: None,
            access_list: None,
        };
        let signed_tx = web3.accounts().sign_transaction(transaction, &_ppkey).await.unwrap();

        let res = web3.eth().send_raw_transaction(Bytes::from(signed_tx.raw_transaction)).await?;

        let mut tx_status = web3.eth().transaction_receipt(res).await?;

        while !tx_status.is_some() || tx_status.unwrap().status == Some(U64::from(0)) {
            println!("{:?} -- Harvest All tx  -- {:?}", time::Instant::now(), res);
            tx_status = web3.eth().transaction_receipt(res).await?;
            let delay = time::Duration::from_secs(3);
            thread::sleep(delay);
        }
    } else if upgrade_mode {
        for planet_id in planets_for_address {
            let planet_uri_future = planet_contract.query("tokenURI", Token::Uint(planet_id), None, Options::default(), None);
            let planet_uri: String = planet_uri_future.await.unwrap();

            let mut response = reqwest::get(&planet_uri)?;
            let price_response: ResponseApi = response.json()?;

            // 0 is for Solar Panel
            if price_response.attributes.attribute_0.value < threshold {
                let next_upgrade_level = price_response.attributes.attribute_0.value + 1;
                let upgrade_cost_future = game_contract.query("resourceInfo", (Token::String("s".to_string()), Token::Uint(U256::from(next_upgrade_level))), None, Options::default(), None);

                let mut upgrade_cost: Vec<U256> = upgrade_cost_future.await?;

                let wallet_iron_amount_future = iron_contract.query("balanceOf", Token::Address(wallet_address), None, Options::default(), None);
                let wallet_iron_amount: U256 = wallet_iron_amount_future.await.unwrap();
                let solar_amount_future = solar_contract.query("balanceOf", Token::Address(wallet_address), None, Options::default(), None);
                let solar_amount: U256 = solar_amount_future.await.unwrap();
                let crystal_amount_future = crystal_contract.query("balanceOf", Token::Address(wallet_address), None, Options::default(), None);
                let crystal_amount: U256 = crystal_amount_future.await.unwrap();

                if upgrade_cost.get(0).unwrap() <= &solar_amount && upgrade_cost.get(1).unwrap() <= &wallet_iron_amount && upgrade_cost.get(2).unwrap() <= &crystal_amount {
                    let nonce = web3.eth().transaction_count(wallet_address, Option::from(BlockNumber::Pending)).await.unwrap();
                    let u64_nonce = nonce.as_u64();
                    let level_up_structure = game_contract.abi().functions.get("levelUpStructure").unwrap().get(0).unwrap().encode_input([Token::String("s".to_string()), Token::Uint(planet_id)].as_ref()).unwrap();

                    let vec = level_up_structure.clone();
                    let bytes = Bytes::from(vec);
                    let estimated_gas_usage = get_gas_usage_estimation(wallet_address, gas_price, &web3, &game_contract, bytes).await;

                    let transaction = TransactionParameters {
                        nonce: Some(U256::from(u64_nonce)),
                        to: Some(game_contract.address()),
                        value: Default::default(),
                        gas_price: Some(gas_price),
                        gas: estimated_gas_usage,
                        data: Bytes::from(level_up_structure.clone()),
                        chain_id: Some(43114_u64),
                        transaction_type: None,
                        access_list: None,
                    };
                    let signed_tx = web3.accounts().sign_transaction(transaction, &_ppkey).await.unwrap();

                    let res = web3.eth().send_raw_transaction(Bytes::from(signed_tx.raw_transaction)).await?;

                    let mut tx_status = web3.eth().transaction_receipt(res).await?;

                    while !tx_status.is_some() {
                        println!("{:?} -- Level up solar panel tx to level {} on planet {}", time::Instant::now(), next_upgrade_level, planet_id);
                        tx_status = web3.eth().transaction_receipt(res).await?;
                        let delay = time::Duration::from_secs(3);
                        thread::sleep(delay);
                    }

                    if tx_status.unwrap().status == Some(U64::from(0)) {
                        panic!("Transaction status -- failed");
                    }
                }

                println!("Cost for upgrading solar panel for planet {} -- {:?}", planet_id, upgrade_cost);
            }

            if price_response.attributes.attribute_1.value < threshold {
                let next_upgrade_level = price_response.attributes.attribute_1.value + 1;
                let upgrade_cost_future = game_contract.query("resourceInfo", (Token::String("m".to_string()), Token::Uint(U256::from(next_upgrade_level))), None, Options::default(), None);

                let upgrade_cost: Vec<U256> = upgrade_cost_future.await?;

                let wallet_iron_amount_future = iron_contract.query("balanceOf", Token::Address(wallet_address), None, Options::default(), None);
                let wallet_iron_amount: U256 = wallet_iron_amount_future.await.unwrap();
                let solar_amount_future = solar_contract.query("balanceOf", Token::Address(wallet_address), None, Options::default(), None);
                let solar_amount: U256 = solar_amount_future.await.unwrap();
                let crystal_amount_future = crystal_contract.query("balanceOf", Token::Address(wallet_address), None, Options::default(), None);
                let crystal_amount: U256 = crystal_amount_future.await.unwrap();

                if upgrade_cost.get(0).unwrap() <= &solar_amount && upgrade_cost.get(1).unwrap() <= &wallet_iron_amount && upgrade_cost.get(2).unwrap() <= &crystal_amount {
                    let nonce = web3.eth().transaction_count(wallet_address, Option::from(BlockNumber::Pending)).await.unwrap();
                    let u64_nonce = nonce.as_u64();
                    let level_up_structure = game_contract.abi().functions.get("levelUpStructure").unwrap().get(0).unwrap().encode_input([Token::String("m".to_string()), Token::Uint(planet_id)].as_ref()).unwrap();

                    let vec = level_up_structure.clone();
                    let bytes = Bytes::from(vec);
                    let estimated_gas_usage = get_gas_usage_estimation(wallet_address, gas_price, &web3, &game_contract, bytes).await;

                    let transaction = TransactionParameters {
                        nonce: Some(U256::from(u64_nonce)),
                        to: Some(game_contract.address()),
                        value: Default::default(),
                        gas_price: Some(gas_price),
                        gas: estimated_gas_usage,
                        data: Bytes::from(level_up_structure.clone()),
                        chain_id: Some(43114_u64),
                        transaction_type: None,
                        access_list: None,
                    };
                    let signed_tx = web3.accounts().sign_transaction(transaction, &_ppkey).await.unwrap();

                    let res = web3.eth().send_raw_transaction(Bytes::from(signed_tx.raw_transaction)).await?;

                    let mut tx_status = web3.eth().transaction_receipt(res).await?;

                    while !tx_status.is_some() {
                        println!("{:?} -- Level up iron mine tx to level {} on planet {}", time::Instant::now(), next_upgrade_level, planet_id);
                        tx_status = web3.eth().transaction_receipt(res).await?;
                        let delay = time::Duration::from_secs(3);
                        thread::sleep(delay);
                    }

                    if tx_status.unwrap().status == Some(U64::from(0)) {
                        panic!("Transaction status -- failed");
                    }
                }

                println!("Cost for upgrading iron mine for planet {} -- {:?}", planet_id, upgrade_cost);
            }

            if price_response.attributes.attribute_2.value < threshold {
                let next_upgrade_level = price_response.attributes.attribute_2.value + 1;
                let upgrade_cost_future = game_contract.query("resourceInfo", (Token::String("c".to_string()), Token::Uint(U256::from(next_upgrade_level))), None, Options::default(), None);

                let upgrade_cost: Vec<U256> = upgrade_cost_future.await?;

                let wallet_iron_amount_future = iron_contract.query("balanceOf", Token::Address(wallet_address), None, Options::default(), None);
                let wallet_iron_amount: U256 = wallet_iron_amount_future.await.unwrap();
                let solar_amount_future = solar_contract.query("balanceOf", Token::Address(wallet_address), None, Options::default(), None);
                let solar_amount: U256 = solar_amount_future.await.unwrap();
                let crystal_amount_future = crystal_contract.query("balanceOf", Token::Address(wallet_address), None, Options::default(), None);
                let crystal_amount: U256 = crystal_amount_future.await.unwrap();

                if upgrade_cost.get(0).unwrap() <= &solar_amount && upgrade_cost.get(1).unwrap() <= &wallet_iron_amount && upgrade_cost.get(2).unwrap() <= &crystal_amount {
                    let nonce = web3.eth().transaction_count(wallet_address, Option::from(BlockNumber::Pending)).await.unwrap();
                    let u64_nonce = nonce.as_u64();
                    let level_up_structure = game_contract.abi().functions.get("levelUpStructure").unwrap().get(0).unwrap().encode_input([Token::String("c".to_string()), Token::Uint(planet_id)].as_ref()).unwrap();

                    let vec = level_up_structure.clone();
                    let bytes = Bytes::from(vec);
                    let estimated_gas_usage = get_gas_usage_estimation(wallet_address, gas_price, &web3, &game_contract, bytes).await;

                    let transaction = TransactionParameters {
                        nonce: Some(U256::from(u64_nonce)),
                        to: Some(game_contract.address()),
                        value: Default::default(),
                        gas_price: Some(gas_price),
                        gas: estimated_gas_usage,
                        data: Bytes::from(level_up_structure.clone()),
                        chain_id: Some(43114_u64),
                        transaction_type: None,
                        access_list: None,
                    };
                    let signed_tx = web3.accounts().sign_transaction(transaction, &_ppkey).await.unwrap();

                    let res = web3.eth().send_raw_transaction(Bytes::from(signed_tx.raw_transaction)).await?;

                    let mut tx_status = web3.eth().transaction_receipt(res).await?;

                    while !tx_status.is_some() {
                        println!("{:?} -- Level up crystal laboratory tx to level {} on planet {}", time::Instant::now(), next_upgrade_level, planet_id);
                        tx_status = web3.eth().transaction_receipt(res).await?;
                        let delay = time::Duration::from_secs(3);
                        thread::sleep(delay);
                    }

                    if tx_status.unwrap().status == Some(U64::from(0)) {
                        panic!("Transaction status -- failed");
                    }
                }

                println!("Cost for upgrading crystal laboratory for planet {} -- {:?}", planet_id, upgrade_cost);
            }
        }
    }

    Ok(())
}

async fn get_gas_usage_estimation(wallet_address: H160, mut gas_price: U256, web3: &Web3<WebSocket>, game_contract: &Contract<WebSocket>, bytes: Bytes) -> U256 {
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

async fn fetch_info(planet_contract: Contract<WebSocket>, game_contract: Contract<WebSocket>, iron_contract: Contract<WebSocket>, solar_contract: Contract<WebSocket>, crystal_contract: Contract<WebSocket>, planets_for_address: Vec<U256>, wallet_address: Address) -> Result<(), Box<dyn Error>> {
    let mut total_iron: f64 = 0.;
    let mut total_solar: f64 = 0.;
    let mut total_crystal: f64 = 0.;

    // List all pending resources
    for planet_id in planets_for_address {
        // DIsplay info about your planet
        let solar_amount_future = game_contract.query("getResourceAmount", (Token::Uint(U256::from(0)), Token::Uint(planet_id)), None, Options::default(), None);
        let iron_amount_future = game_contract.query("getResourceAmount", (Token::Uint(U256::from(1)), Token::Uint(planet_id)), None, Options::default(), None);
        let crystal_amount_future = game_contract.query("getResourceAmount", (Token::Uint(U256::from(2)), Token::Uint(planet_id)), None, Options::default(), None);
        let planet_uri_future = planet_contract.query("tokenURI", Token::Uint(planet_id), None, Options::default(), None);

        let planet_uri: String = planet_uri_future.await.unwrap();

        let mut response = reqwest::get(&planet_uri)?;
        let price_response: ResponseApi = response.json()?;

        let iron_amount: U256 = iron_amount_future.await.unwrap();
        let solar_amount: U256 = solar_amount_future.await.unwrap();
        let crystal_amount: U256 = crystal_amount_future.await.unwrap();

        let iron_amount_decimals = (iron_amount.as_u128() as f64
            / (10_u64.pow(18 as u32)) as f64)
            as f64;
        let solar_amount_decimals = (solar_amount.as_u128() as f64
            / (10_u64.pow(18 as u32)) as f64)
            as f64;
        let crystal_amount_decimals = (crystal_amount.as_u128() as f64
            / (10_u64.pow(18 as u32)) as f64)
            as f64;

        total_iron = total_iron.add(iron_amount_decimals);
        total_solar = total_solar.add(solar_amount_decimals);
        total_crystal = total_crystal.add(crystal_amount_decimals);

        println!("Planet {} has {} iron (mine lvl {}), {} solar (mine lvl {}) and {} crystal (mine lvl {})", price_response.name, iron_amount_decimals, price_response.attributes.attribute_1.value, solar_amount_decimals, price_response.attributes.attribute_0.value, crystal_amount_decimals, price_response.attributes.attribute_2.value);
    }

    println!("In Total you have {} iron, {} solar and {} crystal pending across your planetes", total_iron, total_solar, total_crystal);

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

    println!("In Total you have {} iron, {} solar and {} crystal in your wallet", iron_amount_decimals, solar_amount_decimals, crystal_amount_decimals);
    Ok(())
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

#[derive(Deserialize, Serialize, Debug, Clone)]
struct ResponseApi {
    planetNo: String,
    coordinate: String,
    description: String,
    external_url: String,
    image: String,
    name: String,
    attributes: Attributes,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
struct Attributes {
    #[serde(rename(deserialize = "0"))]
    attribute_0: Attribute,
    #[serde(rename(deserialize = "1"))]
    attribute_1: Attribute,
    #[serde(rename(deserialize = "2"))]
    attribute_2: Attribute,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
struct Attribute {
    trait_type: String,
    value: u32,
}
