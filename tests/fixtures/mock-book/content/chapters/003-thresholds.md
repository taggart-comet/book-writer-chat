# Thresholds

A useful system has a low threshold. It does not ask the writer to remember the whole architecture before making one small improvement.

The threshold is crossed when the next action is visible from the current page. A sentence points to the next paragraph. A paragraph points to the next section. A chapter points to the shape of the book.

When the threshold is too high, the writer starts managing the system instead of writing. That is usually the moment to remove a step, name a file more clearly, or let the interface expose the next available move.

For this mock book, the threshold is deliberately simple: every chapter is a normal Markdown file, every chapter appears in the manifest, and the reader should make continuation obvious.

The user experience should not feel like opening a database record. It should feel like entering a quiet manuscript that happens to be backed by durable state.

Thresholds are easy to miss when the sample content is too thin. A single paragraph can look acceptable in almost any shell. It does not test the way the eye returns to the left edge. It does not test whether chapter controls remain available after a long scroll. It does not test whether a reader can understand where they are after stepping away and coming back.

This chapter stays shorter than the long ones on purpose. Not every chapter in a real book has the same density, and the mock should preserve that unevenness. The contrast between a compact chapter and a longer one gives the renderer a chance to show whether it handles both states without special casing either one.
