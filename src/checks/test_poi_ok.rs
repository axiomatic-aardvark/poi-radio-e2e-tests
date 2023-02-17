use crate::setup::{test_radio::run_test_radio, utils::RadioRuntimeConfig};

#[tokio::main]
pub async fn run_poi_ok() {
    let config = RadioRuntimeConfig::new(false, true);
    run_test_radio(&config).await;
}
