<script lang="ts">
  import { authErrorMessage, authState, submitLogin } from '$lib/web-shell/session';
  import { uiCopy } from '$lib/web-shell/language';

  let username = '';
  let password = '';

  async function handleSubmit(event: SubmitEvent) {
    event.preventDefault();
    await submitLogin(username, password);
    password = '';
  }
</script>

<section class="login-screen">
  <div class="login-card">
    <p class="eyebrow">{$uiCopy.webAuthentication}</p>
    <h1>{$uiCopy.signInToMessenger}</h1>
    <p class="copy">{$uiCopy.deploymentUsesOperator}</p>

    <form class="login-form" on:submit={handleSubmit}>
      <label>
        <span>{$uiCopy.username}</span>
        <input bind:value={username} autocomplete="username" name="username" required />
      </label>

      <label>
        <span>{$uiCopy.password}</span>
        <input
          bind:value={password}
          autocomplete="current-password"
          name="password"
          required
          type="password"
        />
      </label>

      {#if $authErrorMessage}
        <p class="error-banner">{$authErrorMessage}</p>
      {/if}

      <button class="cta" disabled={$authState === 'submitting'} type="submit">
        {$authState === 'submitting' ? $uiCopy.signingIn : $uiCopy.signIn}
      </button>
    </form>
  </div>
</section>

<style>
  .login-screen {
    min-height: 100vh;
    display: grid;
    place-items: center;
    padding: 1.25rem;
  }

  .login-card {
    width: min(100%, 34rem);
    padding: 2.5rem;
    border: 1px solid rgba(64, 40, 20, 0.12);
    background: rgba(255, 252, 247, 0.88);
    box-shadow: 0 1.2rem 3rem rgba(32, 20, 11, 0.12);
  }

  .cta,
  input {
    font: inherit;
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

  .login-form {
    display: grid;
    gap: 1rem;
    margin-top: 1.5rem;
  }

  label {
    display: grid;
    gap: 0.45rem;
  }

  input {
    border: 1px solid rgba(64, 40, 20, 0.18);
    background: rgba(255, 255, 255, 0.85);
    padding: 0.85rem 0.95rem;
  }

  .error-banner {
    margin: 0;
    padding: 1rem;
    color: #8b1f1f;
    border: 1px solid rgba(139, 31, 31, 0.16);
    background: rgba(255, 232, 232, 0.8);
  }

  .cta {
    border: 1px solid rgba(85, 54, 28, 0.18);
    background: #5d3a18;
    color: #fbf2e4;
    padding: 0.85rem 1rem;
    cursor: pointer;
  }
</style>
