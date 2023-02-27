use colored::Colorize;
use poi_radio_e2e_tests::MessagesArc;
use tracing::info;

use crate::setup::{test_radio::run_test_radio, utils::RadioRuntimeConfig};

fn success_handler(messages: MessagesArc) {
    let messages = messages.lock().unwrap();

    if messages.len() == 0 {
        info!("{}", "invalid_sender test is sucessful ✅".green());
        std::process::exit(0);
    } else {
        info!("{}", "invalid_sender test failed".red());
        std::process::exit(1);
    }
}

#[tokio::main]
pub async fn run_invalid_sender() {
    let mut config = RadioRuntimeConfig::new(false, true);
    config.indexer_address = Some("0x002aee240e7a4b356620b0a6053c14a073499413".to_string());
    config.operator_address = Some("0x92239c8f2baba65dc4de65bd9fa16defc08699c7".to_string());
    config.indexer_stake = "1".to_string();
    run_test_radio(&config, success_handler).await;
}
