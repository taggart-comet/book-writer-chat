<script lang="ts">
  import { page } from '$app/stores';
  import BookHomeScreen from '$lib/screens/book-home/BookHomeScreen.svelte';
</script>

<BookHomeScreen bookId={$page.params.book_id ?? ''} />
