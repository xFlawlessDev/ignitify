# Ignitify Documentation Content

This directory is the source content for the public Ignitify documentation.
The separate Ignitify marketing repository owns the VitePress application,
navigation, theme, deployment, and generated site output.

## Content contract

- English is the default locale at `docs/`.
- Indonesian translations mirror the same path under `docs/id/`.
- Keep page paths, headings, relative links, and relative asset references
  portable so the marketing site can mount this directory as its VitePress
  documentation root.
- Changes to pages here that add, remove, or rename routes require the matching
  VitePress sidebar and navigation update in the marketing repository.
- Keep this directory to reviewed Markdown content and safe static assets. Do
  not add VitePress configuration, dependencies, generated site output, or
  deployment files here.

Documentation describes implemented behavior. Track planned work in
[`../roadmap.md`](../roadmap.md), not in user-facing reference pages.
