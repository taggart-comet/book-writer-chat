<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { ImagePlus, X } from 'lucide-svelte';

  import { uiCopy } from '$lib/web-shell/language';

  export let draft = '';
  export let disabled = false;
  export let pending = false;
  export let resetToken = 0;
  let selectedImage: File | null = null;
  let fileInput: HTMLInputElement | null = null;
  let lastAppliedResetToken = 0;

  const dispatch = createEventDispatcher<{ submit: { text: string; image: File | null } }>();

  function handleSubmit(event: SubmitEvent) {
    event.preventDefault();
    dispatch('submit', { text: draft, image: selectedImage });
  }

  function handleImageChange(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    selectedImage = input.files?.[0] ?? null;
  }

  function clearSelectedImage() {
    selectedImage = null;
    if (fileInput) {
      fileInput.value = '';
    }
  }

  $: if (resetToken > lastAppliedResetToken) {
    lastAppliedResetToken = resetToken;
    clearSelectedImage();
  }
</script>

<form class="message-composer" on:submit={handleSubmit}>
  <label class="composer-field">
    <span>{$uiCopy.sendMessage}</span>
    <input
      bind:this={fileInput}
      accept="image/png,image/jpeg,image/gif,image/webp"
      class="sr-only"
      disabled={disabled}
      on:change={handleImageChange}
      type="file"
    />
    <button
      aria-label="Add photo"
      class:selected={Boolean(selectedImage)}
      class="upload-trigger"
      disabled={disabled}
      on:click={() => fileInput?.click()}
      type="button"
    >
      <ImagePlus size={18} strokeWidth={2.1} />
    </button>
    <textarea
      bind:value={draft}
      disabled={disabled}
      placeholder={$uiCopy.messagePlaceholder}
      rows="4"
    ></textarea>
  </label>
  {#if selectedImage}
    <div class="selected-image-chip">
      <span>{selectedImage.name}</span>
      <button
        aria-label="Remove photo"
        disabled={disabled}
        on:click|preventDefault={clearSelectedImage}
        type="button"
      >
        <X size={14} strokeWidth={2.2} />
      </button>
    </div>
  {/if}
  <button class="cta" disabled={disabled} type="submit">
    {pending ? $uiCopy.sendingMessage : $uiCopy.sendMessage}
  </button>
</form>

<style>
  .message-composer {
    display: grid;
    gap: 0.9rem;
  }

  label {
    display: grid;
    gap: 0.45rem;
  }

  textarea,
  input,
  .cta {
    font: inherit;
  }

  .composer-field {
    position: relative;
  }

  textarea {
    width: 100%;
    box-sizing: border-box;
    resize: vertical;
    min-height: 7rem;
    padding: 1rem 3.75rem 1rem 1rem;
    border: 1px solid rgba(64, 40, 20, 0.18);
    background: rgba(255, 255, 255, 0.9);
  }

  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    margin: -1px;
    overflow: hidden;
    clip: rect(0, 0, 0, 0);
    white-space: nowrap;
    border: 0;
  }

  .upload-trigger {
    position: absolute;
    top: 2.35rem;
    right: 0.75rem;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 2.2rem;
    height: 2.2rem;
    border: 1px solid rgba(85, 54, 28, 0.14);
    background: rgba(255, 250, 243, 0.94);
    color: #6a431d;
    border-radius: 999px;
    cursor: pointer;
    box-shadow: 0 0.35rem 1.1rem rgba(55, 31, 12, 0.08);
    transition:
      background 120ms ease,
      color 120ms ease,
      transform 120ms ease;
  }

  .upload-trigger:hover:not(:disabled) {
    background: #6a431d;
    color: #fbf2e4;
    transform: translateY(-1px);
  }

  .upload-trigger.selected {
    background: #6a431d;
    color: #fbf2e4;
    border-color: rgba(85, 54, 28, 0.22);
  }

  .cta {
    border: 1px solid rgba(85, 54, 28, 0.18);
    background: #5d3a18;
    color: #fbf2e4;
    padding: 0.85rem 1rem;
    cursor: pointer;
  }

  .selected-image-chip {
    display: inline-flex;
    align-items: center;
    gap: 0.45rem;
    max-width: 100%;
    width: fit-content;
    padding: 0.45rem 0.55rem 0.45rem 0.7rem;
    border: 1px solid rgba(64, 40, 20, 0.1);
    border-radius: 999px;
    background: rgba(255, 248, 238, 0.9);
    color: #4a2d13;
    font-size: 0.92rem;
  }

  .selected-image-chip span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: min(24rem, 62vw);
  }

  .selected-image-chip button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 1.45rem;
    height: 1.45rem;
    border: 0;
    border-radius: 999px;
    background: rgba(106, 67, 29, 0.12);
    color: inherit;
    padding: 0;
    cursor: pointer;
  }

  .selected-image-chip button:hover:not(:disabled) {
    background: rgba(106, 67, 29, 0.2);
  }
</style>
