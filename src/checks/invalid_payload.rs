use crate::checks::deduplicate_messages;
use colored::Colorize;
use poi_radio_e2e_tests::{utils::RadioRuntimeConfig, MessagesArc};
use tracing::{debug, info};

use crate::setup::test_radio::run_test_radio;

use std::any::type_name;

fn type_of<T>(_: T) -> &'static str {
    type_name::<T>()
}

fn success_handler(messages: MessagesArc) {
    let messages = messages.lock().unwrap();

    // Maybe pass in dynamic count here too
    if messages.len() >= 5 {
        let deduped = deduplicate_messages(&messages);
        debug!("deduped {:?}", deduped);

        info!("5 or more valid messages received! Checking payloads");
        assert!(
            messages
                .iter()
                .all(|m| !type_of(&m.1.payload).contains("DummyMsg")),
            "Message found with invalid payload",
        );
        info!("{}", "invalid_payload test is sucessful ✅".green());
        std::process::exit(0);
    }
}

#[tokio::main]
pub async fn run_invalid_payload() {
    let config = RadioRuntimeConfig::new(false, true);
    run_test_radio(&config, success_handler).await;
}
