<script lang="ts">
  import { goto } from '$app/navigation';
  import { onMount } from 'svelte';

  import { createBook, fetchBooks, type WebBook } from '$lib/api';
  import { sortBooks, formatTimestamp, apiFailureMessage } from '$lib/web-shell/format';
  import { uiCopy, uiLanguage, toggleUiLanguage } from '$lib/web-shell/language';
  import { withAuthorizedRequest } from '$lib/web-shell/session';

  type LoadState = 'idle' | 'loading' | 'ready' | 'empty' | 'error';

  let books: WebBook[] = [];
  let booksState: LoadState = 'idle';
  let booksErrorMessage = '';
  let createBookTitle = '';
  let createBookLanguage: WebBook['language'] = 'ru';
  let createBookPending = false;
  let createBookErrorMessage = '';

  onMount(() => {
    void loadBooks();
  });

  async function loadBooks() {
    booksState = 'loading';
    booksErrorMessage = '';

    try {
      books = sortBooks(await withAuthorizedRequest((token) => fetchBooks(token)));
      booksState = books.length > 0 ? 'ready' : 'empty';
    } catch (error) {
      books = [];
      booksState = 'error';
      booksErrorMessage = apiFailureMessage(error, $uiCopy.failedToLoadBooks);
    }
  }

  async function submitCreateBook(event: SubmitEvent) {
    event.preventDefault();
    if (createBookPending) {
      return;
    }

    createBookPending = true;
    createBookErrorMessage = '';

    try {
      const book = await withAuthorizedRequest((token) =>
        createBook(token, {
          title: createBookTitle.trim(),
          language: createBookLanguage
        })
      );
      books = sortBooks([...books.filter((existing) => existing.book_id !== book.book_id), book]);
      booksState = 'ready';
      createBookTitle = '';
      await goto(`/books/${encodeURIComponent(book.book_id)}`);
    } catch (error) {
      createBookErrorMessage = apiFailureMessage(error, $uiCopy.failedToCreateBook);
    } finally {
      createBookPending = false;
    }
  }

  function openBook(bookId: string) {
    return goto(`/books/${encodeURIComponent(bookId)}`);
  }
</script>

<svelte:head>
  <title>{$uiCopy.books} | {$uiCopy.appName}</title>
</svelte:head>

<main class="screen">
  <section class="hero">
    <div class="utility-row">
      <button class="language-toggle" on:click={toggleUiLanguage} type="button">
        {$uiLanguage === 'ru' ? $uiCopy.switchToEnglish : $uiCopy.switchToRussian}
      </button>
    </div>

    <p class="eyebrow">{$uiCopy.booksLandingEyebrow}</p>
    <h1>{$uiCopy.booksLandingTitle}</h1>
    <p class="lede">{$uiCopy.booksLandingCopy}</p>
  </section>

  <section class="layout">

    <section class="card list-card">
      <div class="section-intro">
        <p class="eyebrow">{$uiCopy.availableBooks}</p>
      </div>

      {#if booksState === 'loading'}
        <p class="empty-state">{$uiCopy.loadingBooks}</p>
      {:else if booksState === 'error'}
        <p class="error-banner">{booksErrorMessage}</p>
      {:else if booksState === 'empty'}
        <p class="empty-state">{$uiCopy.noBooks}</p>
      {:else}
        <div class="book-list">
          {#each books as book}
            <article class="book-item">
              <div>
                <h3>{book.title}</h3>
                <p>{book.subtitle}</p>
                <small>{$uiCopy.updatedAt(formatTimestamp(book.updated_at, $uiLanguage, $uiCopy))}</small>
              </div>
              <button class="cta small-cta" on:click={() => openBook(book.book_id)} type="button">
                {$uiCopy.openBook}
              </button>
            </article>
          {/each}
        </div>
      {/if}
    </section>
    <form class="card create-card" on:submit={submitCreateBook}>
      <div>
        <p class="eyebrow">{$uiCopy.createBook}</p>
        <h2>{$uiCopy.provisionWorkspace}</h2>
      </div>

      <label>
        <span>{$uiCopy.bookTitle}</span>
        <input bind:value={createBookTitle} name="title" required />
      </label>

      <label>
        <span>{$uiCopy.language}</span>
        <select bind:value={createBookLanguage} name="language">
          <option value="ru">{$uiCopy.languageLabels.ru}</option>
          <option value="en">{$uiCopy.languageLabels.en}</option>
        </select>
      </label>

      {#if createBookErrorMessage}
        <p class="error-banner">{createBookErrorMessage}</p>
      {/if}

      <button class="cta" disabled={createBookPending} type="submit">
        {createBookPending ? $uiCopy.creating : $uiCopy.createBookAction}
      </button>
    </form>
  </section>
</main>

<style>
  .screen {
    width: min(100%, 82rem);
    margin: 0 auto;
    padding: 2rem 1.25rem 3rem;
  }

  .hero,
  .card {
    border: 1px solid rgba(64, 40, 20, 0.12);
    background: rgba(255, 252, 247, 0.84);
    box-shadow: 0 1.2rem 3rem rgba(32, 20, 11, 0.12);
  }

  .hero {
    padding: 1.8rem;
    margin-bottom: 1rem;
  }

  .utility-row {
    display: flex;
    justify-content: flex-end;
    margin-bottom: 1rem;
  }

  .language-toggle,
  .cta,
  input,
  select {
    font: inherit;
  }

  .language-toggle {
    border: 1px solid rgba(85, 54, 28, 0.18);
    background: rgba(255, 255, 255, 0.72);
    color: #3c2a1a;
    padding: 0.45rem 0.7rem;
    cursor: pointer;
  }

  .eyebrow,
  .lede,
  h1,
  h2,
  h3,
  p {
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
    font-size: clamp(2.2rem, 5vw, 4.1rem);
    line-height: 0.98;
  }

  h2 {
    font-size: 1.5rem;
    line-height: 1.1;
  }

  .lede {
    margin-top: 1rem;
    max-width: 42rem;
    line-height: 1.6;
  }

  .layout {
    display: grid;
    grid-template-columns: minmax(18rem, 24rem) minmax(0, 1fr);
    gap: 1rem;
  }

  .card {
    padding: 1.25rem;
  }

  .create-card,
  .list-card {
    display: grid;
    gap: 1rem;
    align-content: start;
  }

  label {
    display: grid;
    gap: 0.45rem;
  }

  input,
  select {
    border: 1px solid rgba(64, 40, 20, 0.18);
    background: rgba(255, 255, 255, 0.85);
    padding: 0.85rem 0.95rem;
  }

  .cta {
    border: 1px solid rgba(85, 54, 28, 0.18);
    background: #5d3a18;
    color: #fbf2e4;
    padding: 0.85rem 1rem;
    cursor: pointer;
  }

  .small-cta {
    padding: 0.7rem 0.9rem;
  }

  .book-list {
    display: grid;
    gap: 0.85rem;
  }

  .book-item {
    display: flex;
    justify-content: space-between;
    gap: 1rem;
    align-items: start;
    padding: 1rem;
    border: 1px solid rgba(85, 54, 28, 0.12);
    background: rgba(255, 255, 255, 0.55);
  }

  .book-item p,
  .book-item small {
    margin-top: 0.4rem;
    line-height: 1.5;
  }

  .empty-state,
  .error-banner {
    padding: 1rem;
    border: 1px solid rgba(85, 54, 28, 0.12);
    background: rgba(255, 255, 255, 0.5);
  }

  .error-banner {
    color: #8b1f1f;
    border-color: rgba(139, 31, 31, 0.16);
    background: rgba(255, 232, 232, 0.8);
  }

  @media (max-width: 900px) {
    .screen {
      padding: 1rem 1rem 2rem;
    }

    .layout {
      grid-template-columns: 1fr;
    }

    .book-item {
      display: grid;
    }
  }
</style>
