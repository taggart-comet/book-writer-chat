<script lang="ts">
  import type { PendingReaderMention } from '$lib/reader-mentions';
  import { uiCopy } from '$lib/web-shell/language';

  export let mention: PendingReaderMention;
  export let onDismiss: () => void;
</script>

<article class="pending-mention-card">
  <div class="pending-mention-header">
    <div>
      <strong>{$uiCopy.pendingReaderMention}</strong>
      <span>
        {mention.target_mode === 'new_conversation'
          ? $uiCopy.mentionNewConversation
          : $uiCopy.mentionLatestConversation}
      </span>
    </div>
    <button class="ghost-button" type="button" on:click={onDismiss}>{$uiCopy.dismiss}</button>
  </div>
  <p class="pending-mention-source">{mention.payload.reference_label}</p>
  <blockquote>{mention.payload.excerpt}</blockquote>
  <pre>{mention.payload.message_text}</pre>
</article>

<style>
  .pending-mention-card {
    display: grid;
    gap: 0.8rem;
    padding: 1rem;
    border: 1px solid rgba(128, 91, 46, 0.22);
    background: rgba(232, 191, 122, 0.16);
  }

  .pending-mention-header {
    display: flex;
    justify-content: space-between;
    gap: 1rem;
    align-items: start;
  }

  .pending-mention-header div,
  blockquote {
    display: grid;
    gap: 0.35rem;
    margin: 0;
  }

  .pending-mention-source,
  .pending-mention-header span {
    margin: 0;
    font-size: 0.82rem;
    opacity: 0.82;
  }

  pre {
    margin: 0;
    padding: 0.85rem;
    overflow: auto;
    white-space: pre-wrap;
    background: rgba(255, 255, 255, 0.55);
    border: 1px solid rgba(85, 54, 28, 0.12);
  }

  .ghost-button {
    border: 1px solid rgba(85, 54, 28, 0.18);
    padding: 0.45rem 0.7rem;
    background: transparent;
    cursor: pointer;
    font: inherit;
  }
</style>
