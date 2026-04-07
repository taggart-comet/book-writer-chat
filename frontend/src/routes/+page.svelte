<script lang="ts">
  const readerExample = '/reader/your-signed-token';
</script>

<svelte:head>
  <title>Book Writer Chat</title>
</svelte:head>

<div class="shell">
  <div class="hero">
    <p class="eyebrow">Messenger to manuscript</p>
    <h1>Book drafts with a browser-grade reading surface.</h1>
    <p class="lede">
      Initialize a conversation, send a bot-directed writing request, and open the signed reader link to inspect the current draft.
    </p>
    <a class="cta" href={readerExample}>Open reader route shape</a>
  </div>
</div>

<style>
  :global(body) {
    margin: 0;
    font-family: Georgia, 'Times New Roman', serif;
    background:
      radial-gradient(circle at top left, rgba(210, 183, 120, 0.22), transparent 28rem),
      linear-gradient(180deg, #f8f1e5 0%, #efe4d2 100%);
    color: #2e241c;
  }

  .shell {
    min-height: 100vh;
    display: grid;
    place-items: center;
    padding: 2rem;
  }

  .hero {
    max-width: 46rem;
    padding: 3rem;
    border: 1px solid rgba(64, 40, 20, 0.12);
    background: rgba(255, 252, 247, 0.82);
    box-shadow: 0 1.5rem 4rem rgba(54, 33, 15, 0.12);
    backdrop-filter: blur(12px);
  }

  .eyebrow {
    margin: 0 0 0.75rem;
    text-transform: uppercase;
    letter-spacing: 0.16em;
    font-size: 0.75rem;
    color: #805b2e;
  }

  h1 {
    margin: 0;
    font-size: clamp(2.5rem, 6vw, 4.8rem);
    line-height: 0.96;
  }

  .lede {
    font-size: 1.1rem;
    line-height: 1.7;
    max-width: 35rem;
    margin: 1.5rem 0 2rem;
  }

  .cta {
    display: inline-block;
    padding: 0.9rem 1.2rem;
    background: #2e241c;
    color: #f8f1e5;
    text-decoration: none;
  }
</style>
