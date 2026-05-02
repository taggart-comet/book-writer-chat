<script lang="ts">
  import { onMount } from 'svelte';

  import AuthStatusScreen from '$lib/screens/auth/AuthStatusScreen.svelte';
  import LoginScreen from '$lib/screens/auth/LoginScreen.svelte';
  import { initializeUiLanguage, uiLanguage } from '$lib/web-shell/language';
  import { authState, initializeSession } from '$lib/web-shell/session';

  onMount(() => {
    initializeUiLanguage();
    document.documentElement.lang = 'ru';
    void initializeSession();
  });

  $: if (typeof document !== 'undefined') {
    document.documentElement.lang = $uiLanguage;
  }
</script>

{#if $authState === 'booting'}
  <AuthStatusScreen />
{:else if $authState === 'authenticated'}
  <slot />
{:else}
  <LoginScreen />
{/if}
