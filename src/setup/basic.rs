use poi_radio_e2e_tests::MessagesArc;

use crate::setup::test_radio::run_test_radio;
use poi_radio_e2e_tests::utils::RadioRuntimeConfig;

fn success_handler(_messages: MessagesArc) {}

#[tokio::main]
pub async fn run_basic_instance() {
    let config = RadioRuntimeConfig::default_config();
    run_test_radio(&config, success_handler).await;
}
