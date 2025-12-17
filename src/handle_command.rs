use std::{collections::HashMap, process::exit};

use snap_coin::{
    api::client::Client,
    blockchain_data_provider::BlockchainDataProvider,
    build_transaction,
    core::transaction::{TransactionId, TransactionInput, TransactionOutput},
    crypto::{
        Hash,
        keys::{Private, Public},
    },
    to_nano, to_snap,
};

use crate::{input::read_pin, save_last_login};

/// Encrypt and save wallets
fn persist(wallets: &HashMap<String, Private>, pin: &str) {
    match crate::encryption::encrypt_wallets(wallets, pin) {
        Some(bytes) => match crate::wallet_path() {
            Ok(path) => {
                if let Err(e) = std::fs::write(path, bytes) {
                    eprintln!("Failed to save wallets: {}", e);
                }
            }
            Err(e) => eprintln!("Could not determine wallet path: {}", e),
        },
        None => eprintln!("Failed to encrypt wallets — wallets NOT saved!"),
    }
}

/// Handle CLI commands
pub async fn handle_command(
    client: &Client,
    wallets: &mut HashMap<String, Private>,
    current_wallet: &mut String,
    pin: &str,
    command: String,
    used_session_inputs: &mut Vec<TransactionInput>
) -> Result<(), anyhow::Error> {
    let mut parts = command.trim().split_whitespace();
    let cmd = match parts.next() {
        Some(c) => c,
        None => return Ok(()),
    };
    let args: Vec<&str> = parts.collect();

    let wallet = match wallets.get(current_wallet) {
        Some(w) => w,
        None => {
            println!("Current wallet '{}' not found.", current_wallet);
            return Ok(());
        }
    };
    let public = wallet.to_public();

    match cmd {
        "help" => {
            println!("Available commands:");
            println!("  balance                    - Show wallet balance");
            println!("  available                  - List available UTXOs");
            println!("  history                    - Show transaction history");
            println!("  tx-info <txid>             - Show transaction details");
            println!("  send <addr> <amt>...       - Send SNAP to addresses");
            println!("  wallet <subcmd> [<wallet>] - Wallet management commands");
            println!("    subcommands:");
            println!(
                "      delete [<wallet>]      - Delete the specified wallet (default: current)"
            );
            println!(
                "      private [<wallet>]     - Show private key of the wallet (default: current)"
            );
            println!(
                "      public [<wallet>]      - Show public key of the wallet (default: current)"
            );
            println!(
                "      switch [<wallet>]      - Switch to the specified wallet (default: current)"
            );

            println!("  change-pin                 - Change wallet PIN");
            println!("  help                       - Show this help message");
            println!("  clear                      - Clears output history");
            println!("  exit, quit                 - Exit the wallet");
        }

        "balance" => {
            let balance = to_snap(client.get_balance(public).await?);
            println!("Balance: {} SNAP", balance);
        }

        "available" => {
            let utxos = client.get_available_transaction_outputs(public).await?;
            let mut utxo_map: HashMap<Hash, Vec<(usize, TransactionOutput)>> = HashMap::new();
            for (tx_hash, tx_output, index) in utxos {
                utxo_map
                    .entry(tx_hash)
                    .or_default()
                    .push((index, tx_output));
            }

            println!("Available UTXOs:");
            for (tx_hash, outputs) in utxo_map {
                println!("  Transaction: {}", tx_hash.dump_base36());
                for (index, tx_output) in outputs {
                    println!(
                        "    - Output Index: {}, Amount: {}",
                        index,
                        to_snap(tx_output.amount)
                    );
                }
            }
        }

        "history" => {
            let history = client.get_transactions_of_address(public).await?;
            println!("Transaction History ({} items):", history.len());
            for tx_id in history {
                println!("  - {}", tx_id.dump_base36());
            }
        }

        "tx-info" => {
            if args.len() != 1 {
                println!("Usage: tx-info <TXID>");
                return Ok(());
            }
            if let Some(tx_id) = TransactionId::new_from_base36(args[0]) {
                match client.get_transaction(&tx_id).await? {
                    Some(tx) => {
                        println!("Transaction Details: {}", tx_id.dump_base36());
                        println!("{:#?}", tx);
                    }
                    None => println!("Transaction not found: {}", args[0]),
                }
            } else {
                println!("Invalid TX ID: {}", args[0]);
            }
        }

        "send" => {
            if args.len() % 2 != 0 || args.len() < 2 {
                println!("Usage: send <receiver> <amount> [...more pairs]");
                return Ok(());
            }

            let mut payments = Vec::new();
            let mut iter = args.iter();
            while let Some(receiver) = iter.next() {
                if let Some(amount_str) = iter.next() {
                    match amount_str.parse::<f64>() {
                        Ok(amount) => {
                            if let Some(receiver) = Public::new_from_base36(receiver) {
                                payments.push((receiver, to_nano(amount)));
                            } else {
                                println!("Invalid public address: {}", receiver);
                            }
                        }
                        Err(_) => {
                            println!("Invalid amount: {}", amount_str);
                            return Ok(());
                        }
                    }
                }
            }

            let transaction = build_transaction(client, *wallet, payments, used_session_inputs.clone()).await;
            if let Err(ref e) = transaction {
                println!("Failed to create transaction: {}", e);
                return Ok(());
            }

            let mut transaction = transaction.unwrap();
            println!("Computing Proof of Work...");
            transaction.compute_pow(&client.get_transaction_difficulty().await?, None)?;
            let tx_id = transaction.transaction_id.unwrap();
            println!("Created transaction: {}", tx_id.dump_base36());

            if pin != read_pin("Enter 6-digit PIN to confirm: ")? {
                println!("PIN incorrect!");
                return Ok(());
            }

            println!("Submitting transaction...");

            let used_inputs = transaction.inputs.clone();
            let status = client.submit_transaction(transaction).await?;
            println!("Transaction submission status: {:?}", status);

            println!("Validating submission...");
            if client
                .get_mempool()
                .await?
                .iter()
                .any(|tx| tx.transaction_id == Some(tx_id))
            {
                println!("Transaction successfully submitted.");
                used_session_inputs.extend_from_slice(&used_inputs);
                println!("Saved spent UTXOs to session.");
            } else {
                println!("Transaction failed to submit.");
            }
        }

        // ---------------- Wallet management ----------------
        "wallet" => {
            if args.is_empty() {
                println!("Usage: wallet <delete|private|public|switch> [wallet_name]");
                return Ok(());
            }

            let subcmd = args[0];
            let name = if args.len() > 1 {
                args[1]
            } else {
                current_wallet.as_str()
            };

            match subcmd {
                "delete" => {
                    if !wallets.contains_key(name) {
                        println!("Wallet '{}' not found.", name);
                        return Ok(());
                    }
                    let confirm =
                        read_pin(&format!("Enter PIN to confirm deletion of '{}': ", name))?;
                    if confirm != pin {
                        println!("Incorrect PIN. Wallet not deleted.");
                        return Ok(());
                    }
                    wallets.remove(name);
                    persist(wallets, pin);
                    println!("Wallet '{}' deleted.", name);

                    if current_wallet == name {
                        if let Some(first) = wallets.keys().next() {
                            *current_wallet = first.clone();
                            println!("Switched to wallet '{}'.", current_wallet);
                        } else {
                            return Err(anyhow::Error::msg("No wallets remaining."));
                        }
                    }
                }

                "private" => {
                    let wallet = match wallets.get(name) {
                        Some(w) => w,
                        None => {
                            println!("Wallet '{}' not found.", name);
                            return Ok(());
                        }
                    };
                    let confirm =
                        read_pin(&format!("Enter PIN to view private key of '{}': ", name))?;
                    if confirm != pin {
                        println!("Incorrect PIN. Cannot show private key.");
                        return Ok(());
                    }
                    println!("Private key of '{}': {}", name, wallet.dump_base36());
                }

                "public" => {
                    let wallet = match wallets.get(name) {
                        Some(w) => w,
                        None => {
                            println!("Wallet '{}' not found.", name);
                            return Ok(());
                        }
                    };
                    println!(
                        "Public key of '{}': {}",
                        name,
                        wallet.to_public().dump_base36()
                    );
                }

                "switch" => {
                    if !wallets.contains_key(name) {
                        println!("Wallet '{}' not found.", name);
                        return Ok(());
                    }
                    save_last_login(name.to_string())?;
                    *current_wallet = name.to_string();
                    println!("Switched to wallet '{}'.", current_wallet);
                }

                _ => println!("Unknown wallet subcommand: {}", subcmd),
            }
        }

        "change-pin" => {
            let confirm = read_pin("Enter current PIN: ")?;
            if confirm != pin {
                println!("Incorrect PIN. Cannot change pin.");
                return Ok(());
            }
            let new = read_pin("Create a new 6-digit wallet PIN: ")?;
            if new != read_pin("Confirm new 6-digit PIN: ")? {
                println!("PINs do not match. Cannot change pin.");
            } else {
                persist(&wallets, &new);
                println!("Changed PIN.");
                exit(0);
            }
        }

        _ => println!(
            "Unknown command: '{}'. Type 'help' for available commands.",
            cmd
        ),
    }

    Ok(())
}
