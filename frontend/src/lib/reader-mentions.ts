export type ReaderMentionSource = {
  file_path: string;
  start_line: number;
  start_character: number;
  end_line: number;
  end_character: number;
};

export type ReaderMentionPayload = {
  kind: 'reader_selection';
  book_id: string;
  excerpt: string;
  source: ReaderMentionSource;
  reference_label: string;
  message_text: string;
  created_at: string;
};

export type PendingReaderMention = {
  target_book_id: string;
  target_conversation_id: string;
  target_mode: 'new_conversation' | 'latest_active_conversation';
  payload: ReaderMentionPayload;
};

const STORAGE_KEY = 'book-writer-chat.reader-mention';

function quoteExcerpt(value: string): string {
  return value
    .trim()
    .split(/\r?\n/)
    .map((line) => `> ${line}`)
    .join('\n');
}

export function buildReaderMentionPayload(input: {
  bookId: string;
  sourceFile: string;
  startLine: number;
  startCharacter: number;
  endLine: number;
  endCharacter: number;
  excerpt: string;
}): ReaderMentionPayload {
  const referenceLabel = `${input.sourceFile}:${input.startLine}:${input.startCharacter}-${input.endLine}:${input.endCharacter}`;
  return {
    kind: 'reader_selection',
    book_id: input.bookId,
    excerpt: input.excerpt.trim(),
    source: {
      file_path: input.sourceFile,
      start_line: input.startLine,
      start_character: input.startCharacter,
      end_line: input.endLine,
      end_character: input.endCharacter
    },
    reference_label: referenceLabel,
    message_text: `${referenceLabel}\n\n${quoteExcerpt(input.excerpt)}`,
    created_at: new Date().toISOString()
  };
}

export function storePendingReaderMention(value: PendingReaderMention) {
  if (typeof window === 'undefined') {
    return;
  }

  window.sessionStorage.setItem(STORAGE_KEY, JSON.stringify(value));
}

export function loadPendingReaderMention(): PendingReaderMention | null {
  if (typeof window === 'undefined') {
    return null;
  }

  const raw = window.sessionStorage.getItem(STORAGE_KEY);
  if (!raw) {
    return null;
  }

  try {
    return JSON.parse(raw) as PendingReaderMention;
  } catch {
    window.sessionStorage.removeItem(STORAGE_KEY);
    return null;
  }
}

export function clearPendingReaderMention() {
  if (typeof window === 'undefined') {
    return;
  }

  window.sessionStorage.removeItem(STORAGE_KEY);
}
