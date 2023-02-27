use poi_radio_e2e_tests::MessagesArc;

use crate::setup::test_radio::run_test_radio;
use poi_radio_e2e_tests::utils::RadioRuntimeConfig;

fn success_handler(_messages: MessagesArc) {}

#[tokio::main]
pub async fn run_invalid_payload_instance() {
    let mut config = RadioRuntimeConfig::default_config();
    config.invalid_payload = true;
    run_test_radio(&config, success_handler).await;
}
