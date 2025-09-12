import { describe, it, expect, vi } from 'vitest';
import { fetchJson, queryClient } from '../lib/api';

describe('fetchJson', () => {
  it('throws on non-ok response', async () => {
    const mockRes = {
      ok: false,
      text: () => Promise.resolve('bad'),
      json: () => Promise.resolve({}),
    } as any;
    const spy = vi.spyOn(globalThis as any, 'fetch').mockResolvedValue(mockRes);
    await expect(fetchJson('/x')).rejects.toThrow('bad');
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
