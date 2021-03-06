mod utils;

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
use crate::utils::{get_web3, instantiate_contract, ResponseApi, get_gas_usage_estimation, get_current_nonce, fetch_current_resources};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();

    // START SETUP CONFIG FROM CMD ARG
    let mut harvest_mode = false;
    let mut upgrade_mode = false;
    let mut fetch_info_mode = false;
    let mut upgrade_solar = false;
    let mut upgrade_mine = false;
    let mut upgrade_crystal = false;
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
            upgrade_solar = bool::from_str(args.get(6).unwrap())?;
            upgrade_mine = bool::from_str(args.get(7).unwrap())?;
            upgrade_crystal = bool::from_str(args.get(8).unwrap())?;
        } else if cmd.eq_ignore_ascii_case("fetchInfo") {
            fetch_info_mode = true;
        }
    }
    // END SETUP CONFIG FROM CMD ARG

    // Get Web3 Instance
    let web3 = get_web3("wss://api.avax.network/ext/bc/C/ws").await;

    // START INSTANTIATION OF ALL CONTRACTS WE WILL USE
    let planet_contract = instantiate_contract(&web3, &Address::from_str("0x0C3b29321611736341609022C23E981AC56E7f96").unwrap(), "abi/novax_planet.abi").await;
    let game_contract = instantiate_contract(&web3, &Address::from_str("0x2aa2a9ef24a209f47f42Cb97Bd19D881e33F3956").unwrap(), "abi/novax_game.abi").await;
    let metal_contract = instantiate_contract(&web3, &Address::from_str("0x4C1057455747e3eE5871D374FdD77A304cE10989").unwrap(), "abi/erc20.abi").await;
    let solar_contract = instantiate_contract(&web3, &Address::from_str("0xE6eE049183B474ecf7704da3F6F555a1dCAF240F").unwrap(), "abi/erc20.abi").await;
    let crystal_contract = instantiate_contract(&web3, &Address::from_str("0x70b4aE8eb7bd572Fc0eb244Cd8021066b3Ce7EE4").unwrap(), "abi/erc20.abi").await;
    // END INSTANTIATION OF ALL CONTRACTS

    // We fetch the planets owned by the address we passed as first argument
    let planets_for_address_future =
        planet_contract.query("tokensOfOwner", Token::Address(wallet_address), None, Options::default(), None);

    let planets_for_address: Vec<U256> = planets_for_address_future.await.unwrap();

    // Now we trigger the 'command' the user selected.
    if fetch_info_mode {
        fetch_info(planet_contract, game_contract, metal_contract, solar_contract, crystal_contract, planets_for_address, wallet_address).await;
    } else if harvest_mode {
        harvest_all(wallet_address, _ppkey, gas_price, &web3, &game_contract, planets_for_address).await?
    } else if upgrade_mode {
        upgrade_buildings(upgrade_solar, upgrade_mine, upgrade_crystal, threshold, wallet_address, _ppkey, gas_price, &web3, planet_contract, &game_contract, &metal_contract, &solar_contract, &crystal_contract, planets_for_address).await?
    }

    Ok(())
}

async fn upgrade_buildings(upgrade_solar: bool, upgrade_mine: bool, upgrade_crystal: bool, threshold: u32, wallet_address: H160, _ppkey: SecretKey, gas_price: U256, web3: &Web3<WebSocket>, planet_contract: Contract<WebSocket>, game_contract: &Contract<WebSocket>, metal_contract: &Contract<WebSocket>, solar_contract: &Contract<WebSocket>, crystal_contract: &Contract<WebSocket>, planets_for_address: Vec<U256>) -> Result<(), Box<dyn Error>> {
    for planet_id in planets_for_address {
        let planet_uri_future = planet_contract.query("tokenURI", Token::Uint(planet_id), None, Options::default(), None);
        let planet_uri: String = planet_uri_future.await.unwrap();

        let mut response = reqwest::get(&planet_uri)?;
        let price_response: ResponseApi = response.json()?;

        // 0 is for Solar Panel
        if upgrade_solar && price_response.attributes.attribute_0.value < threshold {
            let next_upgrade_level = price_response.attributes.attribute_0.value + 1;
            let upgrade_cost_future = game_contract.query("resourceInfo", (Token::String("s".to_string()), Token::Uint(U256::from(next_upgrade_level))), None, Options::default(), None);

            let mut upgrade_cost: Vec<U256> = upgrade_cost_future.await?;

            let (wallet_metal_amount, solar_amount, crystal_amount, metal_amount_decimals, solar_amount_decimals, crystal_amount_decimals, upgrade_metal_amount_decimals, upgrade_solar_amount_decimals, upgrade_crystal_amount_decimals) = fetch_current_resources(wallet_address, &metal_contract, &solar_contract, &crystal_contract, &mut upgrade_cost).await;

            if upgrade_cost.get(0).unwrap() <= &solar_amount && upgrade_cost.get(1).unwrap() <= &wallet_metal_amount && upgrade_cost.get(2).unwrap() <= &crystal_amount {
                let u64_nonce = get_current_nonce(wallet_address, &web3).await;

                let level_up_structure = game_contract.abi().functions.get("levelUpStructure").unwrap().get(0).unwrap().encode_input([Token::String("s".to_string()), Token::Uint(planet_id)].as_ref()).unwrap();

                let vec = level_up_structure.clone();
                let bytes = Bytes::from(vec);
                let estimated_gas_usage = get_gas_usage_estimation(wallet_address, gas_price, &web3, &game_contract, &bytes).await;

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
            } else {
                println!("We don't have enough resources to perform this upgrade");
                println!("We would need {:?} s / {:?} m / {:?} c but only have {:?} s / {:?} m / {:?} c", upgrade_solar_amount_decimals, upgrade_metal_amount_decimals, upgrade_crystal_amount_decimals, solar_amount_decimals, metal_amount_decimals, crystal_amount_decimals);
            }

            println!("Cost for upgrading solar panel for planet {} -- {:?}", planet_id, upgrade_cost);
        } else {
            println!("Solar panels on this planet {} are already at the wanted level", planet_id);
        }

        if upgrade_mine && price_response.attributes.attribute_1.value < threshold {
            let next_upgrade_level = price_response.attributes.attribute_1.value + 1;
            let upgrade_cost_future = game_contract.query("resourceInfo", (Token::String("m".to_string()), Token::Uint(U256::from(next_upgrade_level))), None, Options::default(), None);

            let upgrade_cost: Vec<U256> = upgrade_cost_future.await?;

            let (wallet_metal_amount, solar_amount, crystal_amount, metal_amount_decimals, solar_amount_decimals, crystal_amount_decimals, upgrade_metal_amount_decimals, upgrade_solar_amount_decimals, upgrade_crystal_amount_decimals) = fetch_current_resources(wallet_address, &metal_contract, &solar_contract, &crystal_contract, &upgrade_cost).await;

            if upgrade_cost.get(0).unwrap() <= &solar_amount && upgrade_cost.get(1).unwrap() <= &wallet_metal_amount && upgrade_cost.get(2).unwrap() <= &crystal_amount {
                let u64_nonce = get_current_nonce(wallet_address, &web3).await;

                let level_up_structure = game_contract.abi().functions.get("levelUpStructure").unwrap().get(0).unwrap().encode_input([Token::String("m".to_string()), Token::Uint(planet_id)].as_ref()).unwrap();

                let vec = level_up_structure.clone();
                let bytes = Bytes::from(vec);
                let estimated_gas_usage = get_gas_usage_estimation(wallet_address, gas_price, &web3, &game_contract, &bytes).await;

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
                    println!("{:?} -- Level up metal mine tx to level {} on planet {}", time::Instant::now(), next_upgrade_level, planet_id);
                    tx_status = web3.eth().transaction_receipt(res).await?;
                    let delay = time::Duration::from_secs(3);
                    thread::sleep(delay);
                }

                if tx_status.unwrap().status == Some(U64::from(0)) {
                    panic!("Transaction status -- failed");
                }
            } else {
                println!("We don't have enough resources to perform this upgrade");
                println!("We would need {:?} s / {:?} m / {:?} c but only have {:?} s / {:?} m / {:?} c", upgrade_solar_amount_decimals, upgrade_metal_amount_decimals, upgrade_crystal_amount_decimals, solar_amount_decimals, metal_amount_decimals, crystal_amount_decimals);
            }

            println!("Cost for upgrading metal mine for planet {} -- {:?}", planet_id, upgrade_cost);
        } else {
            println!("Metal mine on this planet {} is already at the wanted level", planet_id);
        }

        if upgrade_crystal && price_response.attributes.attribute_2.value < threshold {
            let next_upgrade_level = price_response.attributes.attribute_2.value + 1;
            let upgrade_cost_future = game_contract.query("resourceInfo", (Token::String("c".to_string()), Token::Uint(U256::from(next_upgrade_level))), None, Options::default(), None);

            let upgrade_cost: Vec<U256> = upgrade_cost_future.await?;

            let (wallet_metal_amount, solar_amount, crystal_amount, metal_amount_decimals, solar_amount_decimals, crystal_amount_decimals, upgrade_metal_amount_decimals, upgrade_solar_amount_decimals, upgrade_crystal_amount_decimals) = fetch_current_resources(wallet_address, &metal_contract, &solar_contract, &crystal_contract, &upgrade_cost).await;

            if upgrade_cost.get(0).unwrap() <= &solar_amount && upgrade_cost.get(1).unwrap() <= &wallet_metal_amount && upgrade_cost.get(2).unwrap() <= &crystal_amount {
                let u64_nonce = get_current_nonce(wallet_address, &web3).await;

                let level_up_structure = game_contract.abi().functions.get("levelUpStructure").unwrap().get(0).unwrap().encode_input([Token::String("c".to_string()), Token::Uint(planet_id)].as_ref()).unwrap();

                let vec = level_up_structure.clone();
                let bytes = Bytes::from(vec);
                let estimated_gas_usage = get_gas_usage_estimation(wallet_address, gas_price, &web3, &game_contract, &bytes).await;

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
            } else {
                println!("We don't have enough resources to perform this upgrade");
                println!("We would need {:?} s / {:?} m / {:?} c but only have {:?} s / {:?} m / {:?} c", upgrade_solar_amount_decimals, upgrade_metal_amount_decimals, upgrade_crystal_amount_decimals, solar_amount_decimals, metal_amount_decimals, crystal_amount_decimals);
            }

            println!("Cost for upgrading crystal laboratory for planet {} -- {:?}", planet_id, upgrade_cost);
        } else {
            println!("Crystal Laboratory on this planet {} is already at the wanted level", planet_id);
        }
    }
    Ok(())
}

async fn harvest_all(wallet_address: H160, _ppkey: SecretKey, gas_price: U256, web3: &Web3<WebSocket>, game_contract: &Contract<WebSocket>, planets_for_address: Vec<U256>) -> Result<(), Box<dyn Error>> {
    let u64_nonce = get_current_nonce(wallet_address, &web3).await;
    let mut tokens_array_planets_id: Vec<Token> = Vec::new();

    for planet_id in planets_for_address {
        tokens_array_planets_id.push(Token::Uint(planet_id));
    }

    let harvest_all = game_contract.abi().functions.get("harvestAll").unwrap().get(0).unwrap().encode_input([Token::Array(tokens_array_planets_id)].as_ref()).unwrap();

    let vec = harvest_all.clone();
    let bytes = Bytes::from(vec);
    let estimated_gas_usage = get_gas_usage_estimation(wallet_address, gas_price, &web3, &game_contract, &bytes).await;

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

    Ok(())
}

async fn fetch_info(planet_contract: Contract<WebSocket>, game_contract: Contract<WebSocket>, metal_contract: Contract<WebSocket>, solar_contract: Contract<WebSocket>, crystal_contract: Contract<WebSocket>, planets_for_address: Vec<U256>, wallet_address: Address) -> Result<(), Box<dyn Error>> {
    let mut total_metal: f64 = 0.;
    let mut total_metal_sec: f64 = 0.;
    let mut total_metal_min: f64 = 0.;
    let mut total_metal_hour: f64 = 0.;
    let mut total_metal_day: f64 = 0.;
    let mut total_solar: f64 = 0.;
    let mut total_solar_sec: f64 = 0.;
    let mut total_solar_min: f64 = 0.;
    let mut total_solar_hour: f64 = 0.;
    let mut total_solar_day: f64 = 0.;
    let mut total_crystal: f64 = 0.;
    let mut total_crystal_sec: f64 = 0.;
    let mut total_crystal_min: f64 = 0.;
    let mut total_crystal_hour: f64 = 0.;
    let mut total_crystal_day: f64 = 0.;

    // We iterate over the planets id list owned by the user.
    for planet_id in planets_for_address {
        // For this planet_id we query the pendin amount of solar / metal / crystal and the URI containing the metadata.
        let solar_amount_future = game_contract.query("getResourceAmount", (Token::Uint(U256::from(0)), Token::Uint(planet_id)), None, Options::default(), None);
        let metal_amount_future = game_contract.query("getResourceAmount", (Token::Uint(U256::from(1)), Token::Uint(planet_id)), None, Options::default(), None);
        let crystal_amount_future = game_contract.query("getResourceAmount", (Token::Uint(U256::from(2)), Token::Uint(planet_id)), None, Options::default(), None);
        let planet_uri_future = planet_contract.query("tokenURI", Token::Uint(planet_id), None, Options::default(), None);

        let planet_uri: String = planet_uri_future.await.unwrap();

        // We make a HTTP GET request to the URL containing the metadata.
        let mut response = reqwest::get(&planet_uri)?;
        let price_response: ResponseApi = response.json()?;


        let metal_amount: U256 = metal_amount_future.await.unwrap();
        let solar_amount: U256 = solar_amount_future.await.unwrap();
        let crystal_amount: U256 = crystal_amount_future.await.unwrap();

        let metal_amount_decimals = (metal_amount.as_u128() as f64
            / (10_u64.pow(18 as u32)) as f64)
            as f64;
        let solar_amount_decimals = (solar_amount.as_u128() as f64
            / (10_u64.pow(18 as u32)) as f64)
            as f64;
        let crystal_amount_decimals = (crystal_amount.as_u128() as f64
            / (10_u64.pow(18 as u32)) as f64)
            as f64;

        // We add the amount of 'pending' resource of this planet to the total amount of pending resources across ALL planets.
        total_metal = total_metal.add(metal_amount_decimals);
        total_solar = total_solar.add(solar_amount_decimals);
        total_crystal = total_crystal.add(crystal_amount_decimals);

        total_crystal_sec = total_crystal_sec + (1. * price_response.attributes.attribute_0.value as f64 * 0.0001);
        total_crystal_min = total_crystal_min + (60. * price_response.attributes.attribute_0.value as f64 * 0.0001);
        total_crystal_hour = total_crystal_hour + (3600. * price_response.attributes.attribute_0.value as f64 * 0.0001);
        total_crystal_day = total_crystal_day + (86400. * price_response.attributes.attribute_0.value as f64 * 0.0001);

        total_metal_sec = total_metal_sec + (1. * price_response.attributes.attribute_0.value as f64 * 0.002);
        total_metal_min = total_metal_min + (60. * price_response.attributes.attribute_0.value as f64 * 0.002);
        total_metal_hour = total_metal_hour + (3600. * price_response.attributes.attribute_0.value as f64 * 0.002);
        total_metal_day = total_metal_day + (86400. * price_response.attributes.attribute_0.value as f64 * 0.002);

        total_solar_sec = total_solar_sec + (1. * price_response.attributes.attribute_0.value as f64 * 0.001);
        total_solar_min = total_solar_min + (60. * price_response.attributes.attribute_0.value as f64 * 0.001);
        total_solar_hour = total_solar_hour + (3600. * price_response.attributes.attribute_0.value as f64 * 0.001);
        total_solar_day = total_solar_day + (86400. * price_response.attributes.attribute_0.value as f64 * 0.001);

        println!("Planet {} has {} metal (mine lvl {}), {} solar (mine lvl {}) and {} crystal (mine lvl {})", price_response.name, metal_amount_decimals, price_response.attributes.attribute_1.value, solar_amount_decimals, price_response.attributes.attribute_0.value, crystal_amount_decimals, price_response.attributes.attribute_2.value);
    }

    println!("In total you have {} metal, {} solar and {} crystal pending across your planetes", total_metal, total_solar, total_crystal);

    println!("In total you produce {} c/s || {} c/m || {} c/h || {} c/d across all your planets", total_crystal_sec, total_crystal_min, total_crystal_hour, total_crystal_day);
    println!("In total you produce {} m/s || {} m/m || {} m/h || {} m/d across all your planets", total_metal_sec, total_metal_min, total_metal_hour, total_metal_day);
    println!("In total you produce {} s/s || {} s/m || {} s/h || {} s/d across all your planets", total_solar_sec, total_solar_min, total_solar_hour, total_solar_day);

    // Here we query the current owned amount of each resource (they are ERC20) for the user.
    let wallet_metal_amount_future = metal_contract.query("balanceOf", Token::Address(wallet_address), None, Options::default(), None);
    let wallet_metal_amount: U256 = wallet_metal_amount_future.await.unwrap();
    let solar_amount_future = solar_contract.query("balanceOf", Token::Address(wallet_address), None, Options::default(), None);
    let solar_amount: U256 = solar_amount_future.await.unwrap();
    let crystal_amount_future = crystal_contract.query("balanceOf", Token::Address(wallet_address), None, Options::default(), None);
    let crystal_amount: U256 = crystal_amount_future.await.unwrap();

    let metal_amount_decimals = (wallet_metal_amount.as_u128() as f64
        / (10_u64.pow(18 as u32)) as f64)
        as f64;
    let solar_amount_decimals = (solar_amount.as_u128() as f64
        / (10_u64.pow(18 as u32)) as f64)
        as f64;
    let crystal_amount_decimals = (crystal_amount.as_u128() as f64
        / (10_u64.pow(18 as u32)) as f64)
        as f64;

    println!("In Total you have {} metal, {} solar and {} crystal in your wallet + pending resources", (metal_amount_decimals + total_metal), (solar_amount_decimals + total_solar), (crystal_amount_decimals + total_crystal));
    Ok(())
}




