import { useState } from 'react';
import { fetchJson, postJson } from '../lib/api';

type PostKind = 'threads' | 'replies';

interface AuthorAttribution {
  subject: string;
  details: {
    provider?: string;
    username?: string;
    address?: string;
  };
}

interface Props {
  kind: PostKind;
  id: number;
}

export function ModeratorAuthorControls({ kind, id }: Props) {
  const [author, setAuthor] = useState<AuthorAttribution | null>(null);
  const [reason, setReason] = useState('');
  const [status, setStatus] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  async function revealAuthor() {
    setLoading(true);
    setStatus(null);
    try {
      setAuthor(await fetchJson<AuthorAttribution>(`/admin/${kind}/${id}/author`));
    } catch (error) {
      setStatus(error instanceof Error ? error.message : 'Unable to load author');
    } finally {
      setLoading(false);
    }
  }

  async function banAuthor() {
    if (!author || !reason.trim()) return;
    setLoading(true);
    setStatus(null);
    try {
      await postJson('/admin/bans', {
        subject: author.subject,
        reason: reason.trim(),
      });
      setReason('');
      setStatus('Subject banned');
    } catch (error) {
      setStatus(error instanceof Error ? error.message : 'Unable to ban subject');
    } finally {
      setLoading(false);
    }
  }

  if (!author) {
    return (
      <span className="inline-flex items-center gap-2">
        <button
          type="button"
          className="btn btn-ghost btn-xs"
          onClick={revealAuthor}
          disabled={loading}
        >
          {loading ? 'Loading author...' : 'Identify author'}
        </button>
        {status && <span className="text-error text-xs">{status}</span>}
      </span>
    );
  }

  return (
    <span className="inline-flex flex-wrap items-center gap-2 text-xs">
      <span className="font-mono" title={author.subject}>
        {author.details.username || author.details.address || author.subject}
      </span>
      <input
        className="input input-bordered input-xs w-40"
        maxLength={500}
        placeholder="Ban reason"
        value={reason}
        onChange={(event) => setReason(event.target.value)}
      />
      <button
        type="button"
        className="btn btn-error btn-xs"
        onClick={banAuthor}
        disabled={loading || !reason.trim()}
      >
        Ban subject
      </button>
      <button type="button" className="btn btn-ghost btn-xs" onClick={() => setAuthor(null)}>
        Hide
      </button>
      {status && (
        <span className={status === 'Subject banned' ? 'text-success' : 'text-error'}>
          {status}
        </span>
      )}
    </span>
  );
}
