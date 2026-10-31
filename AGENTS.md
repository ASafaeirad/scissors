# Agent notes

## UI: use the Material 3 design system

Scissors is migrating its GPUI interface to Material Design 3. **New UI and any UI you touch
should use the M3 tokens and components**, not the legacy Scissors ones.

| Use | Instead of |
| --- | --- |
| `crate::material` tokens (`material::scheme(color_theme)`, `typography::*`, `shape::*`, `spacing::*`, `Elevation`, `motion::*`) | `theme::ThemePalette`, `theme::RADIUS_*`, `theme::TEXT_*` |
| `app::m3` components (`Button`, `IconButton`, `Chip`, `Switch`, `Slider`, `SegmentedButton`, `SelectCard`, `Radio`/`RadioItem`, `TextField`, `LinearProgress`, `Dialog`/`scrim`, `NavigationRail`, `toolbar`) | `app::components` (`scissors_text_button`, `scissors_icon_button`, `scissors_slider`, …) |
| `material::Symbol` (Material Symbols, outlined and filled) | `assets::ICON_*` Hugeicons |
| `M3Text::m3_type(typography::ROLE)` (Roboto) | `text_size(theme::ui_rem(theme::TEXT_*))` |

### Where things live

- `scissors-app/src/material/`: tokens only, no GPUI rendering. Baseline M3 scheme (source color
  `#6750A4`), light and dark.
- `scissors-app/src/app/m3/`: GPUI components built on those tokens. Read the module doc in
  `m3/mod.rs` first.
- `scissors-app/assets/icons/material/`: bundled symbol SVGs. To add one, follow the header
  comment in `material/icons.rs` (download both styles, add one line to `symbols!`).
- Source design: the claude.ai/design "Material 3 Design System" project. Its readme covers
  content and visual rules (sentence case, roles over raw colors, filled icon = selected).

### Rules

- Read colors from `ColorScheme` roles and pair every fill with its `on_*` role. Never hardcode
  colors; `cargo xtask ci` rejects color literals under `scissors-app/src/app/`.
- Pass lengths through `theme::ui_rem(...)` so they follow the user's UI scale.
- Depth comes from `surface_container*` roles first; use `elevation_shadows` sparingly.
- Components use builders that end in `.build(scheme, window, cx)` and return a
  `Stateful<Div>`; attach `.on_click(cx.listener(...))` to it. Multi-target components take
  `.on_select(cx.listener(|this, index: &usize, window, cx| ...))`.
- Icon-only controls need an accessible label (`IconButton::new(id, symbol, "Label")`).
- If a component you need doesn't exist yet (menus, tabs, snackbars…), add it
  to `app::m3` following the existing files: a pure, unit-tested `*_colors` function plus a
  builder. Don't fall back to the legacy components for new UI.
- Known gaps: GPUI has no letter spacing (tracking tokens are not applied), pressed-corner
  morphs aren't animated, `m3::Slider` draws state only (the caller handles input), and `m3::TextField` draws decoration
  only (pass `input::m3_text_input` as its editable child).

### Verifying UI changes

```bash
SCISSORS_GPUI_VISUAL_FIXTURE=m3-gallery SCISSORS_VISUAL_THEME=dark cargo xtask run
SCISSORS_GPUI_VISUAL_FIXTURE=m3-gallery SCISSORS_VISUAL_THEME=light cargo xtask run
cargo test -p scissors-app --lib -- material:: m3::
cargo xtask ci   # the full gate: fmt, tests, clippy -D warnings, theme literal audit
```

Add new M3 components to the gallery (`app/m3/gallery.rs`) so they can be reviewed in both
themes. See `CONTRIBUTING_VISUAL_FIXTURES.md` for the other fixtures.
