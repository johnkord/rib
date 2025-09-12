import { QueryClient } from '@tanstack/react-query';
import { getAuthToken } from './auth';

export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      // Disable automatic refetching when window gains focus
      refetchOnWindowFocus: false,
      // Keep other default behaviors for manual refresh functionality
      refetchOnMount: true,
      refetchOnReconnect: true,
    },
  },
});

export const API_BASE =
  import.meta.env.VITE_API_BASE ?? (import.meta.env.DEV ? 'http://localhost:8080' : '');

// helper to build versioned API paths (adds /api/v1 if missing)
function apiUrl(path: string) {
  const p = path.startsWith('/api/') ? path : `/api/v1${path}`;
  return `${API_BASE}${p}`;
}

async function handle<T>(res: Response): Promise<T> {
  if (!res.ok) throw new Error(await res.text());
  return res.json() as Promise<T>;
}

export async function fetchJson<T>(path: string): Promise<T> {
  const token = getAuthToken();
  const headers: HeadersInit = {
    'Content-Type': 'application/json',
  };

  if (token) {
    headers['Authorization'] = `Bearer ${token}`;
  }

  const res = await fetch(apiUrl(path), { headers });
  return handle<T>(res);
}

export async function postJson<TReq, TRes>(path: string, body: TReq): Promise<TRes> {
  const token = getAuthToken();
  const headers: HeadersInit = {
    'Content-Type': 'application/json',
  };

  if (token) {
    headers['Authorization'] = `Bearer ${token}`;
  }

  const res = await fetch(apiUrl(path), {
    method: 'POST',
    headers,
    body: JSON.stringify(body),
  });
  return handle<TRes>(res);
}

export async function patchJson(path: string, data: any) {
  const token = getAuthToken();
  const headers: HeadersInit = {
    'Content-Type': 'application/json',
  };

  if (token) {
    headers['Authorization'] = `Bearer ${token}`;
  }

  const res = await fetch(apiUrl(path), {
    method: 'PATCH',
    headers,
    body: JSON.stringify(data),
  });
  return handle(res);
}

export async function uploadImage(
  file: File,
): Promise<{ hash: string; mime: string; size: number }> {
  const token = getAuthToken();
  const form = new FormData();
  form.append('file', file);

  const headers: HeadersInit = {};
  if (token) {
    headers['Authorization'] = `Bearer ${token}`;
  }

  const res = await fetch(apiUrl('/images'), {
    method: 'POST',
    body: form,
    headers,
  });

  // 201 Created  ➜ new upload
  // 200 OK       ➜ duplicate (idempotent response)
  if (res.status === 201 || res.status === 200) {
    return res.json();
  }
  // any other status is an error
  throw new Error(await res.text());
}

// Helper
/**
 * Builds a full URL to an uploaded image/hash.
 * In dev it points to the backend (`http://localhost:8080/images/{hash}`),
 * in production it stays a relative `/images/{hash}`.
 */
export function imageUrl(hash: string) {
  return `${API_BASE}/images/${hash}`; // public non-versioned route
}

// -------- Bitcoin Auth helpers -----------------------------------
export async function requestBitcoinChallenge(address: string): Promise<{ challenge: string }> {
  return postJson('/auth/bitcoin/challenge', { address });
}

export async function verifyBitcoinAddress(
  address: string,
  signature: string,
): Promise<{ token: string }> {
  return postJson('/auth/bitcoin/verify', { address, signature });
}
// -----------------------------------------------------------------

// --- ApiClient class ---
export class ApiClient {
  baseUrl: string;

  constructor(baseUrl: string) {
    this.baseUrl = baseUrl;
  }

  async setDiscordRole(discordId: string, role: string): Promise<void> {
    const token = getAuthToken();
    if (!token) throw new Error('Not authenticated');

    const response = await fetch(apiUrl('/admin/discord-roles'), {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        Authorization: `Bearer ${token}`,
      },
      body: JSON.stringify({ discord_id: discordId, role }),
    });

    if (!response.ok) {
      const error = await response.json();
      throw new Error(error.message || 'Failed to set role');
    }
  }

  async softDelete(kind: 'boards' | 'threads' | 'replies', id: number) {
    await this._moderation(kind, id, 'soft-delete', 'POST');
  }
  async restore(kind: 'boards' | 'threads' | 'replies', id: number) {
    await this._moderation(kind, id, 'restore', 'POST');
  }
  async hardDelete(kind: 'boards' | 'threads' | 'replies', id: number) {
    await this._moderation(kind, id, '', 'DELETE');
  }

  private async _moderation(kind: string, id: number, action: string, method: 'POST' | 'DELETE') {
    const token = getAuthToken();
    if (!token) throw new Error('Not authenticated');
    const path = action ? `/admin/${kind}/${id}/${action}` : `/admin/${kind}/${id}`;
    const res = await fetch(apiUrl(path), {
      method,
      headers: { Authorization: `Bearer ${token}` },
    });
    if (!res.ok) throw new Error(await res.text());
  }
}

// Export an instance of ApiClient
export const apiClient = new ApiClient(`${API_BASE}/api/v1`);
