import { describe, it, expect, vi } from 'vitest';
import { fetchJson } from '../lib/api';

describe('fetchJson', () => {
  it('throws on non-ok response', async () => {
    const mockRes = { ok: false, text: () => Promise.resolve('bad'), json: () => Promise.resolve({}) } as any;
    const spy = vi.spyOn(globalThis as any, 'fetch').mockResolvedValue(mockRes);
    await expect(fetchJson('/x')).rejects.toThrow('bad');
    spy.mockRestore();
  });
});
