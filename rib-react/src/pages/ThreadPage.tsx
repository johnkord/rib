import { FormEvent, useState, useMemo, useEffect } from 'react';
import { useParams, Link, useLocation } from 'react-router-dom';
import { useReplies, useCreateReply } from '../hooks/useReplies';
import { useAuth } from '../hooks/useAuth';
import { useQuery } from '@tanstack/react-query';
import { fetchJson, imageUrl, apiClient } from '../lib/api';
import { useBoards } from '../hooks/useBoards';
import MediaModal from '../components/MediaModal';
import { linkifyText } from '../lib/linkify';

interface Thread {
  id: number;
  board_id: number;
  subject: string;
  body: string;
  created_at: string;
  bump_time:  string;
  image_hash?: string;
  mime?: string | null;
  deleted_at?: string | null;
}
type MediaItem = { hash: string; mime: string | null };

export function ThreadPage() {
  const { id } = useParams();
  const threadId = id ? Number(id) : null;
  const { user } = useAuth();
  const thread = useQuery<Thread>({
    queryKey: ['thread', threadId, user?.role === 'admin'],
    queryFn: () => fetchJson(`/threads/${threadId}${user?.role === 'admin' ? '?include_deleted=1' : ''}`),
    enabled: !!threadId
  });
  // Admins should, by default, see soft-deleted replies so they don't "disappear" immediately.
  const [showDeleted, setShowDeleted] = useState(false);
  const [autoSet, setAutoSet] = useState(false);
  useEffect(() => {
    if (user?.role === 'admin' && !autoSet) {
      setShowDeleted(true);
      setAutoSet(true);
    }
  }, [user, autoSet]);
  const { data: replies, isFetching, refetch: refreshReplies } = useReplies(threadId, user?.role === 'admin' && showDeleted);
    const createReply = useCreateReply();
    const [content, setContent] = useState('');
    const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [file, setFile] = useState<File | null>(null);
  const { data: boards } = useBoards(false);
  const boardSlug = boards?.find(b => b.id === thread.data?.board_id)?.slug;
  const location = useLocation();
  const [viewer, setViewer] = useState<{ index: number; items: MediaItem[] } | null>(null);

  // highlighted reply id (permanent while hash matches) --------------
  const highlightId = useMemo(() => {
    const m = location.hash.match(/^#p(\d+)$/);
    return m ? Number(m[1]) : null;
  }, [location.hash]);
  // ------------------------------------------------------------------

  function onFileChange(e: React.ChangeEvent<HTMLInputElement>) {
    setFile(e.target.files?.[0] ?? null);
  }

  async function onSubmit(e: FormEvent) {
    e.preventDefault();
    if (!threadId) return;
    // require at least text OR a file
    if (!content.trim() && !file) {             // CHANGED
      setError('Text or attachment required');
      return;
    }
    try {
      setSubmitting(true);
      setError(null);
      await createReply(threadId, content.trim(), file);
      setContent('');
      setFile(null);
    } catch (err: any) {
      setError(err.message);
    } finally {
      setSubmitting(false);
    }
  }

  // --- scroll to reply if hash present (retry loop) -------------------
  useEffect(() => {
    if (!location.hash || !replies) return;
    const targetId = location.hash.slice(1);
    let attempts = 0;
    const timer = setInterval(() => {
      const el = document.getElementById(targetId);
      if (el) {
        el.scrollIntoView({ behavior: 'smooth', block: 'center' });
        clearInterval(timer);
      } else if (attempts++ > 10) {            // give up after ~1 s
        clearInterval(timer);
      }
    }, 100);                                   // retry every 100 ms
    return () => clearInterval(timer);
  }, [location.hash, replies]);
  // -------------------------------------------------------------------

  // sort replies ascending by created_at
  const sortedReplies = useMemo(
    () => [...(replies ?? [])].sort(
      (a, b) => new Date(a.created_at).getTime() - new Date(b.created_at).getTime()
    ),
    [replies]
  );

  // gather media in display order ------------------------------------
  const mediaItems = useMemo<MediaItem[]>(() => {
    const items: MediaItem[] = [];
    if (thread.data?.image_hash) items.push({ hash: thread.data.image_hash, mime: thread.data.mime ?? null });
    sortedReplies.forEach(r => { if (r.image_hash) items.push({ hash: r.image_hash, mime: r.mime ?? null }); });
    return items;
  }, [thread.data, sortedReplies]);
  // ------------------------------------------------------------------

  // helper to open viewer --------------------------------------------
  const openViewer = (hash: string) => {
    const idx = mediaItems.findIndex(m => m.hash === hash);
    if (idx !== -1) setViewer({ index: idx, items: mediaItems });
  };
  // ------------------------------------------------------------------

  return (
    <div>
      {/* top back-link */}
      {boardSlug && (
        <p className="mb-2">
          <Link className="link" to={`/${boardSlug}`}>&larr; Back to Threads</Link>
        </p>
      )}

      {/* permalink to this thread */}
      {!thread.isFetching && thread.data && (
        <p className="mb-1 text-sm text-gray-500">
          Link:&nbsp;
        {user?.role === 'admin' && (
          <label className="flex items-center gap-1 text-xs ml-3">
            <input type="checkbox" checked={showDeleted} onChange={e=>{ setShowDeleted(e.target.checked); refreshReplies(); }} /> Show deleted
          </label>
        )}
          <Link className="link" to={`/thread/${thread.data.id}`}>/thread/{thread.data.id}</Link>
        </p>
      )}

      {/* thread meta ------------------------------------------------ */}
      {thread.isFetching && <p>Loading thread…</p>}
      {!thread.isFetching && thread.data && (
        <>
          <p className="text-sm text-gray-500 mb-1">
            Last post {new Date(thread.data.bump_time).toLocaleString()} •
            &nbsp;Created {new Date(thread.data.created_at).toLocaleString()}
          </p>
          <h1 className="text-2xl mb-2">
            <Link className="link" to={`/thread/${thread.data.id}`}>{thread.data.subject}</Link>
          </h1>
          {user && (user.role === 'admin' || user.role === 'moderator') && thread.data.deleted_at && (
            <div className="alert alert-warning mb-3 py-2 px-3 text-sm flex items-center justify-between">
              <span>Thread deleted (admin view)</span>
              <div className="flex gap-2">
                <button className="btn btn-xs" onClick={async()=>{ await apiClient.restore('threads', thread.data!.id); await thread.refetch(); }}>Restore</button>
                <button className="btn btn-xs btn-error" onClick={async()=>{ if(confirm('Hard delete forever?')) { await apiClient.hardDelete('threads', thread.data!.id); window.location.href = '/'; } }}>Hard Delete</button>
              </div>
            </div>
          )}
          {user && (user.role === 'admin' || user.role === 'moderator') && !thread.data.deleted_at && (
            <div className="mb-3 flex gap-2">
              <button className="btn btn-xs" onClick={async()=>{ await apiClient.softDelete('threads', thread.data!.id); await thread.refetch(); }}>Soft Delete</button>
              <button className="btn btn-xs btn-error" onClick={async()=>{ if(confirm('Hard delete thread permanently?')) { await apiClient.hardDelete('threads', thread.data!.id); window.location.href = '/'; } }}>Hard Delete</button>
            </div>
          )}
          <p className={`mb-4 whitespace-pre-wrap ${thread.data.deleted_at ? 'opacity-60 line-through' : ''}`}>
            {linkifyText(thread.data.body)}
          </p>
        </>
      )}
      {/* ------------------------------------------------------------ */}

      {!thread.isFetching && thread.data && thread.data.image_hash && (
        thread.data.mime?.startsWith('image/')
          ? (
            <img
              className="max-w-xs mb-4 cursor-pointer"
              src={imageUrl(thread.data.image_hash)}
              alt="attachment"
              onClick={() => openViewer(thread.data.image_hash!)}
            />
          )
          : (
            <video
              className="max-w-xs mb-4 cursor-pointer"
              controls
              src={imageUrl(thread.data.image_hash)}
              onClick={() => openViewer(thread.data.image_hash!)}
            ></video>
          )
      )}
      <h2 className="text-lg font-semibold mb-2 flex items-center"> {/* UPDATED */}
        Replies
        <button
          className="btn btn-xs ml-2"
          onClick={async () => {
            await refreshReplies();   // update replies list
            await thread.refetch();   // ALSO update bump_time / last-post meta
          }}
          disabled={isFetching}
        >
          {isFetching ? 'Refreshing…' : 'Refresh'}
        </button>
      </h2>
      <form className="mb-4 space-y-2" onSubmit={onSubmit}>
        <textarea className="textarea textarea-bordered w-full" rows={3} placeholder="Reply..." value={content} onChange={(e)=>setContent(e.target.value)} />
        <input type="file" accept="image/*,video/*" onChange={onFileChange} />
        {error && <p className="text-red-600 text-sm">{error}</p>}
        <button className="btn btn-secondary" disabled={submitting}>{submitting ? 'Posting…' : 'Reply'}</button>
      </form>
      {isFetching && <p>Loading replies…</p>}
      {!isFetching && (
        <ul>
          {sortedReplies.map(r => {
            const deleted = !!r.deleted_at;
            return (
              <li
                key={r.id}
                id={`p${r.id}`}
                className={`mb-2 border-b pb-2 transition ${highlightId === r.id ? 'bg-yellow-100 dark:bg-yellow-200' : ''} ${deleted ? 'opacity-60' : ''}`}
              >
                <div className="mb-1 text-xs text-gray-500 flex items-center gap-2">
                  <Link className="link" to={`/thread/${threadId}#p${r.id}`}>No.{r.id}</Link>
                  <span>{new Date(r.created_at).toLocaleString()}</span>
                  {deleted && <span className="badge badge-error badge-outline">Deleted</span>}
          {user && (user.role === 'admin' || user.role === 'moderator') && (
                    <div className="ml-auto flex gap-1">
                      {!deleted && <button className="btn btn-ghost btn-xs" onClick={async()=>{ await apiClient.softDelete('replies', r.id); await refreshReplies(); }}>Soft</button>}
                      {deleted && <button className="btn btn-ghost btn-xs" onClick={async()=>{ await apiClient.restore('replies', r.id); await refreshReplies(); }}>Restore</button>}
            {user.role === 'admin' && <button className="btn btn-ghost btn-xs text-error" onClick={async()=>{ if(confirm('Hard delete this reply permanently?')) { await apiClient.hardDelete('replies', r.id); await refreshReplies(); } }}>Hard</button>}
                    </div>
                  )}
                </div>
                <p className={`whitespace-pre-wrap ${deleted ? 'line-through' : ''}`}>
                  {linkifyText(r.content)}
                </p>
                {r.image_hash && r.mime?.startsWith('image/') && (
                  <img
                    className="max-w-xs mt-1 cursor-pointer"
                    src={imageUrl(r.image_hash)}
                    alt="attachment"
                    onClick={() => openViewer(r.image_hash!)}
                  />
                )}
                {r.image_hash && r.mime?.startsWith('video/') && (
                  <video
                    className="max-w-xs mt-1 cursor-pointer"
                    controls
                    src={imageUrl(r.image_hash)}
                    onClick={() => openViewer(r.image_hash!)}
                  ></video>
                )}
              </li>
            );
          })}
        </ul>
      )}

      {/* media viewer -------------------------------------------------- */}
      {viewer && (
        <MediaModal
          hash={viewer.items[viewer.index].hash}
          mime={viewer.items[viewer.index].mime}
          hasPrev={viewer.index > 0}
          hasNext={viewer.index < viewer.items.length - 1}
          onPrev={viewer.index > 0 ? () => setViewer(v => v && ({ ...v, index: v.index - 1 })) : undefined}
          onNext={viewer.index < viewer.items.length - 1 ? () => setViewer(v => v && ({ ...v, index: v.index + 1 })) : undefined}
          onClose={() => setViewer(null)}
        />
      )}
      {/* --------------------------------------------------------------- */}

      {/* bottom back-link */}
      {boardSlug && (
        <p className="mt-6">
          <Link className="link" to={`/${boardSlug}`}>&larr; Back to Threads</Link>
        </p>
      )}
    </div>
  );
}