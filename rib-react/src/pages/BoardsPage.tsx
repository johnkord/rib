import { FormEvent, useState, useEffect } from 'react';
import { useBoards, useCreateBoard } from '../hooks/useBoards';
import { Link } from 'react-router-dom';
import { useAuth } from '../hooks/useAuth';

export function BoardsPage() {
  const { user } = useAuth();
  const [showDeleted, setShowDeleted] = useState(false);
  const [autoSet, setAutoSet] = useState(false); // ensure we only force-enable once
  useEffect(() => {
    if (user?.role === 'admin' && !autoSet) {
      setShowDeleted(true);
      setAutoSet(true);
    }
  }, [user, autoSet]);
  const { data, isFetching, refetch } = useBoards(user?.role === 'admin' && showDeleted);
  const createBoard = useCreateBoard();
  const [slug, setSlug] = useState('');
  const [title, setTitle] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  async function onSubmit(e: FormEvent) {
    e.preventDefault();
    if (submitting) return;
    setError(null);
    if (!slug.trim() || !title.trim()) { setError('Slug and title required'); return; }
    try {
      setSubmitting(true);
  await createBoard(slug.trim(), title.trim());
  refetch();
      setSlug(''); setTitle('');
    } catch (err: any) { setError(err.message); }
    finally { setSubmitting(false); }
  }

  return (
    <div>
      <div className="flex items-center justify-between mb-4">
        <h1 className="text-2xl">Boards</h1>
  {user?.role === 'admin' && (
          <label className="flex items-center gap-2 text-sm cursor-pointer">
            <input type="checkbox" checked={showDeleted} onChange={e=>{setShowDeleted(e.target.checked); refetch();}} />
            Show deleted
          </label>
        )}
      </div>

      {/* Create-board form - Admins only ------------------------ */}
      {user?.role === 'admin' ? (
        <form className="mb-6 space-y-2" onSubmit={onSubmit}>
          <input className="input input-bordered w-full" placeholder="Slug" value={slug} onChange={(e)=>setSlug(e.target.value)} />
          <input className="input input-bordered w-full" placeholder="Title" value={title} onChange={(e)=>setTitle(e.target.value)} />
          {error && <p className="text-red-600 text-sm">{error}</p>}
          <button className="btn btn-primary" disabled={submitting}>{submitting ? 'Creating…' : 'Create Board'}</button>
        </form>
      ) : (
        <p>
        
        </p>
      )}
      {/* -------------------------------------------------------- */}

      <ul>
        {isFetching && <li>Loading…</li>}
        {!isFetching && data?.map(b => (
          <li key={b.id} className={`flex items-center gap-3 mb-1 ${b.deleted_at ? 'opacity-60' : ''}`}>
            <Link className="link" to={`/${b.slug}`}>/{b.slug}/ - {b.title}</Link>
            {b.deleted_at && <span className="badge badge-error badge-sm">Deleted</span>}
            {user?.role === 'admin' && (
              <span className="flex items-center gap-1 ml-2">
                {!b.deleted_at && <button className="btn btn-ghost btn-xs" onClick={async()=>{ await (await import('../lib/api')).apiClient.softDelete('boards', b.id); refetch(); }}>Soft</button>}
                {b.deleted_at && <button className="btn btn-ghost btn-xs" onClick={async()=>{ await (await import('../lib/api')).apiClient.restore('boards', b.id); refetch(); }}>Restore</button>}
                <button className="btn btn-ghost btn-xs text-error" onClick={async()=>{ if(confirm('Hard delete board and all threads?')) { await (await import('../lib/api')).apiClient.hardDelete('boards', b.id); refetch(); } }}>Hard</button>
              </span>
            )}
          </li>
        ))}
      </ul>
    </div>
  );
}
