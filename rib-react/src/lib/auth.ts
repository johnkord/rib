export interface User {
  username: string;
  role: 'user' | 'moderator' | 'admin';
  token: string;
}

const storeKey = 'auth';
const AUTH_TOKEN_KEY = 'rib_auth_token';

export function getCurrentUser(): User | null {
  if (typeof localStorage === 'undefined') return null;
  const raw = localStorage.getItem(storeKey);
  return raw ? (JSON.parse(raw) as User) : null;
}

let current: User | null = getCurrentUser();
const listeners = new Set<(u: User | null) => void>();

function emit() {
  listeners.forEach((l) => l(current));
}

export function subscribe(listener: (u: User | null) => void) {
  listeners.add(listener);
  listener(current);
  return () => listeners.delete(listener);
}

function persist() {
  if (typeof localStorage === 'undefined') return;
  if (current) localStorage.setItem(storeKey, JSON.stringify(current));
  else localStorage.removeItem(storeKey);
}

export async function login(username: string, _password: string) {
  current = { username, role: 'moderator', token: 'dev-token' }; // placeholder
  persist();
  emit();
  return current;
}

export function logout() {
  current = null;
  persist();
  emit();
}

export function setAuthToken(token: string): void {
  localStorage.setItem(AUTH_TOKEN_KEY, token);
  window.dispatchEvent(new Event('auth-token-set'));
}

export function getAuthToken(): string | null {
  return localStorage.getItem(AUTH_TOKEN_KEY);
}

export function removeAuthToken(): void {
  localStorage.removeItem(AUTH_TOKEN_KEY);
  window.dispatchEvent(new Event('auth-token-set'));
}
