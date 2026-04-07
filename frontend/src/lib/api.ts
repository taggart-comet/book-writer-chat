import { apiUrl } from '$lib/config';

export type ReaderSummary = {
  book_id: string;
  title: string;
  subtitle: string;
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

export class ReaderApiError extends Error {
  code: string;
  status: number;

  constructor(message: string, options: { code?: string; status: number }) {
    super(message);
    this.name = 'ReaderApiError';
    this.code = options.code ?? 'unknown_error';
    this.status = options.status;
  }
}

async function getJson<T>(path: string): Promise<T> {
  const response = await fetch(apiUrl(path));
  if (!response.ok) {
    const text = await response.text();
    try {
      const payload = JSON.parse(text) as ReaderApiErrorPayload;
      throw new ReaderApiError(payload.message ?? text, {
        code: payload.code,
        status: response.status
      });
    } catch {
      throw new ReaderApiError(text, {
        status: response.status
      });
    }
  }
  return await response.json();
}

export function fetchSummary(token: string) {
  return getJson<ReaderSummary>(`/api/reader/summary?token=${encodeURIComponent(token)}`);
}

export function fetchContent(
  token: string,
  options: { cursor?: string | null; revisionId?: string | null } = {}
) {
  const params = new URLSearchParams({ token });
  if (options.cursor) {
    params.set('cursor', options.cursor);
  }
  if (options.revisionId) {
    params.set('revision_id', options.revisionId);
  }
  return getJson<ReaderContent>(`/api/reader/content?${params.toString()}`);
}

export function fetchRevision(token: string) {
  return getJson<ReaderRevision>(`/api/reader/revision?token=${encodeURIComponent(token)}`);
}

export function fetchJob(token: string) {
  return getJson<ReaderJob>(`/api/reader/job?token=${encodeURIComponent(token)}`);
}
