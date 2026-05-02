<script lang="ts">
  import { goto } from '$app/navigation';
  import { onMount } from 'svelte';
  import { ArrowLeft, Eye } from 'lucide-svelte';

  import { createConversation, fetchBooks, fetchConversations, type WebBook, type WebConversation } from '$lib/api';
  import {
    apiFailureMessage,
    findLatestActiveConversation,
    formatTimestamp,
    sortConversations
  } from '$lib/web-shell/format';
  import { uiCopy, uiLanguage } from '$lib/web-shell/language';
  import { withAuthorizedRequest } from '$lib/web-shell/session';

  export let bookId: string;

  type LoadState = 'idle' | 'loading' | 'ready' | 'empty' | 'error';

  let book: WebBook | null = null;
  let conversations: WebConversation[] = [];
  let conversationsState: LoadState = 'idle';
  let conversationsErrorMessage = '';
  let createConversationPending = false;
  let createConversationErrorMessage = '';
  let latestConversationId: string | null = null;

  onMount(() => {
    void loadScreen();
  });

  async function loadScreen() {
    conversationsState = 'loading';
    conversationsErrorMessage = '';

    try {
      const [books, nextConversations] = await Promise.all([
        withAuthorizedRequest((token) => fetchBooks(token)),
        withAuthorizedRequest((token) => fetchConversations(token, bookId))
      ]);

      book = books.find((item) => item.book_id === bookId) ?? null;
      conversations = sortConversations(nextConversations);
      latestConversationId = findLatestActiveConversation(conversations)?.conversation_id ?? null;
      conversationsState = conversations.length > 0 ? 'ready' : 'empty';
    } catch (error) {
      conversations = [];
      conversationsState = 'error';
      conversationsErrorMessage = apiFailureMessage(error, $uiCopy.failedToLoadConversations);
    }
  }

  function openBookHome() {
    return goto(`/books/${encodeURIComponent(bookId)}`);
  }

  function openPreview() {
    return goto(`/reader/${encodeURIComponent(bookId)}`);
  }

  function openConversation(conversationId: string) {
    return goto(
      `/books/${encodeURIComponent(bookId)}/edits/${encodeURIComponent(conversationId)}`
    );
  }

  async function startConversation() {
    if (createConversationPending) {
      return;
    }

    createConversationPending = true;
    createConversationErrorMessage = '';

    try {
      const conversation = await withAuthorizedRequest((token) => createConversation(token, bookId));
      await goto(
        `/books/${encodeURIComponent(bookId)}/edits/${encodeURIComponent(conversation.conversation_id)}`
      );
    } catch (error) {
      createConversationErrorMessage = apiFailureMessage(
        error,
        $uiCopy.failedToCreateConversation
      );
    } finally {
      createConversationPending = false;
    }
  }
</script>

<svelte:head>
  <title>{book?.title ?? $uiCopy.pravki} | {$uiCopy.appName}</title>
</svelte:head>

<main class="screen">
  <section class="hero">
    <div class="utility-row">
      <button class="back-link" on:click={openBookHome} type="button">
        <ArrowLeft size={18} strokeWidth={1.8} aria-hidden="true" />
        <span>{$uiCopy.backToBook}</span>
      </button>

      <button class="preview-link" on:click={openPreview} type="button">
        <Eye size={18} strokeWidth={1.8} aria-hidden="true" />
        <span>{$uiCopy.previewBook}</span>
      </button>
    </div>

    <p class="eyebrow">{$uiCopy.editsEyebrow}</p>
    <h1>{book?.title ?? $uiCopy.pravki}</h1>
    {#if book}
      <p class="subtitle">{book.subtitle}</p>
    {/if}
    <p class="lede">{$uiCopy.editsCopy}</p>
  </section>

  <section class="card">
    <div class="section-header">
      <div>
        <p class="eyebrow">{$uiCopy.conversationList}</p>
        <h2>{$uiCopy.editsTitle}</h2>
      </div>
      <button
        class="cta"
        disabled={createConversationPending}
        on:click={startConversation}
        type="button"
      >
        {createConversationPending ? $uiCopy.creating : $uiCopy.newConversation}
      </button>
    </div>

    {#if createConversationErrorMessage}
      <p class="error-banner">{createConversationErrorMessage}</p>
    {/if}

    {#if conversationsState === 'loading'}
      <p class="empty-state">{$uiCopy.loadingConversations}</p>
    {:else if conversationsState === 'error'}
      <p class="error-banner">{conversationsErrorMessage}</p>
    {:else if conversationsState === 'empty'}
      <p class="empty-state">{$uiCopy.noConversations}</p>
    {:else}
      <div class="conversation-list">
        {#each conversations as conversation}
          <button class="conversation-item" on:click={() => openConversation(conversation.conversation_id)} type="button">
            <span class="title-row">
              <strong>{conversation.title}</strong>
              {#if conversation.conversation_id === latestConversationId}
                <span class="badge">{$uiCopy.latestActive}</span>
              {/if}
            </span>
            <span class="meta">
              {$uiCopy.lastActivity(formatTimestamp(conversation.last_active_at, $uiLanguage, $uiCopy))}
            </span>
          </button>
        {/each}
      </div>
    {/if}
  </section>
</main>

<style>
  .screen {
    width: min(100%, 78rem);
    margin: 0 auto;
    padding: 2rem 1.25rem 3rem;
  }

  .hero,
  .card {
    border: 1px solid rgba(64, 40, 20, 0.12);
    background: rgba(255, 252, 247, 0.84);
    box-shadow: 0 1.2rem 3rem rgba(32, 20, 11, 0.12);
  }

  .hero,
  .card {
    padding: 1.5rem;
  }

  .hero {
    margin-bottom: 1rem;
  }

  .utility-row,
  .section-header {
    display: grid;
    gap: 1rem;
    align-items: center;
  }

  .utility-row {
    grid-template-columns: max-content 1fr max-content;
  }

  .section-header {
    justify-content: space-between;
  }

  .back-link,
  .preview-link,
  .cta,
  .conversation-item {
    font: inherit;
  }

  .back-link,
  .preview-link {
    display: inline-flex;
    align-items: center;
    gap: 0.6rem;
    border: 1px solid rgba(85, 54, 28, 0.18);
    background: rgba(255, 255, 255, 0.72);
    color: #3c2a1a;
    padding: 0.8rem 1.1rem;
    cursor: pointer;
    transition:
      transform 140ms ease,
      background-color 140ms ease,
      border-color 140ms ease,
      box-shadow 140ms ease;
  }

  .back-link:hover,
  .preview-link:hover {
    background: rgba(255, 255, 255, 0.94);
    border-color: rgba(85, 54, 28, 0.28);
    box-shadow: 0 0.8rem 1.8rem rgba(60, 42, 26, 0.1);
    transform: translateY(-1px);
  }

  .back-link {
    grid-column: 1;
    justify-content: flex-start;
  }

  .preview-link {
    grid-column: 3;
    justify-content: center;
    min-width: 11.5rem;
  }

  .eyebrow,
  .subtitle,
  .lede,
  h1,
  h2,
  p {
    margin: 0;
  }

  .eyebrow {
    margin-top: 2rem;
    margin-bottom: 0.75rem;
    text-transform: uppercase;
    letter-spacing: 0.1em;
    font-size: 0.78rem;
    color: #805b2e;
  }

  h1 {
    font-size: clamp(2rem, 4.8vw, 3.5rem);
    line-height: 0.98;
  }

  .subtitle,
  .lede {
    margin-top: 0.9rem;
    line-height: 1.6;
  }

  .cta {
    border: 1px solid rgba(85, 54, 28, 0.18);
    background: #5d3a18;
    color: #fbf2e4;
    padding: 0.85rem 1rem;
    cursor: pointer;
  }

  .conversation-list {
    display: grid;
    gap: 0.85rem;
    margin-top: 1rem;
  }

  .conversation-item {
    display: grid;
    gap: 0.45rem;
    width: 100%;
    text-align: left;
    padding: 0.95rem;
    border: 1px solid rgba(85, 54, 28, 0.12);
    background: rgba(255, 255, 255, 0.55);
    cursor: pointer;
  }

  .title-row {
    display: flex;
    justify-content: space-between;
    gap: 0.75rem;
    align-items: center;
  }

  .meta {
    font-size: 0.82rem;
    opacity: 0.8;
  }

  .badge {
    padding: 0.2rem 0.55rem;
    font-size: 0.72rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: #7a4d1f;
    background: rgba(232, 191, 122, 0.22);
    border: 1px solid rgba(128, 91, 46, 0.2);
  }

  .empty-state,
  .error-banner {
    margin-top: 1rem;
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

    .section-header,
    .title-row {
      display: grid;
    }
  }
</style>
