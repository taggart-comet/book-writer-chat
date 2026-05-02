<script lang="ts">
  import { page } from '$app/stores';
  import ConversationChatScreen from '$lib/screens/conversation-chat/ConversationChatScreen.svelte';
</script>

<ConversationChatScreen
  bookId={$page.params.book_id ?? ''}
  conversationId={$page.params.conversation_id ?? ''}
/>
