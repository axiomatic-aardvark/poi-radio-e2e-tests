use crate::checks::deduplicate_messages;
use crate::setup::constants::{MOCK_SUBGRAPH_GOERLI, MOCK_SUBGRAPH_MAINNET};
use colored::Colorize;
use poi_radio_e2e_tests::{utils::RadioRuntimeConfig, MessagesArc};
use tracing::{debug, info};

use crate::setup::test_radio::run_test_radio;

fn success_handler(messages: MessagesArc) {
    let messages = messages.lock().unwrap();

    // Maybe pass in dynamic count here too
    if messages.len() >= 5 {
        let deduped = deduplicate_messages(&messages);
        debug!("deduped {:?}", deduped);

        info!("5 or more valid messages received! Checking content topics");
        let test_topics = &[MOCK_SUBGRAPH_MAINNET, MOCK_SUBGRAPH_GOERLI];
        let found_all = test_topics.iter().all(|test_topic| {
            messages
                .iter()
                .any(|message| message.1.identifier == *test_topic)
        });

        assert!(
            found_all,
            "Did not find both {} and {} in the messages",
            MOCK_SUBGRAPH_MAINNET, MOCK_SUBGRAPH_GOERLI
        );
        info!(
            "{}",
            "correct_filtering_default_topics test is sucessful ✅".green()
        );
        std::process::exit(0);
    }
}

#[tokio::main]
pub async fn run_correct_filtering_default_topics() {
    let config = RadioRuntimeConfig::new(false, true);
    run_test_radio(&config, success_handler).await;
}
