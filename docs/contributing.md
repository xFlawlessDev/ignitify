# Contribution guide

Contributions should preserve the architecture boundaries and update public contracts when behavior changes.

## Before changing code

- Read `AGENTS.md` in the repository being changed.
- Check the worktree first; do not discard someone else's changes.
- Identify the owning layer: domain, persistence, control plane, HTTP, dashboard, or documentation.
- For cross-layer changes, start with the contract/domain and move toward the UI, not the other way around.

## Backend changes

1. Add tests for new domain, access, persistence, or lifecycle rules.
2. Use a new numbered SQL migration for persisted data; do not edit migrations that have already been used.
3. Keep handlers as HTTP adapters and put external side effects in workers/runtime adapters.
4. Map errors to safe responses and never expose internal details.
5. Run:

```bash
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

## Dashboard changes

1. Update the typed client in `src/lib/api/` before changing a view.
2. Put reusable behavior in a composable or Pinia store.
3. Use semantic Tailwind tokens and the existing UI components.
4. Add a focused Vitest spec for meaningful state/API changes.
5. From `ignitify/frontend`, run:

```bash
vp run check
vp run test
vp run build
```

## Documentation changes

- Treat `docs/` as the source content for the public documentation published by
  the Ignitify marketing site. This repository does not own the VitePress
  application or its deployment.
- Keep the default English page and its Indonesian counterpart under `docs/id/`
  in sync when adding or changing content.
- Preserve file-based paths and relative links so the marketing repository can
  mount this directory directly in its VitePress content root.
- Update the VitePress navigation, theme, language switcher, and documentation
  build in the marketing repository, together with the content change that
  requires them.
- Do not add VitePress dependencies, configuration, generated site output, or
  marketing-site assets to this control-plane repository.

## Public templates

Template information, changes, and contributions are managed in [xFlawlessDev/ignitify-templates](https://github.com/xFlawlessDev/ignitify-templates). Use that repository as the single source of truth for templates.

## A good pull request

Explain the problem, behavior change, required migration/configuration, and verification performed. Keep unrelated refactors out of the change. If a check cannot run, document the specific limitation instead of claiming it passed.
