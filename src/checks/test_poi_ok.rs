use poi_radio_e2e_tests::MessagesArc;
use tracing::{debug, info};

use crate::setup::{test_radio::run_test_radio, utils::RadioRuntimeConfig};

fn success_handler(messages: MessagesArc) {
    let messages = messages.lock().unwrap();

    if messages.len() >= 10 {
        debug!("{:?}", messages);

        info!("10 valid messages received!");
        std::process::exit(0);
    }
}

#[tokio::main]
pub async fn run_poi_ok() {
    let config = RadioRuntimeConfig::new(false, true);
    run_test_radio(&config, success_handler).await;
}
