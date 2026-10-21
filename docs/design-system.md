# Ivory Studio Design System

Selena's console is not a generic admin template. It borrows the language of a printed field notebook: warm paper, editorial serif headlines, thin rules, a coral annotation color, and deep teal for live/system states.

## Principles

1. **Quiet surface, sharp signal.** Most of the page is warm neutral; coral is reserved for actions, errors, and the active route; teal marks live/healthy state.
2. **Editorial hierarchy.** Display serif headlines, small uppercase labels, and compact mono metadata. Avoid oversized dashboard cards with no point of view.
3. **One idea per page.** Each page opens with a sentence that explains the control surface, not a generic "Welcome back".
4. **Motion with restraint.** 160–240ms transitions, page fade/slide, modal rise. Respect `prefers-reduced-motion`.
5. **Density where it matters.** Logs, keys, and routes are dense and scannable; overview and settings get breathing room.

## Color tokens

| Token | Ivory | Dusk |
|---|---|---|
| canvas | `#F6F1E7` | `#15120F` |
| paper | `#FFFBF3` | `#1D1915` |
| ink | `#211D19` | `#F4EADC` |
| muted | `#7B7166` | `#9D9080` |
| rule | `#E3D8C8` | `#362E26` |
| coral | `#D9634F` | `#EF8A74` |
| teal | `#2C6E68` | `#74BDB2` |
| gold | `#B8892D` | `#D9AD56` |

Theme modes: `system` (default, follows `prefers-color-scheme` live), `ivory`, `dusk`. The resolved theme is applied via `data-theme` on `<html>`.

## Type

- Display: Fraunces / Iowan Old Style / Palatino fallback.
- UI: Manrope / Avenir Next / Segoe UI fallback.
- Mono: JetBrains Mono / SFMono / Consolas fallback.
- Display headings use tight tracking and weights 400–500; labels are 8px uppercase with 0.2em tracking.

## Layout

- Sidebar: 252px, collapsible below 960px.
- Top bar: 72px; page title in UI sans.
- Desktop title bar: 40px, fixed above the app shell, drag region, window controls on the right.
- Content: 38px horizontal padding, max width 1540px, inner `.page-scroll` owns scrolling. The title bar never scrolls.
- Cards: 14–16px radius, 1px warm rule, soft warm shadow; no heavy glassmorphism.

## Title bar

The desktop title bar intentionally contains **no product logo**. The sidebar owns the Selena wordmark. The title bar is a thin contextual chrome: a small coral status dot plus `CONTROL ROOM`, a drag region, and window controls. This avoids the duplicated mark that occurs when a branded title bar sits directly above a branded sidebar.

The control buttons use Tauri window APIs and are only rendered when `isTauri()`. The title bar is not rendered in the browser.

## Components

- `ProtocolBadge`: Chat = teal, Responses = coral, Messages = gold.
- `StatusPill`: live/ready, warning, danger.
- `MetricCard`: eyebrow, large serif value, mono footnote.
- `DataTable`: compact, thin rules, hover row, status code chips.
- `SseTimeline`: numbered event rows with mono payloads.
- `CommandPalette`: `⌘K` / `Ctrl+K` navigation.
- `Select`: custom portal dropdown; no native `<select>` chrome. Warm paper menu, coral focus, keyboard Escape/ArrowDown support.
- Empty states use a small coral seal and a single executable next step.

## Visual QA checklist

- At 1240×780 logical, the three endpoint cards fit and the window controls remain visible.
- Scrolling only moves `.page-scroll`; title bar and sidebar remain fixed.
- Light and dark themes both maintain WCAG AA contrast for body text.
- No logo appears twice in the same viewport.
- Tables remain usable with keyboard focus and horizontal overflow.
