import createClient, { type Middleware } from 'openapi-fetch';
import type { components, paths } from './schema';

export type Session = components['schemas']['SessionResponse'];
export type AuditEntry = components['schemas']['AuditEntry'];
export type ApiError = components['schemas']['ApiError'];
export type Role = 'admin' | 'gm' | 'leitura';

let csrfToken = '';

/** Remember the CSRF token of the current session (sent back on every mutation). */
export function setCsrfToken(token: string) {
  csrfToken = token;
}

const csrf: Middleware = {
  onRequest({ request }) {
    if (request.method !== 'GET' && request.method !== 'HEAD' && csrfToken) {
      request.headers.set('X-CSRF-Token', csrfToken);
    }
    return request;
  },
};

export const api = createClient<paths>({ baseUrl: '/', credentials: 'same-origin' });
api.use(csrf);

/** Error carrying the API's stable code, for messages and redirects. */
export class ApiFailure extends Error {
  constructor(
    readonly status: number,
    readonly code: string,
    message: string,
  ) {
    super(message);
  }
}

export function fail(response: Response, error: ApiError | undefined): never {
  throw new ApiFailure(
    response.status,
    error?.code ?? 'indisponivel',
    error?.message ?? 'Servidor indisponível.',
  );
}
