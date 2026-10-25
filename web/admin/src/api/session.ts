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

export async function logout(): Promise<void> {
  const { error, response } = await api.POST('/api/session/logout');
  if (!response.ok) fail(response, error);
  setCsrfToken('');
}
