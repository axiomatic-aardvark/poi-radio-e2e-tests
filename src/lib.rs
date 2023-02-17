use anyhow::anyhow;
use colored::*;
use ethers_contract::EthAbiType;
use ethers_core::types::transaction::eip712::Eip712;
use ethers_derive_eip712::*;
use num_bigint::BigUint;
use once_cell::sync::Lazy;
use once_cell::sync::OnceCell;
use prost::Message;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::{
    collections::HashMap,
    error::Error,
    sync::{Arc, Mutex},
};
use tokio::sync::Mutex as AsyncMutex;
use tracing::error;

use graphcast_sdk::{
    graphcast_agent::{
        message_typing::{get_indexer_stake, GraphcastMessage},
        GraphcastAgent,
    },
    graphql::{client_network::query_network_subgraph, client_registry::query_registry_indexer},
};

#[derive(Eip712, EthAbiType, Clone, Message, Serialize, Deserialize)]
#[eip712(
    name = "Graphcast POI Radio",
    version = "0",
    chain_id = 1,
    verifying_contract = "0xc944e90c64b2c07662a292be6244bdf05cda44a7"
)]
pub struct RadioPayloadMessage {
    #[prost(string, tag = "1")]
    pub identifier: String,
    #[prost(string, tag = "2")]
    pub content: String,
}

impl RadioPayloadMessage {
    pub fn new(identifier: String, content: String) -> Self {
        RadioPayloadMessage {
            identifier,
            content,
        }
    }

    pub fn payload_content(&self) -> String {
        self.content.clone()
    }
}

#[derive(Debug, Clone)]
pub struct Network {
    pub name: NetworkName,
    pub interval: u64,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct BlockPointer {
    pub hash: String,
    pub number: u64,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SubgraphStatus {
    pub network: String,
    pub block: BlockPointer,
}

pub struct BlockClock {
    pub current_block: u64,
    pub compare_block: u64,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum NetworkName {
    Goerli,
    Mainnet,
    Gnosis,
    Hardhat,
    ArbitrumOne,
    ArbitrumGoerli,
    Avalanche,
    Polygon,
    Celo,
    Optimism,
    Unknown,
}

impl NetworkName {
    pub fn from_string(name: &str) -> Self {
        match name {
            "goerli" => NetworkName::Goerli,
            "mainnet" => NetworkName::Mainnet,
            "gnosis" => NetworkName::Gnosis,
            "hardhat" => NetworkName::Hardhat,
            "arbitrum-one" => NetworkName::ArbitrumOne,
            "arbitrum-goerli" => NetworkName::ArbitrumGoerli,
            "avalanche" => NetworkName::Avalanche,
            "polygon" => NetworkName::Polygon,
            "celo" => NetworkName::Celo,
            "optimism" => NetworkName::Optimism,
            _ => NetworkName::Unknown,
        }
    }
}

impl fmt::Display for NetworkName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            NetworkName::Goerli => "goerli",
            NetworkName::Mainnet => "mainnet",
            NetworkName::Gnosis => "gnosis",
            NetworkName::Hardhat => "hardhat",
            NetworkName::ArbitrumOne => "arbitrum-one",
            NetworkName::ArbitrumGoerli => "arbitrum-goerli",
            NetworkName::Avalanche => "avalanche",
            NetworkName::Polygon => "polygon",
            NetworkName::Celo => "celo",
            NetworkName::Optimism => "optimism",
            NetworkName::Unknown => "unknown",
        };

        write!(f, "{name}")
    }
}

pub static NETWORKS: Lazy<Vec<Network>> = Lazy::new(|| {
    vec![
        Network {
            name: NetworkName::from_string("goerli"),
            interval: 2,
        },
        Network {
            name: NetworkName::from_string("mainnet"),
            interval: 4,
        },
        Network {
            name: NetworkName::from_string("gnosis"),
            interval: 5,
        },
        Network {
            name: NetworkName::from_string("hardhat"),
            interval: 5,
        },
        Network {
            name: NetworkName::from_string("arbitrum-one"),
            interval: 5,
        },
        Network {
            name: NetworkName::from_string("arbitrum-goerli"),
            interval: 5,
        },
        Network {
            name: NetworkName::from_string("avalanche"),
            interval: 5,
        },
        Network {
            name: NetworkName::from_string("polygon"),
            interval: 5,
        },
        Network {
            name: NetworkName::from_string("celo"),
            interval: 5,
        },
        Network {
            name: NetworkName::from_string("optimism"),
            interval: 5,
        },
    ]
});

pub type RemoteAttestationsMap = HashMap<String, HashMap<u64, Vec<Attestation>>>;
pub type LocalAttestationsMap = HashMap<String, HashMap<u64, Attestation>>;

/// A global static (singleton) instance of GraphcastAgent. It is useful to ensure that we have only one GraphcastAgent
/// per Radio instance, so that we can keep track of state and more easily test our Radio application.
pub static GRAPHCAST_AGENT: OnceCell<GraphcastAgent> = OnceCell::new();

/// A global static (singleton) instance of A GraphcastMessage vector.
/// It is used to save incoming messages after they've been validated, in order
/// defer their processing for later, because async code is required for the processing but
/// it is not allowed in the handler itself.
pub static MESSAGES: OnceCell<Arc<Mutex<Vec<(String, GraphcastMessage<RadioPayloadMessage>)>>>> =
    OnceCell::new();

/// Updates the `blocks` HashMap to include the new attestation.
pub fn update_blocks(
    block_number: u64,
    blocks: &HashMap<u64, Vec<Attestation>>,
    npoi: String,
    stake: BigUint,
    address: String,
) -> HashMap<u64, Vec<Attestation>> {
    let mut blocks_clone: HashMap<u64, Vec<Attestation>> = HashMap::new();
    blocks_clone.extend(blocks.clone());
    blocks_clone.insert(
        block_number,
        vec![Attestation::new(npoi, stake, vec![address])],
    );
    blocks_clone
}

/// Generate default topics that is operator address resolved to indexer address
/// and then its active on-chain allocations
pub async fn active_allocation_hashes(
    network_subgraph: &str,
    indexer_address: String,
) -> Result<Vec<String>, Box<dyn Error>> {
    Ok(
        query_network_subgraph(network_subgraph.to_string(), indexer_address.clone())
            .await?
            .indexer_allocations(),
    )
}

/// This function processes the global messages map that we populate when
/// messages are being received. It constructs the remote attestations
/// map and returns it if the processing succeeds.
pub async fn process_messages(
    messages: Arc<Mutex<Vec<(String, GraphcastMessage<RadioPayloadMessage>)>>>,
    registry_subgraph: &str,
    network_subgraph: &str,
) -> Result<RemoteAttestationsMap, anyhow::Error> {
    let mut remote_attestations: RemoteAttestationsMap = HashMap::new();
    let messages = AsyncMutex::new(messages.lock().unwrap());

    for (_, msg) in messages.lock().await.iter() {
        let radio_msg = &msg.payload.clone().unwrap();
        let sender = msg.recover_sender_address()?;
        let sender_stake = get_indexer_stake(
            query_registry_indexer(registry_subgraph.to_string(), sender.clone()).await?,
            network_subgraph,
        )
        .await?;

        // Check if there are existing attestations for the block
        let blocks = remote_attestations
            .entry(msg.identifier.to_string())
            .or_default();
        let attestations = blocks.entry(msg.block_number).or_default();

        let existing_attestation = attestations
            .iter_mut()
            .find(|a| a.npoi == radio_msg.payload_content());

        match existing_attestation {
            Some(existing_attestation) => {
                existing_attestation.stake_weight += sender_stake;
                if !existing_attestation.senders.contains(&sender) {
                    existing_attestation.senders.push(sender);
                }
            }
            None => {
                attestations.push(Attestation::new(
                    radio_msg.payload_content().to_string(),
                    sender_stake,
                    vec![sender],
                ));
            }
        }
    }
    Ok(remote_attestations)
}

/// A wrapper around an attested NPOI, tracks Indexers that have sent it plus their accumulated stake
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Attestation {
    pub npoi: String,
    pub stake_weight: BigUint,
    pub senders: Vec<String>,
}

impl Attestation {
    pub fn new(npoi: String, stake_weight: BigUint, senders: Vec<String>) -> Self {
        Attestation {
            npoi,
            stake_weight,
            senders,
        }
    }

    /// Used whenever we receive a new attestation for an NPOI that already exists in the store
    pub fn update(base: &Self, address: String, stake: BigUint) -> Result<Self, anyhow::Error> {
        if base.senders.contains(&address) {
            Err(anyhow!(
                "{}",
                "There is already an attestation from this address. Skipping..."
                    .to_string()
                    .yellow()
            ))
        } else {
            let senders = [base.senders.clone(), vec![address]].concat();
            Ok(Self::new(
                base.npoi.clone(),
                base.stake_weight.clone() + stake,
                senders,
            ))
        }
    }
}

/// Saves NPOIs that we've generated locally, in order to compare them with remote ones later
pub fn save_local_attestation(
    local_attestations: &mut LocalAttestationsMap,
    attestation: Attestation,
    ipfs_hash: String,
    block_number: u64,
) {
    let blocks = local_attestations.get(&ipfs_hash);

    match blocks {
        Some(blocks) => {
            let mut blocks_clone: HashMap<u64, Attestation> = HashMap::new();
            blocks_clone.extend(blocks.clone());
            blocks_clone.insert(block_number, attestation);
            local_attestations.insert(ipfs_hash, blocks_clone);
        }
        None => {
            let mut blocks_clone: HashMap<u64, Attestation> = HashMap::new();
            blocks_clone.insert(block_number, attestation);
            local_attestations.insert(ipfs_hash, blocks_clone);
        }
    }
}

/// Custom callback for handling the validated GraphcastMessage, in this case we only save the messages to a local store
/// to process them at a later time. This is required because for the processing we use async operations which are not allowed
/// in the handler.
pub fn attestation_handler() -> impl Fn(Result<GraphcastMessage<RadioPayloadMessage>, anyhow::Error>)
{
    |msg: Result<GraphcastMessage<RadioPayloadMessage>, anyhow::Error>| match msg {
        Ok(msg) => {
            let sender = msg.recover_sender_address().unwrap();

            MESSAGES.get().unwrap().lock().unwrap().push((sender, msg));
        }
        Err(err) => {
            error!("{}", err);
        }
    }
}

pub enum CompareError {
    Critical(String),
    NonCritical(String),
}

impl std::fmt::Display for CompareError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            CompareError::Critical(msg) => write!(f, "Critical error: {}", msg),
            CompareError::NonCritical(msg) => write!(f, "Non-critical error: {}", msg),
        }
    }
}

/// Compares local attestations against remote ones using the attestation stores we populated while processing saved GraphcastMessage messages.
/// It takes our attestation (NPOI) for a given subgraph on a given block and compares it to the top-attested one from the remote attestations.
/// The top remote attestation is found by grouping attestations together and increasing their total stake-weight every time we see a new message
/// with the same NPOI from an Indexer (NOTE: one Indexer can only send 1 attestation per subgraph per block). The attestations are then sorted
/// and we take the one with the highest total stake-weight.
pub fn compare_attestations(
    attestation_block: u64,
    remote: RemoteAttestationsMap,
    local: Arc<Mutex<LocalAttestationsMap>>,
) -> Result<String, CompareError> {
    let local = local.lock().unwrap();

    // Iterate & compare
    if let Some((ipfs_hash, blocks)) = local.iter().next() {
        let attestations = blocks.get(&attestation_block);
        match attestations {
            Some(local_attestation) => {
                let remote_blocks = remote.get(ipfs_hash);
                match remote_blocks {
                    Some(remote_blocks) => {
                        let remote_attestations = remote_blocks.get(&attestation_block);

                        match remote_attestations {
                            Some(remote_attestations) => {
                                let mut remote_attestations = remote_attestations.clone();

                        remote_attestations
                        .sort_by(|a, b| a.stake_weight.partial_cmp(&b.stake_weight).unwrap());

                    let most_attested_npoi = &remote_attestations.last().unwrap().npoi;
                    if most_attested_npoi == &local_attestation.npoi {
                        return Ok(format!(
                            "POIs match for subgraph {ipfs_hash} on block {attestation_block}!"
                        ));
                    } else {
                        return Err(CompareError::Critical(format!(
                            "POIs don't match for subgraph {ipfs_hash} on block {attestation_block}!"
                        )
                        .red()
                        .bold().to_string()));
                    }
                            },
                            None => {
                                return Err(CompareError::NonCritical(format!(
                                    "No record for subgraph {ipfs_hash} on block {attestation_block} found in remote attestations"
                                )
                                .yellow().to_string()
                               ));
                            }
                        }
                    }
                    None => {
                        return Err(CompareError::NonCritical(format!("No attestations for subgraph {ipfs_hash} on block {attestation_block} found in remote attestations store. Continuing...", ).yellow().to_string()))
                    }
                }
            }
            None => {
                return Err(CompareError::NonCritical(format!("No attestation for subgraph {ipfs_hash} on block {attestation_block} found in local attestations store. Continuing...", ).yellow().to_string()))
            }
        }
    }

    Err(CompareError::NonCritical(
            "The comparison did not execute successfully for on block {attestation_block}. Continuing...".yellow().to_string(),
        )
    )
}
