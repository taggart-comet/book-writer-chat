<svelte:options runes={true} />

<script lang="ts">
  import { goto } from '$app/navigation';
  import { onDestroy, onMount } from 'svelte';
  import {
    AlertTriangle,
    BookOpenText,
    ChevronLeft,
    ChevronRight,
    Copy,
    Link2,
    ListTree,
    LoaderCircle,
    Menu,
    Pencil
  } from 'lucide-svelte';
  import {
    createConversation,
    fetchConversations,
    fetchContent,
    fetchJob,
    fetchRevision,
    fetchSummary,
    ReaderApiError,
    type ReaderContent,
    type ReaderJob,
    type ReaderRevision,
    type ReaderSummary,
    type WebConversation
  } from '$lib/api';
  import {
    buildReaderMentionPayload,
    storePendingReaderMention
  } from '$lib/reader-mentions';
  import { loadStoredToken } from '$lib/web-auth';

  const { params } = $props<{ params: { book_id: string } }>();

  const pollingStatuses = new Set(['received', 'accepted', 'running']);
  const BOOK_WORDS_PER_PAGE = 320;
  const BOOK_FONT_SIZE_PX = 18;

  type NoticeTone = 'neutral' | 'warm' | 'success' | 'danger';
  type ReaderViewState = 'loading' | 'ready' | 'empty' | 'render_failed' | 'error';
  type ReaderLanguage = 'en' | 'ru';

  type ReaderNotice = {
    tone: NoticeTone;
    title: string;
    body: string;
  };

  type BookPage = {
    pageNumber: number;
    chapterId: string;
    chapterTitle: string;
    sourceFile: string;
    html: string;
  };

  type ChapterNavItem = {
    chapterId: string;
    title: string;
    pageIndex: number;
    pageNumber: number;
  };

  type SourcePoint = {
    sourceFile: string;
    line: number;
    character: number;
  };

  const readerCopy = {
    en: {
      reader: 'Reader',
      loadingTitle: 'Loading the latest draft',
      loadingCopy: 'The reader is assembling the current revision and chapter sequence.',
      readerUnavailable: 'Reader unavailable',
      readerLoadFailed: 'The reader could not load this draft.',
      tryAgain: 'Try again',
      chapterNavigation: 'Chapter navigation',
      chapters: 'Chapters',
      openMenu: 'Open chapters',
      closeMenu: 'Close menu',
      openEdits: 'Open editing conversations',
      pageAbbr: 'p.',
      position: 'Position',
      pageSettings: 'Page settings',
      wordsPerPage: `${BOOK_WORDS_PER_PAGE} words per page`,
      bookTextSize: `${BOOK_FONT_SIZE_PX}px book text`,
      noPagesYet: 'No pages yet',
      noChapterSelected: 'No chapter selected',
      pagePosition: (page: number, total: number) => `Page ${page} of ${total}`,
      renderFailedTitle: 'Latest revision could not be rendered',
      renderFailedBody: 'The draft exists, but the browser view for this revision is unavailable.',
      draftUpdatedTitle: 'Draft updated',
      runningTitle: 'New pages may be on the way',
      runningBody: 'The page will refresh while the current writing job is running.',
      shellReadyTitle: 'The shell is ready',
      shellReadyBody: 'This book does not have any rendered chapters yet.',
      accessDeniedTitle: 'Reader view is unavailable',
      requestFailedTitle: 'Reader request failed',
      renderOutputUnavailable: 'Render output unavailable',
      noChaptersYet: 'No chapters yet',
      emptyReaderBody: 'Once the first revision is rendered, the reading view will appear here.',
      settingChapterPages: 'Setting chapter pages',
      selectionToolbarLabel: 'Selected text actions',
      copyText: 'Copy text',
      mentionLines: 'Mention these lines',
      mentionInNewConversation: 'in a new conversation',
      mentionInLatestConversation: 'in the latest active conversation',
      mentionLoadingTargets: 'Checking conversations…',
      mentionRequiresAuth: 'Sign in to the web messenger to mention selected text.',
      mentionNoActiveConversation: 'No active conversation exists for this book yet.',
      mentionCreateFailed: 'Failed to create the new conversation target.',
      mentionNavigationFailed: 'Failed to open the messenger handoff.',
      mentionTargetLoadFailed: 'Failed to load messenger conversations for this book.',
      copiedText: 'Copied text',
      previousPage: 'Previous page',
      nextPage: 'Next page',
      loadingPages: 'Loading pages',
      loadRemainingPages: 'Load remaining pages',
      staleRevision: (oldRevision: string, newRevision: string) =>
        `A newer revision replaced ${oldRevision} with ${newRevision}. The page reset to the latest chapter sequence.`,
      staleContinuation:
        'The draft changed while you were reading. The page reloaded to keep continuation aligned with the newest render.'
    },
    ru: {
      reader: 'Читалка',
      loadingTitle: 'Загружаем последний черновик',
      loadingCopy: 'Читательский вид собирает текущую версию и порядок глав.',
      readerUnavailable: 'Читательский вид недоступен',
      readerLoadFailed: 'Не удалось загрузить этот черновик.',
      tryAgain: 'Попробовать еще раз',
      chapterNavigation: 'Навигация по главам',
      chapters: 'Главы',
      openMenu: 'Открыть главы',
      closeMenu: 'Закрыть меню',
      openEdits: 'Перейти к правкам',
      pageAbbr: 'с.',
      position: 'Позиция',
      pageSettings: 'Параметры страницы',
      wordsPerPage: `${BOOK_WORDS_PER_PAGE} слов на страницу`,
      bookTextSize: `Текст книги ${BOOK_FONT_SIZE_PX}px`,
      noPagesYet: 'Страниц пока нет',
      noChapterSelected: 'Глава не выбрана',
      pagePosition: (page: number, total: number) => `Страница ${page} из ${total}`,
      renderFailedTitle: 'Последнюю версию не удалось отобразить',
      renderFailedBody: 'Черновик существует, но вид для этой версии недоступен.',
      draftUpdatedTitle: 'Черновик обновлен',
      runningTitle: 'Скоро могут появиться новые страницы',
      runningBody: 'Страница будет обновляться, пока выполняется текущая задача написания.',
      shellReadyTitle: 'Читательский вид готов',
      shellReadyBody: 'У этой книги пока нет отображенных глав.',
      accessDeniedTitle: 'Читательский вид недоступен',
      requestFailedTitle: 'Запрос к читательскому виду не удался',
      renderOutputUnavailable: 'Отображение недоступно',
      noChaptersYet: 'Глав пока нет',
      emptyReaderBody: 'Когда первая версия будет отображена, читательский вид появится здесь.',
      settingChapterPages: 'Размечаем страницы глав',
      selectionToolbarLabel: 'Действия с выделенным текстом',
      copyText: 'Копировать текст',
      mentionLines: 'Упомянуть эти строки',
      mentionInNewConversation: 'в новом разговоре',
      mentionInLatestConversation: 'в последнем активном разговоре',
      mentionLoadingTargets: 'Проверяем разговоры…',
      mentionRequiresAuth: 'Войдите в веб-мессенджер, чтобы упомянуть выделенный текст.',
      mentionNoActiveConversation: 'Для этой книги пока нет активного разговора.',
      mentionCreateFailed: 'Не удалось создать новый разговор для упоминания.',
      mentionNavigationFailed: 'Не удалось открыть передачу в мессенджер.',
      mentionTargetLoadFailed: 'Не удалось загрузить разговоры мессенджера для этой книги.',
      copiedText: 'Текст скопирован',
      previousPage: 'Предыдущая страница',
      nextPage: 'Следующая страница',
      loadingPages: 'Загружаем страницы',
      loadRemainingPages: 'Загрузить остальные страницы',
      staleRevision: (oldRevision: string, newRevision: string) =>
        `Новая версия заменила ${oldRevision} на ${newRevision}. Страница открыла последний порядок глав.`,
      staleContinuation:
        'Черновик изменился во время чтения. Страница обновилась, чтобы продолжить с новой версии.'
    }
  };

  let summary: ReaderSummary | null = $state(null);
  let revision: ReaderRevision | null = $state(null);
  let job: ReaderJob | null = $state(null);
  let chapters: ReaderContent[] = $state([]);
  let loading = $state(true);
  let loadingMore = $state(false);
  let loadingSequence = $state(false);
  let nextCursor: string | null = $state(null);
  let activePageIndex = $state(0);
  let viewState: ReaderViewState = $state('loading');
  let errorTitle = $state(readerCopy.en.readerUnavailable);
  let errorMessage = $state(readerCopy.en.readerLoadFailed);
  let renderFailureMessage = $state('');
  let staleRevisionNotice = $state('');
  let selectionToolbarVisible = $state(false);
  let selectionToolbarX = $state(0);
  let selectionToolbarY = $state(0);
  let selectionToolbarText = $state('');
  let selectionClipboardStatus = $state('');
  let selectionToolbarPoints: { start: SourcePoint; end: SourcePoint } | null = $state(null);
  let mentionMenuOpen = $state(false);
  let mentionMenuLoading = $state(false);
  let mentionMenuAuthenticated = $state(false);
  let mentionMenuError = $state('');
  let mentionLatestConversation: WebConversation | null = $state(null);
  let mobileMenuOpen = $state(false);
  let pageContentElement: HTMLDivElement | null = $state(null);
  let refreshTimer: ReturnType<typeof window.setTimeout> | null = null;

  function language(): ReaderLanguage {
    return summary?.language === 'ru' ? 'ru' : 'en';
  }

  function copy() {
    return readerCopy[language()];
  }

  const pageTitle = $derived.by(() => (summary ? `${summary.title} | ${copy().reader}` : copy().reader));
  const activeRevisionId = $derived.by(() => revision?.revision_id ?? summary?.last_revision_id ?? null);
  const surfaceNotice = $derived.by(() =>
    resolveSurfaceNotice(viewState, job, renderFailureMessage, staleRevisionNotice)
  );
  const bookPages = $derived.by(() => paginateChapters(chapters));
  const activePage = $derived.by(() => {
    if (bookPages.length === 0) {
      return null;
    }

    return bookPages[Math.min(activePageIndex, bookPages.length - 1)];
  });
  const chapterNavItems = $derived.by(() => buildChapterNavItems(chapters, bookPages));
  const pagePositionLabel = $derived.by(() =>
    activePage ? copy().pagePosition(activePage.pageNumber, bookPages.length) : copy().noPagesYet
  );
  const activeChapterTitle = $derived.by(() => activePage?.chapterTitle ?? copy().noChapterSelected);

  function compactId(value: string): string {
    return value.length > 14 ? `${value.slice(0, 8)}...${value.slice(-4)}` : value;
  }

  function titleCase(value: string | null | undefined): string {
    if (!value) {
      return 'Unknown';
    }

    return value
      .split('_')
      .filter(Boolean)
      .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
      .join(' ');
  }

  function wordCount(value: string): number {
    return value.trim().split(/\s+/).filter(Boolean).length;
  }

  function htmlWordCount(value: string): number {
    if (typeof DOMParser === 'undefined') {
      return wordCount(value.replace(/<[^>]+>/g, ' '));
    }

    const parsed = new DOMParser().parseFromString(value, 'text/html');
    return wordCount(parsed.body.textContent ?? '');
  }

  function htmlLayoutWeight(value: string): number {
    const words = htmlWordCount(value);
    return /<img\b/i.test(value) ? Math.max(words, Math.round(BOOK_WORDS_PER_PAGE * 0.65)) : words;
  }

  function paginateChapters(currentChapters: ReaderContent[]): BookPage[] {
    const pages: BookPage[] = [];

    for (const chapter of currentChapters) {
      const parserUnavailable = typeof document === 'undefined';
      const container = parserUnavailable ? null : document.createElement('template');
      const blocks = container
        ? (() => {
            container.innerHTML = chapter.html;
            return Array.from(container.content.childNodes)
              .map((node) => {
                if (node.nodeType === Node.TEXT_NODE) {
                  const text = node.textContent?.trim();
                  return text ? `<p>${text}</p>` : '';
                }

                if (node instanceof HTMLElement) {
                  return node.outerHTML;
                }

                return '';
              })
              .filter(Boolean);
          })()
        : [chapter.html];

      let pageHtml = '';
      let pageWords = 0;

      for (const block of blocks.length > 0 ? blocks : [chapter.html]) {
        const blockWords = htmlLayoutWeight(block);
        const shouldStartPage =
          pageHtml && pageWords + blockWords > BOOK_WORDS_PER_PAGE && pageWords > BOOK_WORDS_PER_PAGE * 0.45;

        if (shouldStartPage) {
          pages.push({
            pageNumber: pages.length + 1,
            chapterId: chapter.chapter_id,
            chapterTitle: chapter.title,
            sourceFile: chapter.source_file,
            html: pageHtml
          });
          pageHtml = '';
          pageWords = 0;
        }

        pageHtml += block;
        pageWords += blockWords;
      }

      if (pageHtml || chapter.html.trim()) {
        pages.push({
          pageNumber: pages.length + 1,
          chapterId: chapter.chapter_id,
          chapterTitle: chapter.title,
          sourceFile: chapter.source_file,
          html: pageHtml || chapter.html
        });
      }
    }

    return pages;
  }

  function buildChapterNavItems(
    currentChapters: ReaderContent[],
    currentPages: BookPage[]
  ): ChapterNavItem[] {
    return currentChapters.map((chapter) => {
      const pageIndex = Math.max(
        currentPages.findIndex((page) => page.chapterId === chapter.chapter_id),
        0
      );

      return {
        chapterId: chapter.chapter_id,
        title: chapter.title,
        pageIndex,
        pageNumber: currentPages[pageIndex]?.pageNumber ?? 1
      };
    });
  }

  function goToPage(pageIndex: number) {
    activePageIndex = Math.min(Math.max(pageIndex, 0), Math.max(bookPages.length - 1, 0));
    mobileMenuOpen = false;
    clearSelectionToolbar();
  }

  function closeMobileMenu() {
    mobileMenuOpen = false;
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      mobileMenuOpen = false;
    }
  }

  function previousPage() {
    goToPage(activePageIndex - 1);
  }

  function nextPage() {
    goToPage(activePageIndex + 1);
  }

  async function openEdits() {
    mobileMenuOpen = false;
    await goto(`/books/${encodeURIComponent(summary?.book_id ?? params.book_id)}/edits`);
  }

  async function fetchRemainingChapters(firstChapter: ReaderContent, firstCursor: string | null) {
    const sequence = [firstChapter];
    let cursor = firstCursor;

    while (cursor) {
      const nextChapter = await fetchContent(params.book_id, {
        cursor,
        revisionId: firstChapter.revision_id
      });

      sequence.push(nextChapter);
      cursor = nextChapter.next_cursor;
    }

    return sequence;
  }

  function resolveSurfaceNotice(
    currentState: ReaderViewState,
    currentJob: ReaderJob | null,
    currentRenderFailure: string,
    currentStaleNotice: string
  ): ReaderNotice | null {
    if (currentState === 'render_failed') {
      return {
        tone: 'danger',
        title: copy().renderFailedTitle,
        body: currentRenderFailure || copy().renderFailedBody
      };
    }

    if (currentStaleNotice) {
      return {
        tone: 'warm',
        title: copy().draftUpdatedTitle,
        body: currentStaleNotice
      };
    }

    if (currentJob && pollingStatuses.has(currentJob.status)) {
      return {
        tone: 'warm',
        title: copy().runningTitle,
        body: currentJob.user_facing_message ?? copy().runningBody
      };
    }

    if (currentState === 'empty') {
      return {
        tone: 'neutral',
        title: copy().shellReadyTitle,
        body: copy().shellReadyBody
      };
    }

    return null;
  }

  function handleLoadError(cause: unknown) {
    if (cause instanceof ReaderApiError) {
      if (cause.code === 'render_failed') {
        viewState = 'render_failed';
        renderFailureMessage = cause.message;
        return;
      }

      errorTitle = titleCase(cause.code);
      errorMessage = cause.message;
      viewState = 'error';
      return;
    }

    errorTitle = copy().requestFailedTitle;
    errorMessage = cause instanceof Error ? cause.message : copy().readerLoadFailed;
    viewState = 'error';
  }

  function clearSelectionToolbar() {
    selectionToolbarVisible = false;
    selectionToolbarText = '';
    selectionClipboardStatus = '';
    selectionToolbarPoints = null;
    mentionMenuOpen = false;
    mentionMenuLoading = false;
    mentionMenuAuthenticated = false;
    mentionMenuError = '';
    mentionLatestConversation = null;
  }

  function characterCount(value: string): number {
    return Array.from(value).length;
  }

  function selectedPointFromSpan(span: HTMLElement, offset: number): SourcePoint | null {
    const sourceFile = span.dataset.sourceFile;
    const startLine = Number(span.dataset.sourceStartLine);
    const startCharacter = Number(span.dataset.sourceStartChar);

    if (!sourceFile || !Number.isFinite(startLine) || !Number.isFinite(startCharacter)) {
      return null;
    }

    const prefix = (span.textContent ?? '').slice(0, offset);
    const prefixLines = prefix.split('\n');
    const line = startLine + prefixLines.length - 1;
    const character =
      prefixLines.length === 1
        ? startCharacter + characterCount(prefix)
        : characterCount(prefixLines[prefixLines.length - 1] ?? '') + 1;

    return { sourceFile, line, character };
  }

  function selectionSourcePoints(range: Range): { start: SourcePoint; end: SourcePoint } | null {
    if (!pageContentElement || !range.intersectsNode(pageContentElement)) {
      return null;
    }

    const walker = document.createTreeWalker(pageContentElement, NodeFilter.SHOW_TEXT);
    const points: { start: SourcePoint; end: SourcePoint }[] = [];

    while (walker.nextNode()) {
      const textNode = walker.currentNode as Text;
      if (!textNode.textContent || !range.intersectsNode(textNode)) {
        continue;
      }

      const span = textNode.parentElement?.closest<HTMLElement>('[data-source-file]');
      if (!span) {
        continue;
      }

      const startOffset = textNode === range.startContainer ? range.startOffset : 0;
      const endOffset = textNode === range.endContainer ? range.endOffset : textNode.textContent.length;

      if (endOffset <= startOffset) {
        continue;
      }

      const start = selectedPointFromSpan(span, startOffset);
      const end = selectedPointFromSpan(span, endOffset);

      if (start && end) {
        points.push({ start, end });
      }
    }

    const first = points[0];
    const last = points[points.length - 1];
    return first && last ? { start: first.start, end: last.end } : null;
  }

  function formatSourceReference(start: SourcePoint, end: SourcePoint): string {
    if (start.sourceFile === end.sourceFile) {
      return `${start.sourceFile}:${start.line}:${start.character}-${end.line}:${end.character}`;
    }

    return `${start.sourceFile}:${start.line}:${start.character} -> ${end.sourceFile}:${end.line}:${end.character}`;
  }

  function updateSelectionToolbar() {
    const selection = window.getSelection();
    if (!selection || selection.rangeCount === 0 || selection.isCollapsed) {
      clearSelectionToolbar();
      return;
    }

    const selectedText = selection.toString().trim();
    const range = selection.getRangeAt(0);
    const points = selectionSourcePoints(range);

    if (!selectedText || !points) {
      clearSelectionToolbar();
      return;
    }

    const rect = range.getBoundingClientRect();
    selectionToolbarText = selectedText;
    selectionToolbarPoints = points;
    selectionToolbarX = Math.min(Math.max(rect.left + rect.width / 2, 96), window.innerWidth - 96);
    selectionToolbarY = Math.max(rect.top - 12, 16);
    selectionClipboardStatus = '';
    mentionMenuOpen = false;
    mentionMenuError = '';
    mentionLatestConversation = null;
    selectionToolbarVisible = true;
  }

  async function writeClipboard(value: string) {
    if (navigator.clipboard) {
      await navigator.clipboard.writeText(value);
      return;
    }

    const textarea = document.createElement('textarea');
    textarea.value = value;
    textarea.setAttribute('readonly', '');
    textarea.style.position = 'fixed';
    textarea.style.opacity = '0';
    document.body.appendChild(textarea);
    textarea.select();
    document.execCommand('copy');
    textarea.remove();
  }

  async function copySelectionText() {
    await writeClipboard(selectionToolbarText);
    selectionClipboardStatus = copy().copiedText;
  }

  function sortConversations(items: WebConversation[]) {
    return [...items].sort(
      (left, right) =>
        new Date(right.last_active_at).getTime() - new Date(left.last_active_at).getTime()
    );
  }

  function findLatestActiveConversation(items: WebConversation[]) {
    return (
      sortConversations(items).reduce<WebConversation | null>((latest, candidate) => {
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
      }, null) ?? null
    );
  }

  async function openMentionMenu() {
    mentionMenuOpen = !mentionMenuOpen;
    selectionClipboardStatus = '';

    if (!mentionMenuOpen) {
      mentionMenuError = '';
      return;
    }

    const token = loadStoredToken();
    if (!token) {
      mentionMenuAuthenticated = false;
      mentionMenuError = copy().mentionRequiresAuth;
      mentionLatestConversation = null;
      return;
    }
    mentionMenuAuthenticated = true;

    if (!summary?.book_id) {
      mentionMenuError = copy().mentionTargetLoadFailed;
      mentionLatestConversation = null;
      return;
    }

    mentionMenuLoading = true;
    mentionMenuError = '';
    try {
      const conversations = await fetchConversations(token, summary.book_id);
      mentionLatestConversation = findLatestActiveConversation(conversations);
      if (!mentionLatestConversation) {
        mentionMenuError = copy().mentionNoActiveConversation;
      }
    } catch {
      mentionLatestConversation = null;
      mentionMenuError = copy().mentionTargetLoadFailed;
    } finally {
      mentionMenuLoading = false;
    }
  }

  function currentMentionPayload() {
    if (!summary?.book_id || !selectionToolbarPoints || !selectionToolbarText) {
      return null;
    }

    return buildReaderMentionPayload({
      bookId: summary.book_id,
      sourceFile: selectionToolbarPoints.start.sourceFile,
      startLine: selectionToolbarPoints.start.line,
      startCharacter: selectionToolbarPoints.start.character,
      endLine: selectionToolbarPoints.end.line,
      endCharacter: selectionToolbarPoints.end.character,
      excerpt: selectionToolbarText
    });
  }

  async function handoffMention(
    targetConversationId: string,
    targetMode: 'new_conversation' | 'latest_active_conversation'
  ) {
    const payload = currentMentionPayload();
    if (!payload || !summary?.book_id) {
      mentionMenuError = copy().mentionNavigationFailed;
      return;
    }

    storePendingReaderMention({
      target_book_id: summary.book_id,
      target_conversation_id: targetConversationId,
      target_mode: targetMode,
      payload
    });

    clearSelectionToolbar();
    await goto(
      `/books/${encodeURIComponent(summary.book_id)}/edits/${encodeURIComponent(targetConversationId)}`
    );
  }

  async function mentionInNewConversation() {
    const token = loadStoredToken();
    if (!token || !summary?.book_id) {
      mentionMenuError = copy().mentionRequiresAuth;
      return;
    }

    mentionMenuLoading = true;
    mentionMenuError = '';
    try {
      const conversation = await createConversation(token, summary.book_id);
      await handoffMention(conversation.conversation_id, 'new_conversation');
    } catch {
      mentionMenuError = copy().mentionCreateFailed;
    } finally {
      mentionMenuLoading = false;
    }
  }

  async function mentionInLatestConversation() {
    if (!mentionLatestConversation) {
      mentionMenuError = copy().mentionNoActiveConversation;
      return;
    }

    await handoffMention(
      mentionLatestConversation.conversation_id,
      'latest_active_conversation'
    );
  }

  async function loadReader(options: { background?: boolean } = {}) {
    const background = options.background ?? false;

    if (!background) {
      loading = true;
      viewState = 'loading';
      staleRevisionNotice = '';
    }

    try {
      const nextSummary = await fetchSummary(params.book_id);
      const [nextRevision, nextJob] = await Promise.all([
        fetchRevision(params.book_id).catch(() => null),
        fetchJob(params.book_id).catch(() => null)
      ]);

      summary = nextSummary;
      revision = nextRevision;
      job = nextJob;
      renderFailureMessage = '';

      if (!nextSummary.last_revision_id) {
        chapters = [];
        nextCursor = null;
        activePageIndex = 0;
        viewState = 'empty';
        return;
      }

      if (nextRevision?.render_status === 'failed') {
        chapters = [];
        nextCursor = null;
        activePageIndex = 0;
        renderFailureMessage = nextRevision.render_error ?? nextRevision.summary;
        viewState = 'render_failed';
        return;
      }

      const firstChapter = await fetchContent(params.book_id, {
        revisionId: nextSummary.last_revision_id
      });

      const previousRevisionId = chapters[0]?.revision_id;
      const changedRevision = Boolean(
        background &&
          previousRevisionId &&
          previousRevisionId !== firstChapter.revision_id
      );

      loadingSequence = true;
      chapters = await fetchRemainingChapters(firstChapter, firstChapter.next_cursor);
      nextCursor = null;
      activePageIndex = 0;
      viewState = chapters.some((chapter) => chapter.html.trim()) || nextSummary.chapter_count > 0 ? 'ready' : 'empty';

      if (changedRevision) {
        staleRevisionNotice = copy().staleRevision(
          compactId(previousRevisionId),
          compactId(firstChapter.revision_id)
        );
      }
    } catch (cause) {
      if (!background) {
        handleLoadError(cause);
      }
    } finally {
      loading = false;
      loadingSequence = false;
      scheduleRefresh();
    }
  }

  async function loadMore() {
    if (!nextCursor || loadingMore) {
      return;
    }

    loadingMore = true;

    try {
      let cursor: string | null = nextCursor;
      const loadedChapters = [...chapters];

      while (cursor) {
        const nextChapter = await fetchContent(params.book_id, {
          cursor,
          revisionId: chapters[0]?.revision_id ?? activeRevisionId
        });

        loadedChapters.push(nextChapter);
        cursor = nextChapter.next_cursor;
      }

      chapters = loadedChapters;
      nextCursor = null;
    } catch (cause) {
      if (cause instanceof ReaderApiError && cause.code === 'stale_revision') {
        staleRevisionNotice = copy().staleContinuation;
        await loadReader({ background: true });
        return;
      }

      handleLoadError(cause);
    } finally {
      loadingMore = false;
    }
  }

  function scheduleRefresh() {
    if (refreshTimer) {
      window.clearTimeout(refreshTimer);
      refreshTimer = null;
    }

    if (job && pollingStatuses.has(job.status)) {
      refreshTimer = window.setTimeout(() => {
        void loadReader({ background: true });
      }, 5000);
    }
  }

  onMount(() => {
    document.addEventListener('selectionchange', updateSelectionToolbar);
    window.addEventListener('keydown', handleKeydown);
    void loadReader();
  });

  onDestroy(() => {
    document.removeEventListener('selectionchange', updateSelectionToolbar);
    window.removeEventListener('keydown', handleKeydown);
    if (refreshTimer) {
      window.clearTimeout(refreshTimer);
    }
  });
</script>

<svelte:head>
  <title>{pageTitle}</title>
</svelte:head>

{#if loading}
  <section class="state-screen">
    <div class="state-card">
      <LoaderCircle class="spin" size={20} />
      <p class="state-title">{copy().loadingTitle}</p>
      <p class="state-copy">{copy().loadingCopy}</p>
    </div>
  </section>
{:else if viewState === 'error'}
  <section class="state-screen">
    <div class="state-card error-card">
      <AlertTriangle size={20} />
      <p class="state-title">{errorTitle}</p>
      <p class="state-copy">{errorMessage}</p>
      <button class="ghost-button" type="button" onclick={() => void loadReader()}>
        {copy().tryAgain}
      </button>
    </div>
  </section>
{:else if summary}
  <main class="reader-shell" lang={language()} style={`--book-font-size: ${BOOK_FONT_SIZE_PX}px`}>
    <div class="mobile-reader-bar">
      <button
        class="topbar-action"
        type="button"
        aria-controls="reader-chapter-menu"
        aria-expanded={mobileMenuOpen}
        aria-label={copy().openMenu}
        onclick={() => (mobileMenuOpen = true)}
      >
        <Menu size={19} />
      </button>
      <div>
        <span>{pagePositionLabel}</span>
        <strong>{activeChapterTitle}</strong>
      </div>
      <button class="topbar-action" type="button" aria-label={copy().openEdits} onclick={() => void openEdits()}>
        <Pencil size={18} />
      </button>
    </div>

    {#if mobileMenuOpen}
      <button
        class="menu-scrim"
        type="button"
        aria-label={copy().closeMenu}
        onclick={closeMobileMenu}
      ></button>
    {/if}

    <section class="frame">
      <aside id="reader-chapter-menu" class:open={mobileMenuOpen} class="spine">
        <div class="spine-actions">
          <button class="ghost-button" type="button" onclick={() => void openEdits()}>
            <Pencil size={16} />
            {copy().openEdits}
          </button>
        </div>

        <div class="spine-title-row">
          <div>
            <h1>{summary.title}</h1>
          </div>
          <button class="spine-close" type="button" aria-label={copy().closeMenu} onclick={closeMobileMenu}>
            <ChevronLeft size={18} />
          </button>
        </div>

        {#if chapterNavItems.length > 0}
          <nav class="chapter-nav" aria-label={copy().chapterNavigation}>
            <div class="nav-heading">
              <ListTree size={16} />
              <span>{copy().chapters}</span>
            </div>

            <div class="chapter-list">
              {#each chapterNavItems as item}
                <button
                  class:active={activePage?.chapterId === item.chapterId}
                  class="chapter-link"
                  type="button"
                  onclick={() => goToPage(item.pageIndex)}
                >
                  <span>{item.title}</span>
                  <small>{copy().pageAbbr} {item.pageNumber}</small>
                </button>
              {/each}
            </div>
          </nav>
        {/if}

        <div class="meta-block">
          <div>
            <span class="meta-label">{copy().position}</span>
            <strong>{pagePositionLabel}</strong>
            <span>{activeChapterTitle}</span>
          </div>
          <div>
            <span class="meta-label">{copy().pageSettings}</span>
            <strong>{copy().wordsPerPage}</strong>
            <span>{copy().bookTextSize}</span>
          </div>
        </div>
      </aside>

      <section class="book">
        {#if surfaceNotice}
          <div class={`notice ${surfaceNotice.tone}`}>
            <p>{surfaceNotice.title}</p>
            <span>{surfaceNotice.body}</span>
          </div>
        {/if}

        {#if viewState === 'render_failed'}
          <section class="empty-state">
            <AlertTriangle size={18} />
            <div>
              <p>{copy().renderOutputUnavailable}</p>
              <span>{renderFailureMessage}</span>
            </div>
          </section>
        {:else if viewState === 'empty'}
          <section class="empty-state">
            <BookOpenText size={18} />
            <div>
              <p>{copy().noChaptersYet}</p>
              <span>{copy().emptyReaderBody}</span>
            </div>
          </section>
        {:else}
          <div class="book-stage" aria-live="polite">
            {#if loadingSequence}
              <div class="sequence-loader">
                <LoaderCircle size={16} class="spin" />
                <span>{copy().settingChapterPages}</span>
              </div>
            {/if}

            <article class="book-page-shell">
              <div class="page leaf">
                {#if activePage}
                  <header class="page-header">
                    <span>{summary.title}</span>
                    <span>{pagePositionLabel}</span>
                  </header>

                  <div class="page-content" bind:this={pageContentElement}>
                    <div class="chapter-content">{@html activePage.html}</div>
                  </div>

                  <footer class="page-footer">{activePage.pageNumber}</footer>
                {/if}
              </div>
            </article>

            {#if selectionToolbarVisible}
              <div
                class="selection-toolbar"
                style={`left: ${selectionToolbarX}px; top: ${selectionToolbarY}px;`}
                onmousedown={(event) => event.preventDefault()}
                role="toolbar"
                tabindex="-1"
                aria-label={copy().selectionToolbarLabel}
              >
                <button type="button" onclick={copySelectionText}>
                  <Copy size={14} />
                  {copy().copyText}
                </button>
                <button type="button" onclick={openMentionMenu}>
                  <Link2 size={14} />
                  {copy().mentionLines}
                </button>
                {#if mentionMenuOpen}
                  <div class="selection-menu" role="menu" aria-label={copy().mentionLines}>
                    <button
                      type="button"
                      onclick={mentionInNewConversation}
                      disabled={mentionMenuLoading || !mentionMenuAuthenticated}
                    >
                      {copy().mentionInNewConversation}
                    </button>
                    <button
                      type="button"
                      onclick={mentionInLatestConversation}
                      disabled={
                        mentionMenuLoading ||
                        !mentionMenuAuthenticated ||
                        !mentionLatestConversation
                      }
                    >
                      {copy().mentionInLatestConversation}
                    </button>
                    {#if mentionMenuLoading}
                      <span>{copy().mentionLoadingTargets}</span>
                    {:else if mentionMenuError}
                      <span>{mentionMenuError}</span>
                    {:else if selectionToolbarPoints}
                      <span>
                        {formatSourceReference(selectionToolbarPoints.start, selectionToolbarPoints.end)}
                      </span>
                    {/if}
                  </div>
                {/if}
                {#if selectionClipboardStatus}
                  <span>{selectionClipboardStatus}</span>
                {/if}
              </div>
            {/if}
          </div>

          <div class="page-controls">
            <button class="ghost-button" type="button" onclick={previousPage} disabled={activePageIndex === 0}>
              <ChevronLeft size={16} />
              {copy().previousPage}
            </button>
            <span>{pagePositionLabel}</span>
            <button
              class="ghost-button"
              type="button"
              onclick={nextPage}
              disabled={activePageIndex >= Math.max(bookPages.length - 1, 0)}
            >
              {copy().nextPage}
              <ChevronRight size={16} />
            </button>
          </div>

          {#if nextCursor}
            <div class="continuation">
              <button class="continue-button" type="button" onclick={loadMore} disabled={loadingMore}>
                {#if loadingMore}
                  <LoaderCircle size={16} class="spin" />
                  {copy().loadingPages}
                {:else}
                  {copy().loadRemainingPages}
                {/if}
              </button>
            </div>
          {/if}
        {/if}
      </section>
    </section>
</main>
{/if}

<style>
  :global(html),
  :global(body) {
    height: 100%;
    margin: 0;
    color: #171a17;
    font-family: Baskerville, 'Iowan Old Style', 'Palatino Linotype', 'Book Antiqua', serif;
    background: #e7ebe4;
  }

  :global(button) {
    font: inherit;
  }

  :global(.chapter-content p) {
    margin: 0 0 0.95rem;
    text-align: justify;
    text-wrap: pretty;
  }

  :global(.chapter-content h1),
  :global(.chapter-content h2),
  :global(.chapter-content h3),
  :global(.chapter-content h4) {
    margin: 1.35rem 0 0.75rem;
    line-height: 1.18;
    break-after: avoid;
  }

  :global(.chapter-content blockquote) {
    margin: 1.1rem 0;
    padding-left: 1rem;
    border-left: 3px solid #7e1f32;
    color: #3f4943;
  }

  :global(.chapter-content img) {
    display: block;
    width: min(100%, 34rem);
    max-height: 58vh;
    height: auto;
    margin: 1.2rem auto;
    object-fit: contain;
    border-radius: 8px;
  }

  :global(.chapter-content p:has(img)) {
    margin: 1.2rem 0;
    text-align: center;
  }

  .reader-shell {
    box-sizing: border-box;
    height: 100vh;
    height: 100dvh;
    min-height: 0;
    overflow: hidden;
    padding: 1.25rem;
  }

  .mobile-reader-bar,
  .menu-scrim,
  .spine-close {
    display: none;
  }

  .frame {
    height: calc(100vh - 2.5rem);
    height: calc(100dvh - 2.5rem);
    min-height: 0;
    max-width: 96rem;
    margin: 0 auto;
    display: grid;
    grid-template-columns: minmax(17rem, 21rem) minmax(0, 1fr);
    gap: 1.25rem;
    align-items: start;
  }

  .spine,
  .state-card {
    border: 1px solid #b8c0b6;
    box-shadow: 0 1rem 2.4rem rgba(23, 26, 23, 0.12);
  }

  .spine {
    position: sticky;
    top: 1.25rem;
    max-height: calc(100vh - 2.5rem);
    overflow: auto;
    padding: 1.55rem 1.45rem 1.35rem;
    background: #ffffff;
    border-radius: 18px;
  }

  .spine-title-row {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: 0.5rem;
    margin-top: 1.3rem;
    padding-bottom: 1.1rem;
    border-bottom: 1px solid #e3e8e1;
  }

  .meta-label,
  .nav-heading,
  .page-header,
  .page-footer {
    text-transform: uppercase;
    letter-spacing: 0.08em;
    font-size: 0.7rem;
    color: #55625a;
  }

  h1,
  .state-title {
    font-family: 'Iowan Old Style', Baskerville, 'Palatino Linotype', serif;
    font-weight: 500;
  }

  h1 {
    margin: 0;
    font-size: 2.1rem;
    line-height: 0.96;
    letter-spacing: -0.02em;
    text-wrap: balance;
  }

  .state-copy,
  .notice span,
  .empty-state span,
  .meta-block span {
    color: #55625a;
    line-height: 1.55;
  }

  .meta-block {
    display: grid;
    gap: 0.8rem;
    margin-top: 1.25rem;
    padding: 1rem 0 0.2rem;
    border-top: 1px solid #d6ddd4;
  }

  .meta-block div {
    display: grid;
    gap: 0.2rem;
  }

  .meta-block strong,
  .empty-state p,
  .notice p {
    font-weight: 600;
  }

  .meta-block span:last-child {
    display: block;
    margin-top: 0.22rem;
  }

  .chapter-nav {
    margin-top: 1.2rem;
  }

  .nav-heading {
    display: flex;
    align-items: center;
    gap: 0.45rem;
    margin-bottom: 0.85rem;
  }

  .chapter-list {
    display: grid;
    gap: 0.75rem;
    max-height: 42vh;
    overflow: auto;
    padding-right: 0.3rem;
  }

  .chapter-link {
    width: 100%;
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 0.75rem;
    align-items: center;
    padding: 0.95rem 1.05rem;
    text-align: left;
    color: #171a17;
    background: linear-gradient(180deg, #fbfcfa 0%, #f5f7f2 100%);
    border: 1px solid #d6ddd4;
    border-radius: 16px;
    cursor: pointer;
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.72);
  }

  .chapter-link span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .chapter-link small {
    color: #55625a;
  }

  .chapter-link.active,
  .chapter-link:hover {
    border-color: #7e1f32;
    background: #ffffff;
  }

  .spine-actions {
    margin-top: 0;
  }

  .ghost-button,
  .continue-button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 0.55rem;
    border-radius: 8px;
    cursor: pointer;
    transition:
      transform 140ms ease,
      background 140ms ease,
      border-color 140ms ease;
  }

  .ghost-button {
    padding: 0.68rem 0.85rem;
    color: #171a17;
    background: #ffffff;
    border: 1px solid #b8c0b6;
  }

  .spine-actions .ghost-button {
    width: 100%;
    justify-content: flex-start;
    padding: 0.95rem 1.05rem;
    border-radius: 16px;
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.7);
  }

  .continue-button {
    padding: 0.75rem 1rem;
    color: #ffffff;
    background: #2c4735;
    border: 1px solid #2c4735;
  }

  .ghost-button:hover,
  .continue-button:hover {
    transform: translateY(-1px);
  }

  .ghost-button:disabled,
  .continue-button:disabled {
    opacity: 0.7;
    cursor: wait;
    transform: none;
  }

  .book {
    align-self: stretch;
    display: flex;
    flex-direction: column;
    min-height: 0;
    height: 100%;
    overflow: hidden;
    margin: 0;
    background: #2c4735;
    border: 1px solid #263c2e;
    border-radius: 8px;
    box-shadow:
      inset 0 0 0 0.35rem #3f5f49,
      0 1.4rem 2.8rem rgba(23, 26, 23, 0.22);
    padding: 1.25rem;
  }

  .notice,
  .empty-state {
    display: grid;
    grid-template-columns: 1fr;
    gap: 0.35rem;
    padding: 0.85rem 1rem;
    border-radius: 8px;
    border: 1px solid #d6ddd4;
    margin-bottom: 1rem;
  }

  .notice.neutral,
  .empty-state {
    background: #f6f8f4;
  }

  .notice.warm {
    background: #f4efd8;
  }

  .notice.success {
    background: #e3f0e5;
  }

  .notice.danger {
    background: #f5e4e4;
  }

  .empty-state {
    grid-template-columns: auto 1fr;
    align-items: start;
    gap: 0.8rem;
  }

  .empty-state p,
  .notice p {
    margin: 0;
  }

  .book-stage {
    position: relative;
    display: flex;
    min-height: 0;
    flex: 1 1 auto;
  }

  .sequence-loader {
    position: absolute;
    top: 0.75rem;
    right: 0.75rem;
    z-index: 3;
    display: inline-flex;
    align-items: center;
    gap: 0.45rem;
    padding: 0.45rem 0.6rem;
    color: #ffffff;
    background: #7e1f32;
    border-radius: 8px;
  }

  .selection-toolbar {
    position: fixed;
    z-index: 20;
    display: inline-flex;
    align-items: flex-start;
    gap: 0.35rem;
    max-width: min(30rem, calc(100vw - 1rem));
    padding: 0.35rem;
    color: #ffffff;
    background: #171a17;
    border: 1px solid #3f4943;
    border-radius: 8px;
    box-shadow: 0 0.75rem 1.5rem rgba(23, 26, 23, 0.24);
    transform: translate(-50%, -100%);
  }

  .selection-toolbar button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 0.35rem;
    min-height: 2rem;
    padding: 0.35rem 0.55rem;
    color: #171a17;
    background: #ffffff;
    border: 1px solid #d6ddd4;
    border-radius: 8px;
    cursor: pointer;
    white-space: nowrap;
  }

  .selection-toolbar button:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  .selection-menu {
    display: grid;
    gap: 0.35rem;
    min-width: 16rem;
    padding: 0.25rem;
    border-radius: 8px;
    background: #232924;
  }

  .selection-menu button {
    justify-content: flex-start;
    width: 100%;
  }

  .selection-toolbar span {
    padding: 0 0.45rem;
    font-size: 0.82rem;
    white-space: normal;
  }

  .book-page-shell {
    position: relative;
    width: min(46rem, 100%);
    min-height: 0;
    height: 100%;
    margin: 0 auto;
    isolation: isolate;
  }

  .leaf {
    min-width: 0;
  }

  .page {
    position: relative;
    display: grid;
    grid-template-rows: auto 1fr auto;
    min-height: 0;
    height: 100%;
    overflow: hidden;
    padding: 2.25rem 2.5rem 1.75rem;
    color: #171a17;
    background: #fbfbf7;
    border: 1px solid #d6ddd4;
    border-radius: 8px;
    box-shadow: inset 0 0 0 1px #ffffff;
  }

  .page-header {
    display: flex;
    justify-content: space-between;
    gap: 1rem;
    padding-bottom: 0.8rem;
    border-bottom: 1px solid #d6ddd4;
  }

  .page-header span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .page-content {
    min-height: 0;
    overflow: auto;
    overscroll-behavior: contain;
    padding-top: 1.35rem;
    scrollbar-gutter: stable;
  }

  :global(.page-content h1),
  :global(.page-content h2) {
    margin: 0 0 1rem;
    font-size: 1.75rem;
    line-height: 1.08;
    text-wrap: balance;
  }

  .chapter-content {
    font-size: var(--book-font-size);
    line-height: 1.58;
  }

  .page-footer {
    display: flex;
    justify-content: center;
    padding-top: 0.8rem;
    border-top: 1px solid #d6ddd4;
  }

  .page-controls {
    flex: 0 0 auto;
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 1rem;
    margin-top: 1rem;
    color: #ffffff;
  }

  .page-controls span {
    text-align: center;
  }

  .continuation {
    flex: 0 0 auto;
    display: flex;
    justify-content: center;
    margin-top: 2rem;
  }

  .state-screen {
    min-height: 100vh;
    display: grid;
    place-items: center;
    padding: 1.5rem;
  }

  .state-card {
    width: min(30rem, 100%);
    padding: 2rem;
    text-align: center;
    border-radius: 8px;
    background: #ffffff;
  }

  .state-title {
    margin: 0.85rem 0 0.45rem;
    font-size: 1.45rem;
  }

  .state-copy {
    margin: 0;
  }

  .error-card {
    color: #702c1f;
  }

  .spin {
    animation: spin 1s linear infinite;
  }

  @media (max-width: 960px) {
    .reader-shell {
      padding: 0.75rem;
      padding-top: 4.75rem;
    }

    .frame {
      height: calc(100vh - 5.5rem);
      height: calc(100dvh - 5.5rem);
      min-height: 0;
      display: block;
    }

    .spine {
      position: fixed;
      top: 0;
      bottom: 0;
      left: 0;
      z-index: 30;
      width: min(22rem, calc(100vw - 2.25rem));
      max-height: none;
      border-radius: 0 8px 8px 0;
      box-shadow: 0 1.5rem 3rem rgba(23, 26, 23, 0.28);
      transform: translateX(calc(-100% - 1rem));
      transition: transform 180ms ease;
      overscroll-behavior: contain;
    }

    .spine.open {
      transform: translateX(0);
    }

    .spine-title-row {
      grid-template-columns: minmax(0, 1fr) auto;
      align-items: start;
    }

    .spine-close {
      display: inline-flex;
      align-items: center;
      justify-content: center;
      width: 2.5rem;
      height: 2.5rem;
      color: #171a17;
      background: #f6f8f4;
      border: 1px solid #d6ddd4;
      border-radius: 8px;
      cursor: pointer;
    }

    .spine h1 {
      font-size: 1.7rem;
    }

    .chapter-list {
      max-height: 48vh;
    }

    .mobile-reader-bar {
      position: fixed;
      top: 0.75rem;
      left: 0.75rem;
      right: 0.75rem;
      z-index: 15;
      display: grid;
      grid-template-columns: auto minmax(0, 1fr) auto;
      align-items: center;
      gap: 0.75rem;
      min-height: 3.25rem;
      padding: 0.45rem 0.65rem;
      color: #171a17;
      background: rgba(255, 255, 255, 0.95);
      border: 1px solid #b8c0b6;
      border-radius: 8px;
      box-shadow: 0 0.85rem 1.8rem rgba(23, 26, 23, 0.16);
      backdrop-filter: blur(10px);
    }

    .mobile-reader-bar div {
      display: grid;
      gap: 0.12rem;
      min-width: 0;
    }

    .mobile-reader-bar span {
      overflow: hidden;
      color: #55625a;
      font-size: 0.82rem;
      text-overflow: ellipsis;
      white-space: nowrap;
    }

    .mobile-reader-bar strong {
      overflow: hidden;
      font-size: 0.95rem;
      font-weight: 600;
      text-overflow: ellipsis;
      white-space: nowrap;
    }

    .topbar-action {
      display: inline-flex;
      align-items: center;
      justify-content: center;
      width: 2.35rem;
      height: 2.35rem;
      color: #ffffff;
      background: #2c4735;
      border: 1px solid #2c4735;
      border-radius: 8px;
      cursor: pointer;
    }

    .menu-scrim {
      position: fixed;
      inset: 0;
      z-index: 25;
      display: block;
      padding: 0;
      background: rgba(23, 26, 23, 0.42);
      border: 0;
      cursor: pointer;
    }

    .book {
      height: 100%;
      min-height: 0;
      padding: 0.65rem;
    }

    .book-page-shell {
      height: 100%;
      min-height: auto;
    }

    .page {
      min-height: 0;
      height: 100%;
      max-height: none;
      padding: 1.4rem 1.25rem 1rem;
      border-radius: 8px;
      border: 1px solid #d6ddd4;
    }

    .page-controls {
      flex-wrap: wrap;
      align-items: center;
      gap: 0.75rem;
    }

    .page-controls span {
      order: -1;
      width: 100%;
    }

    .page-controls .ghost-button {
      flex: 0 1 calc(50% - 0.375rem);
      min-width: 0;
    }

    .selection-toolbar {
      flex-wrap: wrap;
      justify-content: center;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .spin {
      animation: none;
    }

    .spine {
      transition: none;
    }

    .ghost-button,
    .continue-button {
      transition: none;
    }
  }

  @keyframes spin {
    from {
      transform: rotate(0deg);
    }

    to {
      transform: rotate(360deg);
    }
  }
</style>
