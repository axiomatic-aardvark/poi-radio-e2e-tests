use std::collections::HashSet;

use graphcast_sdk::graphcast_agent::message_typing::GraphcastMessage;
use poi_radio_e2e_tests::RadioPayloadMessage;

pub mod correct_filtering_default_topics;
pub mod correct_filtering_different_topics;
pub mod invalid_block_hash;
pub mod invalid_sender;
pub mod invalid_time;
pub mod test_num_messages;
pub mod test_poi_ok;

pub fn deduplicate_messages(
    messages: &[(String, GraphcastMessage<RadioPayloadMessage>)],
) -> Vec<(String, GraphcastMessage<RadioPayloadMessage>)> {
    let mut seen_senders = HashSet::new();
    messages
        .iter()
        .filter(|(sender, _)| seen_senders.insert(sender.clone()))
        .cloned()
        .collect()
}
