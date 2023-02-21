use std::collections::HashSet;

use graphcast_sdk::graphcast_agent::message_typing::GraphcastMessage;
use poi_radio_e2e_tests::{MessagesArc, RadioPayloadMessage};
use tracing::info;

use crate::setup::{test_radio::run_test_radio, utils::RadioRuntimeConfig};

fn deduplicate_messages(
    messages: &[(String, GraphcastMessage<RadioPayloadMessage>)],
) -> Vec<(String, GraphcastMessage<RadioPayloadMessage>)> {
    let mut seen_senders = HashSet::new();
    messages
        .iter()
        .filter(|(sender, _)| seen_senders.insert(sender.clone()))
        .cloned()
        .collect()
}

macro_rules! success_handler_fn {
    ($instances:expr) => {
        move |messages: MessagesArc| {
            let messages = messages.lock().unwrap();

            if (messages.len() as u32) < $instances {
                return;
            }

            let messages = messages.iter().cloned().collect::<Vec<_>>();
            let block = messages
                .last()
                .expect("Message vec to not be empty")
                .1
                .block_number;
            let messages = messages
                .into_iter()
                .filter(|(_, msg)| msg.block_number == block)
                .collect::<Vec<_>>();
            let deduped = deduplicate_messages(&messages);

            let deduped_len = deduped.len() as u32;
            assert!(
                deduped_len >= ($instances as f32 * 0.7) as u32,
                "Expected deduped arr length to be at least 70% of mock senders count."
            );

            info!("num_messages test is sucessfull");
            std::process::exit(0);
        }
    };
}

#[tokio::main]
pub async fn run_num_messages(count: u32) {
    let config = RadioRuntimeConfig::new(false, true);
    run_test_radio(&config, success_handler_fn!(count)).await;
}
