mod connector;
mod core;
mod env;

use log::info;

use crate::connector::app::run_app_and_swap;
#[tokio::main]
pub async fn main() {
    pretty_env_logger::init();
    info!("Starting the bot...");
    let _bot = match run_app_and_swap().await {
        Ok(bot) => bot,
        Err(e) => {
            log::error!("Error: {}", e);
            return;
        }
    };
}
