# Small Systems

Large plans fail when they depend on heroic focus every day.

Small systems succeed because they keep the next action obvious:

1. open the same workspace
2. continue from the current chapter
3. leave the manuscript in a clean state for the next pass

That same principle applies to this repository.

The mock book is here to make the first visual check trivial: seed the local state, open the signed reader link, and inspect the UI before wiring a real bot or a public deployment.

The mistake is to treat smallness as a lack of ambition. A careful small system is often the only way a larger system earns the right to exist. It lets one person trace a request from the chat message, through the job record, into the workspace, and back out through the reader without needing a diagram taped to the wall.

For this mock, the small system is deliberately concrete. There is a manifest that names the book. There are Markdown files for frontmatter, chapters, and backmatter. There is a style file that can be read separately from the prose. The whole thing can be inspected with ordinary tools before anyone asks the live application to render it.

That concreteness matters in the reading experience. If a chapter is too short, the shell never has to prove how it behaves after the first screen. It never has to show what happens when a paragraph wraps on a narrow phone, when a list interrupts a long passage, or when the table of contents contains enough entries to feel like a book rather than a sample card.

So the mock book should be small in architecture and larger in texture. Ten chapters are enough to show navigation and progression. A few longer chapters are enough to show scrolling, spacing, and pacing. The fixture stays easy to reason about while becoming harder to fool.

In practice, the best local mock is not the prettiest one. It is the one that fails loudly when the renderer is shallow. It contains repeated headings, varied paragraph lengths, and a few ordinary edge cases: a list, an image, a long run of prose, and a chapter ending that does not land exactly where the viewport expects it to land.

That is the spirit of this chapter. Build the smallest structure that can still carry realistic pressure. Then keep it nearby, because every design pass and every backend change will eventually need the same question answered again: does the book still feel like a book?
