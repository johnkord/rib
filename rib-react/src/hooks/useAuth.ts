import { useState, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { getAuthToken, removeAuthToken } from '../lib/auth';
import { fetchJson } from '../lib/api';

interface User {
  id: string;                // was number
  username: string;
  discord_id: string;
  role: 'user' | 'moderator' | 'admin';
}

export function useAuth() {
  const [user, setUser] = useState<User | null>(null);
  const [loading, setLoading] = useState(true);
  const navigate = useNavigate();

  useEffect(() => {
    async function load() {
      const token = getAuthToken();
      if (!token) { setUser(null); setLoading(false); return; }
      try {
        const u = await fetchJson<User>('/auth/me');   // path helper adds /api/v1
        setUser(u);
      } catch {
        removeAuthToken();
        setUser(null);
      } finally {
        setLoading(false);
      }
    }
    load();
    const handler = () => load();          // refetch on token change
    window.addEventListener('auth-token-set', handler);
    window.addEventListener('storage', handler); // multi-tab
    return () => {
      window.removeEventListener('auth-token-set', handler);
      window.removeEventListener('storage', handler);
    };
  }, []);

  const logout = () => {
    removeAuthToken();
    setUser(null);
    navigate('/');
  };

  return { user, loading, logout };
}
