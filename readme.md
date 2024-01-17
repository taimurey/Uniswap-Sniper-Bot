# Ethereum Volume Bot

This project contains an Ethereum volume bot that loops over multiple wallets to perform transactions. The bot is designed to interact with the Ethereum blockchain using the Ethers.rs library, which provides a Rust interface to Ethereum nodes.

## Overview

The bot reads settings from a `settings.json` file, which includes information about the wallets to use, the token to buy, slippage, amount of ETH to buy for each wallet, gas settings, delay between each wallet buy, number of rounds, and RPC settings.

The bot supports both private and public transactions. If the `PrivateTransaction` setting is `true`, the bot will use the Flashbots protocol to bundle transactions and send them directly to miners. Otherwise, it will use regular Ethereum transactions.

The bot also supports automatic slippage. If the `autoSlippage` setting is `true`, the bot will use a slippage of 50%. Otherwise, it will use the slippage specified in the settings.

## Modules

- `connector`: Contains the main application logic, including reading settings from the JSON file and running the bot.
- `env_setup`: Contains environment setup code.
- `txns`: Contains code for interacting with the Ethereum blockchain, including sending transactions and interacting with smart contracts.


## Uniswap V2 Transaction Function

a) **Public Uniswap V2 Transaction**
- The `uniswap_v2_transaction` function in the `txns` module is responsible for performing a Uniswap V2 transaction. It takes in parameters such as the amount of ETH to use, the token address to buy, slippage percentage, gas settings, and wallet information. It then calculates the optimal transaction parameters, creates a transaction, and sends it to the Ethereum network.

- The function first retrieves the current transaction count (nonce), gas details, Uniswap V2 contract, and client middleware. It then calculates the amount of input Ether required for the transaction and the amount of output tokens expected. It adjusts the output tokens based on the slippage percentage and constructs the transaction data.

- The function then sends the transaction and waits for the transaction receipt. If the transaction is successful, it prints the transaction hash. If the transaction fails, it returns an error.

b)  **MEV Transaction Sender to Flashbots & Block Builders**

- `flashbots_swap`: Contains the `uniswap_v2_bundler` function which is responsible for creating and sending a bundle of transactions to the Ethereum network using the Flashbots protocol. This function takes in parameters such as the amount of ETH to use, the token address to buy, slippage percentage, gas settings, and wallet information. It then calculates the optimal transaction parameters, creates a bundle of transactions, and sends it to the Ethereum network.

## Settings JSON

Settings JSON file is self explanatory.

Usage Example:
```bash
{
    "wallets": {
        "wallet1": "0xYourWalletAddress1",
        "wallet2": "0xYourWalletAddress2",
        "wallet3": "0xYourWalletAddress3"
    },
    "tokenToBuy": "0xYourTokenAddress",
    "slippage": 0.01,   
    "autoSlippage": false,                             
    "amountOfETHToBuy": {
        "wallet1": 0.1,                              
        "wallet2": 0.2,                                 
        "wallet3": 0.3                             
    },
    "BuyExtraGas": 3,                            
    "MinerTip": 5,                                
    "delayBetweenEachWalletBuy": 5,
    "numberOfRounds": 3,
    "PrivateTransaction": true,
    "rpc": {
        "Url_Https": "https://mempool.merkle.io/rpc/eth/pk_mbs_4bde7f72b572527acd45d58ff707ae38",
        "Url_Wss": "wss://mempool.merkle.io/rpc/eth/pk_mbs_4bde7f72b572527acd45d58ff707ae38"
    }
}
```


## Install Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

<!-- ## Clone the Repositery

```bash
git clone https://github.com/taimurey/Volume-Bot-Ethereum.git
``` -->

## Install the Dependencies

```bash
cargo install
```




## Running the Bot

To run the bot, use the following command:

```bash
cargo run
```

Before running the bot, make sure to set up the `settings.json` file with your desired settings.

## Logging

The bot uses the `pretty_env_logger` crate for logging. To enable logging, set the `RUST_LOG` environment variable to your desired log level. For example, to enable info level logging, use the following command:

```bash
export RUST_LOG=info
```


