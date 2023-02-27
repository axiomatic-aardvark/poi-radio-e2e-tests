use colored::Colorize;
use poi_radio_e2e_tests::{utils::RadioRuntimeConfig, MessagesArc};
use tracing::{debug, info};

use crate::setup::test_radio::run_test_radio;

fn success_handler(messages: MessagesArc) {
    let messages = messages.lock().unwrap();

    if messages.len() >= 5 {
        debug!("{:?}", messages);

        info!("5 valid messages received!");
        info!("{}", "poi_ok test is sucessful ✅".green());
        std::process::exit(0);
    }
}

#[tokio::main]
pub async fn run_poi_ok() {
    let config = RadioRuntimeConfig::new(false, true);
    run_test_radio(&config, success_handler).await;
}
