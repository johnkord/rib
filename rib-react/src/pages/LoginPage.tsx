import { useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { API_BASE, requestBitcoinChallenge, verifyBitcoinAddress } from '../lib/api';
import { setAuthToken } from '../lib/auth';

export function LoginPage() {
  const navigate = useNavigate();

  useEffect(() => {
    // Token capture handled globally in App; no-op here
  }, []);

  const handleDiscordLogin = () => {
    // use absolute backend URL so the browser is redirected to Actix, not Vite
    window.location.href = `${API_BASE}/api/v1/auth/discord/login`;
  };

  // ---- Bitcoin Flow State ----
  const [btcAddress, setBtcAddress] = useState('');
  const [challenge, setChallenge] = useState<string | null>(null);
  const [signature, setSignature] = useState('');
  const [step, setStep] = useState<'input' | 'sign' | 'verifying' | 'done'>('input');
  const [error, setError] = useState<string | null>(null);

  const startBitcoin = async () => {
    setError(null);
    try {
      const { challenge } = await requestBitcoinChallenge(btcAddress.trim());
      setChallenge(challenge);
      setStep('sign');
    } catch (e: any) {
      setError(e.message || 'Failed to get challenge');
    }
  };

  const completeBitcoin = async () => {
    if (!challenge) return;
    setError(null);
    setStep('verifying');
    try {
      const { token } = await verifyBitcoinAddress(btcAddress.trim(), signature.trim());
      setAuthToken(token);
      setStep('done');
      navigate('/');
    } catch (e: any) {
      setError(e.message || 'Verification failed');
      setStep('sign');
    }
  };

  return (
    <div className="flex flex-col items-center justify-center min-h-screen">
      <div className="card w-96 bg-base-100 shadow-xl">
        <div className="card-body">
          <h2 className="card-title">Login to RIB</h2>
          <p className="text-sm text-gray-500 mb-4">
            Authenticate with your Discord account to post and participate.
          </p>
          <div className="space-y-6">
            <div className="card-actions justify-center">
              <button className="btn btn-primary gap-2 w-full" onClick={handleDiscordLogin}>
                <svg className="w-5 h-5" viewBox="0 0 24 24" fill="currentColor">
                  <path d="M20.317 4.37a19.791 19.791 0 0 0-4.885-1.515a.074.074 0 0 0-.079.037c-.21.375-.444.864-.608 1.25a18.27 18.27 0 0 0-5.487 0a12.64 12.64 0 0 0-.617-1.25a.077.077 0 0 0-.079-.037A19.736 19.736 0 0 0 3.677 4.37a.07.07 0 0 0-.032.027C.533 9.046-.32 13.58.099 18.057a.082.082 0 0 0 .031.057a19.9 19.9 0 0 0 5.993 3.03a.078.078 0 0 0 .084-.028a14.09 14.09 0 0 0 1.226-1.994a.076.076 0 0 0-.041-.106a13.107 13.107 0 0 1-1.872-.892a.077.077 0 0 1-.008-.128a10.2 10.2 0 0 0 .372-.292a.074.074 0 0 1 .077-.01c3.928 1.793 8.18 1.793 12.062 0a.074.074 0 0 1 .078.01c.12.098.246.198.373.292a.077.077 0 0 1-.006.127a12.299 12.299 0 0 1-1.873.892a.077.077 0 0 0-.041.107c.36.698.772 1.362 1.225 1.993a.076.076 0 0 0 .084.028a19.839 19.839 0 0 0 6.002-3.03a.077.077 0 0 0 .032-.054c.5-5.177-.838-9.674-3.549-13.66a.061.061 0 0 0-.031-.03zM8.02 15.33c-1.183 0-2.157-1.085-2.157-2.419c0-1.333.956-2.419 2.157-2.419c1.21 0 2.176 1.096 2.157 2.42c0 1.333-.956 2.418-2.157 2.418zm7.975 0c-1.183 0-2.157-1.085-2.157-2.419c0-1.333.955-2.419 2.157-2.419c1.21 0 2.176 1.096 2.157 2.42c0 1.333-.946 2.418-2.157 2.418z" />
                </svg>
                Login with Discord
              </button>
            </div>
            <div className="divider text-xs">OR</div>
            <div>
              <h3 className="font-semibold mb-2">Bitcoin Ownership</h3>
              <p className="text-xs text-gray-500 mb-3">
                Prove you control a Bitcoin address with at least 0.01 BTC by signing a challenge.
              </p>
              <div className="form-control mb-2">
                <label className="label">
                  <span className="label-text text-xs">Bitcoin Address</span>
                </label>
                <input
                  className="input input-bordered input-sm"
                  value={btcAddress}
                  onChange={(e) => setBtcAddress(e.target.value)}
                  placeholder="bc1... or 1..."
                  disabled={step !== 'input'}
                />
              </div>
              {challenge && step !== 'input' && (
                <div className="mb-2">
                  <label className="label">
                    <span className="label-text text-xs">Challenge (sign exactly this text)</span>
                  </label>
                  <textarea
                    className="textarea textarea-bordered w-full textarea-xs"
                    value={challenge}
                    readOnly
                    rows={3}
                  />
                </div>
              )}
              {step === 'sign' && (
                <div className="form-control mb-2">
                  <label className="label">
                    <span className="label-text text-xs">Base64 Signature</span>
                  </label>
                  <textarea
                    className="textarea textarea-bordered textarea-xs"
                    rows={2}
                    value={signature}
                    onChange={(e) => setSignature(e.target.value)}
                    placeholder="Paste signature your wallet produced"
                  />
                </div>
              )}
              {error && <div className="text-error text-xs mb-2">{error}</div>}
              <div className="flex gap-2">
                {step === 'input' && (
                  <button
                    className="btn btn-sm"
                    onClick={startBitcoin}
                    disabled={!btcAddress.trim()}
                  >
                    Get Challenge
                  </button>
                )}
                {step === 'sign' && (
                  <button
                    className="btn btn-sm btn-primary"
                    onClick={completeBitcoin}
                    disabled={!signature.trim()}
                  >
                    Verify & Login
                  </button>
                )}
                {step === 'verifying' && (
                  <button className="btn btn-sm loading">Verifying...</button>
                )}
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
