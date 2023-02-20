use std::collections::HashSet;

use graphcast_sdk::graphcast_agent::message_typing::GraphcastMessage;
use poi_radio_e2e_tests::{MessagesArc, RadioPayloadMessage};
use tracing::debug;

use crate::setup::{test_radio::run_test_radio, utils::RadioRuntimeConfig};

fn deduplicate_messages(messages: &Vec<(String, GraphcastMessage<RadioPayloadMessage>)> ) -> Vec<(String, GraphcastMessage<RadioPayloadMessage>)> {
    let mut seen_senders = HashSet::new();
    messages
        .iter()
        .filter(|(sender, _)| seen_senders.insert(sender.clone()))
        .cloned()
        .collect()
}

macro_rules! success_handler_fn {
    ($expected_len:expr) => {
        move |messages: MessagesArc| {
            let messages = messages.lock().unwrap();

            if messages.len() < 10 {
                return;
            }

            let messages = messages.iter().cloned().collect::<Vec<_>>();
            let block = messages.first().unwrap().1.block_number;
            let messages = messages.into_iter().filter(|(_, msg)| msg.block_number == block).collect::<Vec<_>>();
            let deduped = deduplicate_messages(&messages);

            debug!("these are the deduped messages for block {} {:?}", block, deduped);

            assert_eq!(deduped.len() as u32, $expected_len);
            std::process::exit(0);
        }
    };
}

#[tokio::main]
pub async fn run_num_messages(count: u32) {
    let config = RadioRuntimeConfig::new(false, true);
    run_test_radio(&config, success_handler_fn!(count)).await;
}
