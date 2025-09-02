# Bitcoin message signer – React + TypeScript

```tsx
// src/components/BitcoinSigner.tsx
import React, { useState } from 'react';
import * as bitcoin from 'bitcoinjs-lib';
import * as wif from 'wif';
import * as ecc from 'tiny-secp256k1';
import { ECPairFactory } from 'ecpair';

const ECPair = ECPairFactory(ecc);

const BitcoinSigner: React.FC = () => {
  const [wifKey, setWifKey] = useState('');
  const [challenge, setChallenge] = useState('');
  const [address, setAddress] = useState('');
  const [signature, setSignature] = useState('');

  const sign = () => {
    try {
      const { privateKey, compressed } = wif.decode(wifKey);
      const keyPair = ECPair.fromPrivateKey(privateKey, { compressed });
      const { address: addr } = bitcoin.payments.p2pkh({
        pubkey: keyPair.publicKey,
        network: bitcoin.networks.bitcoin
      });
      if (!addr) throw new Error('Failed to derive address');
      setAddress(addr);

      // Bitcoin Signed Message prefix
      const prefix = Buffer.from('\u0018Bitcoin Signed Message:\n');
      const msgBuffer = Buffer.from(challenge, 'utf8');
      const lengthVarInt = bitcoin.varuint.encode(msgBuffer.length);
      const hash = bitcoin.crypto.hash256(
        Buffer.concat([prefix, lengthVarInt, msgBuffer])
      );
      const sigObj = keyPair.sign(hash);
      const sig = Buffer.concat([
        sigObj,
        Buffer.from([compressed ? 0x01 : 0x00]) // recId placeholder
      ]).toString('base64');

      setSignature(sig);
    } catch (e) {
      alert(`Error: ${(e as Error).message}`);
    }
  };

  return (
    <div>
      <h3>Bitcoin Message Signer</h3>
      <label>
        WIF Private Key:
        <input value={wifKey} onChange={e => setWifKey(e.target.value)} />
      </label>
      <br />
      <label>
        Challenge Message:
        <input value={challenge} onChange={e => setChallenge(e.target.value)} />
      </label>
      <br />
      <button onClick={sign}>Sign</button>
      {address && (
        <>
          <p>Bitcoin Address: {address}</p>
          <p>Signature (Base64): {signature}</p>
        </>
      )}
    </div>
  );
};

export default BitcoinSigner;
```
