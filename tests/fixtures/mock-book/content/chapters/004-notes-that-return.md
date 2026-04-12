# Notes That Return

Some notes are only useful once. They capture a passing idea, settle a detail, and disappear into the draft.

Other notes return. They become rules of thumb, recurring questions, or design pressure that shapes many chapters at once.

The reader interface should make returning easy. A person checking a long manuscript needs landmarks: visible headings, revision context, clear loading states, and enough spacing to keep the eye from losing its place.

This chapter is filler in the best possible sense. It creates enough vertical rhythm to make awkward spacing, repeated headings, narrow columns, and weak navigation harder to miss.

If the UI only works for a title page and two paragraphs, it has not yet met the manuscript.

![A quiet desk with manuscript pages beside a window](assets/images/quiet-desk-photo.png)

The image in this chapter is intentionally ordinary: a desk, a window, a few pages, and enough light to suggest a working day. It gives the renderer a mixed-content chapter where prose must sit above and below a visual element. That is useful even before the project has a richer image registry, because the plain Markdown image path still exercises the HTML that the frontend receives today.

The note returns in stages. First it exists as a message, small enough to be typed between other obligations. Then it becomes a job, which means the system has acknowledged that the request should produce a change. Then it becomes an edit, which means a file in the manuscript has moved from one state to another. Finally it becomes a rendered page, which is the moment the writer can decide whether the change belongs.

The reader experience should make that final moment calm. The writer should not have to wonder whether the image swallowed the following paragraph, whether the caption spacing broke the chapter, or whether a narrow screen turned the manuscript into a pile of awkward fragments. The mock cannot prove every future layout, but it can make those failures visible early.

That is also why the prose continues after the image. A photo placed at the end of a chapter is easier to handle: the renderer can simply stop. A photo placed in the middle asks a better question. Can the page recover its rhythm? Can the next paragraph resume cleanly? Can the reader keep moving without treating the image as an accidental ending?
