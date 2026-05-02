<script lang="ts">
  import { uiCopy } from '$lib/web-shell/language';
</script>

<section class="state-screen">
  <div class="state-card">
    <p class="eyebrow">{$uiCopy.webAuthentication}</p>
    <h1>{$uiCopy.restoringSession}</h1>
    <p class="copy">{$uiCopy.restoringSessionCopy}</p>
  </div>
</section>

<style>
  .state-screen {
    min-height: 100vh;
    display: grid;
    place-items: center;
    padding: 1.25rem;
  }

  .state-card {
    width: min(100%, 34rem);
    padding: 2.5rem;
    border: 1px solid rgba(64, 40, 20, 0.12);
    background: rgba(255, 252, 247, 0.88);
    box-shadow: 0 1.2rem 3rem rgba(32, 20, 11, 0.12);
  }

  .eyebrow,
  .copy,
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
    font-size: clamp(2.1rem, 5vw, 3.4rem);
    line-height: 0.98;
  }

  .copy {
    margin-top: 1rem;
    line-height: 1.6;
  }
</style>
