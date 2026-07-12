import { render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { AuthProvider, useAuth } from '../hooks/useAuth';

function AuthConsumer({ label }: { label: string }) {
  const { loading, user } = useAuth();
  return <span data-testid={label}>{loading ? 'loading' : (user?.username ?? 'anonymous')}</span>;
}

describe('AuthProvider', () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('shares one anonymous session request across consumers', async () => {
    const fetchSpy = vi.spyOn(globalThis, 'fetch').mockResolvedValue({
      ok: true,
      json: () => Promise.resolve(null),
    } as Response);

    render(
      <MemoryRouter>
        <AuthProvider>
          <AuthConsumer label="first" />
          <AuthConsumer label="second" />
        </AuthProvider>
      </MemoryRouter>,
    );

    await waitFor(() => expect(screen.getByTestId('first').textContent).toBe('anonymous'));
    expect(screen.getByTestId('second').textContent).toBe('anonymous');
    expect(fetchSpy).toHaveBeenCalledTimes(1);
    expect(fetchSpy).toHaveBeenCalledWith(
      expect.stringContaining('/api/v1/auth/me'),
      expect.objectContaining({ credentials: 'include' }),
    );
  });

  it('shares the authenticated user across consumers', async () => {
    const fetchSpy = vi.spyOn(globalThis, 'fetch').mockResolvedValue({
      ok: true,
      json: () =>
        Promise.resolve({
          id: '42:alice',
          username: 'alice',
          discord_id: '42',
          role: 'moderator',
        }),
    } as Response);

    render(
      <MemoryRouter>
        <AuthProvider>
          <AuthConsumer label="first" />
          <AuthConsumer label="second" />
        </AuthProvider>
      </MemoryRouter>,
    );

    await waitFor(() => expect(screen.getByTestId('first').textContent).toBe('alice'));
    expect(screen.getByTestId('second').textContent).toBe('alice');
    expect(fetchSpy).toHaveBeenCalledTimes(1);
  });
});
