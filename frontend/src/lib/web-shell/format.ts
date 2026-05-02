import type { ApiError, WebBook, WebConversation } from '$lib/api';
import type { AppCopy, UiLanguage } from '$lib/web-shell/copy';

export function sortBooks(items: WebBook[]) {
  return [...items].sort((left, right) => left.title.localeCompare(right.title));
}

export function sortConversations(items: WebConversation[]) {
  return [...items].sort(
    (left, right) =>
      new Date(right.last_active_at).getTime() - new Date(left.last_active_at).getTime()
  );
}

export function findLatestActiveConversation(items: WebConversation[]) {
  return sortConversations(items).reduce<WebConversation | null>((latest, candidate) => {
    if (!latest) {
      return candidate;
    }

    const latestAt = new Date(latest.last_active_at).getTime();
    const candidateAt = new Date(candidate.last_active_at).getTime();
    if (candidateAt > latestAt) {
      return candidate;
    }

    if (candidateAt === latestAt && candidate.created_at > latest.created_at) {
      return candidate;
    }

    return latest;
  }, null);
}

export function formatTimestamp(
  value: string | undefined,
  language: UiLanguage,
  currentCopy: AppCopy
) {
  if (!value) {
    return currentCopy.timestampUnavailable;
  }

  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return currentCopy.timestampUnavailable;
  }

  return new Intl.DateTimeFormat(language === 'ru' ? 'ru-RU' : 'en-US', {
    dateStyle: 'medium',
    timeStyle: 'short'
  }).format(date);
}

export function roleLabel(role: string, currentCopy: AppCopy) {
  if (role === 'assistant') {
    return 'Codex';
  }

  if (role === 'user') {
    return currentCopy.you;
  }

  return role;
}

export function apiFailureMessage(error: unknown, fallback: string) {
  if ((error as ApiError | undefined)?.name === 'ApiError') {
    const apiError = error as ApiError;
    return apiError.message || fallback;
  }

  return fallback;
}
