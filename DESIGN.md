# DESIGN.md — my-font-craft (Font Creation Tool)

## 1. Visual Theme

A Solid.js-based font creation tool with warm earth tone skeuomorphism and pixel art influences. The interface feels like a friendly craft workshop — cream paper backgrounds, wooden brown accents, and hand-drawn pixel art iconography. Approachable and cozy rather than clinical or techy.

### Shared Magazine Page Format

my-font-craft belongs to the shared magazine-like page format with break-and-shift, know-it-break-it, and gilga. Treat these projects as a series of web-based fashion/editorial magazine spreads rather than four separately designed websites. They should reuse nearly the same page system; only photography, accent color, and subject matter should change.

- Book / fashion magazine / women's cooking or interior magazine / PDF-like page composition.
- Design like an art director for stylish magazines such as Pen, not like a conventional website designer.
- Dark background with thick white gothic/sans-serif display type.
- Rectangular section headings: a square number block followed by a contrasting title rectangle.
- Text floating over photos or scanned paper.
- Print-like page rhythm instead of stacked web cards.
- Shared components should be reusable across the four projects: magazine section header, full-bleed photo spread, caption strip, numbered feature block.
- Avoid parallax and fixed photo backgrounds with only text scrolling. They do not feel book-like.
- Default behavior: photos and text scroll together as one page/spread.
- A page break can simply be one strong photo, illustration, scan, or spread.
- Use grid compositions that place photos and text side by side with generous whitespace around the text.
- Treat photos as page material, not just decorative backgrounds.

my-font-craft-specific variation: night wooden desk photos, desk lamp light, paper templates, handwritten glyphs, scanned sheets, ink/paper/wood accent colors.

## 2. Color Palette

| Token          | Value              | Usage                                            |
| -------------- | ------------------ | ------------------------------------------------ |
| `bg`           | `#fff8e7`          | Cream — page background, canvas area             |
| `text`         | `#333333`          | Primary body text                                |
| `accent`       | `#5d4e37`          | Brown — primary buttons, active states, headings |
| `accent-light` | `#8b7355`          | Lighter brown — secondary buttons, hover states  |
| `success`      | `#27ae60`          | Success messages, save confirmations             |
| `error`        | `#e74c3c`          | Error messages, validation failures              |
| `border`       | `#e0d8c8`          | Default borders, dividers, input outlines        |
| `shadow`       | `rgba(0,0,0,0.08)` | Card/panel drop shadows                          |
| `bg-hover`     | `#f0e8d6`          | Hovered card/row background                      |

## 3. Typography

| Role             | Font                                | Size    | Weight |
| ---------------- | ----------------------------------- | ------- | ------ |
| Logo / branding  | `"Courier New", Courier, monospace` | 28px    | 700    |
| Headings         | `Georgia, "Times New Roman", serif` | 20–24px | 700    |
| Body             | `Georgia, "Times New Roman", serif` | 14–16px | 400    |
| Code / glyph IDs | `"Courier New", Courier, monospace` | 13px    | 400    |
| Button labels    | `Georgia, serif`                    | 14px    | 600    |

The serif body font gives a bookish, typographic feel appropriate for a font creation tool. Monospace is reserved for the logo treatment and technical values (Unicode points, glyph metrics).

## 4. Component Stylings

### Pixel Art SVG Icons

- Constructed from 4px square blocks
- Colors use `accent` (#5d4e37) or `accent-light` (#8b7355)
- No anti-aliasing — `shape-rendering: crispEdges`

### Gradient Buttons

- Background: linear gradient from `accent-light` to `accent`
- Color: white
- Border: none
- Border-radius: `4px`
- Padding: 8px 16px
- Hover: gradient shifts lighter
- Active: gradient shifts darker

### Drop Zone (File Upload)

- Border: 2px dashed `border`
- Border-radius: 8px
- Background: transparent (hover: `bg-hover`)
- Text: `accent-light`, centered
- Transition: border-color 0.2s

### Cards / Panels

- Background: `#ffffff`
- Border: 1px solid `border`
- Border-radius: `8px`
- Box-shadow: `0 4px 8px rgba(0,0,0,0.08)`
- Padding: 16–24px

### Status Messages

- Success: `#27ae60` background tint, dark green text
- Error: `#e74c3c` background tint, dark red text
- Warning: `#f39c12` background tint, dark orange text
- Info: `#3498db` background tint, dark blue text
- Border-radius: 4px, padding: 12px 16px

### Glyph Editor Canvas

- White background within a bordered card
- Grid lines: `border` color at 50% opacity
- Active glyph cell highlighted with `accent` border

## 5. Layout Principles

- Max-width: `960px`, centered with auto margins
- Single-column primary flow with card sections
- Glyph grid within cards, responsive column count
- Consistent spacing: 24px between sections, 16px internal padding
- Toolbar / action bar pinned at top of editor view

## 6. Depth & Elevation

| Level    | Shadow                         | Usage                            |
| -------- | ------------------------------ | -------------------------------- |
| Flat     | None                           | Page background, inline elements |
| Raised   | `0 4px 8px rgba(0,0,0,0.08)`   | Cards, panels, toolbars          |
| Floating | `0 8px 16px rgba(0,0,0,0.12)`  | Dropdowns, popovers              |
| Modal    | `0 12px 32px rgba(0,0,0,0.15)` | Modal dialogs                    |

Shadows are soft and warm — never harsh or dark. The skeuomorphic warmth comes from subtle elevation, not dramatic contrasts.

## 7. Do's and Don'ts

**Do:**

- Use cream (#fff8e7) as the page background, white for cards
- Use serif fonts (Georgia) for all readable text
- Build icons from 4px pixel blocks with crispEdges rendering
- Keep border-radius small: 4px for buttons, 8px for cards
- Use gradient buttons for primary actions, flat buttons for secondary

**Don't:**

- Use sans-serif fonts for body text
- Apply dark themes — this is a warm, light-only design
- Use border-radius larger than 8px
- Add heavy animations or transitions beyond simple hover effects
- Use colors outside the earth tone palette for UI elements

## 8. Responsive Behavior

| Breakpoint | Behavior                                          |
| ---------- | ------------------------------------------------- |
| > 960px    | Max-width container, comfortable spacing          |
| 768–960px  | Slight padding reduction, same layout             |
| < 768px    | Single column, stacked sections, full-width cards |

- Glyph grid columns reduce on narrow screens
- Drop zone becomes full-width on mobile
- Toolbar wraps to multiple rows if needed
- Touch targets minimum 44px on mobile

## 9. Agent Prompt Guide

When building new components for my-font-craft:

- **Page background**: Always `#fff8e7` (cream)
- **Card background**: `#ffffff` with `0 4px 8px rgba(0,0,0,0.08)` shadow and 8px radius
- **Primary actions**: Gradient button from `#8b7355` to `#5d4e37`, white text, 4px radius
- **Text**: `#333` in Georgia/serif for body, Courier New for code/metrics
- **Borders**: `#e0d8c8` everywhere
- **Icons**: Pixel art style, 4px grid blocks, `accent` color
- **Status feedback**: Bootstrap-like colored banners (success green, error red, warning orange, info blue)
- **Max content width**: 960px, never wider
- **File inputs**: Dashed border drop zone, never native file inputs
