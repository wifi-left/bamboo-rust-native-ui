# FAQ

> Short answers to the questions that come up before adopting the framework.

## Why not egui or another immediate-mode library?

Immediate-mode UIs relayout and repaint the whole window every frame. A desktop
music player is idle most of the time: a static page, one track, a progress bar
that changes once per second. Bamboo Native UI keeps retained widgets, records
damage, paints only dirty rectangles into a persistent back buffer and blits
only those pieces. Idle means zero paints.

The price is that the application computes rectangles and invalidates after
state changes. See [Design principles](../overview/design-principles.md) for the
full trade-off table.

## Is it cross-platform?

No. The window host, GDI/Direct2D backends, native menus, tray, dialogs and
UIA provider are Windows-only (`#[cfg(windows)]`). The portable layer (`Ui`,
`Widget`, `Painter`, `Canvas`, `Event`, geometry, theme and the layout model)
compiles everywhere, which is what lets the widget tests run without a window.

## How is DPI handled?

The process is per-monitor DPI aware v2
(`DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2`). Every rectangle, font size and
radius in the public API is in DIPs; the platform scales by `dpi / 96`. A DPI
change calls `Host::dpi_changed` after the font cache was dropped, then the
normal resize path recomputes rectangles. Percent and `pill` values in a
`.buixml` document re-resolve.

## What fonts can I use?

The theme family defaults to `Microsoft YaHei UI`, and glyph runs use
`Segoe MDL2 Assets`. `ThemeSettings::font_family` selects another **installed
family name**; `WidgetProps::font`/`font_size`/`font_family` overrides one
widget. There is no font-file loading: use a family the system already has.
`ThemeSettings::font_size` scales the role styles, clamped to `0.85..=1.4` of
the 13.5 DIP reference.

## Does Chinese text work?

Yes. The default family covers Chinese, and wrapping breaks between characters
when a line has no spaces, so CJK paragraphs wrap correctly. Source files are
UTF-8. Some library defaults (accent preset labels, playback repeat labels) ship
in Chinese because they mirror the player; use your own labels where you need a
different language.

## How do images work?

Create a `UiImage`, register it with `Ui::set_image`, and let a widget show it
by `ImageId`. The store is page-scoped; registering the same id replaces the
pixels. There are no paths, no file loading and no image-name registry: the
application decodes or generates pixels and hands them over. In `.buixml`,
`image` attributes on child elements are the numeric `ImageId` (`image="7"`).
See [Images](../api/images.md).

## Can a `.buixml` document bind an event to a Rust function?

No. A document assigns geometry, properties, styles and initial content. Clicks,
keys, drags and focus stay in Rust: implement `Widget::input`, use the
`on_click`/`on_key_down` observers, or bind with `Ui::bind`. A document can
create a widget through a `WidgetRegistry` and give it a `command`, but the
command still resolves to an application message through your `CommandMap`.

## How does hot reload work?

With `layout-xml`, poll the file, call `LayoutDocument::parse`, compare
`generation()` and call `Ui::apply_layout`. On a parse or apply error the
previous document stays active. The library does no file IO and starts no
watcher. Compiled layouts (`layout-compiled`, `layout_xml!`) have no hot
reload: regenerate and rebuild. The `layout_xml!` macro also cannot register a
file dependency, so a `.buixml` edit alone does not trigger a rebuild.

## Native text editor or painted editor?

`TextEdit` (and `SearchBox`, `TextArea`, `PasswordBox`) use a native `EDIT`
child by default: the platform owns the session, caret, selection and context
menu, and the application receives `edit_changed`/`edit_committed`. Calling
`.painted()` keeps the same widget API but edits in place with framework
painting: vertically centered single-line text, configurable caret/selection
colors, inline IME composition with candidate placement, undo/clipboard and
live UIA text, with no native child. Choose native for maximum platform
fidelity; choose painted for consistent styling and inline composition. See
[TextEdit](../widgets/inputs/text-edit.md).

## Does it have automatic layout?

No measure pass: the application assigns rectangles. Use `Ui::set_rect`, the
`bamboo_native_ui::layout` helpers (`VStack`, `HStack`, `padded`, `split_h`,
`split_v`, `grid`) or a `.buixml` document. Widgets have a
`preferred_height` used only by the optional vertical stack helper, never for
page layout. This is deliberate: resizing is arithmetic, not a constraint
solve.

## Why does my `apply_layout` do nothing?

It is atomic. Any error returns `LayoutFailure` and mutates nothing; the
previous layout stays. Print `failure` (or iterate `failure.diagnostics`) and
check the reported line/code. A successful parse whose generation did not
change also skips content writes. See [Debugging](debugging.md).

## Can I animate?

Yes, explicitly. Install `WindowHandle::set_animation_timer(Some(duration))`,
advance state in `Host::timer`, invalidate only the animated rectangle and call
`ui.flush_damage(window)`. Stop the timer when the animation ends. There is no
frame loop and no repaint while idle. See [Performance](performance.md).

## What is the license?

Apache License 2.0. Commercial and closed-source use is allowed if you keep the
copyright notice and the `NOTICE` file and state significant changes when you
modify the source. You do not have to display credit in the application UI. See
`LICENSE` and `NOTICE` in the repository root.

## How do I use it as a git submodule?

The player tracks this repository at `native-ui/` and depends on it by path:

```toml
bamboo-native-ui = { path = "native-ui/crates/native-ui" }
```

Fresh clones must use `git clone --recursive` or
`git submodule update --init --recursive`. Do not add `native-ui/` to the
parent `.gitignore`: the parent must track the gitlink, otherwise a fresh clone
cannot build. Add feature flags on the dependency as needed (see
[Feature flags](feature-flags.md)).

## See also

- [Introduction](../overview/introduction.md)
- [Design principles](../overview/design-principles.md)
- [Architecture](../overview/architecture.md)
- [Theming](theming.md)
- [XML layout authoring](xml-layout-authoring.md)
- [Feature flags](feature-flags.md)
