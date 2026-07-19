---
name: MyFontCraft
version: 1
description: Visual design system for the handwritten font creation app.
tokens:
  colors:
    night: '#0b0805'
    ink: '#e9dcc4'
    ink_dim: '#b8b8b8'
    ink_faint: '#8f8f8f'
    lamp: '#e8b96a'
    lamp_bright: '#ffd98a'
    lamp_glow: 'rgba(232, 185, 106, 0.5)'
    ember: '#dfa050'
    error: '#e28877'
    paper: '#f2e9d4'
  typography:
    serif: 'Georgia, Times New Roman, Hiragino Mincho ProN, Yu Mincho, YuMincho, BIZ UDMincho, Noto Serif JP, Noto Serif CJK JP, serif'
    mono: 'ui-monospace, SF Mono, Menlo, Cascadia Code, Consolas, Liberation Mono, monospace'
    logo_size: '1.15rem'
    page_title_size: '2.3rem'
    section_title_size: '1.45rem'
    subsection_title_size: '1.1rem'
    body_size: '1rem'
    line_height: '1.9'
  layout:
    page_max: '72rem'
    measure: '38rem'
    measure_wide: '54rem'
    page_pad_x: 'clamp(1.25rem, 6vw, 5rem)'
    indent: 'clamp(3.25rem, 7vw, 5.5rem)'
    subindent: 'clamp(1.75rem, 4vw, 3rem)'
    section_gap: '3rem'
    item_gap: '0.7rem'
  shapes:
    panel: 'forbidden'
    card: 'forbidden except glyph paper slips and modal/inspector surfaces'
    button_box: 'forbidden'
    border: 'forbidden for page grouping'
  components:
    action: 'gold underlined text'
    page_structure: 'h1 > h2 > indented body'
    list: 'real ol or ul only'
---

# Overview

MyFontCraft is a dark, single-photo workbench. Every route sits on the same fixed night-desk photograph with a readable scrim. The interface is made from words, indentation, whitespace, and paper-slip glyph images. It must not look like a generic card-based web app.

The design system is intentionally narrow. If a visual decision is not in this file, do not invent it in implementation.

# Colors

Use only the color tokens in the YAML front matter.

Gold is reserved for interactive text, focus, progress, completed/adopted light, and explicit state highlights. Non-clickable explanatory text must not be gold or yellowish gray.

Neutral gray text uses `ink_dim` or `ink_faint`. These are true grays, not warm beige. If text is not clickable and not a warning/error/success state, it must use `ink` or neutral gray.

Warnings use `ember`. Errors use `error`. Success uses `lamp`, not green. No blue, green, purple, beige palette expansion, or extra accent colors.

# Typography

All readable UI text uses the serif stack. Monospace is allowed only for technical values such as counts, percentages, Unicode codes, and build hashes.

Each page has exactly one visible page title. The page title is `h1` and must match the link label that opened the page.

Use `h2` for page sections. Use `h3` only for real subsections inside a section. Do not use custom heading-like labels, caption labels, badges, decorative dots, or fake list row labels in informational pages.

For workflow screens, section headings must be action instructions, not bare nouns. Use labels such as `フォント名を入力してください` and `対象文字を選んでください`, not `フォント名` or `対象文字`.

The home headline `手書きの字が、フォントになります。` must stay on one line at common smartphone widths when there is enough physical width. Do not use `text-wrap: balance` or arbitrary forced line breaks for Japanese headings.

# Layout

The page column is left-set so the lamp and paper remain visible on desktop. Page structure is always:

1. `h1` page title.
2. `section.page-section`.
3. `h2` section heading.
4. `.section__body` for all content belonging to that heading.

Content under a heading is always indented. A heading and its body must never share the same left edge. Nested subsection content uses `.section__body--nested`.

Actions belong inside the indented body of the section they advance. They must not float as top-level text.

Lists must be real `ol` or `ul`. Ordered operations use `ol`. Unordered facts use `ul`. Never simulate a list with repeated paragraphs. Never make a list without numbers or bullets.

Use the available horizontal space. Do not force a narrow measure that creates ugly Japanese line breaks while the viewport still has room. Split copy at sentence boundaries only when a paragraph contains multiple substantial sentences. Do not split short related notes into many one-line paragraphs.

# Elevation & Depth

Depth comes from the photograph and light, not panels.

Text sits directly on the desk with a dark text shadow. Glyph crops may look like paper slips with `paper` background and a small shadow. Drag-over and inspector states may use a warm light pool. Do not introduce rectangular cards, bordered groups, section backgrounds, or raised panels for ordinary page content.

# Shapes

Page grouping uses indentation and whitespace only. Cards, rounded panels, borders, filled buttons, and outlined buttons are forbidden for normal page content.

Native form controls are allowed where they communicate actual input state, such as checkboxes and text inputs. Text inputs use underline only. File inputs are hidden behind text actions.

# Components

## Links and Actions

Actions use `.act`: gold, underlined text. Hover/focus may brighten and glow. Underline is required so clickability is not communicated by color alone.

Quiet actions use neutral gray text with underline. They are still clickable and must not be mistaken for plain explanatory text.

## Footer

Footer order is:

1. Internal explanation link: `このサイトについて`.
2. External author link: `作者サイト`.
3. Copyright.
4. Build information only when available.

Do not use decorative separators such as `|`. Do not use the English label `About`.

## PWA Install Prompt

Do not show an in-app PWA install prompt. It creates an unrelated workflow and competes with the font creation flow.

## Template Page

The operation order is chronological:

1. Choose optional font name and target characters.
2. Download the PDF.
3. Print and write.
4. Go to `フォントを作成する`.

`PDFをダウンロード` appears before the next-page link.

## Upload Page

The upload choices must explain intent, not just list file buttons. Separate these sections:

1. Select photographed template images.
2. Add characters to an existing font.
3. Load many pages by folder or ZIP.

Each section has a heading, an indented explanatory sentence, and its action inside the same indented body.

# Do's and Don'ts

Do:

- Use this file as the only visual source of truth for UI implementation.
- Keep text hierarchy to page title, section heading, subsection heading, body text, marked lists, inputs, and text actions.
- Indent every body from its heading.
- Use real marked lists whenever content is a list.
- Keep Japanese explanatory sentences punctuated with `。`.
- Avoid duplicated copy between a paragraph and a placeholder.

Don't:

- Use colors not listed in the tokens.
- Use yellowish gray for non-clickable text.
- Add emoji, decorative symbols, heading dots, fake bullets, or arbitrary separators.
- Use unmarked lists.
- Center text unless the component has a specific interaction reason.
- Put a link on a page whose destination page title does not match the link label.
- Add page-specific one-off text styles for informational pages.
