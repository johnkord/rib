import { FormEvent, useState, useMemo } from 'react';
import { useParams, Link } from 'react-router-dom';
import { useBoards } from '../hooks/useBoards';
import { useThreads, useCreateThread } from '../hooks/useThreads';
import { imageUrl } from '../lib/api';
import MediaModal from '../components/MediaModal';

export function BoardThreadsPage() {
  const { slug } = useParams();
  const { data: boards } = useBoards();
  const board = useMemo(() => boards?.find(b => b.slug === slug), [boards, slug]);
  const boardId = board?.id ?? null;
  const { data: threads, isFetching } = useThreads(boardId);
  const createThread = useCreateThread();
  const [subject, setSubject] = useState('');
  const [file, setFile] = useState<File | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [viewer, setViewer] = useState<{ hash: string; mime: string | null } | null>(null);

  function onFileChange(e: React.ChangeEvent<HTMLInputElement>) {
    setFile(e.target.files?.[0] ?? null);
  }

  async function onSubmit(e: FormEvent) {
    e.preventDefault();
    if (!boardId) { setError('Board not found'); return; }
    if (!subject.trim()) { setError('Subject required'); return; }
    try {
      setSubmitting(true); setError(null);
      await createThread(boardId, subject.trim(), file);
      setSubject(''); setFile(null);
    } catch (err: any) { setError(err.message); }
    finally { setSubmitting(false); }
  }

  // local sort as fallback -------------------------------------------
  const sortedThreads = useMemo(
    () => [...(threads ?? [])].sort(
      (a, b) => new Date(b.bump_time).getTime() - new Date(a.bump_time).getTime()
    ),
    [threads]
  );
  // ------------------------------------------------------------------

  return (
    <div>
      {/* top back-link */}
      <p className="mb-2">
        <Link className="link" to="/">&larr; Back to Boards</Link>
      </p>

      <h1 className="text-xl font-semibold mb-3">/{slug}/ – Threads</h1>
      <form className="mb-6 space-y-2" onSubmit={onSubmit}>
        <input className="input input-bordered w-full" placeholder="New thread subject" value={subject} onChange={(e)=>setSubject(e.target.value)} />
        <input
          type="file"
          accept="image/*,video/*"          // was images only
          onChange={onFileChange}
        />
        {error && <p className="text-red-600 text-sm">{error}</p>}
        <button className="btn btn-primary" disabled={submitting}>{submitting ? 'Posting…' : 'Post Thread'}</button>
      </form>
      <ul>
        {isFetching && <li>Loading…</li>}
        {!isFetching && sortedThreads.map(t => (
          <li key={t.id} className="mb-1">
            <span className="text-xs text-gray-500 mr-1">#{t.id}</span>
            <Link className="link font-medium" to={`/thread/${t.id}`}>{t.subject}</Link>
            <span className="ml-2 text-xs text-gray-500">
              (last {new Date(t.bump_time).toLocaleString()}, created {new Date(t.created_at).toLocaleString()})
            </span>
            {t.image_hash && (
              t.mime?.startsWith('image/')
                ? <img className="inline-block h-6 ml-2 cursor-pointer" src={imageUrl(t.image_hash)} alt=""
                        onClick={() => setViewer({ hash: t.image_hash!, mime: t.mime })} />
                : <span className="ml-2 cursor-pointer"
                        onClick={() => setViewer({ hash: t.image_hash!, mime: t.mime })}>📹</span>
            )}
          </li>
        ))}
      </ul>

      {/* bottom back-link */}
      <p className="mt-6">
        <Link className="link" to="/">&larr; Back to Boards</Link>
      </p>

      {/* media viewer -------------------------------------------------- */}
      {viewer && (
        <MediaModal
          hash={viewer.hash}
          mime={viewer.mime}
          onClose={() => setViewer(null)}
        />
      )}
      {/* --------------------------------------------------------------- */}
    </div>
  );
}
