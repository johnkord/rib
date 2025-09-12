import React, { useState } from 'react';

// Keeping the component name for minimal churn; acts as the About page now.
export function About() {
  const btcAddress = 'bc1qlawxetusaugute86w3yc8m72xggak5lkjgqd2p';
  const [copied, setCopied] = useState(false);
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(btcAddress);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (_) {
      // ignore
    }
  };
  return (
    <div className="max-w-2xl mx-auto py-8">
      <div className="card bg-base-100 shadow">
        <div className="card-body space-y-5">
          <h1 className="card-title text-2xl">About</h1>
          <div className="space-y-3 text-sm leading-relaxed">
            <p>
              <span className="font-semibold">rib</span> is an anonymous image / video board.
              Browsing is open, but posting threads or replies requires authentication to reduce
              spam and abuse while still preserving pseudonymity for regular users.
            </p>
            <p className="font-medium">Authentication methods:</p>
            <ul className="list-disc list-inside space-y-1">
              <li>
                <span className="font-semibold">Discord Role:</span> A Discord ID can be manually
                granted a role by the operator. Once assigned, the user can create threads and
                upload content.
              </li>
              <li>
                <span className="font-semibold">Bitcoin Proof:</span> Prove ownership of a Bitcoin
                address holding at least <code className="px-1 bg-base-200 rounded">0.01 BTC</code>{' '}
                by signing the provided challenge message. A valid signature grants posting
                capability without going through Discord.
              </li>
            </ul>
            <p>
              No passwords are stored by rib; access derives from external proofs (Discord OAuth) or
              cryptographic ownership of a qualifying Bitcoin address.
            </p>
            <hr className="my-2" />
            <p>
              This instance (<span className="font-semibold">rib.curlyquote.com</span>) is built and
              operated by <strong>John Kordich</strong>.
            </p>
          </div>
          <div className="space-y-2 text-sm">
            <div>
              <span className="font-medium">Name:</span> John Kordich
            </div>
            <div>
              <span className="font-medium">Contact:</span> A @ B where A = jkordich and B =
              gmail.com
            </div>
            <div>
              <span className="font-medium">Discord:</span> curlyquote
            </div>
            <div className="flex items-center flex-wrap gap-2">
              <span className="font-medium">Donations (BTC):</span>
              <code className="px-1 py-0.5 bg-base-200 rounded text-xs break-all">
                {btcAddress}
              </code>
              <button className="btn btn-xs" onClick={copy}>
                {copied ? 'Copied' : 'Copy'}
              </button>
            </div>
            <div>
              <span className="font-medium">GitHub Repo:</span>{' '}
              <a
                className="link"
                href="https://github.com/johnkord/rib"
                target="_blank"
                rel="noopener noreferrer"
              >
                github.com/johnkord/rib
              </a>
            </div>
          </div>
          <p className="text-xs text-gray-500">
            Email is presented in a slightly obfuscated form to reduce automated scraping. Replace
            the placeholders to contact.
          </p>
        </div>
      </div>
    </div>
  );
}
