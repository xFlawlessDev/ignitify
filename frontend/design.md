# Ignitify Visual System

Source of truth: this file and `src/assets/styles/global.css`.

## Product Intent

Ignitify is an infrastructure control plane. Interface must feel quiet, fast, and reliable during repeated operational work. Dense information, plain language, explicit status, and predictable navigation win over decorative UI.

## Themes

- Light is default. Dark activates through `html.dark`.
- Use semantic Tailwind classes: `bg-background`, `text-foreground`, `bg-card`, `border-border`, `bg-primary`, and `text-muted-foreground`.
- Do not hard-code semantic UI colors in Vue components. Static palette utilities exist only for charts, terminal output, and explicit state visuals.
- Theme choice persists through `useControlPlanePreferences`.

## Palette

| Role | Dark | Light | Use |
| --- | --- | --- | --- |
| Canvas | `#101010` | `#f6f5f4` | page background |
| Surface | `#1d1a18` | `#ffffff` | cards and popovers |
| Border | `#3d3a39` | `#d3cfca` | dividers and controls |
| Foreground | `#eeeeee` | `#1d1a18` | primary text |
| Muted | `#8a8380` | `#706965` | secondary text |
| Signal | `#ee6018` | `#d9500c` | live, queued, action required |
| Healthy | `#a0ca92` | `#47823e` | healthy status only |
| Destructive | `#d85a5a` | `#b42318` | destructive actions and errors |

Orange and green are state colors, never default button or panel fills. Every status also needs text or an icon.

## Typography

- Sans: Geist, then system UI.
- Mono: Geist Mono, then system monospace.
- Use weight `400` by default; `500` only for constrained emphasis.
- Do not use `600+` display text.
- Labels use uppercase mono at 11-12px with `letter-spacing: 0`.
- Do not scale font size with viewport width.

## Layout

- Desktop content maximum: `1200px`.
- Use 8px spacing scale; common gaps: `8`, `16`, `24`, `32`, `40`.
- Prefer tables, dividers, and compact grids for operational data.
- Cards frame repeated items, dialogs, and tools. Do not wrap page sections in decorative cards.
- Buttons and compact controls: 3px radius. Cards: 10px. Large framed tools: 20px.
- Keep fixed controls dimensionally stable across states.

## Components

- Use existing shadcn-vue primitives in `src/components/ui/` before adding new primitives.
- Use `@lucide/vue` icons in icon controls. Every unfamiliar icon needs a tooltip.
- Use tabs for bounded detail areas. Avoid long stacked settings pages.
- Main action uses neutral contrast. Destructive action needs explicit confirmation when it can lose data.
- Empty, loading, error, disabled, and success states are required for async control-plane views.

## Motion

- Keep transitions mechanical: 150-200ms for color, border, opacity, and transform.
- No gradients, shadows, glow, glass, parallax, or decorative animation.
- Respect `prefers-reduced-motion` for nonessential motion.

## Status

- Signal: live, queued, deploying, action required.
- Healthy: ready, running, successful.
- Neutral: inactive, unknown, no data.
- Destructive: failed, stopped by error, irreversible action.

Never rely on dot color alone; pair it with readable status text.
