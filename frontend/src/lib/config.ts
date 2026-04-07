const rawBackendBaseUrl = (import.meta.env.PUBLIC_BACKEND_BASE_URL ?? '').trim();

export const backendBaseUrl = rawBackendBaseUrl.replace(/\/+$/, '');

export function apiUrl(path: string): string {
  if (!backendBaseUrl) {
    return path;
  }

  return `${backendBaseUrl}${path}`;
}
