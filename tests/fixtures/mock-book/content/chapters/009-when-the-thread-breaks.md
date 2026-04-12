# When The Thread Breaks

Long drafts break the thread. The reader looks away, answers a message, returns later, and needs to recover their place.

That recovery should be designed deliberately.

Headings should be distinct. Chapter spacing should be generous without becoming wasteful. The loaded state should make it clear whether more content exists below.

When a manuscript is being generated through chat, this matters even more. The reader is not only reading; they are deciding what to ask for next.

A good interface helps them re-enter the conversation with the book.

The mock book is one of those places. It is stable enough that a visual regression can be noticed, but plain enough that nobody has to decode the content before deciding whether the page is wrong. If the tenth chapter disappears, the manifest is suspect. If the photo in chapter four does not load, the asset path is suspect. If a long chapter becomes unreadable, the reader layout is suspect.

That kind of failure is useful. It narrows the search. It turns a vague complaint into a smaller question: did the backend render the Markdown, did the frontend receive the HTML, did the browser resolve the asset path, or did the style layer make the result unusable?

The thread will still break in ways no fixture can predict. A live messenger can send malformed text. A local workspace can be moved. A deployment can serve frontend assets from a different base path than development. The mock book does not replace those tests. It simply gives the team a known object to hold while the unknown parts are changing.
