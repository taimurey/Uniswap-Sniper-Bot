mod connector;
mod env_setup;
mod txns;

use crate::connector::app::run_app_and_swap;
#[tokio::main]
pub async fn main() {
    pretty_env_logger::init();
    let _bot = run_app_and_swap().await;
}
