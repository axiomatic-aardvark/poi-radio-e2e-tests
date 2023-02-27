use colored::Colorize;
use poi_radio_e2e_tests::{utils::RadioRuntimeConfig, MessagesArc};
use tracing::{debug, info};

use crate::{checks::deduplicate_messages, setup::test_radio::run_test_radio};

fn success_handler(messages: MessagesArc) {
    let messages = messages.lock().unwrap();

    if messages.len() >= 5 {
        let deduped = deduplicate_messages(&messages);
        debug!("deduped {:?}", deduped);

        info!("5 valid messages received!");
        assert!(
            messages
                .iter()
                .all(|m| m.1.payload.as_ref().unwrap().content != *"0xMyOwnPoi"),
            "Message found with POI sent from same instance",
        );

        info!("{}", "skip_messages_from_self test is sucessful ✅".green());
        std::process::exit(0);
    }
}

#[tokio::main]
pub async fn run_skip_messages_from_self() {
    let mut config = RadioRuntimeConfig::new(false, true);
    config.poi = "0xMyOwnPoi".to_string();
    run_test_radio(&config, success_handler).await;
}
