import type { ApiErrorBody } from '@/types/api';

/**
 * Base URL of the Baitler backend, from `VITE_API_BASE_URL` (read from the
 * repo-root `.env`; see `vite.config.ts` `envDir`).
 *
 * In development it defaults to the local dev backend. In a production build the
 * variable is required — we throw rather than silently bake in `localhost`,
 * which would produce a quietly broken deployment.
 */
function resolveBaseUrl(): string {
  const fromEnv = import.meta.env.VITE_API_BASE_URL;
  if (fromEnv) return fromEnv;
  if (import.meta.env.DEV) return 'http://localhost:8080';
  throw new Error('VITE_API_BASE_URL must be set for production builds');
}

export const API_BASE_URL: string = resolveBaseUrl();

/** Error thrown for failed requests, carrying the backend's code/message. */
export class ApiError extends Error {
  readonly status: number;
  readonly code: string;

  constructor(status: number, code: string, message: string, options?: ErrorOptions) {
    super(message, options);
    this.name = 'ApiError';
    this.status = status;
    this.code = code;
  }
}

function isErrorBody(value: unknown): value is ApiErrorBody {
  if (typeof value !== 'object' || value === null || !('error' in value)) return false;
  const error = (value as { error?: unknown }).error;
  return (
    typeof error === 'object' &&
    error !== null &&
    typeof (error as { code?: unknown }).code === 'string' &&
    typeof (error as { message?: unknown }).message === 'string'
  );
}

/**
 * Typed JSON fetch wrapper around the backend.
 *
 * - Sends credentials so session cookies work once auth lands (added last).
 * - Forwards an `AbortSignal` when supplied (e.g. TanStack Query's `signal`).
 * - On a non-2xx response, throws {@link ApiError} from the standard
 *   `{ error: { code, message } }` envelope.
 * - On a 2xx response whose non-empty body fails to parse, throws rather than
 *   silently returning `null` (which would hide a protocol error).
 *
 * Returns `null` for a genuinely empty 2xx body (e.g. `204 No Content`).
 */
export async function apiFetch<T>(path: string, init: RequestInit = {}): Promise<T> {
  const { headers, ...rest } = init;

  let res: Response;
  try {
    res = await fetch(`${API_BASE_URL}${path}`, {
      credentials: 'include',
      headers: { Accept: 'application/json', ...headers },
      ...rest,
    });
  } catch (cause) {
    // Network-level failure (backend down, CORS, DNS, offline).
    throw new ApiError(0, 'network_error', 'Could not reach the server', { cause });
  }

  const raw = await res.text();
  let body: unknown = null;
  let parseFailed = false;
  if (raw.length > 0) {
    try {
      body = JSON.parse(raw);
    } catch {
      parseFailed = true;
    }
  }

  if (!res.ok) {
    if (isErrorBody(body)) {
      throw new ApiError(res.status, body.error.code, body.error.message);
    }
    throw new ApiError(res.status, 'http_error', res.statusText || 'Request failed');
  }

  if (parseFailed) {
    throw new ApiError(res.status, 'invalid_response', 'Server returned an invalid response');
  }

  return body as T;
}
