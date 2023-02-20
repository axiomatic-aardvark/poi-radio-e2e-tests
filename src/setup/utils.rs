use graphcast_sdk::graphcast_agent::message_typing::GraphcastMessage;
use poi_radio_e2e_tests::RadioPayloadMessage;
use rand::{thread_rng, Rng};
use secp256k1::SecretKey;
use sha3::{Digest, Keccak256};
use std::{env, net::TcpListener};
use tracing::{debug, error, info};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

pub fn generate_random_address() -> String {
    let mut rng = thread_rng();
    let mut private_key = [0u8; 32];
    rng.fill(&mut private_key[..]);

    let private_key = SecretKey::from_slice(&private_key).expect("Error parsing secret key");

    let public_key =
        secp256k1::PublicKey::from_secret_key(&secp256k1::Secp256k1::new(), &private_key)
            .serialize_uncompressed();

    let address_bytes = &Keccak256::digest(&public_key[1..])[12..];

    info!("random address: {}", hex::encode(address_bytes));
    format!("0x{}", hex::encode(address_bytes))
}

pub fn empty_attestation_handler(
) -> impl Fn(Result<GraphcastMessage<RadioPayloadMessage>, anyhow::Error>) {
    |msg: Result<GraphcastMessage<RadioPayloadMessage>, anyhow::Error>| match msg {
        Ok(msg) => {
            debug!("Message received: {:?}", msg);
            debug!("This is a setup instance. Continuing...");
        }
        Err(err) => {
            error!("{}", err);
        }
    }
}

pub fn get_random_port() -> String {
    let listener = TcpListener::bind("localhost:0").unwrap();
    let port = listener.local_addr().unwrap().port().to_string();
    debug!("Random port: {}", port);

    port
}

pub async fn setup_mock_server(block_number: u64, address: &String) -> String {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/graphcast-registry"))
        .respond_with(ResponseTemplate::new(200).set_body_string(format!(
            r#"{{
                "data": {{
                  "indexers": [
                    {{
                      "graphcastID": "{}",
                      "id": "{}"
                    }}
                  ]
                }},
                "errors": null,
                "extensions": null
              }}
              "#,
              address,
              address,
        )))
        .mount(&mock_server)
        .await;

    Mock::given(method("POST"))
        .and(path("/network-subgraph"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"{
                "data": {
                    "indexer" : {
                        "stakedTokens": "100000000000000000000000",
                        "allocations": [{
                            "subgraphDeployment": {
                                "ipfsHash": "QmggQnSgia4iDPWHpeY6aWxesRFdb8o5DKZUx96zZqEWrB"
                            }
                        }]
                    },
                    "graphNetwork": {
                        "minimumIndexerStake": "100000000000000000000000"
                    }
                },
                "errors": null
            }"#,
        ))
        .mount(&mock_server)
        .await;

    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(ResponseTemplate::new(200).set_body_string(format!(
            r#"{{
                "data": {{
                  "proofOfIndexing": "0x25331f98b82ca7f3966256bf508a7ede52e715b631dfa3d73b846bb7617f6b9e",
                  "blockHashFromNumber":"4dbba1ba9fb18b0034965712598be1368edcf91ae2c551d59462aab578dab9c5",
                  "indexingStatuses": [
                    {{
                      "subgraph": "QmggQnSgia4iDPWHpeY6aWxesRFdb8o5DKZUx96zZqEWrB",
                      "synced": true,
                      "health": "healthy",
                      "fatalError": null,
                      "chains": [
                        {{
                          "network": "mainnet",
                          "latestBlock": {{
                            "number": "{block_number}",
                            "hash": "b30395958a317ccc06da46782f660ce674cbe6792e5573dc630978c506114a0a"
                          }},
                          "chainHeadBlock": {{
                            "number": "{block_number}",
                            "hash": "b30395958a317ccc06da46782f660ce674cbe6792e5573dc630978c506114a0a"
                          }}
                        }}
                      ]
                    }}
                  ]
                }}
              }}
              "#),
        ))
        .mount(&mock_server)
        .await;

    mock_server.uri()
}

pub fn setup_mock_env_vars(mock_server_uri: &String) {
    env::set_var(
        "GRAPH_NODE_STATUS_ENDPOINT",
        format!("{}{}", mock_server_uri, "/graphql"),
    );

    env::set_var(
        "REGISTRY_SUBGRAPH_ENDPOINT",
        format!("{}{}", mock_server_uri, "/graphcast-registry"),
    );

    env::set_var(
        "NETWORK_SUBGRAPH_ENDPOINT",
        format!("{}{}", mock_server_uri, "/network-subgraph"),
    );
}

pub struct RadioRuntimeConfig {
    pub is_setup_instance: bool,
    pub panic_if_poi_diverged: bool,
}

impl RadioRuntimeConfig {
    pub fn default_config() -> Self {
        RadioRuntimeConfig {
            is_setup_instance: true,
            panic_if_poi_diverged: false,
        }
    }
    pub fn new(is_setup_instance: bool, panic_if_poi_diverged: bool) -> Self {
        RadioRuntimeConfig {
            is_setup_instance,
            panic_if_poi_diverged,
        }
    }
}
