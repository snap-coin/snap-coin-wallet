# Snap Coin Wallet
## General
Snap Coin Wallet is a CLI tool for interacting, and storing coins. It connects to a Snap Coin Node (Snap Coin API) and interacts with the network. It stores many wallets (private public key-pairs) and allows for quick login with a 6 digit PIN.

## Installation
To install Snap Coin Wallet you need to have already set up a Snap Coin node, and have a API server enabled and running. [Install a Node](https://github.com/snap-coin/snap-coin-node) 

To install Snap Coin Wallet, run:
```bash
cargo install snap-coin-wallet
```
Make sure you have cargo, and rust installed.

## Usage
### Command
The snap coin wallet command `snap-coin-wallet` Always takes only one command line argument, that is the node API address, like this:
```bash
snap-coin-wallet 127.0.0.1:3003
```
The default API port is 3003, however this depends on the node and node configuration that you are running.

### Warning
Please always use nodes that **you trust**, which in 99% of the time is only a local node! A malicious node can, spoof, hide, capture, fake all the data that you access via the wallet (excluding the wallet private keys, that are only stored locally, and are encrypted).'

### The CLI
Once you start the wallet, log in to a wallet, you are able to access the CLI, which is how you will interact with the wallet.
There are many commands available, a list of them and what they do can be seen by running `help`

### Available commands:
```bash
balance                    - Show wallet balance
available                  - List available UTXOs
history                    - Show transaction history
tx-info <txid>             - Show transaction details
send <addr> <amt>...       - Send SNAP to addresses
wallet <subcmd> [<wallet>] - Wallet management commands
subcommands:
    delete [<wallet>]      - Delete the specified wallet (default: current)
    private [<wallet>]     - Show private key of the wallet (default: current)
    public [<wallet>]      - Show public key of the wallet (default: current)
    switch [<wallet>]      - Switch to the specified wallet (default: current)
change-pin                 - Change wallet PIN
help                       - Show this help message
clear                      - Clears output history
exit, quit                 - Exit the wallet
```