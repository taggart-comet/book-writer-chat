import { get, writable } from 'svelte/store';

import {
  ApiError,
  type AuthSessionResponse,
  fetchWebSession,
  login,
  refresh
} from '$lib/api';
import { uiCopy } from '$lib/web-shell/language';
import {
  clearStoredToken,
  loadStoredAuthSession,
  storeAuthSession,
  type StoredAuthSession
} from '$lib/web-auth';

export type AuthState = 'booting' | 'anonymous' | 'submitting' | 'authenticated';

const SESSION_RESUME_RETRY_DELAYS_MS = [250, 500, 1000, 2000];

export const authState = writable<AuthState>('booting');
export const authToken = writable<string | null>(null);
export const sessionUser = writable('');
export const authErrorMessage = writable('');

let initializePromise: Promise<void> | null = null;
let refreshPromise: Promise<boolean> | null = null;

function toStoredAuthSession(session: AuthSessionResponse): StoredAuthSession {
  return {
    accessToken: session.access_token,
    accessTokenExpiresAt: session.access_token_expires_at,
    refreshToken: session.refresh_token,
    refreshTokenExpiresAt: session.refresh_token_expires_at
  };
}

function resetAnonymousState() {
  clearStoredToken();
  authToken.set(null);
  sessionUser.set('');
  authState.set('anonymous');
}

function wait(delayMs: number) {
  return new Promise<void>((resolve) => {
    window.setTimeout(resolve, delayMs);
  });
}

async function refreshSession() {
  if (refreshPromise) {
    return refreshPromise;
  }

  refreshPromise = (async () => {
    const storedSession = loadStoredAuthSession();
    if (!storedSession?.refreshToken) {
      return false;
    }

    try {
      const nextSession = toStoredAuthSession(await refresh(storedSession.refreshToken));
      storeAuthSession(nextSession);
      authToken.set(nextSession.accessToken);
      return true;
    } catch {
      return false;
    } finally {
      refreshPromise = null;
    }
  })();

  return refreshPromise;
}

async function resumeSession(session: StoredAuthSession) {
  authToken.set(session.accessToken);

  for (const delayMs of [0, ...SESSION_RESUME_RETRY_DELAYS_MS]) {
    if (delayMs > 0) {
      await wait(delayMs);
    }

    try {
      const webSession = await withAuthorizedRequest((token) => fetchWebSession(token));
      sessionUser.set(webSession.username);
      authErrorMessage.set('');
      authState.set('authenticated');
      return;
    } catch (error) {
      if (error instanceof ApiError && error.status === 401) {
        return;
      }

      if (error instanceof ApiError && error.code === 'network_error') {
        continue;
      }

      resetAnonymousState();
      authErrorMessage.set(sessionFailureMessage(error));
      return;
    }
  }

  resetAnonymousState();
  authErrorMessage.set(get(uiCopy).loginUnavailable);
}

export async function initializeSession() {
  if (initializePromise) {
    return initializePromise;
  }

  initializePromise = (async () => {
    const storedSession = loadStoredAuthSession();
    if (!storedSession) {
      authState.set('anonymous');
      return;
    }

    await resumeSession(storedSession);
  })().finally(() => {
    initializePromise = null;
  });

  return initializePromise;
}

export async function withAuthorizedRequest<T>(request: (token: string) => Promise<T>) {
  const token = get(authToken);
  if (!token) {
    throw new ApiError('Authentication is required.', {
      code: 'missing_token',
      status: 401
    });
  }

  try {
    return await request(token);
  } catch (error) {
    if (!(error instanceof ApiError) || error.status !== 401) {
      throw error;
    }

    const refreshed = await refreshSession();
    const nextToken = get(authToken);
    if (!refreshed || !nextToken) {
      resetAnonymousState();
      authErrorMessage.set(get(uiCopy).sessionExpired);
      throw error;
    }

    return await request(nextToken);
  }
}

export async function submitLogin(username: string, password: string) {
  authState.set('submitting');
  authErrorMessage.set('');

  try {
    const response = await login(username.trim(), password);
    const session = toStoredAuthSession(response);
    storeAuthSession(session);
    await resumeSession(session);
  } catch (error) {
    authState.set('anonymous');
    authErrorMessage.set(loginFailureMessage(error));
  }
}

function loginFailureMessage(error: unknown) {
  if (error instanceof ApiError && error.status === 401) {
    return get(uiCopy).invalidUsernameOrPassword;
  }

  return get(uiCopy).loginUnavailable;
}

function sessionFailureMessage(error: unknown) {
  if (error instanceof ApiError && error.status === 401) {
    return get(uiCopy).sessionExpired;
  }

  return get(uiCopy).unableToRestoreSession;
}
