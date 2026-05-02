<script lang="ts">
  import { goto } from '$app/navigation';
  import { onMount } from 'svelte';

  import { fetchBooks, type WebBook } from '$lib/api';
  import { apiFailureMessage } from '$lib/web-shell/format';
  import { uiCopy } from '$lib/web-shell/language';
  import { withAuthorizedRequest } from '$lib/web-shell/session';

  export let bookId: string;

  let book: WebBook | null = null;
  let errorMessage = '';
  let loading = true;

  onMount(() => {
    void loadBook();
  });

  async function loadBook() {
    loading = true;
    errorMessage = '';

    try {
      const books = await withAuthorizedRequest((token) => fetchBooks(token));
      book = books.find((item) => item.book_id === bookId) ?? null;
      if (!book) {
        errorMessage = $uiCopy.failedToLoadBooks;
      }
    } catch (error) {
      errorMessage = apiFailureMessage(error, $uiCopy.failedToLoadBooks);
    } finally {
      loading = false;
    }
  }

  function openBooks() {
    return goto('/');
  }

  function openPreview() {
    return goto(`/reader/${encodeURIComponent(bookId)}`);
  }

  function openEdits() {
    return goto(`/books/${encodeURIComponent(bookId)}/edits`);
  }
</script>

<svelte:head>
  <title>{book?.title ?? $uiCopy.books} | {$uiCopy.appName}</title>
</svelte:head>

<main class="screen">
  <section class="hero">
    <div class="utility-row">
      <button class="back-link" on:click={openBooks} type="button">{$uiCopy.backToBooks}</button>
    </div>

    {#if loading}
      <p class="eyebrow">{$uiCopy.bookHomeEyebrow}</p>
      <h1>{$uiCopy.loadingBooks}</h1>
    {:else if errorMessage}
      <p class="error-banner">{errorMessage}</p>
    {:else if book}
      <p class="eyebrow">{$uiCopy.bookHomeEyebrow}</p>
      <h1>{book.title}</h1>
      <div class="action-grid">
        <button class="action-card" on:click={openPreview} type="button">
          <span class="action-icon" aria-hidden="true">
            <svg viewBox="0 0 24 24" focusable="false">
              <path
                d="M1.5 12s3.8-6.5 10.5-6.5S22.5 12 22.5 12 18.7 18.5 12 18.5 1.5 12 1.5 12Z"
              />
              <circle cx="12" cy="12" r="3.5" />
            </svg>
          </span>
          <span class="action-copy">
            <strong>{$uiCopy.previewBook}</strong>
            <span>{$uiCopy.readerShortcutCopy}</span>
          </span>
        </button>

        <button class="action-card" on:click={openEdits} type="button">
          <span class="action-icon" aria-hidden="true">
            <svg viewBox="0 0 24 24" focusable="false">
              <path
                d="M3 17.25V21h3.75L18.8 8.95l-3.75-3.75L3 17.25Z"
              />
              <path d="m14.95 5.2 3.75 3.75" />
            </svg>
          </span>
          <span class="action-copy">
            <strong>{$uiCopy.pravki}</strong>
            <span>{$uiCopy.editsShortcutCopy}</span>
          </span>
        </button>
      </div>
    {/if}
  </section>
</main>

<style>
  .screen {
    width: min(100%, 72rem);
    margin: 0 auto;
    padding: 2rem 1.25rem 3rem;
  }

  .hero {
    padding: 1.8rem;
    border: 1px solid rgba(64, 40, 20, 0.12);
    background: rgba(255, 252, 247, 0.84);
    box-shadow: 0 1.2rem 3rem rgba(32, 20, 11, 0.12);
  }

  .utility-row {
    display: flex;
    justify-content: space-between;
    gap: 1rem;
    margin-bottom: 1rem;
  }

  .back-link,
  .action-card {
    font: inherit;
  }

  .back-link {
    border: 1px solid rgba(85, 54, 28, 0.18);
    background: rgba(255, 255, 255, 0.72);
    color: #3c2a1a;
    padding: 0.45rem 0.7rem;
    cursor: pointer;
  }

  .eyebrow,
  h1 {
    margin: 0;
  }

  .eyebrow {
    margin-bottom: 0.75rem;
    text-transform: uppercase;
    letter-spacing: 0.1em;
    font-size: 0.78rem;
    color: #805b2e;
  }

  h1 {
    font-size: clamp(2.2rem, 5vw, 4rem);
    line-height: 0.98;
  }

  .action-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 1rem;
    margin-top: 1.5rem;
  }

  .action-card {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    align-items: center;
    gap: 1rem;
    padding: 1.3rem;
    text-align: left;
    cursor: pointer;
    border: 1px solid rgba(85, 54, 28, 0.14);
    background: rgba(255, 255, 255, 0.55);
  }

  .action-copy {
    display: grid;
    gap: 0.65rem;
  }

  .action-copy strong,
  .action-copy span {
    margin: 0;
  }

  .action-icon {
    display: grid;
    place-items: center;
    width: 4.5rem;
    height: 4.5rem;
    color: #6c4b25;
    background: rgba(233, 220, 203, 0.42);
    border: 1px solid rgba(108, 75, 37, 0.14);
  }

  .action-icon svg {
    width: 2.6rem;
    height: 2.6rem;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.5;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .error-banner {
    margin: 0;
    padding: 1rem;
    color: #8b1f1f;
    border: 1px solid rgba(139, 31, 31, 0.16);
    background: rgba(255, 232, 232, 0.8);
  }

  @media (max-width: 900px) {
    .screen {
      padding: 1rem 1rem 2rem;
    }

    .utility-row,
    .action-grid {
      grid-template-columns: 1fr;
      display: grid;
    }

    .action-card {
      grid-template-columns: 1fr;
      align-items: start;
    }
  }
</style>
