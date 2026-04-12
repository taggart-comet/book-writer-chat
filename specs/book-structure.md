# Book Structure Specification

> Status: This is a high-level specification and is read-only by default. It should be changed only with explicit approval from the engineer.

## Purpose

This specification defines how a book project should be stored on disk so that:

- an agent can edit it reliably
- the backend can assemble it deterministically
- the frontend can render it beautifully
- illustrations and other media can be integrated in a controlled way

## Design Goals

The storage format should optimize for four things at the same time:

- easy text editing by an agent
- low ambiguity in file layout and naming
- stable assembly into a single logical book
- enough structure for high-quality rendering and styling

## Recommended High-Level Approach

The recommended approach is a hybrid format:

- Markdown files for the actual book content
- YAML files for book-level metadata, structure, and rendering-related configuration
- a predictable directory layout for chapters, sections, and assets

This format should be treated as the current recommended internal contract for MVP, while still allowing later evolution if implementation pressure reveals better boundaries.

This is the best current direction because:

- Markdown is easy for an agent to read, diff, and edit
- YAML is suitable for explicit metadata and ordering
- the combination keeps prose separate from configuration
- rendering can be deterministic without requiring the prose files to carry all layout logic inline

## Core Principle

The manuscript text should stay primarily in Markdown.

Structural ordering, style defaults, asset metadata, and rendering configuration should stay in YAML.

That separation reduces accidental formatting drift and makes it easier for the agent to modify prose without corrupting layout rules.

## Proposed Book Workspace Layout

A single book workspace should follow a predictable structure similar to this.

At the system level, each book workspace should live under a local books root such as `books/` or `books-data/`.

Each conversation maps to one book workspace folder under that root.

That local books root must be ignored by Git so manuscript data and assets do not leak into the main repository.

One possible layout is:

```text
books/
  conversation-slug/
    book.yaml
    style.yaml
    assets/
      images/
    content/
      frontmatter/
      chapters/
      backmatter/
```

Recommended meanings:

- `books/`: local root for all conversation/book workspaces
- `conversation-slug/`: one conversation-owned book directory
- `book.yaml`: canonical manifest for book identity, ordering, and assembly
- `style.yaml`: presentation defaults and layout policies for rendering
- `assets/images/`: illustrations, photographs, diagrams, schemas, and related media
- `content/frontmatter/`: title page, preface, introduction, and similar opening material
- `content/chapters/`: main manuscript content
- `content/backmatter/`: appendix, notes, glossary, references, and similar ending material

## Canonical Book Manifest

`book.yaml` should be the canonical manifest for how files are assembled into a book.

It should define at least:

- book identifier
- conversation identifier or stable conversation slug
- title and subtitle
- language
- ordered content entries
- asset directories
- rendering profile identifier
- optional repository binding metadata

Suggested example shape:

```yaml
book_id: habits-for-busy-parents
conversation_key: telegram-123456
title: Habits for Busy Parents
subtitle: Small Systems for Real Life
language: en
render_profile: standard-book
repository:
  provider: github
  url: https://github.com/example/habits-for-busy-parents
content:
  - id: title-page
    kind: frontmatter
    file: content/frontmatter/001-title-page.md
  - id: preface
    kind: frontmatter
    file: content/frontmatter/010-preface.md
  - id: chapter-1
    kind: chapter
    file: content/chapters/001-begin-small.md
  - id: appendix-a
    kind: backmatter
    file: content/backmatter/001-resources.md
assets:
  images_dir: assets/images
```

The currently supported language values are `en` for English and `ru` for Russian. Existing manifests with a missing or unrecognized language should be treated as English by default.

## File Naming Conventions

Naming should be deterministic and easy for an agent to extend.

Recommended rules:

- use numeric prefixes for ordered content files
- use lowercase kebab-case names
- keep filenames descriptive but short
- avoid spaces and special characters

Examples:

- `001-begin-small.md`
- `002-building-routines.md`
- `010-preface.md`
- `001-resources.md`

This convention makes ordering explicit even if a file is viewed outside the manifest, and it reduces ambiguity when the agent creates new content files.

## Markdown Content Rules

Markdown files should contain prose-first content with minimal structural overhead.

Recommended rules:

- use one top-level heading for the document title when needed
- use normal Markdown headings for internal structure
- keep inline HTML to a minimum
- prefer semantic Markdown over visual formatting hacks
- avoid embedding rendering-critical layout rules directly in prose unless explicitly supported

Markdown should remain readable as plain text.

## Frontmatter Policy

There are two viable approaches:

1. keep metadata only in YAML manifests and keep Markdown files mostly pure prose
2. allow a small YAML frontmatter block in individual Markdown files for local metadata

The recommended default is:

- keep global and ordering metadata in `book.yaml`
- allow optional lightweight frontmatter in Markdown files only for local content metadata that clearly belongs to that file

Examples of acceptable local metadata:

- local title override
- summary
- illustration references for that section
- optional visibility or render hints

## Rendering Style Configuration

`style.yaml` should define book-wide presentation defaults that the renderer can use consistently.

This file should not contain raw CSS. It should define semantic style choices that the frontend or render pipeline maps to actual presentation.

Examples of style configuration areas:

- typography profile
- chapter opening style
- paragraph width policy
- image default alignment
- caption style
- callout or quote style

Suggested example shape:

```yaml
theme: classic-readable
typography:
  body: book-serif
  headings: editorial-serif
  scale: medium
layout:
  page_width: readable
  chapter_opening: spacious
images:
  default_alignment: center
  default_width: medium
  captions: enabled
```

## Illustration And Media Model

The book format must support illustrations, photographs, diagrams, schemas, and similar media.

The storage model should separate:

- the media file itself
- the metadata describing how it should be rendered
- the place in the manuscript where it is referenced

## Recommended Image Storage Model

Image files should live under `assets/images/`.

V1 messenger image intake stores downloaded Telegram and MAX image files in this directory with deterministic safe filenames such as `telegram-<message-id>-<index>.png` or `max-<message-id>-<index>.png`. The backend supplies the saved workspace-relative paths to the authoring agent, and the agent places plain Markdown image references in manuscript files. The richer central YAML registry described below remains the preferred long-term model, but it is not required for v1 placement.

Each image may optionally have associated metadata recorded either:

- centrally in YAML
- locally in the content file that uses it

The safer long-term default is a central YAML registry because it allows consistent reuse and validation.

Suggested example:

```yaml
images:
  morning-routine-diagram:
    file: assets/images/morning-routine-diagram.png
    alt: Morning routine flow diagram
    caption: A simple morning routine that fits into a school-day schedule.
    default_alignment: right
    default_width: medium
```

## Image Placement In Markdown

Markdown content should be able to reference an image in a way that is easy for the agent to edit but still gives the renderer enough structure.

A plain Markdown image syntax alone is probably too weak for the full rendering needs.

A better approach is to support a lightweight explicit embed block, for example:

```md
![image:morning-routine-diagram]
```

or a short directive form such as:

```md
{{image: morning-routine-diagram}}
```

The final directive syntax still needs to be chosen, but the key requirement is that:

- the content file references an image by stable identifier
- rendering details can be resolved through YAML metadata
- the agent can insert or move the image without editing low-level presentation code

## Image Layout Options

The image metadata model should support at least:

- alignment: `left`, `right`, `center`, `full`
- width: `small`, `medium`, `large`, `full`
- caption
- alt text
- optional wrap behavior

This satisfies the requirement that images may appear left-aligned, right-aligned, centered, or in larger page-spanning modes.

## Assembly Model

The backend should assemble the book by:

1. reading `book.yaml`
2. resolving the ordered content file list
3. loading Markdown files in that order
4. resolving image and style metadata
5. producing a normalized renderable representation for the frontend

This gives the renderer a deterministic input even if the original manuscript is split across many files.

The workspace folder itself is the concrete on-disk representation of the book in the system.

In other words:

- one messenger conversation
- one book
- one workspace folder

## Why This Is Better Than Single-File Storage

A single huge Markdown file would be simpler initially, but it is weaker for:

- agent reliability on large books
- deterministic section ordering
- localized edits
- media metadata management
- future revision tracking

A multi-file Markdown plus YAML manifest approach is more scalable and still stays simple enough to edit automatically.

## Validation Requirements

The backend should eventually validate that:

- every content file referenced in `book.yaml` exists
- ordering identifiers are unique
- image references resolve to known assets
- style values conform to supported enums or profiles
- required metadata fields are present

## Open Questions

- Should per-file Markdown frontmatter be allowed from the start, or deferred?
- What exact inline directive syntax should be used for image embeds?
- Should style configuration be purely semantic, or should limited low-level render hints be allowed?
- Should non-image assets such as tables, diagrams, or downloadable appendices use the same registry model?
