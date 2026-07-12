import { createContext, createElement, ReactNode, useContext, useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { removeAuthToken } from '../lib/auth';
import { fetchJson, logoutSession } from '../lib/api';

export interface User {
  id: string; // was number
  username: string;
  discord_id: string;
  role: 'user' | 'moderator' | 'admin';
}

interface AuthContextValue {
  user: User | null;
  loading: boolean;
  refresh: () => Promise<void>;
  logout: () => Promise<void>;
}

const AuthContext = createContext<AuthContextValue | null>(null);

export function AuthProvider({ children }: { children: ReactNode }) {
  const [user, setUser] = useState<User | null>(null);
  const [loading, setLoading] = useState(true);
  const navigate = useNavigate();

  const refresh = async () => {
    try {
      const currentUser = await fetchJson<User | null>('/auth/me');
      setUser(currentUser);
    } catch {
      removeAuthToken();
      setUser(null);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    void refresh();
    const handler = () => void refresh();
    window.addEventListener('storage', handler); // multi-tab
    return () => {
      window.removeEventListener('storage', handler);
    };
  }, []);

  const logout = async () => {
    try {
      await logoutSession();
    } finally {
      removeAuthToken();
      setUser(null);
      navigate('/');
    }
  };

  return createElement(
    AuthContext.Provider,
    { value: { user, loading, refresh, logout } },
    children,
  );
}

export function useAuth() {
  const context = useContext(AuthContext);
  if (!context) {
    throw new Error('useAuth must be used within AuthProvider');
  }
  return context;
}
