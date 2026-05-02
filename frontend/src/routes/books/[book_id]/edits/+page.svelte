<script lang="ts">
  import { page } from '$app/stores';
  import BookEditsScreen from '$lib/screens/book-edits/BookEditsScreen.svelte';
</script>

<BookEditsScreen bookId={$page.params.book_id ?? ''} />
