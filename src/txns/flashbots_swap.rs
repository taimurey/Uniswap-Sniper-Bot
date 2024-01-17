use ethers::types::transaction::eip2718::TypedTransaction;
use ethers_flashbots::BundleRequest;
use spinners::{Spinner, Spinners};
use std::str::FromStr;
use std::sync::Arc;

use crate::txns::contracts::{
    deadline_timestamp, load_flashbots_client_middleware, load_uniswap_v2_mempool, WETH_ADDRESS,
};

use ethers::prelude::*;
use ethers::types::U256;
use ethers_core::types::Eip1559TransactionRequest;
use ethers_signers::Signer;
use tokio::join;

pub async fn uniswap_v2_bundler(
    value: U256,
    token_address: &str,
    slippage_percentage: f64,
    buy_extra_gas: U256,
    miner_tip: U256,
    maxbuy_amount: U256,
    wallet: &LocalWallet,
    provider: Arc<Provider<Http>>,
) -> eyre::Result<()> {
    let mut sp = Spinner::new(Spinners::Dots9, "Waiting for transaction hash...".into());
    let (nonce_result, gas_details_result, uniswap_v2_contract_result, client_result) = join!(
        provider.get_transaction_count(wallet.address(), None),
        provider.estimate_eip1559_fees(None),
        load_uniswap_v2_mempool(wallet),
        load_flashbots_client_middleware(&wallet, wallet, provider.clone().into(),)
    );

    let nonce = nonce_result.map_err(|e| eyre::eyre!("Failed to get transaction count: {}", e))?;

    let (max_fee_per_gas, max_priority_fee_per_gas) =
        gas_details_result.map_err(|_| eyre::eyre!("Failed to estimate EIP-1559 fees"))?;

    let uniswap_v2_contract = uniswap_v2_contract_result.or_else(|e| {
        Err(eyre::eyre!(
            "Failed to load uniswap v2 mempool contract: {}",
            e
        ))
    })?;

    let client =
        client_result.or_else(|e| Err(eyre::eyre!("Failed to load client middleware: {}", e)))?;

    // Run the asynchronous operations in parallel

    let tokenaddress = H160::from_str(token_address)?;
    let path = vec![*WETH_ADDRESS, tokenaddress];

    let mut value_to_use = value; // Define a new variable to store the resultant value

    if maxbuy_amount != U256::zero() {
        let get_input_ether_method = uniswap_v2_contract
            .method::<_, Vec<U256>>("getAmountsIn", (maxbuy_amount, path.clone()))
            .or_else(|_| Err(eyre::eyre!("Uniswap V2 Router contract method not found")))?;

        let get_input_ether_result = get_input_ether_method.call().await;

        let get_input_ether = get_input_ether_result
            .map_err(|e| eyre::eyre!("Failed to get amount of Intokens: {}", e))?;

        println!("get_input_ether: {:?}", get_input_ether);

        value_to_use = if value > get_input_ether[0] {
            get_input_ether[0]
        } else {
            value
        };
    }

    println!("value: {:?}", value);

    let get_output_tokens_method = uniswap_v2_contract
        .method::<_, Vec<U256>>("getAmountsOut", (value_to_use, path.clone()))
        .or_else(|_| Err(eyre::eyre!("Uniswap V2 Router contract method not found")))?;

    let get_output_tokens_result = get_output_tokens_method.call().await;

    let get_output_tokens = get_output_tokens_result
        .map_err(|e| eyre::eyre!("Failed to get amount of Outtokens: {}", e))?;

    // Ensure get_output_tokens is not empty
    let last_token_value = match get_output_tokens.last() {
        Some(value) => *value,
        None => return Err(eyre::eyre!("get_output_tokens is empty")),
    };

    println!("Slippage percentage: {}", slippage_percentage);

    // Calculate the slippage adjustment
    let slippage_multiplier = U256::from((slippage_percentage * 1e18) as u128);
    let slippage_adjustment = last_token_value
        .checked_mul(slippage_multiplier)
        .and_then(|result| result.checked_div(U256::from(1_000_000_000_000_000_000u128)))
        .ok_or_else(|| eyre::eyre!("Overflow occurred during slippage adjustment calculation"))?;

    let amount_out_tokens = last_token_value
        .checked_sub(slippage_adjustment)
        .ok_or_else(|| eyre::eyre!("Overflow occurred during amount out tokens calculation"))?;

    let to = wallet.address();

    let call_data = uniswap_v2_contract
        .method::<_, H160>(
            "swapExactETHForTokensSupportingFeeOnTransferTokens",
            (amount_out_tokens, path.clone(), to, deadline_timestamp()),
        )
        .or_else(|_| Err(eyre::eyre!("Uniswap V2 Router contract method not found")))?;

    let transaction_data = call_data
        .calldata()
        .ok_or_else(|| eyre::eyre!("Failed to get calldata"))?;

    println!("Transaction data: {:?}", transaction_data);

    let estimated_gas = U256::from(313252);
    // // // Constructing the EIP1559 transaction
    let txn_request = Eip1559TransactionRequest::new()
        .from(wallet.address())
        .to(uniswap_v2_contract.address())
        .value(value_to_use)
        .gas(estimated_gas)
        .max_priority_fee_per_gas(max_priority_fee_per_gas + miner_tip)
        .max_fee_per_gas(max_fee_per_gas + buy_extra_gas)
        .data(transaction_data)
        .nonce(nonce);

    let block_number = client
        .0
        .get_block_number()
        .await
        .map_err(|e| eyre::eyre!("Failed to get the current block number: {}", e))?;
    let mut tx = TypedTransaction::Eip1559(txn_request.clone());
    client
        .0
        .fill_transaction(&mut tx, None)
        .await
        .map_err(|e| eyre::eyre!("Failed to fill the transaction: {}", e))?;

    let signature = client
        .0
        .signer()
        .sign_transaction(&tx)
        .await
        .map_err(|e| eyre::eyre!("Failed to sign the transaction: {}", e))?;

    let rlp_signed_tx = tx.rlp_signed(&signature);

    let bundle_swap_ethfor_tokens_v2 = BundleRequest::new()
        .push_transaction(rlp_signed_tx.clone())
        .set_block(block_number)
        .set_simulation_block(block_number)
        .set_simulation_timestamp(0);

    let simulated_bundle = client
        .1
        .inner()
        .simulate_bundle(&bundle_swap_ethfor_tokens_v2)
        .await
        .map_err(|e| eyre::eyre!("Failed to simulate the bundle: {}", e))?;
    println!(
        "Simulated bundle for block {}: {:?}",
        block_number, simulated_bundle
    );

    let pending_bundle_swap_ethfor_tokens_v2 = client
        .0
        .inner()
        .send_bundle(&bundle_swap_ethfor_tokens_v2)
        .await
        .map_err(|e| eyre::eyre!("Failed to send the bundle: {}", e))?;

    //   let mut pending_tx_hash = H256::zero();

    for response in pending_bundle_swap_ethfor_tokens_v2.iter() {
        match response {
            Ok(pending_bundle) => {
                let pending_bundle = pending_bundle;

                // Transaction hash from the bundle
                match pending_bundle.transactions.get(0) {
                    Some(hash) => {
                        let pending_tx_hash = *hash;
                        sp.stop_with_message(format!(
                            "Transaction hash found: {:?}",
                            pending_tx_hash
                        ));
                        sp.stop();
                        break;
                    }
                    None => {
                        // Error Return
                        sp.stop();
                        return Err(eyre::eyre!("Failed to get the transaction hash"));
                    }
                };
            }
            Err(e) => {
                // Error Return
                sp.stop();
                return Err(eyre::eyre!("Failed to send the bundle: {}", e));
            }
        }
    }

    Ok(())
}
