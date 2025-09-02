export interface User {
  username: string;
  role: 'user' | 'moderator' | 'admin';
  token: string;
}

const storeKey = 'auth';

export function getCurrentUser(): User | null {
  if (typeof localStorage === 'undefined') return null;
  const raw = localStorage.getItem(storeKey);
  return raw ? (JSON.parse(raw) as User) : null;
}

let current: User | null = getCurrentUser();
const listeners = new Set<(u: User | null) => void>();

function emit() { listeners.forEach((l) => l(current)); }

export function subscribe(listener: (u: User | null) => void) {
  listeners.add(listener); listener(current); return () => listeners.delete(listener);
}

function persist() {
  if (typeof localStorage === 'undefined') return;
  if (current) localStorage.setItem(storeKey, JSON.stringify(current)); else localStorage.removeItem(storeKey);
}

export async function login(username: string, _password: string) {
  current = { username, role: 'moderator', token: 'dev-token' }; // placeholder
  persist(); emit(); return current;
}

export function logout() { current = null; persist(); emit(); }
