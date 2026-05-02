import { apiUrl } from '$lib/config';

export type ReaderSummary = {
  book_id: string;
  title: string;
  subtitle: string;
  language: 'en' | 'ru';
  status: string;
  last_revision_id: string | null;
  last_updated_at: string;
  render_status: string;
  chapter_count: number;
};

export type ReaderContent = {
  revision_id: string;
  content_hash: string;
  chapter_index: number;
  chapter_id: string;
  title: string;
  source_file: string;
  html: string;
  has_more: boolean;
  next_cursor: string | null;
};

export type ReaderRevision = {
  revision_id: string;
  created_at: string;
  source_job_id: string;
  summary: string;
  render_status: string;
  content_hash: string | null;
  render_error: string | null;
};

export type ReaderJob = {
  job_id: string;
  status: string;
  started_at: string | null;
  finished_at: string | null;
  user_facing_message: string | null;
};

export type ReaderApiErrorPayload = {
  code?: string;
  message?: string;
};

export type AuthSessionResponse = {
  access_token: string;
  access_token_expires_at: string;
  refresh_token: string;
  refresh_token_expires_at: string;
};

export type WebSession = {
  username: string;
};

export type RefreshRequest = {
  refresh_token: string;
};

export type WebBook = {
  book_id: string;
  slug: string;
  title: string;
  subtitle: string;
  language: 'en' | 'ru';
  created_at: string;
  updated_at: string;
};

export type CreateBookRequest = {
  title: string;
  language?: 'en' | 'ru';
};

export type WebConversation = {
  conversation_id: string;
  book_id: string;
  title: string;
  created_at: string;
  updated_at: string;
  last_active_at: string;
  status: string;
};

export type CreateConversationRequest = {
  title?: string;
};

export type WebConversationMessage = {
  message_id: string;
  role: string;
  text: string;
  timestamp?: string;
};

export type ConversationMessagesResponse = {
  status: string;
  last_comment: string | null;
  messages: WebConversationMessage[];
};

export type SubmitConversationMessageResponse = {
  conversation_id: string;
  status: string;
};

export class ApiError extends Error {
  code: string;
  status: number;

  constructor(message: string, options: { code?: string; status: number }) {
    super(message);
    this.name = 'ApiError';
    this.code = options.code ?? 'unknown_error';
    this.status = options.status;
  }
}

export class ReaderApiError extends ApiError {
  constructor(message: string, options: { code?: string; status: number }) {
    super(message, options);
    this.name = 'ReaderApiError';
  }
}

async function requestJson<T>(path: string, init?: RequestInit): Promise<T> {
  let response: Response;

  try {
    response = await fetch(apiUrl(path), init);
  } catch (error) {
    if (error instanceof Error) {
      throw new ApiError('The backend is unreachable. Check that the API server is running.', {
        code: 'network_error',
        status: 0
      });
    }

    throw error;
  }

  if (!response.ok) {
    const text = await response.text();
    let code: string | undefined;
    let message = text;

    try {
      const payload = JSON.parse(text) as ReaderApiErrorPayload;
      code = payload.code;
      message = payload.message ?? text;
    } catch {
      // Keep the raw response text when the backend did not return structured JSON.
    }

    throw new ApiError(message, {
      code,
      status: response.status
    });
  }
  return await response.json();
}

async function getJson<T>(path: string): Promise<T> {
  try {
    return await requestJson<T>(path);
  } catch (error) {
    if (error instanceof ApiError) {
      throw new ReaderApiError(error.message, {
        code: error.code,
        status: error.status
      });
    }
    throw error;
  }
}

function rewriteReaderHtmlAssetUrls(html: string): string {
  return html.replace(
    /(<img\b[^>]*\bsrc=)(["'])(\/api\/reader\/[^/"']+\/assets\/)/gi,
    (_match, prefix: string, quote: string, assetPath: string) =>
      `${prefix}${quote}${apiUrl(assetPath)}`
  );
}

export function fetchSummary(bookId: string) {
  return getJson<ReaderSummary>(`/api/reader/${encodeURIComponent(bookId)}/summary`);
}

export function fetchContent(
  bookId: string,
  options: { cursor?: string | null; revisionId?: string | null } = {}
) {
  const params = new URLSearchParams();
  if (options.cursor) {
    params.set('cursor', options.cursor);
  }
  if (options.revisionId) {
    params.set('revision_id', options.revisionId);
  }
  const suffix = params.size > 0 ? `?${params.toString()}` : '';
  return getJson<ReaderContent>(
    `/api/reader/${encodeURIComponent(bookId)}/content${suffix}`
  ).then((content) => ({
    ...content,
    html: rewriteReaderHtmlAssetUrls(content.html)
  }));
}

export function fetchRevision(bookId: string) {
  return getJson<ReaderRevision>(`/api/reader/${encodeURIComponent(bookId)}/revision`);
}

export function fetchJob(bookId: string) {
  return getJson<ReaderJob>(`/api/reader/${encodeURIComponent(bookId)}/job`);
}

export function login(username: string, password: string) {
  return requestJson<AuthSessionResponse>('/api/auth/login', {
    method: 'POST',
    headers: {
      'content-type': 'application/json'
    },
    body: JSON.stringify({
      username,
      password
    })
  });
}

export function refresh(refreshToken: string) {
  return requestJson<AuthSessionResponse>('/api/auth/refresh', {
    method: 'POST',
    headers: {
      'content-type': 'application/json'
    },
    body: JSON.stringify({
      refresh_token: refreshToken
    } satisfies RefreshRequest)
  });
}

export function fetchWebSession(token: string) {
  return requestJson<WebSession>('/api/web/session', {
    headers: {
      authorization: `Bearer ${token}`
    }
  });
}

function authorizedHeaders(token: string, headers: HeadersInit = {}) {
  return {
    ...headers,
    authorization: `Bearer ${token}`
  };
}

export function fetchBooks(token: string) {
  return requestJson<WebBook[]>('/api/books', {
    headers: authorizedHeaders(token)
  });
}

export function createBook(token: string, payload: CreateBookRequest) {
  return requestJson<WebBook>('/api/books', {
    method: 'POST',
    headers: authorizedHeaders(token, {
      'content-type': 'application/json'
    }),
    body: JSON.stringify(payload)
  });
}

export function fetchConversations(token: string, bookId: string) {
  return requestJson<WebConversation[]>(`/api/books/${encodeURIComponent(bookId)}/conversations`, {
    headers: authorizedHeaders(token)
  });
}

export function createConversation(
  token: string,
  bookId: string,
  payload: CreateConversationRequest = {}
) {
  return requestJson<WebConversation>(
    `/api/books/${encodeURIComponent(bookId)}/conversations`,
    {
      method: 'POST',
      headers: authorizedHeaders(token, {
        'content-type': 'application/json'
      }),
      body: JSON.stringify(payload)
    }
  );
}

export function fetchConversationMessages(token: string, bookId: string, conversationId: string) {
  return requestJson<ConversationMessagesResponse>(
    `/api/books/${encodeURIComponent(bookId)}/conversations/${encodeURIComponent(conversationId)}/messages`,
    {
      headers: authorizedHeaders(token)
    }
  );
}

export function createConversationMessage(
  token: string,
  bookId: string,
  conversationId: string,
  payload: FormData
) {
  return requestJson<SubmitConversationMessageResponse>(
    `/api/books/${encodeURIComponent(bookId)}/conversations/${encodeURIComponent(conversationId)}/messages`,
    {
      method: 'POST',
      headers: authorizedHeaders(token),
      body: payload
    }
  );
}
