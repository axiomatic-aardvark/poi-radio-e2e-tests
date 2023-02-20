use super::utils::RadioRuntimeConfig;
use crate::setup::test_radio::run_test_radio;

#[tokio::main]
pub async fn run_basic_instance() {
    let config = RadioRuntimeConfig::default_config();

    run_test_radio(&config).await;
}
