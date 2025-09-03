import { FormEvent, useState } from 'react';
import { useBoards, useCreateBoard } from '../hooks/useBoards';
import { Link } from 'react-router-dom';
import { useAuth } from '../hooks/useAuth';

export function BoardsPage() {
  const { user } = useAuth();
  const { data, isFetching } = useBoards();
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
      setSlug(''); setTitle('');
    } catch (err: any) { setError(err.message); }
    finally { setSubmitting(false); }
  }

  return (
    <div>
      <h1 className="text-2xl mb-4">Boards</h1>

      {/* Create-board form – Admins only ------------------------ */}
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
          <li key={b.id}><Link className="link" to={`/b/${b.slug}`}>/{b.slug}/ – {b.title}</Link></li>
        ))}
      </ul>
    </div>
  );
}
