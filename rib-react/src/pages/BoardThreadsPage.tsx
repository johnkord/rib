import { FormEvent, useState, useMemo } from 'react';
import { useParams, Link } from 'react-router-dom';
import { useBoards, useUpdateBoard } from '../hooks/useBoards';
import { useThreads, useCreateThread } from '../hooks/useThreads';
import { useAuth } from '../hooks/useAuth';
import { apiClient } from '../lib/api';
import { imageUrl } from '../lib/api';
import MediaModal from '../components/MediaModal';

export function BoardThreadsPage() {
  const { slug } = useParams();
  const { user } = useAuth();
  const [showDeleted, setShowDeleted] = useState(false);
  const { data: boards } = useBoards(user?.role === 'admin' && showDeleted);
  const board = useMemo(() => boards?.find(b => b.slug === slug), [boards, slug]);
  const boardId = board?.id ?? null;
  const { data: threads, isFetching, refetch: refreshThreads } = useThreads(boardId, user?.role === 'admin' && showDeleted); // include deleted
  const createThread = useCreateThread();
  const updateBoard = useUpdateBoard();
  const [subject, setSubject] = useState('');
  const [body, setBody]     = useState('');          // NEW
  const [file, setFile] = useState<File | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [viewer, setViewer] = useState<{ hash: string; mime: string | null } | null>(null);
  const [editing, setEditing] = useState(false);
  const [newSlug, setNewSlug] = useState(board?.slug ?? '');
  const [newTitle, setNewTitle] = useState(board?.title ?? '');

  function onFileChange(e: React.ChangeEvent<HTMLInputElement>) {
    setFile(e.target.files?.[0] ?? null);
  }

  async function onSubmit(e: FormEvent) {
    e.preventDefault();
    if (!boardId) { setError('Board not found'); return; }
    if (!subject.trim()) { setError('Subject required'); return; }
    try {
      setSubmitting(true); setError(null);
      await createThread(boardId, subject.trim(), body.trim(), file);  // NEW arg
      setSubject(''); setBody(''); setFile(null);                      // clear
    } catch (err: any) { setError(err.message); }
    finally { setSubmitting(false); }
  }

  async function onEditSubmit(e: FormEvent) {
    e.preventDefault();
    if (!boardId) return;
    await updateBoard(boardId, newSlug.trim(), newTitle.trim());
    setEditing(false);
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

      {/* header row ------------------------------------------------ */}
      <div className="flex items-center justify-between mb-3">
        <div className="flex items-center gap-2">
          <h1 className="text-xl font-semibold">
            /{slug}/ – Threads
          </h1>
          <button                                   /* NEW */
            className="btn btn-sm"
            onClick={() => refreshThreads()}
            disabled={isFetching}
          >
            {isFetching ? 'Refreshing…' : 'Refresh'}
          </button>
          {user?.role === 'admin' && (
            <label className="flex items-center gap-1 text-xs ml-2">
              <input type="checkbox" checked={showDeleted} onChange={e=>{setShowDeleted(e.target.checked); refreshThreads();}} /> Show deleted
            </label>
          )}
        </div>
        {board && !editing && (
          <button className="btn btn-sm" onClick={()=>{
            setNewSlug(board.slug);      // keep current values
            setNewTitle(board.title);
            setEditing(true);
          }}>
            Edit board
          </button>
        )}
      </div>
      {/* ----------------------------------------------------------- */}

      {/* inline edit form (appears under header) ------------------- */}
      {editing && board && (
        <form className="mb-6 space-y-2" onSubmit={onEditSubmit}>
          <input className="input input-bordered w-full" value={newSlug}
                 onChange={e=>setNewSlug(e.target.value)} placeholder="Slug" />
          <input className="input input-bordered w-full" value={newTitle}
                 onChange={e=>setNewTitle(e.target.value)} placeholder="Title" />
          <div className="space-x-2">
            <button className="btn btn-primary btn-sm" type="submit">Save</button>
            <button className="btn btn-sm" type="button" onClick={()=>setEditing(false)}>Cancel</button>
          </div>
        </form>
      )}
      {/* ----------------------------------------------------------- */}

      {/* new thread form ------------------------------------------ */}
      <form className="mb-6 space-y-2" onSubmit={onSubmit}>
        <input className="input input-bordered w-full" placeholder="New thread subject"
               value={subject} onChange={(e)=>setSubject(e.target.value)} />
        <textarea className="textarea textarea-bordered w-full" rows={4}                 // NEW
                  placeholder="Body" value={body} onChange={(e)=>setBody(e.target.value)} />
        <input
          type="file"
          accept="image/*,video/*"          // was images only
          onChange={onFileChange}
        />
        {error && <p className="text-red-600 text-sm">{error}</p>}
        <button className="btn btn-primary" disabled={submitting}>{submitting ? 'Posting…' : 'Post Thread'}</button>
      </form>
      {/* ----------------------------------------------------------- */}

      <ul>
        {isFetching && <li>Loading…</li>}
        {!isFetching && sortedThreads.map(t => (
          <li key={t.id} className={`mb-1 flex items-center gap-2 ${t.deleted_at ? 'opacity-60' : ''}`}>
            <span className="text-xs text-gray-500 mr-1">#{t.id}</span>
            <Link className="link font-medium" to={`/thread/${t.id}`}>{t.subject}</Link>
            {t.deleted_at && <span className="badge badge-error badge-sm">Deleted</span>}
            <span className="ml-2 text-xs text-gray-500">
              (last {new Date(t.bump_time).toLocaleString()}, created {new Date(t.created_at).toLocaleString()})
            </span>
            {t.image_hash && (
              t.mime?.startsWith('image/')
                ? <img className="inline-block h-6 ml-2 cursor-pointer" src={imageUrl(t.image_hash)} alt=""
                       onClick={() => setViewer({ hash: t.image_hash!, mime: t.mime ?? null })} />
                : <span className="ml-2 cursor-pointer"
                       onClick={() => setViewer({ hash: t.image_hash!, mime: t.mime ?? null })}>📹</span>
            )}
            {user?.role === 'admin' && (
              <div className="flex items-center gap-1 ml-2">
                {!t.deleted_at && <button className="btn btn-xs" onClick={async()=>{ await apiClient.softDelete('threads', t.id); refreshThreads(); }}>Soft</button>}
                {t.deleted_at && <button className="btn btn-xs" onClick={async()=>{ await apiClient.restore('threads', t.id); refreshThreads(); }}>Restore</button>}
                <button className="btn btn-xs btn-error" onClick={async()=>{ if(confirm('Hard delete thread? This cannot be undone.')) { await apiClient.hardDelete('threads', t.id); refreshThreads(); } }}>Hard</button>
              </div>
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