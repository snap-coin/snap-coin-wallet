use std::{
    collections::HashMap, env::args, fs::File, io::{Read, Write}, path::PathBuf
};

use anyhow::Error;
use rustyline::Editor;
use rustyline::{error::ReadlineError, history::DefaultHistory};
use snap_coin::{api::client::Client, crypto::keys::Private, economics::DEV_WALLET};

mod encryption;
mod handle_command;
mod input;

use crate::{
    encryption::{decrypt_wallets, encrypt_wallets},
    handle_command::handle_command,
    input::{read_input, read_pin},
};


/// Returns wallet file path
fn wallet_path() -> Result<PathBuf, Error> {
    let home = dirs::home_dir().ok_or_else(|| Error::msg("Could not determine home directory"))?;
    Ok(home.join(".snap-coin-wallet"))
}

/// Returns history file path
fn history_path() -> Result<PathBuf, Error> {
    let home = dirs::home_dir().ok_or_else(|| Error::msg("Could not determine home directory"))?;
    Ok(home.join(".snap-coin-history"))
}

/// Returns last login file path
fn last_login_path() -> Result<PathBuf, Error> {
    let home = dirs::home_dir().ok_or_else(|| Error::msg("Could not determine home directory"))?;
    Ok(home.join(".snap-coin-last-login"))
}

/// Save all wallets with PIN
fn save_wallets(wallets: &HashMap<String, Private>, pin: &str) -> Result<(), Error> {
    let path = wallet_path()?;
    let mut file = File::create(path)?;
    let encrypted =
        encrypt_wallets(wallets, pin).ok_or_else(|| Error::msg("Failed to encrypt wallets"))?;
    file.write_all(&encrypted)?;
    Ok(())
}

/// Load wallets using PIN
fn load_wallets(pin: &str) -> Result<HashMap<String, Private>, Error> {
    let path = wallet_path()?;
    if !path.exists() {
        return Ok(HashMap::new());
    }
    let mut file = File::open(path)?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf)?;
    decrypt_wallets(&buf, pin).ok_or_else(|| Error::msg("Failed to decrypt wallets (wrong PIN?)"))
}

pub fn save_last_login(last_login: String) -> Result<(), Error> {
    let path = last_login_path()?;
    let mut file = File::create(path)?;
    file.write_all(last_login.as_bytes())?;
    Ok(())
}

pub fn load_last_login() -> Result<String, Error> {
    let path = last_login_path()?;
    if !path.exists() {
        return Ok(String::new());
    }

    let mut last_login = String::new();
    File::open(path)?.read_to_string(&mut last_login)?;
    Ok(last_login)
}

/// Select wallet from existing ones
fn select_wallet(wallets: &HashMap<String, Private>) -> Result<String, Error> {
    println!("Available wallets:");
    let last_wallet = load_last_login()?;
    for name in wallets.keys() {
        println!(
            "  - {}{}",
            name,
            if name == &last_wallet {
                " [default]"
            } else {
                ""
            }
        );
    }
    loop {
        let name = read_input("Enter wallet name to login: ");
        if name == "" && last_wallet != "" {
            return Ok(last_wallet);
        }
        if wallets.contains_key(&name) {
            return Ok(name);
        }
        println!("Wallet '{}' not found. Please try again.", name);
    }
}

/// Create new wallet, optionally import from base36 private key
fn create_wallet(wallets: &mut HashMap<String, Private>, pin: &str) -> Result<String, Error> {
    let name = read_input("Enter a name for your new wallet: ");
    if wallets.contains_key(&name) {
        return Err(Error::msg("Wallet already exists"));
    }

    let key_input = read_input("Enter a base36 private key to import (leave empty for random): ");
    let wallet = if key_input.is_empty() {
        Private::new_random()
    } else {
        Private::new_from_base36(&key_input)
            .ok_or_else(|| Error::msg("Invalid base36 private key"))?
    };

    wallets.insert(name.clone(), wallet);
    save_wallets(wallets, pin)?;
    println!("Wallet '{}' created successfully.", name);
    println!();
    println!("Please make sure to save the wallet private key, in a SAFE, OFFLINE LOCATION!");
    println!("Wallet private key (base 36): {}", wallet.dump_base36());
    println!(
        "!!! If you loose this key, you can and will loose your snap coin's. There is NO way to recover them if lost !!!"
    );
    println!("!!! If anyone sees this key, they can and will still your snap coin's !!!");
    println!();

    Ok(name)
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    println!("--- Snap Coin Wallet ---");

    // --- Read PIN ---
    let pin = read_pin("Enter 6-digit wallet PIN: ")?;

    // --- Load wallets ---
    let mut wallets = load_wallets(&pin)?;

    // --- Determine current wallet ---
    let mut current_wallet = if wallets.is_empty() {
        println!("No wallets found. Creating a new wallet.");
        if read_pin("Confirm 6-digit wallet PIN: ")? != pin {
            return Err(Error::msg("PINs don't match"));
        }
        create_wallet(&mut wallets, &pin)?
    } else {
        println!("1) Select existing wallet [default]");
        println!("2) Create new wallet");
        let choice = read_input("Choose option (1 or 2): ");
        let choice = if choice.is_empty() {
            "1"
        } else {
            choice.as_str()
        };

        match choice {
            "1" => select_wallet(&wallets)?,
            "2" => create_wallet(&mut wallets, &pin)?,
            _ => return Err(Error::msg("Invalid choice")),
        }
    };

    let wallet = wallets.get(&current_wallet).unwrap();
    save_last_login(current_wallet.clone())?;
    println!(
        "Loaded wallet '{}' with public key: {}",
        current_wallet,
        wallet.to_public().dump_base36()
    );
    println!(
        "Consider donating to the developer :) {}",
        DEV_WALLET.dump_base36()
    );

    // --- Connect to node ---
    let mut node_addr = "127.0.0.1:3003";

    let args = args().collect::<Vec<String>>();
    if let Some(node) = args.get(1) {
        node_addr = node;
    }

    let client = Client::connect(node_addr.parse()?).await?;
    println!("Connected to node at {}", node_addr);

    // --- Setup Rustyline ---
    let mut rl = Editor::<(), DefaultHistory>::new()?;
    let hist_path = history_path()?;
    if hist_path.exists() {
        rl.load_history(&hist_path).ok();
    }

    loop {
        let readline = rl.readline("snap coin wallet > ");
        match readline {
            Ok(line) => {
                let command = line.trim();
                if command.is_empty() {
                    continue;
                }
                rl.add_history_entry(command)?;

                if ["exit", "e", "quit", "q"].contains(&command) {
                    break;
                }
                if command == "clear" || command == "cls" {
                    rl.clear_history()?;
                    rl.clear_screen()?;
                    continue;
                }

                // Pass mutable references to handle_command
                handle_command(
                    &client,
                    &mut wallets,
                    &mut current_wallet,
                    &pin,
                    command.to_string(),
                )
                .await?;
            }

            Err(ReadlineError::Interrupted) => {
                println!("Interrupted (Ctrl+C)");
                break;
            }
            Err(ReadlineError::Eof) => {
                println!("Exiting (Ctrl+D)");
                break;
            }
            Err(err) => {
                println!("Error: {:?}", err);
                break;
            }
        }
    }

    // --- Save wallet history ---
    rl.save_history(&hist_path).ok();

    Ok(())
}
