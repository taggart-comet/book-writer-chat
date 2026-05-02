import { derived, writable } from 'svelte/store';

import { copy, type UiLanguage } from '$lib/web-shell/copy';

const UI_LANGUAGE_STORAGE_KEY = 'book-writer-chat-ui-language';

export const uiLanguage = writable<UiLanguage>('ru');
export const uiCopy = derived(uiLanguage, ($uiLanguage) => copy[$uiLanguage]);

let initialized = false;

export function initializeUiLanguage() {
  if (initialized || typeof window === 'undefined') {
    return;
  }

  initialized = true;
  const storedLanguage = window.localStorage.getItem(UI_LANGUAGE_STORAGE_KEY);
  if (storedLanguage === 'en' || storedLanguage === 'ru') {
    uiLanguage.set(storedLanguage);
  }
}

export function toggleUiLanguage() {
  uiLanguage.update((current) => {
    const next = current === 'ru' ? 'en' : 'ru';
    if (typeof window !== 'undefined') {
      window.localStorage.setItem(UI_LANGUAGE_STORAGE_KEY, next);
      document.documentElement.lang = next;
    }
    return next;
  });
}
