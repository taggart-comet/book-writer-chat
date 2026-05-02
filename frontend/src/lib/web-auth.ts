const TOKEN_STORAGE_KEY = 'book-writer-chat.jwt';

export type StoredAuthSession = {
  accessToken: string;
  accessTokenExpiresAt: string;
  refreshToken: string;
  refreshTokenExpiresAt: string;
};

export function loadStoredAuthSession(): StoredAuthSession | null {
  if (typeof window === 'undefined') {
    return null;
  }

  const raw = window.localStorage.getItem(TOKEN_STORAGE_KEY);
  if (raw) {
    try {
      const parsed = JSON.parse(raw) as Partial<StoredAuthSession>;
      if (
        typeof parsed.accessToken === 'string' &&
        typeof parsed.accessTokenExpiresAt === 'string' &&
        typeof parsed.refreshToken === 'string' &&
        typeof parsed.refreshTokenExpiresAt === 'string'
      ) {
        return {
          accessToken: parsed.accessToken,
          accessTokenExpiresAt: parsed.accessTokenExpiresAt,
          refreshToken: parsed.refreshToken,
          refreshTokenExpiresAt: parsed.refreshTokenExpiresAt
        };
      }
    } catch {
      window.localStorage.removeItem(TOKEN_STORAGE_KEY);
    }
  }

  const legacyToken = window.sessionStorage.getItem(TOKEN_STORAGE_KEY);
  if (!legacyToken) {
    return null;
  }

  return {
    accessToken: legacyToken,
    accessTokenExpiresAt: '',
    refreshToken: '',
    refreshTokenExpiresAt: ''
  };
}

export function storeAuthSession(session: StoredAuthSession) {
  if (typeof window === 'undefined') {
    return;
  }

  window.localStorage.setItem(TOKEN_STORAGE_KEY, JSON.stringify(session));
  window.sessionStorage.removeItem(TOKEN_STORAGE_KEY);
}

export function loadStoredToken(): string | null {
  return loadStoredAuthSession()?.accessToken ?? null;
}

export function clearStoredToken() {
  if (typeof window === 'undefined') {
    return;
  }

  window.localStorage.removeItem(TOKEN_STORAGE_KEY);
  window.sessionStorage.removeItem(TOKEN_STORAGE_KEY);
}
