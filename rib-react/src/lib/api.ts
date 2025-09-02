import { QueryClient } from '@tanstack/react-query';

export const queryClient = new QueryClient();

export const API_BASE = import.meta.env.VITE_API_BASE ?? (import.meta.env.DEV ? 'http://localhost:8080' : '');

async function handle<T>(res: Response): Promise<T> {
  if (!res.ok) throw new Error(await res.text());
  return res.json() as Promise<T>;
}

export async function fetchJson<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(`${API_BASE}/api/v1${path}`, init);
  return handle<T>(res);
}

export async function postJson<TReq, TRes>(path: string, body: TReq): Promise<TRes> {
  const res = await fetch(`${API_BASE}/api/v1${path}`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  });
  return handle<TRes>(res);
}

export async function uploadImage(
  file: File,
): Promise<{ hash: string; mime: string; size: number }> {
  const fd = new FormData();
  fd.append('file', file);
  const res = await fetch(`${API_BASE}/api/v1/images`, { method: 'POST', body: fd });

  // 201 Created  ➜ new upload
  // 409 Conflict ➜ duplicate (acceptable)
  if (res.status === 201 || res.status === 409) {
    return res.json();
  }
  // any other status is an error
  throw new Error(await res.text());
}

// --- new helper ---
/**
 * Builds a full URL to an uploaded image/hash.
 * In dev it points to the backend (`http://localhost:8080/images/{hash}`),
 * in production it stays a relative `/images/{hash}`.
 */
export function imageUrl(hash: string) {
  return `${API_BASE}/images/${hash}`;
}
