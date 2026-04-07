<svelte:options runes={true} />

<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import {
    AlertTriangle,
    BookOpenText,
    Clock3,
    LoaderCircle,
    RefreshCcw,
    Sparkles
  } from 'lucide-svelte';
  import {
    fetchContent,
    fetchJob,
    fetchRevision,
    fetchSummary,
    ReaderApiError,
    type ReaderContent,
    type ReaderJob,
    type ReaderRevision,
    type ReaderSummary
  } from '$lib/api';

  const { params } = $props<{ params: { token: string } }>();

  const pollingStatuses = new Set(['received', 'accepted', 'running']);

  type NoticeTone = 'neutral' | 'warm' | 'success' | 'danger';
  type ReaderViewState = 'loading' | 'ready' | 'empty' | 'render_failed' | 'error';

  type ReaderNotice = {
    tone: NoticeTone;
    title: string;
    body: string;
  };

  let summary: ReaderSummary | null = $state(null);
  let revision: ReaderRevision | null = $state(null);
  let job: ReaderJob | null = $state(null);
  let chapters: ReaderContent[] = $state([]);
  let loading = $state(true);
  let loadingMore = $state(false);
  let refreshing = $state(false);
  let nextCursor: string | null = $state(null);
  let viewState: ReaderViewState = $state('loading');
  let errorTitle = $state('Reader unavailable');
  let errorMessage = $state('The reader could not load this draft.');
  let renderFailureMessage = $state('');
  let staleRevisionNotice = $state('');
  let refreshTimer: ReturnType<typeof window.setTimeout> | null = null;

  const pageTitle = $derived.by(() => (summary ? `${summary.title} | Reader` : 'Reader'));
  const activeRevisionId = $derived.by(() => revision?.revision_id ?? summary?.last_revision_id ?? null);
  const chapterLabel = $derived.by(() =>
    summary?.chapter_count === 1 ? '1 chapter' : `${summary?.chapter_count ?? 0} chapters`
  );
  const freshnessLabel = $derived.by(() => formatRelativeTime(summary?.last_updated_at ?? null));
  const revisionLabel = $derived.by(() =>
    activeRevisionId ? compactId(activeRevisionId) : 'Not rendered yet'
  );
  const jobBadge = $derived.by(() => resolveJobBadge(job));
  const renderBadge = $derived.by(() => resolveRenderBadge(summary, revision));
  const surfaceNotice = $derived.by(() =>
    resolveSurfaceNotice(viewState, job, renderFailureMessage, staleRevisionNotice)
  );

  function compactId(value: string): string {
    return value.length > 14 ? `${value.slice(0, 8)}...${value.slice(-4)}` : value;
  }

  function formatAbsoluteTime(value: string | null | undefined): string | null {
    if (!value) {
      return null;
    }

    const date = new Date(value);
    if (Number.isNaN(date.getTime())) {
      return null;
    }

    return new Intl.DateTimeFormat(undefined, {
      dateStyle: 'medium',
      timeStyle: 'short'
    }).format(date);
  }

  function formatRelativeTime(value: string | null | undefined): string {
    if (!value) {
      return 'Waiting for the first revision';
    }

    const date = new Date(value);
    if (Number.isNaN(date.getTime())) {
      return 'Revision time unavailable';
    }

    const diffMs = Date.now() - date.getTime();
    const diffMinutes = Math.round(diffMs / 60000);
    const formatter = new Intl.RelativeTimeFormat(undefined, { numeric: 'auto' });

    if (Math.abs(diffMinutes) < 1) {
      return 'Updated just now';
    }

    if (Math.abs(diffMinutes) < 60) {
      return `Updated ${formatter.format(-diffMinutes, 'minute')}`;
    }

    const diffHours = Math.round(diffMinutes / 60);
    if (Math.abs(diffHours) < 48) {
      return `Updated ${formatter.format(-diffHours, 'hour')}`;
    }

    const diffDays = Math.round(diffHours / 24);
    return `Updated ${formatter.format(-diffDays, 'day')}`;
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

  function resolveJobBadge(currentJob: ReaderJob | null) {
    if (!currentJob) {
      return {
        label: 'Quiet',
        detail: 'No active writing job',
        tone: 'neutral' as NoticeTone
      };
    }

    if (pollingStatuses.has(currentJob.status)) {
      return {
        label: 'Updating',
        detail: currentJob.user_facing_message ?? 'A writing job is still working through the draft.',
        tone: 'warm' as NoticeTone
      };
    }

    if (currentJob.status === 'succeeded') {
      return {
        label: 'Current',
        detail: currentJob.user_facing_message ?? 'The latest writing job finished successfully.',
        tone: 'success' as NoticeTone
      };
    }

    if (currentJob.status === 'failed' || currentJob.status === 'timed_out' || currentJob.status === 'cancelled') {
      return {
        label: titleCase(currentJob.status),
        detail: currentJob.user_facing_message ?? 'The latest writing job did not finish cleanly.',
        tone: 'danger' as NoticeTone
      };
    }

    return {
      label: titleCase(currentJob.status),
      detail: currentJob.user_facing_message ?? 'The latest writing job changed state.',
      tone: 'neutral' as NoticeTone
    };
  }

  function resolveRenderBadge(currentSummary: ReaderSummary | null, currentRevision: ReaderRevision | null) {
    const status = currentRevision?.render_status ?? currentSummary?.render_status ?? 'pending';

    if (status === 'ready') {
      return {
        label: 'Readable',
        detail: 'The current revision has a render snapshot ready.',
        tone: 'success' as NoticeTone
      };
    }

    if (status === 'failed') {
      return {
        label: 'Render issue',
        detail: 'The newest revision exists, but its reader render failed.',
        tone: 'danger' as NoticeTone
      };
    }

    return {
      label: 'Rendering',
      detail: 'The backend is still preparing the reader view.',
      tone: 'warm' as NoticeTone
    };
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
        title: 'Latest revision could not be rendered',
        body: currentRenderFailure || 'The draft exists, but the browser view for this revision is unavailable.'
      };
    }

    if (currentStaleNotice) {
      return {
        tone: 'warm',
        title: 'Draft refreshed',
        body: currentStaleNotice
      };
    }

    if (currentJob && pollingStatuses.has(currentJob.status)) {
      return {
        tone: 'warm',
        title: 'New pages may be on the way',
        body: currentJob.user_facing_message ?? 'The page will refresh while the current writing job is running.'
      };
    }

    if (currentState === 'empty') {
      return {
        tone: 'neutral',
        title: 'The shell is ready',
        body: 'This book does not have any rendered chapters yet.'
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

      if (cause.code === 'access_denied') {
        errorTitle = 'Reader link expired or invalid';
        errorMessage = cause.message;
        viewState = 'error';
        return;
      }

      errorTitle = titleCase(cause.code);
      errorMessage = cause.message;
      viewState = 'error';
      return;
    }

    errorTitle = 'Reader request failed';
    errorMessage = cause instanceof Error ? cause.message : 'The reader could not load this draft.';
    viewState = 'error';
  }

  async function loadReader(options: { background?: boolean } = {}) {
    const background = options.background ?? false;

    if (background) {
      refreshing = true;
    } else {
      loading = true;
      viewState = 'loading';
      staleRevisionNotice = '';
    }

    try {
      const nextSummary = await fetchSummary(params.token);
      const [nextRevision, nextJob] = await Promise.all([
        fetchRevision(params.token).catch(() => null),
        fetchJob(params.token).catch(() => null)
      ]);

      summary = nextSummary;
      revision = nextRevision;
      job = nextJob;
      renderFailureMessage = '';

      if (!nextSummary.last_revision_id) {
        chapters = [];
        nextCursor = null;
        viewState = 'empty';
        return;
      }

      if (nextRevision?.render_status === 'failed') {
        chapters = [];
        nextCursor = null;
        renderFailureMessage = nextRevision.render_error ?? nextRevision.summary;
        viewState = 'render_failed';
        return;
      }

      const firstChapter = await fetchContent(params.token, {
        revisionId: nextSummary.last_revision_id
      });

      const previousRevisionId = chapters[0]?.revision_id;
      const changedRevision = Boolean(
        background &&
          previousRevisionId &&
          previousRevisionId !== firstChapter.revision_id
      );

      chapters = [firstChapter];
      nextCursor = firstChapter.next_cursor;
      viewState = firstChapter.html.trim() || nextSummary.chapter_count > 0 ? 'ready' : 'empty';

      if (changedRevision) {
        staleRevisionNotice = `A newer revision replaced ${compactId(previousRevisionId)} with ${compactId(firstChapter.revision_id)}. The page reset to the latest chapter sequence.`;
      }
    } catch (cause) {
      if (!background) {
        handleLoadError(cause);
      }
    } finally {
      loading = false;
      refreshing = false;
      scheduleRefresh();
    }
  }

  async function loadMore() {
    if (!nextCursor || loadingMore) {
      return;
    }

    loadingMore = true;

    try {
      const nextChapter = await fetchContent(params.token, {
        cursor: nextCursor,
        revisionId: chapters[0]?.revision_id ?? activeRevisionId
      });

      chapters = [...chapters, nextChapter];
      nextCursor = nextChapter.next_cursor;
    } catch (cause) {
      if (cause instanceof ReaderApiError && cause.code === 'stale_revision') {
        staleRevisionNotice = 'The draft changed while you were reading. The page reloaded to keep continuation aligned with the newest render.';
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
    void loadReader();
  });

  onDestroy(() => {
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
      <p class="state-title">Loading the latest draft</p>
      <p class="state-copy">The reader is assembling the current revision and chapter sequence.</p>
    </div>
  </section>
{:else if viewState === 'error'}
  <section class="state-screen">
    <div class="state-card error-card">
      <AlertTriangle size={20} />
      <p class="state-title">{errorTitle}</p>
      <p class="state-copy">{errorMessage}</p>
      <button class="ghost-button" type="button" onclick={() => void loadReader()}>
        Try again
      </button>
    </div>
  </section>
{:else if summary}
  <main class="reader-shell">
    <section class="frame">
      <aside class="spine">
        <p class="eyebrow">Draft reader</p>
        <h1>{summary.title}</h1>
        <p class="subtitle">{summary.subtitle}</p>

        <div class="meta-block">
          <div>
            <span class="meta-label">Freshness</span>
            <strong>{freshnessLabel}</strong>
            <span>{formatAbsoluteTime(summary.last_updated_at) ?? 'No timestamp available'}</span>
          </div>
          <div>
            <span class="meta-label">Revision</span>
            <strong>{revisionLabel}</strong>
            <span>{chapterLabel}</span>
          </div>
        </div>

        <div class="status-stack">
          <div class={`status-pill ${renderBadge.tone}`}>
            <Sparkles size={16} />
            <div>
              <strong>{renderBadge.label}</strong>
              <span>{renderBadge.detail}</span>
            </div>
          </div>

          <div class={`status-pill ${jobBadge.tone}`}>
            <Clock3 size={16} />
            <div>
              <strong>{jobBadge.label}</strong>
              <span>{jobBadge.detail}</span>
            </div>
          </div>
        </div>

        <dl class="facts">
          <div>
            <dt>Book state</dt>
            <dd>{titleCase(summary.status)}</dd>
          </div>
          <div>
            <dt>Render state</dt>
            <dd>{titleCase(revision?.render_status ?? summary.render_status)}</dd>
          </div>
          <div>
            <dt>Job state</dt>
            <dd>{titleCase(job?.status ?? 'idle')}</dd>
          </div>
          <div>
            <dt>Job finished</dt>
            <dd>{formatAbsoluteTime(job?.finished_at) ?? 'Not finished yet'}</dd>
          </div>
        </dl>

        <div class="spine-actions">
            <button class="ghost-button" type="button" onclick={() => void loadReader({ background: true })}>
              <RefreshCcw size={16} class={refreshing ? 'spin' : undefined} />
              Refresh draft
            </button>
        </div>
      </aside>

      <section class="book">
        <header class="front-matter">
          <p class="front-kicker">Current view</p>
          <h2>{summary.title}</h2>
          <p class="front-copy">
            {#if viewState === 'empty'}
              The manuscript shell is ready, but no rendered chapter pages exist yet.
            {:else if viewState === 'render_failed'}
              The newest revision exists, but the reader surface could not be built for it.
            {:else}
              A quiet reading surface for the latest draft, loaded a chapter at a time.
            {/if}
          </p>
        </header>

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
              <p>Render output unavailable</p>
              <span>{renderFailureMessage}</span>
            </div>
          </section>
        {:else if viewState === 'empty'}
          <section class="empty-state">
            <BookOpenText size={18} />
            <div>
              <p>No chapters yet</p>
              <span>Once the first revision is rendered, the reading view will appear here.</span>
            </div>
          </section>
        {:else}
          <div class="chapters">
            {#each chapters as chapter}
              <article class="chapter">
                <header class="chapter-header">
                  <p>Chapter {chapter.chapter_index + 1}</p>
                  <h3>{chapter.title}</h3>
                </header>
                <div class="chapter-content">{@html chapter.html}</div>
              </article>
            {/each}
          </div>

          {#if nextCursor}
            <div class="continuation">
              <button class="continue-button" type="button" onclick={loadMore} disabled={loadingMore}>
                {#if loadingMore}
                  <LoaderCircle size={16} class="spin" />
                  Loading the next section
                {:else}
                  Load the next chapter
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
  :global(body) {
    margin: 0;
    color: #221814;
    font-family: Baskerville, 'Iowan Old Style', 'Palatino Linotype', 'Book Antiqua', serif;
    background:
      radial-gradient(circle at top left, rgba(159, 108, 54, 0.14), transparent 26rem),
      linear-gradient(180deg, #efe4d2 0%, #e3d4bf 48%, #d8c6af 100%);
  }

  :global(button) {
    font: inherit;
  }

  :global(.chapter-content p) {
    margin: 0 0 1.25rem;
  }

  :global(.chapter-content h1),
  :global(.chapter-content h2),
  :global(.chapter-content h3),
  :global(.chapter-content h4) {
    margin: 2rem 0 1rem;
    line-height: 1.15;
  }

  :global(.chapter-content blockquote) {
    margin: 1.75rem 0;
    padding-left: 1rem;
    border-left: 3px solid rgba(139, 95, 49, 0.28);
    color: #59463a;
  }

  .reader-shell {
    min-height: 100vh;
    padding: 1.5rem;
  }

  .frame {
    max-width: 90rem;
    margin: 0 auto;
    display: grid;
    grid-template-columns: minmax(18rem, 23rem) minmax(0, 1fr);
    gap: 1.5rem;
    align-items: start;
  }

  .spine,
  .book,
  .state-card {
    border: 1px solid rgba(67, 43, 25, 0.12);
    box-shadow: 0 1.4rem 3.4rem rgba(56, 35, 19, 0.1);
  }

  .spine {
    position: sticky;
    top: 1.5rem;
    padding: 1.75rem;
    background:
      linear-gradient(180deg, rgba(255, 249, 241, 0.94), rgba(246, 235, 217, 0.95)),
      #fbf6ee;
    border-radius: 1.4rem;
  }

  .eyebrow,
  .meta-label,
  .facts dt,
  .chapter-header p,
  .front-kicker {
    text-transform: uppercase;
    letter-spacing: 0.16em;
    font-size: 0.7rem;
    color: #90663d;
  }

  h1,
  h2,
  h3,
  .state-title {
    font-family: 'Iowan Old Style', Baskerville, 'Palatino Linotype', serif;
    font-weight: 500;
  }

  h1 {
    margin: 0;
    font-size: clamp(2.3rem, 4vw, 3.5rem);
    line-height: 0.94;
    text-wrap: balance;
  }

  .subtitle,
    .front-copy,
    .state-copy,
  .notice span,
  .empty-viewState span,
  .status-pill span {
    color: #654f42;
    line-height: 1.65;
  }

  .subtitle {
    margin: 1rem 0 1.75rem;
    font-size: 1.02rem;
  }

  .meta-block {
    display: grid;
    gap: 1rem;
    padding: 1rem 0 1.25rem;
    border-top: 1px solid rgba(67, 43, 25, 0.08);
    border-bottom: 1px solid rgba(67, 43, 25, 0.08);
  }

  .meta-block div,
  .status-pill div {
    display: grid;
    gap: 0.2rem;
  }

  .meta-block strong,
  .status-pill strong,
  .facts dd,
  .empty-viewState p,
  .notice p {
    font-weight: 600;
  }

  .meta-block span:last-child,
  .facts dd,
  .front-kicker,
  .chapter-header p {
    display: block;
    margin-top: 0.22rem;
  }

  .status-stack {
    display: grid;
    gap: 0.9rem;
    margin: 1.25rem 0;
  }

  .status-pill {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 0.75rem;
    align-items: start;
    padding: 0.95rem 1rem;
    border-radius: 1rem;
    border: 1px solid rgba(67, 43, 25, 0.08);
    background: rgba(255, 252, 247, 0.84);
  }

  .status-pill.neutral {
    background: rgba(255, 252, 247, 0.84);
  }

  .status-pill.warm {
    background: rgba(248, 234, 201, 0.75);
  }

  .status-pill.success {
    background: rgba(224, 241, 228, 0.85);
  }

  .status-pill.danger {
    background: rgba(247, 224, 216, 0.9);
  }

  .facts {
    margin: 0;
    display: grid;
    gap: 0.9rem;
  }

  .facts dd {
    margin: 0.25rem 0 0;
    color: #2f221b;
  }

  .spine-actions {
    margin-top: 1.4rem;
  }

  .ghost-button,
  .continue-button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 0.55rem;
    border-radius: 999px;
    cursor: pointer;
    transition:
      transform 140ms ease,
      background 140ms ease,
      border-color 140ms ease;
  }

  .ghost-button {
    padding: 0.8rem 1rem;
    color: #2f221b;
    background: rgba(255, 252, 247, 0.7);
    border: 1px solid rgba(67, 43, 25, 0.12);
  }

  .continue-button {
    padding: 0.95rem 1.3rem;
    color: #fef9f1;
    background: #4d2f1d;
    border: 1px solid #4d2f1d;
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
    min-height: calc(100vh - 3rem);
    padding: clamp(1.5rem, 4vw, 3rem);
    background:
      linear-gradient(180deg, rgba(255, 252, 247, 0.93), rgba(252, 247, 239, 0.96)),
      #fffaf2;
    border-radius: 1.8rem;
  }

  .front-matter {
    padding-bottom: 2rem;
    margin-bottom: 2rem;
    border-bottom: 1px solid rgba(67, 43, 25, 0.12);
  }

  .front-matter h2 {
    margin: 0.4rem 0 0.8rem;
    font-size: clamp(2rem, 4vw, 3rem);
    line-height: 0.98;
    text-wrap: balance;
  }

  .front-copy {
    max-width: 42rem;
    margin: 0;
    font-size: 1.04rem;
  }

  .notice,
  .empty-state {
    display: grid;
    grid-template-columns: 1fr;
    gap: 0.35rem;
    padding: 1rem 1.1rem;
    border-radius: 1rem;
    border: 1px solid rgba(67, 43, 25, 0.08);
    margin-bottom: 1.6rem;
  }

  .notice.neutral,
  .empty-state {
    background: rgba(243, 234, 221, 0.56);
  }

  .notice.warm {
    background: rgba(248, 234, 201, 0.64);
  }

  .notice.success {
    background: rgba(224, 241, 228, 0.72);
  }

  .notice.danger {
    background: rgba(247, 224, 216, 0.82);
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

  .chapters {
    display: grid;
    gap: 2.5rem;
  }

  .chapter {
    position: relative;
    max-width: 46rem;
    margin: 0 auto;
    padding: clamp(1.4rem, 4vw, 2.6rem);
    background:
      linear-gradient(180deg, rgba(255, 255, 255, 0.72), rgba(251, 246, 238, 0.92)),
      #fffdf8;
    border: 1px solid rgba(67, 43, 25, 0.1);
    border-radius: 1.4rem;
    box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.7);
  }

  .chapter::before {
    content: '';
    position: absolute;
    inset: 0.8rem auto 0.8rem 0.8rem;
    width: 0.18rem;
    border-radius: 999px;
    background: linear-gradient(180deg, rgba(139, 95, 49, 0.45), rgba(139, 95, 49, 0));
  }

  .chapter-header {
    margin-bottom: 1.4rem;
    padding-left: 0.9rem;
  }

  .chapter-header h3 {
    margin: 0.3rem 0 0;
    font-size: clamp(1.7rem, 3vw, 2.5rem);
    line-height: 1.02;
  }

  .chapter-content {
    padding-left: 0.9rem;
    font-size: clamp(1.06rem, 1.6vw, 1.16rem);
    line-height: 1.88;
  }

  .continuation {
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
    border-radius: 1.4rem;
    background: rgba(255, 250, 243, 0.92);
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
      padding: 1rem;
    }

    .frame {
      grid-template-columns: 1fr;
    }

    .spine {
      position: static;
    }

    .book {
      min-height: auto;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .spin {
      animation: none;
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
