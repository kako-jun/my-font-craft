# DESIGN.md — my-font-craft (Font Creation Tool)

## 1. Visual Theme

**The night desk.** Every page of my-font-craft happens on a single photographic plate: a dark wooden desk at night, a brass desk lamp, sheets of paper, a fountain pen (`public/night-desk-bg.webp`, fixed full-bleed on all routes). The UI is nothing but words placed on the dark wood and paper slips lit by the lamp. Dark-only — there is no light theme.

No rectangular panels, no cards, no borders, no outlined buttons. Grouping is expressed by indentation, whitespace, line spacing, and type size. Interactive elements are text with a gold underline and a warm glow — "pressable" is shown by light, not by a box. Completed / adopted / done states are **lit** (a small gold bead of light, a warm glow), never dimmed or grayed out.

### Shared Magazine Page Format

my-font-craft belongs to the shared magazine-like page format with break-and-shift, know-it-break-it, and gilga. Treat these projects as a series of web-based fashion/editorial magazine spreads rather than four separately designed websites. They should reuse nearly the same page system; only photography, accent color, and subject matter should change.

- Book / fashion magazine / women's cooking or interior magazine / PDF-like page composition.
- Design like an art director for stylish magazines such as Pen, not like a conventional website designer.
- Dark background with display type floating directly on the photograph (my-font-craft sets it in mincho/serif rather than gothic — the subject is handwriting and type itself).
- Text floating over photos or scanned paper.
- Print-like page rhythm instead of stacked web cards.
- Shared components should be reusable across the four projects: magazine section header, full-bleed photo spread, caption strip, numbered feature block.
- Avoid parallax gimmicks. my-font-craft is the one deliberate variation on "photos and text scroll together": its single night-desk photograph is the constant material of every page — the workbench the whole app takes place on, not a decorative backdrop. Content (text, paper slips) scrolls across it the way objects move across a real desk.
- A page break can simply be one strong photo, illustration, scan, or spread.
- Use grid compositions that place photos and text side by side with generous whitespace around the text.
- Treat photos as page material, not just decorative backgrounds — here the desk photo _is_ the page.

my-font-craft-specific variation: night wooden desk photo, desk lamp light, paper templates, handwritten glyphs treated as physical paper slips, ink/paper/wood/brass accent colors.

## 2. Color Palette

All colors are sampled from the background plate.

| Token         | Value                   | Usage                                                |
| ------------- | ----------------------- | ---------------------------------------------------- |
| `night`       | `#0b0805`               | Base page color (under/around the photo)             |
| `ink`         | `#e9dcc4`               | Primary text — the color of lamplit paper            |
| `ink-dim`     | `#b3a180`               | Secondary text                                       |
| `ink-faint`   | `#857459`               | Captions, hints, disabled                            |
| `lamp`        | `#e8b96a`               | Brass gold — links, actions, lit/adopted/done states |
| `lamp-bright` | `#ffd98a`               | Hover glow                                           |
| `lamp-glow`   | `rgba(232,185,106,0.5)` | Glow shadows around lit elements                     |
| `ember`       | `#dfa050`               | Warning, needs-review flicker, rewrite verdict       |
| `error`       | `#e28877`               | Error text                                           |
| `paper`       | `#f2e9d4`               | Paper-slip background (glyph crops)                  |

Success is expressed with `lamp` (lighting up), not green. There are no cold hues anywhere in the UI.

## 3. Typography

| Role             | Font                                                                                      | Size    | Weight |
| ---------------- | ----------------------------------------------------------------------------------------- | ------- | ------ |
| Logo             | serif stack, italic, letter-spaced                                                        | 18px    | 400    |
| Headings         | `Georgia, "Times New Roman", "Hiragino Mincho ProN", "Yu Mincho", "Noto Serif JP", serif` | 23–37px | 600    |
| Body             | same serif/mincho stack                                                                   | 15–16px | 400    |
| Technical values | `ui-monospace, "SF Mono", Menlo, Consolas, monospace` (`.num`, codes, build)              | 12–13px | 400    |

Everything readable is mincho/serif — the subject is letterforms, and the app reads like a typeset page. Monospace is allowed **only** for technical values (character counts, `U+XXXX` codes, percentages, build sha). No webfonts are loaded: local mincho stacks only, consistent with "nothing leaves the device".

Body line-height is 1.9 (mincho needs generous leading on dark ground). All text carries a soft dark text-shadow for readability over the photograph.

## 4. Component Stylings

### Actions (`.act`)

- Text, not boxes: gold (`lamp`) with a thin gold underline (`text-underline-offset: 0.4em`).
- Hover/focus: brightens to `lamp-bright` and gains a warm glow (`text-shadow: 0 0 14px lamp-glow`).
- Primary actions (`.act--primary`): larger type and a small gold bead of light before the label.
- Quiet actions (`.act--quiet`): `ink-dim`, for secondary paths (reset, restart).
- Disabled: `ink-faint`, underline removed. Never grayed boxes.

### Headings

`h2` carries a small glowing gold dot before the text — a lamp bead instead of a rule or border.

### Forms

- Text input: underline only (no box), gold underline + soft glow when focused.
- Checkboxes: native, `accent-color: lamp`.

### Messages

No colored banner boxes. Colored text with a leading mark: `✕` error (`error`), `!` warning (`ember`), `·` info (`ink-dim`), `◉` success (`lamp`, glowing).

### Drop Zone

No dashed border. A quiet area of centered text actions; on drag-over, a pool of warm light appears (radial gold gradient + inner glow) — the desk is lit where the paper will land.

### Paper slips (scan results)

Glyph crops are displayed as physical paper slips on the desk: white paper (`paper`), 1px radius, soft drop shadow, a deterministic ±2° scatter (nth-child rotation). No cell borders, no grid lines.

- **needs-review** (#110): the slip flickers amber (`ember` box-shadow animation) with a small `!` bead — anomalies catch the eye first.
- **adopted**: gold rim glow + a gold bead (`.scan-grid__cell-lit`) — sorted slips are lit, not dimmed.
- **rewrite** (excluded → retry): slip pushed into shadow (dimmed) with an ember `✕` — it is leaving the desk.
- **empty**: no slip; a faint ghost of the character on the dark wood.

### Inspector (検分ビュー)

A full-screen overlay: one slip large in a pool of lamplight (radial gold gradient on near-black), character + `U+` code + verdict beneath, three text verdicts (採用 / 書き直し / 次へ) with their key labels. `←` `→` navigate, swipe works on touch.

### Exit bar

During review, a fixed bottom strip (gradient scrim, not a panel; `pointer-events: none` except its children) always shows the two exits: "書き直し N 字 → リトライPDF" and "このまま生成 / フォントを生成する".

### Progress

A 2px gold line with a glow, filling across a faint track. Numbers in monospace.

## 5. Layout Principles

- Content column: max-width `46rem`, **set left** (`padding-left: clamp(1.25rem, 6vw, 5rem)`), leaving the lamp and paper visible on the right of the plate on desktop.
- The scrim (`.plate-scrim`) darkens the left/text side of the photo and preserves the lamp highlight.
- Single-column flow; grouping by whitespace and type scale only.
- Density over decoration: no filler leads, no duplicate explanations; copy is verbs and nouns.

## 6. Depth & Light

Depth comes from light, not elevation tokens:

| Level       | Treatment                           | Usage                          |
| ----------- | ----------------------------------- | ------------------------------ |
| On the desk | text-shadow only                    | All text                       |
| Paper slip  | small drop shadow + paper white     | Glyph cells, sample images     |
| Lit         | gold glow (`0 0 10–26px lamp-glow`) | Actions, adopted, done, review |
| Lamplight   | radial gold pool on near-black      | Inspector, drag-over           |

## 7. Do's and Don'ts

**Do:**

- Keep the single night-desk photo as the fixed background of every route.
- Set all readable text in the mincho/serif stack; monospace only for technical values.
- Express "done / adopted / read" by lighting up (gold bead, glow) — positive light.
- Use text underlined in gold for anything pressable.
- Keep needs-review anomalies glowing amber so they are seen first.
- Respect `prefers-reduced-motion` (all animation off).

**Don't:**

- Draw rectangular panels, cards, borders, or outlined/filled buttons.
- Add a light theme — the world is dark, always.
- Dim or gray out completed states.
- Use cold colors (blue/green) anywhere; success is gold.
- Load webfonts or any external resource beyond the app's own assets.
- Pad copy with filler ("〜しましょう", "簡単に", "ようこそ") — verbs and nouns only.

## 8. Responsive Behavior

| Breakpoint | Behavior                                                                                   |
| ---------- | ------------------------------------------------------------------------------------------ |
| > 860px    | Text column left, samples/lamp area breathing on the right                                 |
| 720–860px  | Home samples move above the flow; same column                                              |
| < 720px    | Full-width column; plate repositioned (`38% 50%`) so text sits on dark wood; heavier scrim |

- Slip grid: `minmax(52px, 1fr)`, narrowing to `46px` on mobile; touch targets ≥ 44px.
- Inspector verdicts are tappable (1-tap sort); swipe navigates.
- The exit bar wraps; no horizontal scrolling at 390px.

## 9. Agent Prompt Guide

When building new components for my-font-craft:

- **Background**: never add page backgrounds — the fixed plate + scrim (`.plate` / `.plate-scrim`) is already there. Content floats directly on it.
- **Text**: `ink` (#e9dcc4) mincho/serif with `--shadow-text`; secondary `ink-dim`; captions `ink-faint`.
- **Actions**: `.act` (gold underlined text). Primary = `.act--primary`. Never `<button>` with borders/background.
- **Done/adopted/lit states**: gold bead + glow (see `.scan-grid__cell-lit`). Never dim.
- **Warnings/review**: `ember` amber with flicker; errors `error` text (no boxes).
- **Numbers/technical values**: wrap in `.num` (monospace).
- **Images of glyphs/pages**: render as paper slips (paper white, small shadow, slight rotation).
- **Max content width**: 46rem, left-set. Leave the lamp side of the photo clear on desktop.
- **File inputs**: text-action drop zone with light-pool drag state; never native file inputs.
