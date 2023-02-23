use colored::*;
use ethers::signers::LocalWallet;
/// Radio specific query function to fetch Proof of Indexing for each allocated subgraph
use graphcast_sdk::graphcast_agent::GraphcastAgent;
use graphcast_sdk::graphql::client_network::query_network_subgraph;
use graphcast_sdk::graphql::client_registry::query_registry_indexer;
use graphcast_sdk::{graphcast_id_address, read_boot_node_addresses, NetworkName};
use hex::encode;
use num_bigint::BigUint;
use num_traits::Zero;
use partial_application::partial;
use poi_radio_e2e_tests::{
    attestation_handler, compare_attestations, process_messages, save_local_attestation,
    Attestation, BlockClock, BlockPointer, CompareError, LocalAttestationsMap, MessagesArc,
    RadioPayloadMessage, GRAPHCAST_AGENT, MESSAGES, NETWORKS,
};
use rand::{thread_rng, Rng};
use secp256k1::SecretKey;
use std::collections::HashMap;
use std::env;
use std::sync::{Arc, Mutex};
use std::{thread::sleep, time::Duration};
use tracing::log::warn;
use tracing::{debug, error, info};

use crate::graphql::{query_graph_node_poi, update_network_chainheads};
use crate::setup::constants::{MOCK_SUBGRAPH_GOERLI, MOCK_SUBGRAPH_MAINNET};
use crate::setup::utils::{
    empty_attestation_handler, generate_random_address, get_random_port, setup_mock_env_vars,
    setup_mock_server,
};

use super::utils::RadioRuntimeConfig;

pub async fn run_test_radio<F>(config: &RadioRuntimeConfig, success_handler: F)
where
    F: Fn(MessagesArc),
{
    let mut block_number = 0;
    let indexer_address = config
        .indexer_address
        .clone()
        .unwrap_or(generate_random_address());

    let graphcast_id = config
        .operator_address
        .clone()
        .unwrap_or(generate_random_address());

    let mock_server_uri = setup_mock_server(
        block_number,
        &indexer_address,
        &graphcast_id,
        &config.subgraphs.clone().unwrap_or(vec![
            MOCK_SUBGRAPH_MAINNET.to_string(),
            MOCK_SUBGRAPH_GOERLI.to_string(),
        ]),
        &config.indexer_stake,
        &config.poi,
    )
    .await;
    setup_mock_env_vars(&mock_server_uri);

    let private_key = env::var("PRIVATE_KEY").expect("No private key provided.");
    let registry_subgraph =
        env::var("REGISTRY_SUBGRAPH_ENDPOINT").expect("No registry subgraph endpoint provided.");
    let network_subgraph =
        env::var("NETWORK_SUBGRAPH_ENDPOINT").expect("No network subgraph endpoint provided.");
    let graph_node_endpoint =
        env::var("GRAPH_NODE_STATUS_ENDPOINT").expect("No Graph node status endpoint provided.");

    // Send message every x blocks for which wait y blocks before attestations
    let wait_block_duration = 2;

    let wallet = private_key.parse::<LocalWallet>().unwrap();
    let mut rng = thread_rng();
    let mut private_key = [0u8; 32];
    rng.fill(&mut private_key[..]);

    let private_key = SecretKey::from_slice(&private_key).expect("Error parsing secret key");
    let private_key_hex = encode(private_key.secret_bytes());
    env::set_var("PRIVATE_KEY", &private_key_hex);

    let private_key = env::var("PRIVATE_KEY").unwrap();

    // TODO: Add something random and unique here to avoid noise form other operators
    let radio_name: &str = "test-poi-radio";

    let my_address =
        query_registry_indexer(registry_subgraph.to_string(), graphcast_id_address(&wallet))
            .await
            .ok();

    let graphcast_agent = GraphcastAgent::new(
        private_key,
        radio_name,
        &registry_subgraph,
        &network_subgraph,
        &graph_node_endpoint,
        read_boot_node_addresses(),
        Some("5"),
        config.subgraphs.clone().unwrap_or(vec![
            MOCK_SUBGRAPH_MAINNET.to_string(),
            MOCK_SUBGRAPH_GOERLI.to_string(),
        ]),
        None,
        None,
        Some(get_random_port()),
        None,
    )
    .await
    .unwrap();

    _ = GRAPHCAST_AGENT.set(graphcast_agent);
    _ = MESSAGES.set(Arc::new(Mutex::new(vec![])));

    if config.is_setup_instance {
        GRAPHCAST_AGENT
            .get()
            .unwrap()
            .register_handler(Arc::new(Mutex::new(empty_attestation_handler())))
            .expect("Could not register handler");
    } else {
        GRAPHCAST_AGENT
            .get()
            .unwrap()
            .register_handler(Arc::new(Mutex::new(attestation_handler())))
            .expect("Could not register handler");
    };

    let mut block_store: HashMap<NetworkName, BlockClock> = HashMap::new();
    let mut network_chainhead_blocks: HashMap<NetworkName, BlockPointer> = HashMap::new();
    let local_attestations: Arc<Mutex<LocalAttestationsMap>> = Arc::new(Mutex::new(HashMap::new()));

    let my_stake = if let Some(addr) = my_address.clone() {
        query_network_subgraph(network_subgraph.to_string(), addr)
            .await
            .unwrap()
            .indexer_stake()
    } else {
        BigUint::zero()
    };
    info!(
        "Acting on behave of indexer {:#?} with stake {}",
        my_address, my_stake
    );

    // Main loop for sending messages, can factor out
    // and take radio specific query and parsing for radioPayload
    loop {
        // Update all the chainheads of the network
        // Also get a hash map returned on the subgraph mapped to network name and latest block
        let subgraph_network_latest_blocks = match update_network_chainheads(
            graph_node_endpoint.clone(),
            &mut network_chainhead_blocks,
        )
        .await
        {
            Ok(res) => res,
            Err(e) => {
                error!("Could not query indexing statuses, pull again later: {e}");
                continue;
            }
        };
        debug!(
            "Subgraph network and latest blocks: {:#?}\nNetwork chainhead: {:#?}",
            subgraph_network_latest_blocks, network_chainhead_blocks
        );
        //TODO: check that if no networks had an new message update blocks, sleep for a few seconds and 'continue'

        // Radio specific message content query function
        // Function takes in an identifier string and make specific queries regarding the identifier
        // The example here combines a single function provided query endpoint, current block info based on the subgraph's indexing network
        // Then the function gets sent to agent for making identifier independent queries
        let identifiers = GRAPHCAST_AGENT.get().unwrap().content_identifiers();

        info!("debugging with style {:?}", subgraph_network_latest_blocks);

        for id in identifiers {
            // Get the indexing network of the deployment
            // and update the NETWORK message block
            let (network_name, latest_block) = match subgraph_network_latest_blocks.get(&id.clone())
            {
                Some(network_block) => (
                    NetworkName::from_string(&network_block.network.clone()),
                    network_block.block.clone(),
                ),
                None => {
                    error!("Could not query the subgraph's indexing network, check Graph node's indexing statuses of subgraph deployment {}", id.clone());
                    continue;
                }
            };

            // Get the examination frequency of the network
            let examination_frequency = match NETWORKS
                .iter()
                .find(|n| n.name.to_string() == network_name.to_string())
            {
                Some(n) => n.interval,
                None => {
                    warn!("Subgraph is indexing an unsupported network, please report an issue on https://github.com/graphops/graphcast-rs");
                    continue;
                }
            };

            // Calculate the block to send message about
            let message_block = match network_chainhead_blocks.get(&network_name) {
                Some(BlockPointer { hash: _, number }) => number - number % examination_frequency,
                None => {
                    error!(
                        "Could not get the chainhead block number on network {} and cannot determine the block to send message about",
                        network_name.to_string(),
                    );
                    continue;
                }
            };

            let block_clock = block_store
                .entry(network_name)
                .or_insert_with(|| BlockClock {
                    current_block: 0,
                    compare_block: 0,
                });

            // Wait a bit before querying information on the current block
            if block_clock.current_block == message_block {
                if block_number < 20 {
                    block_number += 1;
                } else {
                    block_number = 0;
                    MESSAGES.get().unwrap().lock().unwrap().clear()
                }
                setup_mock_server(
                    block_number,
                    &indexer_address,
                    &graphcast_id,
                    &config.subgraphs.clone().unwrap_or(vec![
                        MOCK_SUBGRAPH_MAINNET.to_string(),
                        MOCK_SUBGRAPH_GOERLI.to_string(),
                    ]),
                    &config.indexer_stake,
                    &config.poi,
                )
                .await;
                sleep(Duration::from_secs(5));
                continue;
            }

            debug!(
                "{} {} {} {} {} {} {} {}",
                "🔗 Message block: ".cyan(),
                message_block,
                "🔗 Current block from block clock: ".cyan(),
                block_clock.current_block,
                "🔗 Latest block: ".cyan(),
                latest_block.number,
                "🔗 Compare block: ".cyan(),
                block_clock.compare_block
            );

            block_clock.current_block = latest_block.number;

            if block_clock.compare_block != 0 && latest_block.number >= block_clock.compare_block {
                debug!("{}", "Comparing attestations".magenta());

                debug!("{}{:?}", "Messages: ".magenta(), MESSAGES);

                let remote_attestations = process_messages(
                    Arc::clone(MESSAGES.get().unwrap()),
                    &format!("{}{}", &mock_server_uri, "/graphcast-registry"),
                    &network_subgraph,
                )
                .await;
                match remote_attestations {
                    Ok(remote_attestations) => {
                        success_handler(Arc::clone(MESSAGES.get().unwrap()));

                        match compare_attestations(
                            block_clock.compare_block - wait_block_duration,
                            remote_attestations,
                            Arc::clone(&local_attestations),
                        ) {
                            Ok(msg) => {
                                debug!("{}", msg.green().bold());
                            }
                            Err(err) => match err {
                                CompareError::Critical(_) => {
                                    if config.panic_if_poi_diverged {
                                        panic!("{}", err);
                                    } else {
                                        error!("{}", err);
                                    }
                                }
                                CompareError::NonCritical(_) => {
                                    error!("{}", err);
                                }
                            },
                        }
                    }
                    Err(err) => {
                        error!(
                            "{}{}",
                            "An error occured while parsing messages: {}".red().bold(),
                            err
                        );
                    }
                }
            }

            let poi_query =
                partial!( query_graph_node_poi => graph_node_endpoint.clone(), id.clone(), _, _);

            debug!(
                "Checking latest block number and the message block: {0} >?= {message_block}",
                latest_block.number
            );
            if latest_block.number >= message_block {
                block_clock.compare_block = message_block + wait_block_duration;
                let block_hash = match GRAPHCAST_AGENT
                    .get()
                    .unwrap()
                    .get_block_hash(network_name.to_string(), message_block)
                    .await
                {
                    Ok(hash) => hash,
                    Err(e) => {
                        error!("Failed to query graph node for the block hash: {e}");
                        continue;
                    }
                };

                match poi_query(block_hash.clone(), message_block.try_into().unwrap()).await {
                    Ok(content) => {
                        let attestation = Attestation {
                            npoi: content.clone(),
                            stake_weight: my_stake.clone(),
                            senders: Vec::new(),
                        };

                        save_local_attestation(
                            &mut local_attestations.lock().unwrap(),
                            attestation,
                            id.clone(),
                            message_block,
                        );

                        let radio_message = RadioPayloadMessage::new(id.clone(), content.clone());
                        info!(
                            "{}: {:?}",
                            "Attempting to send message".magenta(),
                            radio_message
                        );

                        match GRAPHCAST_AGENT
                            .get()
                            .unwrap()
                            .send_message(
                                id.clone(),
                                network_name,
                                message_block,
                                Some(radio_message),
                            )
                            .await
                        {
                            Ok(sent) => {
                                info!("{}: {}", "Sent message id".green(), sent);
                            }
                            Err(e) => error!("{}: {}", "Failed to send message".red(), e),
                        };
                    }
                    Err(e) => error!("{}: {}", "Failed to query message".red(), e),
                }
            }
        }

        if block_number < 20 {
            block_number += 1;
        } else {
            block_number = 0;
            MESSAGES.get().unwrap().lock().unwrap().clear()
        }
        setup_mock_server(
            block_number,
            &indexer_address,
            &graphcast_id,
            &config.subgraphs.clone().unwrap_or(vec![
                MOCK_SUBGRAPH_MAINNET.to_string(),
                MOCK_SUBGRAPH_GOERLI.to_string(),
            ]),
            &config.indexer_stake,
            &config.poi,
        )
        .await;
        sleep(Duration::from_secs(5));
        continue;
    }
}
