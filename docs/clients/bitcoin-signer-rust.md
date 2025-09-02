# Bitcoin message signer – Rust

```rust
// Cargo.toml
// [dependencies]
// bitcoin          = "0.30"
// bitcoin-message  = "0.4"
// clap             = { version = "4", features = ["derive"] }

use bitcoin::{util::key::PrivateKey, Network};
use bitcoin_message::Message;
use clap::Parser;

/// Simple CLI to sign a message with a WIF private key
#[derive(Parser)]
struct Args {
    /// WIF-encoded private key (main-net)
    wif: String,
    /// Challenge message to sign
    message: String,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Decode WIF to PrivateKey
    let priv_key = PrivateKey::from_wif(&args.wif)?;
    if priv_key.network != Network::Bitcoin {
        anyhow::bail!("Only main-net keys are accepted");
    }

    // Sign the message
    let msg = Message::from_utf8(args.message)?;
    let signature = msg.sign(&priv_key.key);

    // Output results
    let address = priv_key.public_key(&priv_key.network).to_address(Network::Bitcoin);
    println!("Bitcoin Address: {}", address);
    println!("Signature (Base64): {}", signature);

    Ok(())
}
```

Example:

```bash
cargo run --release -- "L5oLFe...yourWIFhere...XyZ" \
  "Prove you own this address at MyService (nonce 123456)"
```
