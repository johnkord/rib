# Bitcoin message signer – C#

```csharp
using NBitcoin;
using System;

class BitcoinAuthClient
{
    public static void SignChallenge(string wifPrivateKey, string challengeMessage)
    {
        // Initialize the secret (private key) for Bitcoin MainNet
        BitcoinSecret secret;
        try
        {
            secret = new BitcoinSecret(wifPrivateKey, Network.Main);
        }
        catch (Exception ex)
        {
            Console.Error.WriteLine("Invalid private key: " + ex.Message);
            return;
        }

        // Derive the Bitcoin address (P2PKH legacy format by default)
        string address = secret.GetAddress(ScriptPubKeyType.Legacy).ToString();
        Console.WriteLine($"Bitcoin Address: {address}");

        // Sign the challenge
        string signature = secret.PrivateKey.SignMessage(challengeMessage);
        Console.WriteLine($"Signature: {signature}");
    }
}

// Example usage
string challenge = "Prove you own this address (nonce 123456)";
SignChallenge("L5oLFe...yourWIFhere...XyZ", challenge);
```
