<script lang="ts">
  import { goto } from '$app/navigation';
  import { onDestroy, onMount, tick } from 'svelte';
  import { ArrowLeft, Eye } from 'lucide-svelte';

  import {
    createConversationMessage,
    fetchBooks,
    fetchConversationMessages,
    fetchConversations,
    type ConversationMessagesResponse,
    type WebBook,
    type WebConversation,
    type WebConversationMessage
  } from '$lib/api';
  import {
    clearPendingReaderMention,
    loadPendingReaderMention,
    type PendingReaderMention
  } from '$lib/reader-mentions';
  import {
    apiFailureMessage,
    formatTimestamp,
    roleLabel,
    sortConversations
  } from '$lib/web-shell/format';
  import { uiCopy, uiLanguage } from '$lib/web-shell/language';
  import { withAuthorizedRequest } from '$lib/web-shell/session';
  import ConversationComposer from '$lib/screens/conversation-chat/ConversationComposer.svelte';
  import PendingReaderMentionCard from '$lib/screens/conversation-chat/PendingReaderMentionCard.svelte';

  export let bookId: string;
  export let conversationId: string;

  type LoadState = 'idle' | 'loading' | 'ready' | 'empty' | 'error';

  const TRANSCRIPT_POLL_INTERVAL_MS = 5000;

  let book: WebBook | null = null;
  let conversations: WebConversation[] = [];
  let transcript: WebConversationMessage[] = [];
  let transcriptState: LoadState = 'idle';
  let conversationsErrorMessage = '';
  let transcriptErrorMessage = '';
  let latestCommentary = '';
  let conversationMessageDraft = '';
  let sendMessagePending = false;
  let sendMessageErrorMessage = '';
  let composerResetToken = 0;
  let pendingMessage: { conversationId: string; text: string } | null = null;
  let pendingReaderMention: PendingReaderMention | null = null;
  let transcriptPollHandle: ReturnType<typeof window.setInterval> | null = null;
  let conversationsRequestId = 0;
  let transcriptRequestId = 0;

  $: selectedConversation =
    conversations.find((conversation) => conversation.conversation_id === conversationId) ?? null;
  $: displayedTranscript = [...transcript].reverse();
  $: activePendingReaderMention =
    pendingReaderMention &&
    pendingReaderMention.target_book_id === bookId &&
    pendingReaderMention.target_conversation_id === conversationId
      ? pendingReaderMention
      : null;

  $: if (
    pendingMessage &&
    pendingMessage.conversationId === conversationId &&
    transcript.some((message) => message.role === 'user' && message.text === pendingMessage?.text)
  ) {
    pendingMessage = null;
  }

  $: if (
    pendingMessage &&
    pendingMessage.conversationId === conversationId &&
    selectedConversation &&
    selectedConversation.status !== 'in_progress'
  ) {
    pendingMessage = null;
  }

  onMount(() => {
    pendingReaderMention = loadPendingReaderMention();
    void loadScreen();
    syncTranscriptPolling();
  });

  onDestroy(() => {
    stopTranscriptPolling();
  });

  async function loadScreen() {
    const [, hasSelectedConversation] = await Promise.all([loadBook(), loadConversations()]);

    if (hasSelectedConversation) {
      await tick();
      await loadTranscript();
      syncTranscriptPolling();
    }
  }

  async function loadBook() {
    try {
      const books = await withAuthorizedRequest((token) => fetchBooks(token));
      book = books.find((item) => item.book_id === bookId) ?? null;
    } catch {
      book = null;
    }
  }

  async function loadConversations() {
    const requestId = ++conversationsRequestId;
    conversationsErrorMessage = '';

    try {
      const nextConversations = await withAuthorizedRequest((token) => fetchConversations(token, bookId));

      if (requestId !== conversationsRequestId) {
        return;
      }

      conversations = sortConversations(nextConversations);
      const hasSelectedConversation = conversations.some(
        (conversation) => conversation.conversation_id === conversationId
      );
      conversationsErrorMessage = hasSelectedConversation ? '' : $uiCopy.conversationNotFound;
      return hasSelectedConversation;
    } catch (error) {
      if (requestId !== conversationsRequestId) {
        return false;
      }

      conversations = [];
      conversationsErrorMessage = apiFailureMessage(error, $uiCopy.failedToLoadConversations);
      return false;
    }
  }

  async function loadTranscript(options: { silent?: boolean } = {}) {
    if (!selectedConversation) {
      transcript = [];
      latestCommentary = '';
      transcriptState = 'error';
      transcriptErrorMessage = $uiCopy.conversationNotFound;
      return;
    }

    const requestId = ++transcriptRequestId;
    if (!options.silent) {
      transcriptState = 'loading';
      transcriptErrorMessage = '';
    }

    try {
      const transcriptResponse = await withAuthorizedRequest((token) =>
        fetchConversationMessages(token, bookId, conversationId)
      );
      const nextTranscript = transcriptResponse.messages;

      if (requestId !== transcriptRequestId) {
        return;
      }

      applyConversationStatus(transcriptResponse);
      transcript = nextTranscript;
      latestCommentary = transcriptResponse.last_comment?.trim() ?? '';
      transcriptState = nextTranscript.length > 0 ? 'ready' : 'empty';
      sendMessageErrorMessage = '';
      syncTranscriptPolling();
    } catch (error) {
      if (requestId !== transcriptRequestId) {
        return;
      }

      transcript = [];
      latestCommentary = '';
      transcriptState = 'error';
      transcriptErrorMessage = apiFailureMessage(error, $uiCopy.failedToLoadTranscript);
    }
  }

  async function submitConversationMessage(event: CustomEvent<{ text: string; image: File | null }>) {
    if (!selectedConversation || sendMessagePending) {
      return;
    }

    const text = event.detail.text.trim();
    if (!text) {
      sendMessageErrorMessage = $uiCopy.messageRequired;
      return;
    }

    sendMessagePending = true;
    sendMessageErrorMessage = '';

    try {
      const payload = new FormData();
      payload.set('text', text);
      if (event.detail.image) {
        payload.set('image', event.detail.image);
      }

      await withAuthorizedRequest((token) => createConversationMessage(token, bookId, conversationId, payload));
      setConversationStatus('in_progress');
      pendingMessage = {
        conversationId,
        text
      };
      conversationMessageDraft = '';
      composerResetToken += 1;
      await loadConversations();
      syncTranscriptPolling();
      void loadTranscript({ silent: true });
    } catch (error) {
      sendMessageErrorMessage = apiFailureMessage(error, $uiCopy.failedToSendMessage);
    } finally {
      sendMessagePending = false;
    }
  }

  function syncTranscriptPolling() {
    stopTranscriptPolling();
    if (
      !selectedConversation ||
      selectedConversation.status !== 'in_progress' ||
      typeof window === 'undefined'
    ) {
      return;
    }

    transcriptPollHandle = window.setInterval(() => {
      void loadTranscript({ silent: true });
    }, TRANSCRIPT_POLL_INTERVAL_MS);
  }

  function stopTranscriptPolling() {
    if (transcriptPollHandle) {
      clearInterval(transcriptPollHandle);
      transcriptPollHandle = null;
    }
  }

  function applyConversationStatus(response: ConversationMessagesResponse) {
    setConversationStatus(response.status);
  }

  function setConversationStatus(status: string) {
    conversations = conversations.map((conversation) =>
      conversation.conversation_id === conversationId ? { ...conversation, status } : conversation
    );
  }

  function dismissPendingReaderMention() {
    clearPendingReaderMention();
    pendingReaderMention = null;
  }

  function openEdits() {
    return goto(`/books/${encodeURIComponent(bookId)}/edits`);
  }

  function openPreview() {
    return goto(`/reader/${encodeURIComponent(bookId)}`);
  }
</script>

<svelte:head>
  <title>{selectedConversation?.title ?? $uiCopy.pravki} | {$uiCopy.appName}</title>
</svelte:head>

<main class="screen">
  <section class="hero">
    <div class="utility-row">
      <button class="back-link" on:click={openEdits} type="button">
        <ArrowLeft size={18} strokeWidth={1.8} aria-hidden="true" />
        <span>{$uiCopy.backToEdits}</span>
      </button>
      <button class="preview-link" on:click={openPreview} type="button">
        <Eye size={18} strokeWidth={1.8} aria-hidden="true" />
        <span>{$uiCopy.previewBook}</span>
      </button>
    </div>

    <p class="eyebrow">{$uiCopy.chatEyebrow}</p>
    <h1>{selectedConversation?.title ?? $uiCopy.chatTitle}</h1>
    {#if book}
      <p class="subtitle">{book.title}</p>
    {/if}
    <p class="lede">{$uiCopy.chatCopy}</p>
  </section>

  <section class="chat-layout">
    <section class="transcript-panel">
      {#if conversationsErrorMessage}
        <p class="error-banner">{conversationsErrorMessage}</p>
      {/if}

      {#if activePendingReaderMention}
        <PendingReaderMentionCard mention={activePendingReaderMention} onDismiss={dismissPendingReaderMention} />
      {/if}

      {#if selectedConversation && selectedConversation.status !== 'in_progress'}
        <ConversationComposer
          bind:draft={conversationMessageDraft}
          disabled={sendMessagePending}
          pending={sendMessagePending}
          resetToken={composerResetToken}
          on:submit={submitConversationMessage}
        />
      {/if}

      {#if sendMessageErrorMessage}
        <p class="error-banner">{sendMessageErrorMessage}</p>
      {/if}

      {#if selectedConversation?.status === 'in_progress'}
        <article class="message-card assistant status-message">
          <div class="message-header status-header">
            <strong>Codex</strong>
            <span class="progress-dots" aria-label="In progress">
              <span></span>
              <span></span>
              <span></span>
            </span>
          </div>
          <p>{latestCommentary || $uiCopy.workInProgress}</p>
        </article>
      {/if}

      {#if pendingMessage && pendingMessage.conversationId === conversationId}
        <article class="message-card">
          <div class="message-header">
            <strong>{$uiCopy.you}</strong>
          </div>
          <p>{pendingMessage.text}</p>
        </article>
      {/if}

      {#if transcriptState === 'loading'}
        <p class="empty-state">{$uiCopy.loadingTranscript}</p>
      {:else if transcriptState === 'error'}
        <p class="error-banner">{transcriptErrorMessage}</p>
      {:else if transcriptState === 'empty'}
        <p class="empty-state">{$uiCopy.transcriptEmpty}</p>
      {:else if transcriptState === 'ready'}
        <div class="transcript-list">
          {#each displayedTranscript as message}
            <article class:assistant={message.role === 'assistant'} class="message-card">
              <div class="message-header">
                <strong>{roleLabel(message.role, $uiCopy)}</strong>
                <span>{formatTimestamp(message.timestamp, $uiLanguage, $uiCopy)}</span>
              </div>
              <p>{message.text}</p>
            </article>
          {/each}
        </div>
      {/if}
    </section>
  </section>
</main>

<style>
  .screen {
    width: min(100%, 86rem);
    margin: 0 auto;
    padding: 2rem 1.25rem 3rem;
  }

  .hero,
  .transcript-panel {
    border: 1px solid rgba(64, 40, 20, 0.12);
    background: rgba(255, 252, 247, 0.84);
    box-shadow: 0 1.2rem 3rem rgba(32, 20, 11, 0.12);
  }

  .hero {
    padding: 1.5rem;
    margin-bottom: 1rem;
  }

  .utility-row {
    display: grid;
    grid-template-columns: max-content 1fr max-content;
    gap: 1rem;
    align-items: center;
  }

  .back-link,
  .preview-link {
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
    font-size: clamp(1.9rem, 4.5vw, 3.3rem);
    line-height: 0.98;
  }

  .subtitle,
  .lede {
    margin-top: 0.9rem;
    line-height: 1.6;
  }

  .chat-layout {
    min-height: 38rem;
  }

  .transcript-panel {
    padding: 1.25rem;
    overflow: hidden;
  }

  .transcript-list {
    display: grid;
    gap: 0.85rem;
  }

  .message-header span {
    font-size: 0.82rem;
    opacity: 0.8;
  }

  .transcript-panel {
    display: grid;
    gap: 1rem;
    align-content: start;
  }

  .message-card {
    display: grid;
    gap: 0.75rem;
    padding: 1rem;
    border: 1px solid rgba(85, 54, 28, 0.1);
    background: rgba(255, 255, 255, 0.6);
  }

  .message-card.assistant {
    background: rgba(240, 224, 202, 0.58);
  }

  .status-message {
    border-color: rgba(28, 92, 85, 0.22);
    background:
      radial-gradient(circle at top right, rgba(131, 205, 193, 0.24), transparent 35%),
      rgba(232, 247, 243, 0.95);
    color: #143c38;
  }

  .message-header {
    display: flex;
    justify-content: space-between;
    gap: 0.75rem;
    align-items: baseline;
  }

  .status-header {
    align-items: center;
  }

  .progress-dots {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
  }

  .progress-dots span {
    width: 0.55rem;
    height: 0.55rem;
    border-radius: 999px;
    background: rgba(28, 92, 85, 0.35);
    animation: status-pulse 1.2s ease-in-out infinite;
  }

  .progress-dots span:nth-child(2) {
    animation-delay: 0.2s;
  }

  .progress-dots span:nth-child(3) {
    animation-delay: 0.4s;
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

  @keyframes status-pulse {
    0%,
    100% {
      transform: scale(0.85);
      opacity: 0.38;
    }

    50% {
      transform: scale(1.15);
      opacity: 1;
      background: #1c5c55;
    }
  }

  @media (max-width: 700px) {
    .screen {
      padding: 1rem 1rem 2rem;
    }

    .message-header {
      display: grid;
    }
  }
</style>
