import { describe, it, expect, vi } from 'vitest';
import { fetchJson, logoutSession, queryClient } from '../lib/api';

describe('fetchJson', () => {
  it('throws on non-ok response', async () => {
    const mockRes = {
      ok: false,
      text: () => Promise.resolve('bad'),
      json: () => Promise.resolve({}),
    } as Response;
    const spy = vi.spyOn(globalThis, 'fetch').mockResolvedValue(mockRes);
    await expect(fetchJson('/x')).rejects.toThrow('bad');
    spy.mockRestore();
  });

  it('extracts structured API error messages', async () => {
    const mockRes = {
      ok: false,
      status: 403,
      text: () => Promise.resolve('{"error":"forbidden"}'),
    } as Response;
    const spy = vi.spyOn(globalThis, 'fetch').mockResolvedValue(mockRes);

    await expect(fetchJson('/x')).rejects.toThrow('forbidden');
    spy.mockRestore();
  });

  it('includes cookies on API requests', async () => {
    const mockRes = {
      ok: true,
      json: () => Promise.resolve({ ok: true }),
    } as Response;
    const spy = vi.spyOn(globalThis, 'fetch').mockResolvedValue(mockRes);

    await fetchJson('/x');

    expect(spy).toHaveBeenCalledWith(
      expect.stringContaining('/api/v1/x'),
      expect.objectContaining({ credentials: 'include' }),
    );
    spy.mockRestore();
  });

  it('logs out through the server session endpoint', async () => {
    const mockRes = { ok: true } as Response;
    const spy = vi.spyOn(globalThis, 'fetch').mockResolvedValue(mockRes);

    await logoutSession();

    expect(spy).toHaveBeenCalledWith(
      expect.stringContaining('/api/v1/auth/logout'),
      expect.objectContaining({ method: 'POST', credentials: 'include' }),
    );
    spy.mockRestore();
  });
});

describe('QueryClient configuration', () => {
  it('should disable refetchOnWindowFocus to prevent automatic refresh', () => {
    const config = queryClient.getDefaultOptions();
    expect(config.queries?.refetchOnWindowFocus).toBe(false);
  });

  it('should keep other refetch behaviors enabled for manual refresh', () => {
    const config = queryClient.getDefaultOptions();
    expect(config.queries?.refetchOnMount).toBe(true);
    expect(config.queries?.refetchOnReconnect).toBe(true);
  });
});
