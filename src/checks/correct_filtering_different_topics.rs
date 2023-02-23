use crate::checks::deduplicate_messages;
use crate::setup::constants::{
    MOCK_SUBGRAPH_GOERLI, MOCK_SUBGRAPH_GOERLI_2, MOCK_SUBGRAPH_MAINNET,
};
use colored::Colorize;
use poi_radio_e2e_tests::MessagesArc;
use tracing::{debug, info};

use crate::setup::{test_radio::run_test_radio, utils::RadioRuntimeConfig};

fn success_handler(messages: MessagesArc) {
    let messages = messages.lock().unwrap();

    // Maybe pass in dynamic count here too
    if messages.len() >= 5 {
        let deduped = deduplicate_messages(&messages);
        debug!("deduped {:?}", deduped);

        info!("5 or more valid messages received! Checking content topics");
        assert!(
            messages
                .iter()
                .any(|m| m.1.identifier == MOCK_SUBGRAPH_MAINNET),
            "No message found with topic {}",
            MOCK_SUBGRAPH_MAINNET
        );
        assert!(
            messages
                .iter()
                .all(|m| m.1.identifier != MOCK_SUBGRAPH_GOERLI),
            "Message found with topic {}",
            MOCK_SUBGRAPH_GOERLI
        );
        info!(
            "{}",
            "correct_filtering_different_topics test is sucessful ✅".green()
        );
        std::process::exit(0);
    }
}

#[tokio::main]
pub async fn run_correct_filtering_different_topics() {
    let subgraphs = vec![
        MOCK_SUBGRAPH_MAINNET.to_string(),
        MOCK_SUBGRAPH_GOERLI_2.to_string(),
    ];
    let config = RadioRuntimeConfig::new(false, true, Some(subgraphs));
    run_test_radio(&config, success_handler).await;
}
