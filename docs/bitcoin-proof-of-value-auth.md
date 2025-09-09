# Bitcoin Wallet Ownership Verification for Authorization
## Challenge-Response via Bitcoin Message Signing

To authorize a user based on Bitcoin holdings, the standard approach is a challenge-response using Bitcoin message signing. In this scheme, the server issues a unique token (challenge) and the user signs it with their wallet’s private key. This produces a signature proving the user has access to the private key (and thus control of the Bitcoin address). This method does not expose the private key itself, only a signature that can be verified against the public address. It is a widely accepted way to demonstrate ownership of a Bitcoin address without making an on-chain transaction. The overall process on Bitcoin mainnet might involve the following steps:

1. **Server Generates Challenge** - User provides their Bitcoin address, and the server generates a one-time random token or message (e.g. including the address and a timestamp or nonce for uniqueness).
2. **User Signs the Message** - The user’s client signs the server-provided token with the private key of that Bitcoin address.
3. **User Sends Signature** - The client sends the resulting signature (and typically the original message or a reference to it) back to the server.
4. **Server Verifies Signature** - The server uses Bitcoin’s message verification to confirm the signature is valid for the given address and challenge.
5. **Server Checks Balance** - If the signature is valid, the server queries a reliable Bitcoin API to confirm the address’s balance is at least 0.01 BTC.
6. **JWT Issuance** - Upon verification of ownership and sufficient funds, the server issues a 24-hour JWT.

Is signing a server token with the private key satisfactory? - Yes. Signing an arbitrary message (the challenge) with a Bitcoin private key is the standard way to prove ownership of a wallet address. In fact, Bitcoin’s built-in message signing (as per the Bitcoin-Qt client or BIP-137 for SegWit addresses) was designed for exactly this purpose. It allows anyone to verify that “message X was signed by the owner of address Y” without revealing the private key. This is considered a safe and satisfactory method of authentication as long as the challenge is unique (to prevent replay attacks) and specific to the context (e.g. include user ID or purpose in the message if needed). For example, you might issue a challenge like: “Login to MyService at 2025-09-01T23:27Z with address 1ABC... - nonce 12345” and ask the user to sign that text. Even if someone else tried to reuse an old signature, it would not match a new unique challenge.

Is there a size limit to the token for signing? - There is no strict small size limit on the message used for Bitcoin signature, because the signing process actually hashes the message (using SHA-256) and then signs the fixed-size hash. In Bitcoin’s message-signing format, the output signature is always a 65-byte value (encoded in Base64 for transport) regardless of the message length. In practice, it’s best to keep the challenge message reasonably short and ASCII-text only (to avoid any encoding issues across different wallets). A few sentences or a random string of a few dozen characters is well within limits - typical wallet UIs handle messages on the order of 100-250 characters without issue. For our purposes, a concise token (e.g. a UUID or random hex string, possibly prefixed with context like “Prove you own address X”) is sufficient and will be properly signed by the private key. In summary, normal-length tokens pose no problem for ECDSA signature; the signature algorithm will hash any message of arbitrary length into a 256-bit digest and sign that.

**Client-side examples were extracted per language**

* [C# signer](./clients/bitcoin-signer-csharp.md)  
* [React / TypeScript signer](./clients/bitcoin-signer-react.md)  
* [Rust signer](./clients/bitcoin-signer-rust.md)   <!-- new line -->

```rust
// Server-side (Rust): verifying the signature and issuing a JWT
// Cargo.toml dependencies:
// bitcoin = "0.30"
// bitcoin-message = "0.4"
// reqwest = { version = "0.11", features = ["json", "blocking"] }
// serde = { version = "1.0", features = ["derive"] }
// jsonwebtoken = "8"
// chrono = "0.4"

use bitcoin::{Address, network::constants::Network};
use bitcoin_message::Message;
use reqwest::blocking::Client;
use serde::Deserialize;
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use chrono::{Duration, Utc};
use std::{error::Error, str::FromStr};

#[derive(Deserialize)]
struct BalanceResponse {
    final_balance: u64,
}

#[derive(serde::Serialize)]
struct Claims {
    bitcoin_address: String,
    exp: usize,
}

fn verify_and_authorize(
    bitcoin_address: &str,
    challenge_message: &str,
    signature_b64: &str,
) -> Result<String, Box<dyn Error>> {
    // 1. Verify the signature
    let address = Address::from_str(bitcoin_address)?;
    if address.network != Network::Bitcoin {
        return Err("Only main-net addresses accepted".into());
    }
    let msg = Message::from_utf8(challenge_message.to_owned())?;
    if !msg.verify(&address, signature_b64)? {
        return Err("Signature verification failed".into());
    }

    // 2. Check the address balance via BlockCypher
    let url = format!(
        "https://api.blockcypher.com/v1/btc/main/addrs/{}/balance",
        bitcoin_address
    );
    let resp: BalanceResponse = Client::new().get(url).send()?.json()?;
    if resp.final_balance < 1_000_000 {
        return Err("Insufficient balance - wallet has less than 0.01 BTC".into());
    }

    // 3. Issue a 24-hour JWT
    let expiration = (Utc::now() + Duration::hours(24)).timestamp() as usize;
    let claims = Claims {
        bitcoin_address: bitcoin_address.to_owned(),
        exp: expiration,
    };
    let jwt = encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(b"ReplaceWithYourJWTSecretKey"),
    )?;
    Ok(jwt)
}
```

Let’s break down what the server code is doing:

  - **Signature Verification**: We use `bitcoin_message::Message::verify`, together with the `bitcoin` crate’s `Address`, to confirm the signature was produced by the private key controlling the address. These crates handle ECDSA verification for legacy and SegWit (bech32) addresses on Bitcoin main-net. This step confirms the user controls the address at the time of signing. (If this fails, the user either doesn’t have the key or provided a wrong signature.)
    - Example: If the server’s challenge was "Prove you are 1LUtd66PcpPx64GERqufPygYEWBQR2PUN6", and the user returns a valid signature, `VerifyMessage` will return true.

  - **Balance Check**: After ownership is verified, we query a Bitcoin explorer API for the address’s balance. In the code above, we use BlockCypher’s public API for Bitcoin mainnet, which returns a JSON including `final_balance` (in satoshis). You could also use alternatives like Blockchain.com’s API or Blockstream’s API - the choice depends on reliability and ease of use. (BlockCypher and Blockchain.com provide convenient JSON responses; Blockstream’s explorer is very reliable as well.) We parse the returned JSON to get the balance. Then we compare it against 1,000,000 sats (0.01 BTC). Only if the balance is ≥ 0.01 BTC do we proceed. (It’s wise to require confirmed balance; the example uses `final_balance`, which includes unconfirmed transactions - you could use `balance` for strictly confirmed funds.)

  - **JWT Generation**: Finally, we create a JWT that expires in 24 hours. We include a claim with the Bitcoin address or a user ID, and sign the token with the server’s secret key (using HMAC-SHA256 in this case). The JWT can then be issued to the client (e.g., returned in an HTTP response) to use for subsequent authenticated requests. The 24-hour expiry means after that period, the user must re-authenticate (prove ownership again) to get a new token. This aligns with the idea that the proof of funds is only valid for a day, ensuring they still hold the BTC and control the key at regular check-ins.

### Additional Considerations

  - **Security of Private Keys**: The private key should never be sent to the server. The proof relies only on the signature. Users who sign manually should be educated to only sign the exact challenge message and not to share their private key. The server should never ask for the key, only for the signature.

  - **Challenge Uniqueness**: Always include something unique (timestamp, nonce, or session-specific data) in the challenge message. This prevents an attacker from reusing an old signature. For instance, incorporating the current date or a one-time random nonce in the message (as shown in the code comments) ensures that each login attempt requires a fresh signature. Bitcoin signatures can be reused for the exact same message, so the server must avoid letting an old signature be valid for new sessions.

  - **Supported Address Types**: Legacy (base58) addresses and SegWit addresses (bech32) can both be used for message signing. Modern wallets support signing for SegWit addresses per BIP-137, so bech32 (bc1...) addresses are acceptable. The verification method in NBitcoin will automatically handle the proper verification algorithm as long as the address string is correct. (At the time of writing, Taproot addresses (bc1p...) are not widely supported by message-signing tools, so you may exclude those or require a different approach for Taproot.)

  - **Reliability of Balance API**: Using a trusted blockchain API is important for accuracy. The example uses BlockCypher, which is known for being developer-friendly. Alternatively, you could query multiple sources or run your own Bitcoin node and use an RPC call (getreceivedbyaddress or similar) to independently verify the balance. The services mentioned (Blockchain.com, Blockstream, BlockCypher) are among the popular choices and have high uptime. If using a third-party API, be mindful of rate limits and consider caching results if needed.

In summary, the strategy is to use Bitcoin’s cryptographic signature capability to prove ownership of an address (and by extension, the funds in it) off-chain, and then grant a time-limited JWT for access. This approach is secure and efficient: the user doesn’t have to send a transaction (no fees, no delays), and the server gets assurance of both identity (control of private key) and holdings (balance check) before authorizing. It is exactly the kind of scenario Bitcoin’s message signing was designed for - “signing a message using a private key to prove you have access to the address” - and has been used in real-world cases like proving fund ownership in bets or audits. By following the above approach and using well-vetted libraries, both the client and server can perform this verification reliably.

### Citations
- Message signing - Bitcoin Wiki  
  https://en.bitcoin.it/wiki/Message_signing
- How can you actually verify your ownership of bitcoin? - Unchained  
  https://www.unchained.com/blog/how-to-verify-ownership-bitcoin
- 5 Must-Know Block Explorer APIs for Bitcoin Developers - MoldStud  
  https://moldstud.com/articles/p-top-5-block-explorer-apis-every-bitcoin-developer-should-know
- Blockchain Programming in C#  
  https://finbuzzactu.files.wordpress.com/2017/06/blockchain-programming-in-csharp.pdf
- Sign/Verify Message in Wallet Clients - Reddit  
  https://www.reddit.com/r/Bitcoin/comments/1ba0w5w/signverify_message_in_wallet_clients/