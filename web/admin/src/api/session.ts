import { queryOptions } from '@tanstack/react-query';
import { api, fail, setCsrfToken, type Session } from './client';

export const sessionQuery = queryOptions({
  queryKey: ['session'],
  queryFn: async (): Promise<Session | null> => {
    const { data, error, response } = await api.GET('/api/session');
    if (response.status === 401) return null;
    if (!data) fail(response, error);
    setCsrfToken(data.csrf_token);
    return data;
  },
  staleTime: 60_000,
  retry: false,
});

export async function login(username: string, password: string): Promise<Session> {
  const { data, error, response } = await api.POST('/api/session/login', {
    body: { username, password },
  });
  if (!data) fail(response, error);
  setCsrfToken(data.csrf_token);
  return data;
}

/** Open registration: a new account without a panel role, logged in. */
export async function register(username: string, password: string): Promise<Session> {
  const { data, error, response } = await api.POST('/api/session/register', {
    body: { username, password },
  });
  if (!data) fail(response, error);
  setCsrfToken(data.csrf_token);
  return data;
}

export async function logout(): Promise<void> {
  const { error, response } = await api.POST('/api/session/logout');
  if (!response.ok) fail(response, error);
  setCsrfToken('');
}

/** Where an account lands after logging in: administration for a role, else its own data. */
export function homeOf(session: Session | null | undefined) {
  return session?.role ? '/contas' : '/minha-conta';
}
