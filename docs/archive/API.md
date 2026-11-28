# Public API (M0)

Everything below is stable enough to build on; later milestones add to it.

## Geometry and color

```rust
use bamboo_native_ui::{Color, Rect, Size};

let area = Rect::new(24.0, 24.0, 240.0, 64.0);
assert!(area.contains(30.0, 30.0));
let accent = Color::rgb([127, 211, 168]);
```

All coordinates are DIPs (Dpi / 96). `Rect::to_px(scale)` snaps outward to
device pixels.

## Theme

```rust
let theme = ThemeSettings {
    accent: [127, 211, 168],
    font_family: Some("Microsoft YaHei UI".into()),
    font_size: 13.5,
    ..ThemeSettings::default()
};
let palette = Palette::new(&theme, /* dark */ true);
```

`Palette` mirrors the player's historical palette field-for-field, so the main
window can switch over without touching page painters.

The host collapses the queued invalidation rectangles to the bounding box it
will blit and paints exactly that box: `ctx.dirty` is one rectangle, and every
widget intersecting it is painted. `WidgetPaint::dirty` exposes the same list
to widgets that draw a full-surface effect (the modal scrim); such a widget
must restrict the effect to those pieces instead of blending the whole window.
A widget whose pixels do not depend on pointer state overrides
`Widget::paints_pointer_state` to return `false`; the container then skips
hover/press/wheel invalidation for it, so a backdrop stays static. A composite
control that changes only chrome (a scrollbar hover tint or thumb press)
returns that rectangle from `Widget::take_repaint_hint` after the input, and the
container invalidates only that strip; returning `None` keeps the whole paint
bounds. Hover motion itself repaints only a real hover change
(`Widget::repaint_on_move_at`, D58/D69).

## Window host

```rust
struct App { /* layout, hover, pressed, ... */ }

impl Host for App {
    fn paint(&mut self, ctx: &mut PaintCtx<'_>) {
        let pal = *ctx.painter.palette();
        let hits = |rect: Rect| ctx.dirty.iter().any(|d| d.intersects(rect));
        if hits(self.button) {
            ctx.canvas.fill_rect(self.button, pal.button_bg);
            ctx.painter.text(
                &mut *ctx.canvas, self.button, "Play",
                ctx.painter.small(), TextAlign::Center, pal.text,
            );
        }
    }

    fn event(&mut self, event: &Event, window: &mut WindowHandle<'_>) -> bool {
        if let Event::PointerMove { pos, .. } = *event {
            let hit = self.hit(pos);
            if hit != self.hover {
                let old = std::mem::replace(&mut self.hover, hit);
                if let Some(rect) = old { window.invalidate(rect); }
                if let Some(rect) = hit { window.invalidate(rect); }
            }
        }
        false
    }
}

fn main() -> windows::core::Result<()> {
    WindowBuilder::new(App::new())
        .title("Demo")
        .size(900.0, 600.0)
        .theme(ThemeSettings::default(), true)
        .run()?;
    Ok(())
}
```

### Renderer selection

```rust
pub enum Renderer { Gdi, D2dSoftware, D2dHardware } // Default = Gdi

WindowBuilder::new(app)
    .renderer(Renderer::D2dHardware) // or BAMBOO_UI_RENDERER=gdi|d2d-software|d2d-hardware
    .run()?;
// inside a callback:
let active = window.renderer(); // Gdi after a Direct2D fallback
```

The window keeps one persistent GDI back buffer and blits only the dirty
bounding box in every mode; the renderer only changes how the frame is drawn
into it. `Gdi` is the software fallback and the default. The Direct2D modes bind
an `ID2D1DCRenderTarget` to the same buffer (hardware by default, Direct2D's
software rasterizer when the driver cannot give one); `D2dSoftware` forces the
software target. Coordinates stay DIPs (the target runs at `96 × scale` DPI),
geometry and text formats are cached, masked images use a geometry layer and
images are premultiplied once. A Direct2D mode that cannot start falls back to
GDI for the window's lifetime (logged under `BAMBOO_UI_DEBUG`); `WindowHandle::
renderer` reports the active one. `Renderer::parse`/`name` round-trip the
selector strings (D101).

### `Host` hooks

| Hook | When |
|---|---|
| `paint(&mut self, ctx)` | `WM_PAINT`; draw only what `ctx.dirty` touches |
| `overlay_paint(&self) -> bool` | a full-window backdrop owns the frame: the page stays in the back buffer and only the overlay layer paints (`ctx.overlay`, D87) |
| `event(&mut self, event, window)` | pointer/keyboard/focus events |
| `timer(&mut self, window)` | animation timer tick |
| `resize(&mut self, size, dpi, window)` | client size changed; recompute layout |
| `dpi_changed(&mut self, dpi, window)` | DPI changed; font cache already dropped |
| `started(&mut self, window)` | window is visible; install timers |
| `close_requested(CloseReason, window) -> bool` | a close request; false vetoes (hide to tray). `User` (title-bar/Alt+F4/`SC_CLOSE`/bare `WM_CLOSE`), `System` (`WM_QUERYENDSESSION`), `Programmatic` (`request_close`) |
| `loaded(window)` | once after the first frame is on screen |
| `visibility_changed(bool, window)` | `WM_SHOWWINDOW` after startup |
| `activated(bool, window)` | the window became active/inactive (`WM_ACTIVATE`) |
| `visual_state_changed(WindowVisualState, window)` | minimized/maximized/normal changed |
| `moved(Point, window)` | the frame moved (screen DIPs) |
| `files_dropped(Vec<String>, window)` | shell file/folder drop (`WM_DROPFILES`) |
| `destroying(window)` | `WM_DESTROY`, handle still alive (`destroy()` stays the after-teardown hook) |
| `edit_changed/committed/canceled` | native text editor events (see below) |
| `menu_command(widget, command, window)` | a menu item was selected, after the popup closed (unless `menu_event` consumed it) |
| `menu_event(event, window) -> MenuDecision` | every native menu's lifecycle, all paths (see Context menus) |
| `menu_bar_command(command, window)` | a native window menu-bar item was selected |
| `dialog_finished(id, result, window)` | a native dialog closed; `result` carries the choice |
| `tray_event(event, window)` | a tray action (`Click`/`DoubleClick`/`RightClick`/`MiddleClick`/`KeySelect`) or a tray-menu command |
| `hotkey(hotkey, window)` | a registered global hotkey fired |
| `hotkey_captured(Some/None, window)` | shortcut capture finished (chord or cancel) |
| `destroy(&mut self)` | window is being destroyed |

`PaintCtx` carries `dirty` (merged DIP rectangles), `size`, `dpi`, `editing`,
`caret_on`, and the overlay fields: `overlay` is true when the back buffer
already holds the page and only the overlay layer should paint, and
`overlay_first` marks the frame that also blends the backdrop. A host that
returns true from `overlay_paint` must skip its page background/sidebar fills
when `ctx.overlay`. `Ui::overlay_active()` / `overlay_split()` expose the same
split (the first visible `Widget::is_backdrop`, which only `Scrim` reports).

### `WindowHandle`

| Method | Meaning |
|---|---|
| `invalidate(rect)` | redraw one widget rectangle (skipped while covered) |
| `invalidate_all()` | redraw everything once (layout/page/theme change) |
| `invalidate_force(rect)` | redraw even while covered (occluder just vanished) |
| `invalidate_all_force()` | full forced repaint |
| `flush()` | paint pending invalidation after the callback returns |
| `raise()` | raise, re-scan exposure, repaint if newly exposed |
| `set_cursor(Cursor)` | Arrow / Hand / IBeam / SizeWE / SizeNS |
| `exposed()`, `visible_pieces()` | current exposure state |
| `edit_active()` | whether a native text editor is open |
| `end_edit()` | commit and close the open editor (page switch) |
| `menu_active()` / `is_menu_open()` | whether a native popup menu is currently open |
| `active_menu_widget()` / `active_menu_source()` | who opened the last menu (tray reports `None` / `Tray`) |
| `open_menu(widget, menu, anchor, avoid)` | open a model as a native popup from application code (deferred) |
| `menu_click(widget, command)` | replay a selection without a popup (posted; hook can consume it) |
| `edit_open_menu(|menu| …)` | edit the live popup's model from a timer; a change closes and rebuilds it |
| `dismiss_menu()` / `dismiss_context_menu()` | close an open menu (no-op otherwise) |
| `set_menu_bar(Option<MenuBarModel>)` | install/replace/remove the native window menu bar |
| `menu_bar()` / `edit_menu_bar(|model| bool)` | read the installed model / edit it and rebuild on change |
| `menu_bar_installed()` | whether a native menu bar is installed |
| `show_dialog(dialog) -> DialogId` | native dialog; modal disables this window, parallel does not |
| `set_tray_icon(Option<TrayIcon>)` | install, update or remove the primary notification-area icon (id 0) |
| `request_close()` | post a close request through `close_requested(Programmatic)` |
| `hide()` / `minimize()` / `maximize()` / `restore()` | window state actions (callbacks follow) |
| `is_visible()` / `is_active()` / `visual_state()` / `frame_position()` | current window state |
| `set_tray_icon_at(id, Option<TrayIcon>)` | install/update/remove any icon by application id; the id comes back in every `TrayEvent` |
| `tray_icon_visible()` | whether the shell accepted the primary tray icon |
| `show_tray_balloon(title, text, icon)` | notification balloon on the primary icon (`MessageIcon::Info/Warning/Error`); the shell owns its look/lifetime |
| `show_tray_balloon_on(id, title, text, icon)` | the same balloon on another icon id |
| `register_hotkey(hotkey) -> bool` | system-wide shortcut; false on conflict with another process |
| `registered_hotkeys()` | every global registration, for persisting `label()` strings |
| `unregister_hotkey(hotkey) -> bool` | release a registration |
| `register_window_hotkey(hotkey, command) -> bool` | window-scoped shortcut: fires `menu_bar_command(command)` while this window has focus; no system registration, false for command 0, a bare printable key or a duplicate chord |
| `registered_window_hotkeys()` | only the application window shortcuts (chord, command), for persistence |
| `unregister_window_hotkey(hotkey)` / `clear_window_hotkeys()` | remove one / every application window shortcut |
| `window_hotkeys()` / `menu_hotkeys()` | active window shortcuts (application entries first, then the menu bar's labels) / only the labels derived from the bar |
| `begin_hotkey_capture()` / `cancel_hotkey_capture()` | capture the next chord for a shortcut editor |
| `hotkey_capture_active()` | whether a capture is in progress |
| `show()` | restore if minimized/hidden, focus and repaint (tray activation) |
| `a11y_focus(id)` | mark the snapshot stale and announce focus after the callback |
| `a11y_refresh()` | mark the snapshot stale after a layout/page change |
| `copy_text(&str) -> bool` | put the text on the clipboard as `CF_UNICODETEXT` (false when another process owns it) |
| `invalidations()` | cumulative rectangle invalidations (diagnostics) |
| `size()`, `dpi()`, `scale()` | current client metrics |
| `set_animation_timer(Option<Duration>)` | install/stop the heartbeat; `None` = idle |
| `set_theme(settings, dark)` | swap theme, drop fonts, repaint once |
| `set_palette(Palette)` / `palette()` | install a whole palette (a `.buitheme` document) and repaint once / read the active palette |
| `set_title(&str)` | window title |
| `quit(i32)` | leave the message loop |

## Events

```rust
pub enum Event {
    PointerMove { pos: Point, buttons: u32 },
    PointerDown { pos: Point, button: MouseButton },
    PointerUp { pos: Point, button: MouseButton },
    PointerDoubleClick { pos: Point, button: MouseButton },
    PointerLeave,
    Wheel { pos: Point, delta_x: f32, delta_y: f32 },
    Key(KeyEvent),
    KeyUp(KeyEvent),
    Char(char),
    Focus(bool),
    Resize(Size),
    DpiChanged(f32),
}
```

`KeyEvent { code: KeyCode, vk: u16, ctrl, shift, alt, repeat }`. No Win32
constants appear in this API. A release is forwarded to `Ui::key_up`, which
hands it to the focused widget (the `TextEdit` `on_key_up` hook); it carries no
navigation meaning.

## Canvas

Backend-abstracted drawing, in DIPs:

```rust
pub trait Canvas {
    fn scale(&self) -> f32;
    fn dpi(&self) -> f32;
    fn fill_rect(&mut self, rect: Rect, color: Color);
    fn fill_translucent(&mut self, rect: Rect, color: Color, alpha: f32); // default: opaque
    fn frame(&mut self, rect: Rect, border: Color, background: Color);
    fn fill_round_rect(&mut self, rect: Rect, radius: f32, color: Color); // default: composition
    fn fill_round_rect_corners(&mut self, rect: Rect, corners: Corners, color: Color); // default: max radius
    fn stroke_round_rect(&mut self, rect: Rect, radius: f32, thickness: f32, color: Color); // default: bands
    fn fill_circle(&mut self, rect: Rect, color: Color); // default: ellipse
    fn text(&mut self, rect: Rect, text: &str, style: &TextStyle, align: TextAlign, color: Color);
    fn text_overflow(&mut self, rect: Rect, text: &str, style: &TextStyle, align: TextAlign,
                     color: Color, overflow: TextOverflow); // default: text()
    fn measure_text(&mut self, text: &str, style: &TextStyle) -> Size;
    fn ellipse(&mut self, rect: Rect, color: Color);
    fn vertical_gradient(&mut self, rect: Rect, top: Color, bottom: Color);
    fn vertical_gradient_round(&mut self, rect: Rect, radius: f32, top: Color, bottom: Color); // default: square
    fn image(&mut self, rect: Rect, crop_square: bool, id: ImageId, data: ImageData<'_>);
    fn image_circle(&mut self, rect: Rect, crop_square: bool, id: ImageId, data: ImageData<'_>); // default: image()
    fn image_round(&mut self, rect: Rect, radius: f32, crop_square: bool, id: ImageId,
                   data: ImageData<'_>); // default: image()
    fn push_clip(&mut self, rect: Rect);
    fn pop_clip(&mut self);
}
```

`TextOverflow::{Ellipsis, Clip}` chooses the single-line edge behavior; the GDI
backend maps it to `DT_END_ELLIPSIS`. A backend that cannot distinguish the two
inherits the default implementation, which paints like `text`.

Images are keyed by `ImageId`; the first call converts RGBA to BGRA and caches
the result (16 MB budget). Repeated identical ids never re-convert.
`image_circle` (avatars) and `image_round` (rounded cover art) multiply the
image alpha by the same anti-aliased coverage cache the fills use and
`AlphaBlend` the result, so the corners reveal what is behind the picture; both
reuse one scratch DIB, and their masked pixels are never cached (the artwork
itself already is). `vertical_gradient_round` is the matching placeholder (D100).

`bamboo_native_ui::text::wrap_ranges(text, max_width, &mut measure)` returns
line **byte ranges** for a greedy word wrap (breaking between characters when a
word has none, as in CJK text; a hard `\n` always starts a line). It takes a
measurement callback, so it is backend-free and testable.
`text::fitting_prefix(text, max_width, &mut measure)` returns the byte length
of the longest prefix that fits, for callers that append their own suffix.

Rounded rectangles are a Canvas primitive:
`Painter::round_rect(canvas, rect, radius, color)` delegates to
`Canvas::fill_round_rect`; `radius = height / 2` paints a pill, and the radius
is clamped to half the shorter side. The default trait implementation composes
three rectangles and four ellipses (no anti-aliasing). The GDI backend draws a
cached, 4×4-supersampled coverage mask (premultiplied BGRA, keyed by pixel
size/radius/color, 8 MB + 128-entry LRU); shapes above 1024 px per side fall
back to the composition. Named radii live in `theme::radius` (`SMALL` = 4,
`MEDIUM` = 6, `LARGE` = 10, `pill(height)`); the checkbox mark also accepts a
per-widget `.corner_radius(dips)`.

Three siblings share that mask cache (D83): `Corners` (independent top-left,
top-right, bottom-right and bottom-left radii, via `Painter::round_rect_corners`
/ `Canvas::fill_round_rect_corners`), an anti-aliased outline
(`Painter::round_rect_stroke` / `Canvas::stroke_round_rect`, thickness in DIPs)
and the inscribed circle (`Painter::circle` / `Canvas::fill_circle`). The
coverage is generated once per pixel size and shape; a color mask is one
premultiplied BGRA bitmap, so a repeated dot costs one `AlphaBlend` and no
rasterization (45 dots of 6×6 measure ~0.16 ms per frame on the reference
machine; a spinner's 12 dots ~0.04 ms).

Images are page-owned. Register owned RGBA once and reference it by id:

```rust
use bamboo_native_ui::{ImageId, UiImage};

ui.set_image(UiImage::gradient(ImageId(1), 96, 96, top, bottom)); // ::new / ::solid
// inside a widget: ctx.image(rect, /* crop square */ true, ImageId(1))
```

`UiImage::new` rejects a buffer whose length is not `width * height * 4`;
`Ui::remove_image` frees it; `Ui::clear` drops the store with the widgets.
`WidgetPaint::image` returns false for an unknown id so a widget can fall back
to a placeholder. The backend still converts and caches the pixels, so a cover
shared by a grid, avatar and card costs one conversion.

## Widgets

```rust
use bamboo_native_ui::widgets::{Button, ListView, Slider, Toggle};
use bamboo_native_ui::{Row, Ui, WidgetId};

let mut ui: Ui<Msg> = Ui::new();
ui.add(Button::new("play", "播放", Msg::TogglePlay).primary());
ui.add(Toggle::new("mute", "静音", false, Msg::Mute));
ui.add(Slider::new("volume", 0.6, 0.0..=1.0, Msg::Volume));
ui.add(ListView::new("queue").row_height(40.0).on_activate(Msg::Play));
ui.set_rows(WidgetId::new("queue"), rows);

// Layout only when the window resizes or the page rebuilds:
ui.set_rect(WidgetId::new("play"), Rect::from_xywh(16.0, 16.0, 120.0, 36.0));
```

| Widget | Notes |
|---|---|
| `Button` | label, optional MDL2 glyph, `.glyph_gap(dips)`, `.variant(ButtonVariant)`, `.primary()`, `.danger()`, `.disabled()` |
| `Toggle` | emits `F(bool)`; `.label_side(LabelSide::Start/End)`, `.on_label("开")`, `.off_label("关")` |
| `Segmented` / `Tabs` | emits the selected index; hover lights only the cell under the pointer. Segmented: `.disable(index)` for a per-option disabled state (arrows skip it). Tabs: `.badge(index, text)` (and `set_badge(index, Option<String>) -> bool` for a live badge that fits a narrow strip), `.closable(on_close)` (the close target or `Delete` on the selected tab emits the close message instead of selecting) and `.label_align(TextAlign)` — centered by default, so a badge or close target never pulls the label off center. The labels are owned at runtime: `Segmented::set_options(Vec<String>)` / `Tabs::set_labels(Vec<String>)` replace them silently (selection clamps, hover clears, per-option disabled flags and badges keep their index), read back through `options()`/`labels()`. The close target is an **application-owned action**: the widget cannot remove its own labels, so it reports the index and the application rebuilds; it takes no default handle (D25/D109) |
| `Slider` | drag emits on release; keyboard adjust mode emits per step. A drag can also be followed live with the additive lifecycle hooks `.on_drag_start(|f32| M)` (the press), `.on_drag(|f32| M)` (every move while held, so the application can preview), `.on_drag_end(|f32| M)` (the release; queued after the usual `on_change`). Value grid: `.step(n)` (keyboard increment), `.snap(true)` (pointer drags and UIA `SetValue` snap onto that grid) or `.discrete(n)` (both at once; measured from `min`). Display is independent: `.ticks(n)` or `.ticks_at(&[values])` marks, `.tick_labels(&[…])` labels them (taller cell), `.value_text(|v| …)` paints a formatted value label |
| `TextEdit` | `.cue("…")`, `.password()`, `.multiline()`, `.align(TextAlign)`, `.font_size(dips)`, `.max_length(n)`; native editor, emits the committed `String`; keeps a closed-field caret/selection. Multiline: `.wrap(bool)` (default on) and `.scrollbars(ScrollBars::{None, Vertical, Horizontal, Both})`; the closed paint shows every hard/wrapped line with themed scrollbars and the wheel scrolls it, while the native control gets the matching `WS_*`/`ES_AUTO*` styles. Line endings are normalized (CRLF from the control becomes LF). `.painted()` switches to framework-painted editing with `EditColors`, `.caret_width`, `.padding`, `.line_spacing` and inline IME (see “Framework-painted editor”). Both the painted editor and the closed multiline view use the shared scrollbar (hover/active tint, pill thumb, thumb drag with the press grab offset, track click); `.scrollbar_style(ScrollBarStyle)` overrides thickness/insets/minimum thumb/colors. A press on a closed multiline field's bar drags the bar instead of opening the native editor. `.read_only(bool)` (or `WidgetProps::read_only`) locks the value without disabling the field: a painted field still takes focus, shows/blinks its caret, selects with the pointer or Shift+arrows and copies, and only refuses inputs that would change the text (typing, IME, paste, cut, delete, undo); UIA reports the element and every `SetValue` as read-only. Application observers on a painted field: `.on_change(|&str| Option<M>)` (every user edit: typing, paste from Ctrl+V or the menu, IME commit, cut/delete/undo; programmatic setters stay silent), `.on_char(|char| Option<M>)` (each keyboard character; an IME result reports through `on_change`), `.on_click` / `.on_right_click` / `.on_double_click` (`|Point| Option<M>`), `.on_key_down` / `.on_key_up` (`|KeyEvent| Option<M>`). They run before the built-in behavior, which still runs; `None` stays silent and at most one message is delivered per input (an earlier observer wins over `on_change`, and `on_char` consumes the change report of the same keystroke) |
| `NumberInput` | integer range, clamps on commit, emits `i64` only when the value changed. `.align(TextAlign)` (right by default), `.font_size(dips)`, `.prefix`/`.suffix` units, `.step_size(n)`, `.steppers()` for clickable up/down zones (arrow cursor, per-zone hover/press fill; they never open the editor and they keep painting while typing — the painted editor covers only the text area, so the arrows never hide the selection and the width never moves; a click on them during a session commits the typed text first and then steps). Editing is a painted session over the text area (D91): typed/pasted/IME text is filtered to digits, one dot and — for a signed range — a minus, Enter/Tab/blur commits (parsing the text back to the raw integer), Escape keeps the value, and the built-in edit menu/undo work as in every painted field. `.decimals(n)` shows and parses `n` fixed decimal places over the integer value (`250` → `2.50`, typing `2.75` commits `275`); `.format(|value| String)` / `.parse(|text| Option<i64>)` replace the default fixed-point text for both the paint and the editor, and `display()` reads the current text. `.read_only(bool)` locks the value without disabling the field: the session still selects and copies, typing and the steppers are refused, and UIA reports read-only; `is_read_only()` reads the flag |
| `ListView` | virtualized, wheel + scrollbar (hover/active thumb, thumb drag, track click), hover/press rows, `set_rows` / `set_selected`; `.empty_text("暂无曲目")`, `.alternating(true)` row tint, `.scrollbar_style(ScrollBarStyle)` |
| `CoverWall` | album cover grid (D68). `CoverWall::new(id).covers(vec![Cover::new("夜曲").subtitle("周杰伦").image(ImageId(10))])`; `.tile(dips)` (target tile width; the row fills the width), `.gap(dips)`, `.caption_height(dips)`, `.empty_text`, `.scrollbar_style`, `.corner_radius(dips)` (default artwork/placeholder corner radius; `Cover::corner_radius(dips)` overrides it per tile, `0` is square), `.on_select(|index| M)` (a click selects), `.on_open(|index| M)` (double click / Enter). Artwork comes from `Ui::set_image` and is painted through `Canvas::image_round`; a missing id paints a deterministic rounded gradient placeholder with the title's first letter. Only intersecting cells are painted and published; the wheel scrolls one row per notch, the shared bar drags, and arrows/Page/Home/End move the selected cover in adjust mode. A pointer click selects without scrolling (the tile is already visible); keyboard/typed selection reveals the row. The selection frame sits in a band reserved around every tile, so selecting a cover never resizes the artwork. Typed access: `set_covers`, `covers_ref`, `selected_index`, `set_selected`, `columns`, `visible_range`, `scroll_offset`, `scroll_to_index`, `cell_rect`. UIA `List` with visible `ListItem` covers (title — artist, the current one selected) and the vertical Scroll pattern (D67) |
| `WaveformSeekBar` | seek bar drawn from normalized amplitudes (D70). `WaveformSeekBar::new(id, progress, on_seek)`; `.samples(&[f32])` (extra samples repeat, an empty list paints a flat line), `.bar_width(dips)`, `.gap(dips)`, `.colors(played, unplayed)`, `.duration(seconds)` (formats the accessible value as `m:ss`). A press seeks and a drag scrubs (each move emits `0..=1`); the played part is accent, the rest muted, the playhead is a line plus a round handle, and a hover marker shows where a click lands. Adjust mode steps by 2 % (Left/Right), Home/End jump, and the widget updates optimistically until the application confirms with `set_progress`. UIA `Slider` with `RangeValue` 0..=1 and `SetValue` (a `%` suffix is accepted). Optional scrub-lifecycle hooks like the slider: `.on_drag_start`, `.on_drag`, `.on_drag_end` (each `|f32| M`, additive to `on_seek`). Typed access: `progress`, `set_progress`, `set_samples`/`samples_ref`, `duration`/`set_duration`, `is_dragging`, `drag_preview` |
| `PlaybackTransport` | transport strip (D72). `PlaybackTransport::new(id, on_action)`: five MDL2 glyph cells — shuffle, previous, play/pause, next, repeat — each emitting a `TransportAction`. `.playing(bool)`, `.shuffle(bool)`, `.repeat(RepeatMode::{Off, All, One})` set the initial display; the application owns all three, so after handling an action it pushes the real state back with `set_playing`/`set_shuffle`/`set_repeat` (each returns whether it changed). The play cell is a filled circle (accent while playing) that swaps its glyph; repeat draws `RepeatMode::One` as a loop marked `1`; shuffle/repeat glyphs are accent while on. A click, Enter or Space on the cursor cell emits the action; Left/Right/Home/End move the cursor, Up/Down leave the strip, and the focus ring follows the cursor cell. Typed access: `is_playing`, `shuffle_enabled`, `repeat_mode`, `cursor`, `set_cursor`. `RepeatMode::next()` cycles Off → All → One and `label()` supplies the status word. UIA `ToolBar` with five Button children; shuffle/repeat are Toggle children carrying On/Off state and play publishes its action name (播放/暂停) |
| `PlaylistView` | reorderable track list (D73) built on `Row` (`title` = track, `subtitle` = artist, `trailing` = duration, `current` = playing). `PlaylistView::new(id).rows(vec![Row::new("夜曲").subtitle("周杰伦").trailing("4:58").current(true)])`; `.row_height(dips)`, `.alternating(bool)`, `.empty_text`, `.scrollbar_style`, `.on_select(|index| M)` (a click selects), `.on_activate(|index| M)` (double click / Enter plays), `.on_reorder(|from, to| M)`. A press-and-drag past the container threshold starts a move: an accent insertion line follows the pointer, holding near the top/bottom scrolls half a row per move, and the drop reports the final index *after* removal (`rows.remove(from); rows.insert(to, track)`). The widget never reorders its copy; the application applies the move and pushes the new order back with `set_rows`. `Alt+Up/Down` moves the selected track from the keyboard (same message), arrows/Page/Home/End move the cursor in adjust mode, Enter plays, and a click selects without reveal-scrolling. Disabled rows cannot be selected, played or dragged. Typed access: `set_rows`, `rows_ref`, `selected_index`, `set_selected`, `visible_range`, `row_rect`, `scroll_offset`, `scroll_to_row`, `ensure_row_visible`, `drag_state`. UIA `List` with visible `ListItem` rows (title — artist, the cursor row selected, a look-ahead row offscreen/clipped) and the vertical Scroll pattern |
| `Table` | virtualized rows under a fixed header. `Table::new(id).columns(vec![TableColumn::new("编号", 64.0), TableColumn::flex("曲名"), TableColumn::new("时长", 72.0).align(TextAlign::Right)]).rows(rows)`; `.header_height`, `.row_height`, `.cell_padding`, `.striped(bool)`, `.grid_lines(bool)`, `.empty_text`, `.scrollbar_style`, `.sort(column, ascending)`, `.on_select(|row| M)`, `.on_activate(|row| M)`, `.on_header_click(|column| M)`, `.on_column_resize(|column, width| M)`, `.on_column_reorder(|from, to| M)`, `.editable(bool)`, `.on_cell_edit(|row, column, text| M)`. Rows are `TableRow::new(["01", "晨雾", "2:00"]).disabled(true)`; missing cells paint empty. Sorting is the application's: a header click emits the column, then the app re-sorts, calls `set_rows` and `set_sort` (the widget paints the triangle). A press within 4 DIPs of a fixed column's header divider starts a width drag (`SizeWE` cursor, 24 DIP floor); the release reports the new width through `on_column_resize` (a flex column has no divider). A header press that moves more than 6 DIPs becomes a column move: an accent insertion line follows the pointer, the release reorders the widget's columns (the sort indicator and cell cursor follow their column) and reports `on_column_reorder(from, to)`; a smaller move stays a header click. With `.editable(true)` a cell edits in place with the shared painted field (D90): Enter/F2 or a double click opens it (all text selected from the keyboard, the caret placed from a click), Enter/Tab commit, Escape reverts, drag/Shift+arrows select, undo/redo and the built-in edit menu are available, and a value containing a newline uses the multiline model (Enter inserts a line, Ctrl+Enter/Tab/blur commits; the value scrolls inside the fixed row height, D90). The widget applies the committed text and reports it through `on_cell_edit`; a read-only instance (`WidgetProps::read_only`) and disabled rows stay navigable but immutable. Fixed widths that exceed the viewport scroll horizontally with a second shared bar; header and cells share the offset and hit-testing follows it. The cell cursor is `(row, column)`; Left/Right move it across columns and the active cell paints an accent frame while the table has focus. Typed access: `selected_row`/`selected_cell`, `select_cell`, `visible_rows`, `scroll_offset`, `scroll_to_row`, `ensure_row_visible`, `row`/`row_count`, `cell_text`/`cell_rect`, `set_cell`, `set_columns`, `set_column_width`, `move_column`, `scroll_x_offset`/`set_scroll_x`, `sort_order`, `column_span`, `is_editing_cell`/`edit_cell`. Arrows/Page/Home/End move the cursor in adjust mode, Enter edits or activates, wheel and the shared scrollbars scroll both axes. UIA `Grid` (DataGrid role) with one `GridCell` per realized cell (`row * columns + column` index, cursor cell selected, the look-ahead row offscreen), one `ColumnHeader` per column (HeaderItem role, not selectable), and the vertical/horizontal Scroll patterns; IGridProvider publishes the full row/column extent and `GetItem`, and each cell exposes IGridItemProvider coordinates, SelectionItem and a Value pattern whose SetValue edits the cell through the same `on_cell_edit` path (locked cells report read-only, D63/D84) |
| `TreeView` | expandable hierarchy. `TreeView::new(id).items(vec![TreeItem::new("音乐库").glyph('\u{E8B7}').secondary("12 项").expanded(true).child(TreeItem::new("专辑").expanded(true).child(TreeItem::new("夜曲").glyph('\u{E8D6}').secondary("3:32")))])`; `.row_height`, `.indent(dips)`, `.chevrons(collapsed, expanded)`, `.empty_text`, `.scrollbar_style`, `.renameable(bool)`, `.draggable(bool)`, `.drag_rules(TreeDragRules)`, `.on_drag_start(|from: &[usize]| bool)`, `.on_drag_over(|from: &[usize], to: &[usize]| bool)`, `.on_drag_end(|from: &[usize], to: &[usize]| bool)`, `.on_select(|path: &[usize]| M)`, `.on_activate(|path: &[usize]| M)`, `.on_toggle(|path: &[usize], expanded| M)`, `.on_rename(|path: &[usize], label| M)`, `.on_move(|from: &[usize], to: &[usize]| M)`. `TreeItem` carries `label`, `glyph: Option<char>` (MDL2 icon column), `secondary: String` (muted right-aligned text), `children`, `expanded` and `disabled`. Clicking the chevron column toggles, clicking the row selects, double click/Enter activates; in adjust mode Right expands or walks to the first child, Left collapses or walks to the parent, Up/Down/Page/Home/End move (disabled rows are skipped), Space toggles, and F2 renames. A rename is a shared painted-field session (typing/IME/paste, Shift+arrows and drag selection, undo/redo, the built-in edit menu, Enter/Tab commit, Escape revert, an empty label ignored; a label containing a newline uses the multiline model per D90) that applies the label and reports it. A press-and-drag past 6 DIPs moves a node: the top/bottom quarter of a row inserts before/after it at that row's depth, the middle drops it inside and outlines the target, an accent line marks the sibling slot, holding outside the viewport auto-scrolls one row, and a drop into the node's own subtree is refused; `draggable(false)` turns moves off, `TreeDragRules { reorder, nest, detach, reparent }` allows or refuses each `TreeDropKind` (same-parent before/after, drop-into-a-row, root-level detach, different-parent reparent — runtime `set_draggable`/`set_drag_rules`), `on_drag_start` may veto one gesture, `on_drag_over` is asked with the prospective `(from, to)` once per target change and may invalidate a target while dragging (no hint, release does nothing), and `on_drag_end` is asked once with `(from, to)` on release and may cancel the drop (`false` leaves the node in place); the widget applies the move (expanding the new parent) and reports the node's old and new paths. The glyph column (18 DIP) and secondary column (96 DIP) are reserved per row; the label ellipsizes between them and neither changes hit-testing or the accessibility name. Typed access: `item(path)`, `is_expanded`, `set_expanded`, `set_items`, `set_selection_path`, `selected_path`, `selected_row`, `visible_row_count`/`visible_rows`, `row_label`/`row_rect`, `scroll_offset`/`scroll_to_row`/`ensure_row_visible`, `is_renaming`/`renaming_path`/`begin_rename`/`rename`, `move_item`. Only visible rows are painted and published; UIA `Tree` with a `TreeItem` per visible row (a look-ahead row is offscreen/clipped), SelectionItem on every row and ExpandCollapse on rows with children (a leaf reports `LeafNode`), plus the vertical Scroll pattern (D81/D85) |
| `PropertyGrid` | sectioned key/value inspector with in-grid editors. `PropertyGrid::new(id).sections(vec![PropertySection::new("音频").row(PropertyRow::new("时长", "3:45")).row(PropertyRow::new("音量", "80").number(0.0, 100.0)).row(PropertyRow::new("音质", "无损").choice(["无损", "高品质", "标准"])).row(PropertyRow::new("收藏", "是").check("是", "否")).row(PropertyRow::new("评分", "4").rating(5))])`; `.header_height`, `.row_height`, `.key_width(dips)`, `.grid_lines(bool)`, `.empty_text`, `.scrollbar_style`, `.read_only(bool)`, `.on_select(|key| M)`, `.on_activate(|key, value| M)`, `.on_change(|key, value| M)`. Kinds: text and number edit inline with the shared painted field (D90; typed characters, IME, paste, drag selection, Shift+arrows, Ctrl+A, undo/redo and the built-in edit menu on right click, Enter/Tab commit, Escape revert; a value containing a newline uses the multiline model, Enter inserts a line and Ctrl+Enter/Tab/blur commits); a number draft filters to `0-9 . -`, steps with Up/Down and clamps/formats on commit; a choice opens an overlay list (click or arrows+Enter, Escape/press-outside closes); a check toggles on click; a rating sets the clicked star and adjusts with Left/Right/RangeValue. `on_change` fires with the new value already applied. Read-only rows (`.read_only(true)`) and a locked grid still navigate and select but open no editor; typed access: `sections_ref`/`section`/`row_count`/`row_by_key`, `value`/`checked`/`rating`/`selected_row`/`selected_key`/`selected_path`, `set_value`/`set_key`/`set_checked`/`set_rating`, `set_sections`/`set_section_rows` (the cursor follows its key), `set_row_read_only`, `select_key`/`ensure_key_visible`/`scroll_to_key`, `scroll_offset`, and `row_rect`/`value_rect`. Arrows/Page/Home/End move the cursor over rows (headers skipped) in adjust mode; wheel and the shared scrollbar scroll. UIA `List` with one `ListItem` per visible row (`键: 值`, cursor selected, locked rows disabled), `Text`/`Heading` children for section titles, and the vertical Scroll pattern (D78); every editable kind also publishes a Value whose `SetValue` applies the text through the same normalisation and `on_change` report as the editor (numbers clamp/format, a choice must name an option, a check accepts its on/off text or true/false, a rating clamps to its stars; locked rows are read-only, D103) |
| `Pager` | emits `F(i32)` (`-1` / `+1`); `.label_with(|page, pages| …)` replaces the default `第 N / M 页`; `set_page_labels(vec![…])` installs per-page labels (the page count becomes the list length, a custom format is dropped, and pages past the list fall back to it); the label exists from creation |
| `ProgressBar` | display only; `set_progress(0..=1)`; `.show_value(true)` paints the percentage; `.indeterminate()` paints a moving segment advanced with `set_phase(0..=1)`; exposes RangeValue when opted in |
| `Card` / `StatCard` | display only; `.padding(dips)`, `.emphasis(true)` for an accent strip / tinted background |
| `Toast` | display only; `.duration(Duration)` / `.duration_secs(..)` for the application's auto-hide timer, `.action("撤销")` paints a right-aligned action (bind `on_click` with `mouse(true)`); `action_rect()`/`action_at(pos)` expose the action's painted zone (from the last paint) so the trigger can tell an action click from a body click |
| `Label` | display only; `.wrap(true)` (a hard `\n` starts a line, lines that do not fully fit are skipped), `.ellipsis(false)` for a hard clip instead of `…`, `.align(TextAlign)`, `.style(LabelStyle::Body/Small/Title)`, `.color(Color)`, `.link(true)` (accent color + underline, pointer input and the hand cursor, so a `Click`/`Activate` binding fires) |
| `CopyableText` | display only; `.copyable(|| Msg)` paints a copy glyph and emits on click, Enter/Space or Ctrl+C (the app writes the clipboard, e.g. `WindowHandle::copy_text`) |
| `MarqueeText` | display only; scrolls only when the text is wider than the rect. `.speed(dips/sec)`, `.gap(dips)`, `.pause(secs)`, `.style(LabelStyle)`. The app advances it with `Ui::edit_widget::<MarqueeText>(id, |m| m.advance(dt))`, which records a repaint only when the offset moved |
| `RichText` | display only; `.run(text)`, `.strong(text)`, `.muted(text)`, `.link(id, text)`, `.on_link(|id| Msg)`, `.wrap(bool)`, `.align`, `.style`, `.runs(Vec<RichRun>)` at runtime, read back through `runs_ref()`. Links are in the accent color, underlined, hand cursor and clickable; the accessible name is the plain concatenation |
| `Checkbox` | `.disabled(true)`, `.check(glyph)` (checked mark, default MDL2 U+E73E; private-use glyphs use the glyph face, ink color follows the accent luminance), `.square()` or `.corner_radius(dips)` for the box body (default 4 DIP rounded; `0` paints plain fills without the corner-ellipse composition, larger values clamp to half the box; `set_corner_radius`/`corner_radius_value` for runtime access); emits the new checked state `F(bool)` on click, Enter or Space; Toggle role |
| `RadioButton` | `.disabled(true)`; emits once when it becomes selected; `.selected()`, `.set_checked(bool)` for the group owner |
| `RadioGroup` | vertical options: `.disable(index)` / `.set_option_disabled`; click selects, arrows/Home/End select in adjust mode (disabled options are skipped); `RadioGroup` role with `RadioItem` children and `Ui::a11y_select_child` |
| `SearchBox` | leading search glyph and a clear target; `.cue`, `.max_length`, `.debounce(Duration)` (a hint the application reads for its own timer). Typing edits in place through a painted session over the text area (D91; arrows/Home/End/Delete/Backspace and Ctrl+A/C/X/Z/Y all edit the live text, as do paste, selection drag, the built-in edit menu and inline IME), Enter/Tab/blur commits through `on_commit`, Escape reverts; `is_editing()` reads the session state. The clear target sits outside the session: clicking it closes the session and emits `""` once. `.read_only(bool)` locks the text without disabling the box: the session still selects and copies, typing/paste/cut/delete/undo and the clear target are refused, and UIA reports read-only; `is_read_only()` reads the flag |
| `Switch`, `NumberStepper`, `PasswordBox` | aliases for `Toggle`, `NumberInput`, and `TextEdit` (call `.password()`); no second implementation |
| `ComboBox` | drop-down list: `.label` (accessible name), `.max_visible(rows)`, `.row_height`, `.drop_up(true)`, `.arrow(closed, open)` / `.arrow_closed(char)` (default solid triangles ▾/▴; a private-use character such as an MDL2 chevron is drawn with the glyph face), `.disabled`. Click/Enter/Alt+Down opens, wheel/PageUp/PageDown/Home/End scroll a long list (the open list gets the shared themed scrollbar (hover/drag, `.scrollbar_style`)), Esc or a press elsewhere closes, Enter selects the highlighted row. While the list is open it owns the navigation keys (`Widget::owns_navigation_keys`), so Up/Down move the highlight instead of moving focus, and Enter/Space select it; Tab still leaves and Esc closes. The list flips above the box automatically when it would leave the window; the open list is an overlay (`paint_bounds`/`overlay`/`dismiss`), so it paints above siblings and is hit-tested first. `ComboBox` UIA role, Value pattern; `a11y_set_value` selects by item text |
| `MultiSelectList` | checkable rows, `.row_height`, `.checked(&[indices])`, `.set_checked`, `.set_selection`, `.check(glyph)`, `.square()`/`.corner_radius(dips)` (row check boxes, same contract as `Checkbox`; runtime `set_corner_radius`/`corner_radius_value`), `.scrollbar_style(ScrollBarStyle)`; click/Space toggles, wheel scrolls, arrows move the cursor, and an overflowing list gets the shared themed scrollbar (hover/drag; a press on the bar never toggles a row). `MultiSelectList` UIA role with `CanSelectMultiple` and selected `ListItem` children |
| `Accordion` / `Collapsible` | vertical collapsible sections: `.body(index, text)` (word-wrapped while open), `.open(index, bool)`, `.single_open(bool)` (one open at a time), `.header_height(dips)`, `.body_height(dips)`, `.chevrons(collapsed, expanded)` (default MDL2 U+E70D/U+E70E; private-use marks use the glyph face), `.label(name)` (accessible container name), `.disabled`. Clicking a header, or Enter/Space in adjust mode, emits `(section, expanded)`; arrows/Home/End move the section cursor; a press inside a body does not toggle. Typed access: `is_expanded`, `open_index`, `set_expanded`, `set_body`, `set_title`, `toggle`. UIA `List` with the open section as the selected `ListItem` (SelectionItem toggles it) and an ExpandCollapse pattern on every section header (Expanded/Collapsed; the UIA Expand/Collapse request runs the same path as a click, D98); `Collapsible` is the one-section build |
| `Popover` | anchor-aware flyout card. `place_popover(anchor, size, placement, viewport, gap)` computes the card rect (`PopoverPlacement::{Below, Above, Left, Right, Auto}`; Auto flips above near the bottom and every mode clamps into the viewport with an 8 DIP margin). `Popover::new(id).anchor(anchor_rect).name("…").pad(dips).corner_radius(dips).arrow(bool).dismiss_on_outside(bool).on_dismiss(|| M)` paints the themed card and a notch pointing at the anchor. The application adds its content widgets **after** the card and places them in `content_rect()`; later widgets paint above it and hit-test first. With light dismissal on (the default) the widget's hit area covers the client viewport while visible: a press outside the card emits `on_dismiss` and is swallowed (the page gets neither the click nor a hover change), a press on the card is swallowed silently. Escape is the application's key, or use a close control inside the card. `paint` refreshes the viewport; call `set_viewport` from layout so light dismissal works before the first frame. Non-focusable; UIA `Group` named by the card |
| `Breadcrumb` | trail of clickable crumbs: the last item is the current page (not a target), earlier items emit their index. `.separator(char)` (default MDL2 U+E76C), `.label(name)`, `.disabled`, `.set_items(Vec<String>)`, typed `items`/`current_index`/`navigate`. Items are measured during paint and cached by width/font/revision; click, arrows/Home/End in adjust mode and SelectionItem navigate. UIA `List` with the current crumb as the selected `ListItem` |
| `NavRail` | vertical navigation rows with an optional leading icon: `.icon(index, glyph)` (private-use codepoints use the glyph face, text characters the UI face), `.row_height(dips)`, `.label(name)`, `.disabled`; click or arrows in adjust mode select (arrows move the selection), the selected row gets the accent tint + left bar. The public `NavItem { label, icon }` model replaces the rows at runtime with `set_items(Vec<NavItem>)` (selection clamps, hover clears), read back through `items()`. Typed `selected`/`set_selected`/`select`. UIA `List` with the selected row as the selected `ListItem` |
| `Splitter` | draggable divider: `Splitter::vertical(id, on_drag)` (drags horizontally) or `Splitter::horizontal(...)`, `.label(name)`, `.disabled(true)`, `is_active`/`is_vertical`, `SizeWE`/`SizeNS` cursor. `on_drag` receives the new divider centre in client DIPs with the press grab offset preserved, so the app clamps to pane minimums and sets the pane/divider rectangles plus invalidations (D1). Not a Tab stop; UIA `Separator` |
| `GroupBox` | titled panel frame: `GroupBox::new(id, title)`, `title()`, `header_height()` (28 DIPs). Paints a card fill, a border, the small title and a rule; children are ordinary widgets the app places inside. Display-only by default (`Group` when exposed) |
| `MenuButton` | label (+ `.glyph(char)`), `.disabled`, `.set_menu`, `.set_label`; click / Enter / `Alt+key` asks the host for the menu **below the button** (native `TrackPopupMenu`, same popup and command replay as a context menu). The label may carry an access key (`"文件(&F)"`); the key character is underlined while the Alt mode is on (see below). The selected command arrives in `Host::menu_command(widget, command, …)`. `Menu` UIA role, items published as `MenuItem` children (checked + disabled state, Invoke runs the command) |
| `SplitButton` | body runs the primary action (`on_primary`), the arrow opens the same native menu below the arrow; Enter runs the primary action. `Menu` role with a description and the same published items |
| `MenuBar` | `.item("&文件", menu)` per top-level menu; clicking a cell, Enter/Space or `Alt+key` opens that cell's native menu below it. Left/Right move the top-level cursor **only while another item exists in that direction**; at the ends and for Up/Down the key moves focus (never trapped). Typed `cursor`/`set_cursor`/`set_items`/`len`. `MenuBar` UIA role: top-level items are `MenuItem` children (the cursor item is selected) and the cursor menu's commands follow them (invoke one to run it) |
| `Toolbar` / `ToolbarItem` | `Toolbar::new(id, vec![ToolbarItem::new(cmd, "播放").glyph('▶').checked(true).disabled(..)], on_command)`. Fixed-width cells (`.item_width(dips)`, default 104) so the overflow split is pure geometry; items that do not fit move behind a chevron whose native popup lists them (`.checked`/`.disabled` carry over). `ToolbarItem::separator()` is a 9 DIP group rule that takes no focus, hover or menu slot. `ToolbarItem::new(cmd, "").glyph('★').icon_only().name("收藏")` makes a square cell whose explicit name feeds accessibility, tooltips and the overflow menu. `.vertical()` stacks the cells (overflow moves to the bottom, `.item_height(dips)` sets the cell height, Up/Down drive the cursor); horizontal bars keep Left/Right. Click/Enter run the item, Home/End jump, and the off-axis/edges move focus. Item labels may carry `&` access keys; underlines follow the application's Alt mode (see `Ui::set_mnemonic_underlines`). `.visible_count`, `.set_checked(index, bool)`, `.set_enabled_item`, `.set_items`, `.items()`, `.is_vertical()`, `.mnemonic_underlines()`. `ToolBar` UIA role: visible buttons + `更多` + the hidden commands as `MenuItem` children |
| `StatusBar` / `StatusSegment` | read-only bottom strip: `.text("就绪")` or `.segment(StatusSegment::new(..).icon(glyph).width(dips).align(TextAlign))`, `.label(name)`. Fixed-width segments take their width, the rest share the remaining space; segment 0 is the message slot updated by `Ui::set_text` (typed `set_segment(i, text)`). `.size_grip(true)` draws the classic diagonal grip at the right and reserves its width; `.progress(0..=1)` adds a right-aligned progress segment (also reserved) with typed `progress_value()`/`set_progress(value)` (clamped, returns whether it changed). Not focusable/interactive; `StatusBar` UIA role with one `Text` child per segment plus a `ProgressBar` child for the progress segment (expose the instance) |
| `Ribbon` / `RibbonTab` / `RibbonGroup` / `RibbonItem` | tabbed command surface: `Ribbon::new(id, vec![RibbonTab::new("开始(&K)").group(RibbonGroup::new("播放").item(RibbonItem::large(cmd, "播放").glyph('▶')).item(RibbonItem::small(cmd, "暂停")))], on_command)`; `.label(name)`, `.on_tab(|index| M)`, `.tab_height(dips)`, `.small_height(dips)`. Large items are full-height tiles (glyph above the label); consecutive small items fill three-row columns; groups paint a divider and a bottom label. A click selects a tab or runs an enabled item. The ribbon is one Tab stop in adjust mode: Left/Right switch tabs on the band (or step between items once Down enters the body), Up/Down move geometrically (Up at the top row returns to the band), Home/End jump, Enter runs the cursor item. Tab titles may carry `&` access keys (`Alt+K` selects the tab); labels follow the Alt underline mode. Typed access: `tabs`, `tab_count`, `selected_tab`, `select_tab`, `tab_rect`, `item_rect`, `set_tabs`, `set_checked(tab, group, item, bool)`, `set_enabled_item`. UIA `Tab` with `TabItem` headers (one selected) plus a `Button` child per command on the selected tab |
| `Sparkline` | mini bar strip scaled from a zero baseline; `Sparkline::new(id, &[f32])`, `.color(Color)`, `.gap(dips)`, `data()`/`set_data(Vec<f32>)`, typed `range()`. Display-only (`Text` when exposed) |
| `Gauge` | open 270° dot-arc meter (same dot primitive as `ProgressRing`); `.value(0..=1)`, `.thickness`, `.show_value(bool)`, `.label(text)`. Display-only (`ProgressBar` range when exposed) |
| `RatingStars` | star rating control: `RatingStars::new(id, stars, value, on_rate)`, `.clearable(true)` (clicking the current value clears to 0), `.read_only`, `.disabled`; hover previews the rating, arrows adjust in adjust mode, `rate`/`set_rating` for the app. `Slider` UIA role with `RangeValue` (0..=stars) and `SetValue` |

Scrollable widgets share one bar implementation, `widgets::scrollbar`:

```rust
use bamboo_native_ui::widgets::{ScrollBarColors, ScrollBarStyle};

let style = ScrollBarStyle::new()
    .thickness(8.0)
    .min_thumb(20.0)
    .page_on_track_click(true)
    .colors(ScrollBarColors::from_palette(&palette));
ui.add(ListView::new("list").scrollbar_style(style));
```

`ScrollBarStyle` carries thickness, edge inset, thumb inset, minimum thumb,
page-vs-jump track clicks and optional colors; `None` colors resolve from the
widget palette each paint (the active color is derived from the hover color and
the accent). The bar owns a `ScrollBarState` (hover/active + the press grab
offset), so a drag keeps the thumb under the pointer and the active tint
survives leaving the track. `ListView`, `MultiSelectList`, `CoverWall`, both
`TextEdit` modes (painted and closed multiline), the `ComboBox` drop-down and
the `ScrollView` use it.
| `TextArea` | alias for `TextEdit` with `.multiline()`: the native editor fills the field; Enter inserts a newline and Tab/Esc/focus loss commit or cancel. `.wrap(false).scrollbars(ScrollBars::Both)` switches to hard-line layout with a horizontal scrollbar; the closed paint follows the same settings |
| `DatePicker` | in-window date field + month-grid popup (D115). `DatePicker::new(id, Date, Msg::Date)`; `Date::{new(y, m, d), today()}`, `display()`, `add_days`/`monday_index`. `.value_range(Some(min), Some(max))` / `.min_date`/`.max_date`/`.clear_range` (typed `set_range`/`range`); out-of-range days paint disabled and cannot be chosen, and `set_value` clamps into the range. Click/Enter opens, click a day or Enter chooses, arrows move the cursor, PageUp/PageDown change the month, Home/End jump to the week's start/end, Escape/outside closes. Typed access `value`/`set_value`, `date`/`set_date`, `is_open`/`open`/`close`, `selectable`, `view_month`. UIA `Button` with the date value and `ExpandCollapse` |
| `TimePicker` | in-window `HH:MM` field + hour/minute columns (D115/D117). `TimePicker::new(id, Time, Msg::Time)`; `Time::{new(h, m), now()}`, `display()`; the same range API as the date picker. The columns render top-to-bottom increasing, so Down steps to the larger value and Up to the smaller, PageDown/PageUp jump five, Left/Right switch units, Enter commits, Escape reverts to the value the popup opened with. The wheel steps the hovered unit and the open popup owns it (`scrolls_with_wheel`), so the page under the popup does not scroll. Clicking an hour keeps the popup open and moves to the minutes; clicking a minute commits and closes. Typed access `value`/`set_value`, `time`/`set_time`, `is_open`/`open`/`close`, `range`/`set_range`, `selectable`. UIA `Button` + `ExpandCollapse` |
| `ColorPicker` | in-window swatch + hex field and an 8×6 palette popup (D115/D117). `ColorPicker::new(id, Color, Msg::Color)` reports opaque colors; `ColorPicker::new_rgba(id, Rgba, Msg::Rgba)` (and `.on_change_rgba(..)` on an existing picker) reports `color::Rgba` — a `Color` plus an alpha byte, `display()` = `#RRGGBB`/`#RRGGBBAA`. `.allow_alpha(true)` adds an eight-step transparency strip (transparent → opaque) under the palette; the value is applied without closing the popup. The property only controls whether the *user* can pick an alpha — `set_rgba`/`set_alpha` keep what the application stores. `.on_more(|| Msg)` adds a "更多颜色…" row (the last keyboard stop) that closes the popup and reports the handoff; the application opens its full dialog there and writes the result back with `set_value`/`set_rgba`. The swatch paints over a checkerboard so translucency reads correctly. `palette()` exposes the built-in colors; arrows move the swatch cursor, Enter selects, Escape closes unchanged. Typed access `value`/`set_value`, `rgba`/`set_rgba`, `color`/`set_color`, `alpha`/`set_alpha`, `allows_alpha`/`set_allow_alpha`, `is_open`/`open`/`close`. UIA `Button` + `ExpandCollapse` |
| `ColorWindow` | the library's own color dialog (D118): `ColorWindow::new(Rgba)` + `.initial(Rgba)`/`.allow_alpha(bool)`/`.parallel()`. The convenience opener is `WindowHandle::show_color_window(initial: Rgba, allow_alpha: bool) -> DialogId`; use `show_dialog(ColorWindow::..)` directly for `.parallel()`. Map `DialogResult::ColorWindow(Option<Rgba>)` in `Host::dialog_finished`. The system chooser is unchanged (`show_dialog(ColorDialog::new())`). It is an owned, theme-following window (not the system chooser) with an HSV field (saturation/value square + hue bar, live drag), a `#RRGGBB`/`#RRGGBBAA` hex editor, an RGB/HSV switch, one label + `Slider` + `NumberInput` row per channel, the built-in presets and OK/Cancel. The transparency row appears only with `allow_alpha`; Escape cancels, Enter confirms outside the editors |
| `Rgba` | `Color` plus an alpha byte (`new`, `opaque`, `alpha_unit`, `display` = `#RRGGBB`/`#RRGGBBAA`); the color picker's and color window's value type |
| `Spinner` | display only; `.speed(deg/sec)`, `.set_phase(deg)`; the app advances it with `Ui::edit_widget::<Spinner>(id, |s| s.advance(dt))` |
| `ProgressRing` | `.value(0..=1)`, `.thickness`, `.show_value(true)`; ProgressBar role with its range/value |
| `Banner` | `.body`, `.action("重试", || Msg)`, `.closable(|| Msg)`, `BannerKind::{Info, Success, Warning, Error}`; the action and close targets emit, the painted action rect drives hover/click feedback |
| `TooltipCard` | display only; `.body`; title + body in the tooltip palette/shape |
| `Snackbar` | alias for `Toast` |
| `Scrim` | translucent input-blocking backdrop for an in-window modal shell; `.alpha(0..=1)`. Its pixels never depend on pointer state (`paints_pointer_state` is false), so moving the pointer or clicking the backdrop does not invalidate the page; only the dirty bbox is re-blended when content under it repaints. It is the overlay split (`Widget::is_backdrop`): with `Host::overlay_paint` the modal dims the cached page instead of repainting it (D87) |
| `Icon` | display only; MDL2 glyph, `.size(dips)` (default 80% of the cell height), `.tint(Color)` |
| `Badge` | display only chip; `.tone(BadgeTone::Neutral/Accent/Success/Warning/Danger)`. `.removable(|| Msg)` paints a `×` target; clicking it or `Delete`/`Backspace` on the focused instance emits the message. Removal is an **application-owned action** (the widget cannot add/remove itself), so it reports and takes no default handle (D25/D109) |
| `Divider` | display only; a horizontal rule by default, `.vertical(true)` for a column, `.label("…")` centers a text label between two line segments, `.color(Color)` |
| `Avatar` | display only; round initials from the name (`"Annie Lennox"` → `AL`, `"竹韵"` → `竹`), `.initials("Z")` overrides, `.tint(Color)` or a stable name-derived color, `.ring(true)` for overlapping avatars; circular `hit_test`. `.image(ImageId)` paints a registered cover center-cropped and clipped to the circle (anti-aliased; the `.ring` follows the circle) and falls back to initials when the id is unknown |
| `Kbd` | display only shortcut chip (`Ctrl+K`) |
| `ColorSwatch`, `SectionTitle`, `EmptyState` | display only; each instance can opt into focus, mouse and the a11y tree |

Application chrome (`MenuButton`, `SplitButton`, `MenuBar`, the `Toolbar`
overflow) does **not** paint menus: each asks the host for a native popup, the
same deferred `TrackPopupMenu` a context menu uses, so menus are instant and
follow the app's dark/light decision: `WindowBuilder`/`WindowHandle::set_theme`
push the page theme into the shell's popup app mode (an undocumented uxtheme
opt-in; a no-op on systems without it), so a dark page gets dark menus. The host places a widget menu below the control
and flips it above (bottom edge at the control's top, never covering the
button) when it does not fit inside the client area — so a top menu bar drops
down and a bottom one opens upward. Pointer context menus keep the click point
and are only kept on screen.

Access keys (`&` in a `MenuButton`, `SplitButton`, `MenuBar` title or toolbar
label) are always parsed; whether the key character is underlined is the
application's policy. The host turns an Alt tap (press and release without a
chord) into `Event::Alt(true)` and the next key, pointer press or focus change
into `Event::Alt(false)`; forward them with `Ui::set_mnemonic_underlines(show)`.
Only chrome widgets that paint labels change, and the batch flush repaints
them in one request. The default is on, so a host that ignores the event keeps
today's always-underlined labels.

Widgets are created once per page; state changes call `Ui::set_*` setters and
never rebuild the tree. Hover changes invalidate exactly the two affected
rectangles.

### Widget properties

Per-instance details that differ between two instances of the same widget live
in `WidgetProps`, applied at creation or on a live page — no rebuild either
way:

```rust
use bamboo_native_ui::{A11yRole, Narration, WidgetProps};

ui.add_with(
    ColorSwatch::new("accent", color, "竹绿"),
    WidgetProps::new()
        .focusable(true)                  // join Tab + the a11y tree
        .mouse(true)                      // hit-testing and trigger bindings
        .cursor(Cursor::Hand)
        .role(A11yRole::Button)           // announce as a button
        .automation_id("accent-swatch")   // UIA AutomationId
        .narrate(
            Narration::new()
                .description("#7FD3A8，点击查看颜色")
                .row(|index, child| format!("第 {} 首，{}", index + 1, child.name)),
        ),
);
ui.on_click(WidgetId::new("accent"), |_event, _ctx| Some(Msg::ShowColor));

// On a live page: focus/visibility changes relocate focus and repaint.
ui.set_props(WidgetId::new("note"), WidgetProps::new().read_only(true), window);
```

| Property | Effect |
|---|---|
| `visible(bool)` / `enabled(bool)` | container-owned state; runtime changes should use `set_visible`/`set_enabled` |
| `focusable(bool)` / `focus_order(i32)` | same tables as `set_focusable`/`set_focus_order` |
| `cursor(Cursor)` | hover shape |
| `mouse(bool)` | display widgets become hit-testable; enables trigger bindings |
| `read_only(bool)` | blocks changes, not focus: a native editor never opens, a painted field keeps a selection/copy session and refuses typing, paste, cut, delete, undo, IME and steppers, and `TextEdit`/`SearchBox`/`NumberInput` each also expose `.read_only(bool)`/`is_read_only()`. The container pushes it into the widget (`Widget::set_read_only`) so composites that edit their own contents lock every editor; UIA reports it. Distinct from `enabled(false)` |
| `name` / `describe` / `narrate(Narration)` | accessible name/description overrides |
| `automation_id` | UIA `AutomationId` (defaults to the widget id) |
| `role(A11yRole)` | role override (a clickable card can announce as a button) |
| `palette(Palette)` | draw this widget with a different palette |
| `font_size(dips)` / `font_family(name)` / `font(FontSpec)` | per-widget UI font. Applies to every built-in widget's text through `WidgetPaint::text`; icon glyph runs keep the MDL2 face, and a text field's native editor receives the same size and family |
| `tooltip(text)` | shown by the host after a 450 ms hover/focus dwell; 5 s auto-hide |
| `tooltip_at(index, text)` | per-item tooltip for a composite control (absolute list row, segmented option, tab); unset items fall back to `tooltip`, an empty string suppresses the tip |
| `context_menu(Menu)` | native popup on right click, the menu key or Shift+F10; selection arrives in `Host::menu_command` |
| `expose(bool)` | force into or out of the accessibility tree |

`Narration` keeps unset strings exactly as the widget derived them. A lazy
`name_with(|info| …)` receives the widget's current `A11yInfo`, so playback
counters and names follow live state; `.row(|index, child| …)` renames the
options/tabs/rows published by a composite control, using the absolute list row
index.

Layout properties (margin, grow, alignment, min/max size) are not here: the
application computes rectangles (D1).

### Typed widget access

Widget-specific behavior that is not a common property goes through the typed
seam. Widgets are stored as trait objects, so `with_widget` reads the concrete
type and `edit_widget` edits it; the container keeps the repaint rule:

```rust
// Read: None when the id is unknown or the widget is not a ListView<Msg>.
let (first, end) = ui
    .with_widget::<ListView<Msg>, _>(queue, |list| list.visible_rows())
    .unwrap_or((0, 0));

// Write: the closure reports whether it changed anything; a change records
// the widget's rectangle (and any it moved from) as pending paint damage.
ui.edit_widget::<ListView<Msg>>(queue, |list| list.scroll_to_row(400));
// The host hands the pending damage to the window once per callback:
ui.flush_damage(window);
// These methods emit no messages; call window.a11y_refresh() yourself if a
// screen reader must see the new state immediately.
```

| Widget | Typed methods |
|---|---|
| `ListView` | `selected_row()`, `visible_rows()`, `scroll_offset()`, `scroll_to_row(row)`, `ensure_row_visible(row)` |
| `CoverWall` | `set_covers(Vec<Cover>)`, `covers_ref()`, `selected_index()`, `set_selected(index)`, `columns()`, `visible_range()`, `scroll_offset()`, `scroll_to_index(index)`, `cell_rect(index)`, `set_corner_radius(dips)` |
| `WaveformSeekBar` | `progress()`, `set_progress(0..=1)`, `set_samples(iter)`/`samples_ref()`, `set_duration(Option<f32>)`/`duration_value()`, `is_dragging()`, `drag_preview()` |
| `PlaybackTransport` | `is_playing()`/`set_playing(bool)`, `shuffle_enabled()`/`set_shuffle(bool)`, `repeat_mode()`/`set_repeat(RepeatMode)`, `cursor()`/`set_cursor(index)`; `RepeatMode::next()`, `active()`, `label()` |
| `PlaylistView` | `set_rows(Vec<Row>)`, `rows_ref()`, `selected_index()`, `set_selected(index)`, `visible_range()`, `row_rect(index)`, `scroll_offset()`, `scroll_to_row(index)`, `ensure_row_visible(index)`, `drag_state() -> Option<(from, insert)>` |
| `Popover` | `content_rect()`, `set_anchor(Rect)`, `set_viewport(Rect)`/`viewport()`, `dismiss_on_outside_enabled()`; `place_popover(anchor, size, placement, viewport, gap)` |
| `Ribbon` | `tabs()`, `tab_count()`, `selected_tab()`, `select_tab(index)`, `tab_rect(index)`, `item_rect(group, item)`, `set_tabs(Vec<RibbonTab>)`, `set_checked(tab, group, item, bool)`, `set_enabled_item(tab, group, item, bool)`, `mnemonic_underlines()` |
| `Slider` | `value()`, `range()`, `set_range(min, max)` (clamps the value), `step_size()`, `is_snapped()`, `set_snap(bool)`, `tick_count()`, `tick_values()`, `value_label()` |
| `TextEdit` | `text()`, `caret()`, `selection()`, `set_caret(pos)`, `set_selection(anchor, caret)`, `select_all()`, `clear()`, `insert_text(text)` (replaces the selection), `is_painted()`, `is_editing()`, `composition()` |
| `NumberInput` | `value()`, `step(delta)` (clamped by the range) |
| `Segmented` / `Tabs` | `selected()`, `select(index)` (runs the widget's selection path, skips disabled options, returns the message), `labels()`/`options()` and the silent `set_labels`/`set_options`; `Ui::select_child` is the container-level equivalent |
| `Pager` | `page()`, `pages()`, `set_pages(n)` (clamps the page, truncates per-page labels, refreshes the label), `label_text()`, `page_labels()`, `set_page_labels(vec![…])`, `set_label_format(fn)`, `clear_label_format()` |
| `Breadcrumb` | `items()`, `current_index()`, `set_items(Vec<String>)`, `navigate(index)` |
| `NavRail` | `items()`, `set_items(Vec<NavItem>)`, `selected()`, `set_selected(index)`, `select(index)`, `len()`/`is_empty()` |
| `RichText` | `plain_text()`, `runs_ref()`, `set_runs(Vec<RichRun>)` |
| `Sparkline` | `data()`, `set_data(Vec<f32>)`, `range()` |
| `Toast` | `text()`, `duration_value()`, `action_text()`; `show(...)` / `hide()` are on the widget, `Ui::set_message` for the app path |
| `ProgressBar` | `value()`, `is_indeterminate()`, `set_phase(0..=1)` (wrapped; advances the moving segment) |

`edit_widget` returns false for a wrong type and records nothing, so a lookup
can be probed without side effects. Like the `Ui::set_*` setters it defers the
repaint: a burst of typed edits (a dialog syncing several widgets, an animation
tick that advances a spinner and a marquee) costs one `Ui::flush_damage`
request. `Ui::set_props`, `Ui::set_enabled`, `Ui::set_message`,
`Ui::set_mnemonic_underlines` and `Ui::mark_dirty` join the same batch.

### Tooltips

`WidgetProps::tooltip(text)` is displayed by the host; an application never
creates or positions the popup. Hovering a widget with a tooltip, or landing
keyboard/assistive focus on it, starts a 450 ms dwell. A hover tip follows the
pointer (offset to the lower right, flipping when it would leave the window);
a keyboard/a11y-focus tip hangs below the focused element — the selected list
row, the selected segmented/tab cell, otherwise the widget itself — and stays
put once shown. Moving between rows/cells inside a composite control is a new
target (`Widget::child_at`), so the tip hides and dwells again. Composite
controls can name each item with `WidgetProps::tooltip_at(index, text)`
(absolute list row, segmented option or tab index): that item's text wins,
unset items fall back to `tooltip`, and an empty string suppresses the tip for
that item. The tip fades
in over about 95 ms (`WS_EX_LAYERED` alpha), is dismissed after 5 s, by a
click, by leaving the widget, or by a layout/DPI/theme change, and fades out on
the way. Hovering wins over keyboard focus while the pointer is over the
window, so a focus tip does not linger over other content; a held button (drag)
hides it.

`Ui` reports the current target through the `UiHost` seam
(`set_tooltip(Option<TooltipRequest>)`), which the window host implements. The
host deduplicates identical requests, so reporting the hover target on every
pointer move does not restart the dwell, and it creates the popup from a timer
rather than inside a `Host` callback. The popup is a themed, owned, topmost
`WS_POPUP` with `WS_EX_NOACTIVATE | WS_EX_TRANSPARENT`: it never takes focus
and mouse messages pass through it. `WindowHandle::tooltip_visible()` exists
for diagnostics and the showcase self-test.

### Per-widget fonts

`WidgetProps::font_size(dips)` and `font_family(name)` change a single
instance's UI text without touching the theme:

```rust
ui.add_with(
    TextEdit::new("search", "", Msg::Search),
    WidgetProps::new().font_size(15.0),          // closed paint + native editor
);
ui.add_with(
    Button::new("play", "Play", Msg::Play),
    WidgetProps::new().font_family("Cascadia Mono").font_size(14.0),
);
```

The container resolves the override onto every text role the built-in widgets
paint (body, small, title, brand), so a widget keeps its own hierarchy except
for the values you set. Icon glyph runs are never restyled (they stay on
Segoe MDL2), and the painter's font cache already keys fonts by style, so a
per-widget font costs no extra cache entry beyond the styles you use. A
per-field `TextEdit::font_size` / `NumberInput::font_size` is the same setting
local to the field; `WidgetProps` wins when both are present. The native editor
receives the family and size through `EditRequest`.

### Context menus

A menu is described declaratively and shown by the host as a native popup; the
application never builds an `HMENU` or sees `WM_COMMAND`:

```rust
use bamboo_native_ui::Menu;

let menu = Menu::new()
    .item(101, "执行主操作")
    .checked_item(102, "随机播放", shuffle)
    .separator()
    .submenu("更多", Menu::new()
        .item(103, "复制当前次数")
        .item(104, "重置计数"))
    .separator()
    .disabled_item(105, "属性（示例禁用）");

ui.add_with(Button::new("play", "播放", Msg::Play), WidgetProps::new().context_menu(menu));
```

`Menu` entries are `MenuItem` (`checked`, `disabled`, `.shortcut("Ctrl+P")`,
`.icon(MenuIcon::from_rgba(w, h, rgba))`), `MenuEntry::separator()` and
submenus (arbitrarily deep, `disabled` supported). Shortcut text is appended
after a tab, so the shell right-aligns it in its own column; the icon becomes
`hbmpItem` and is scaled by the shell to its menu-icon size (16×16 is the
conventional source; the platform converts RGBA to a 32-bit DIB and frees the
handle with the popup). `Menu::shortcut_item(command, label, shortcut)` builds
such an item in one call. Edge and doubled separators are dropped when the
native popup is built. Commands are application-defined nonzero ids; `0` is
reserved for dismissal. `MenuBarModel::separator()` adds a top-level group break: the titles keep a
gap there (no popup, access key or mnemonic), but the bar paints no vertical
rule. The bar strip itself is painted by the host in the page palette —
background, labels, the hover cell and the dividers above and below — so the
shell's light bar rules and hot/focus decorations never show (D80).

The menu opens on a secondary-button release over the widget, on the menu key
(`VK_APPS`), or on `Shift+F10` for the focused widget (anchored at its
lower-left corner; otherwise at the pointer). A widget without a menu, an empty
menu, or a widget whose native editor is open requests nothing — the `EDIT`
child keeps its own paste/undo menu. A `RightClick` binding runs first; the
menu is the widget's own default and is not affected by `ctx.default()`.

`Ui` asks through `UiHost::show_context_menu`, and `WindowHandle` queues the
popup and opens it from a posted message: `TrackPopupMenu` runs a modal loop
that must not run inside `Host::event` (paints and timers would be skipped and
the window state would be borrowed twice). The selection is replayed afterwards
through `Host::menu_command(widget, command, window)`, so the handler can touch
the UI, change labels and invalidate; `window.menu_active()` and
`window.dismiss_context_menu()` are available to timers and tests. The whole
client is repainted once the menu closes.

The model is static. To reflect live state (a checkmark that follows a toggle),
rebuild the menu and install it with `Ui::set_context_menu(id, Some(menu))`;
pass `None` to remove it. The popup uses the native shell menu, so it follows
the system colors and per-monitor DPI but not the page palette.

A widget can ask for a menu of its own: `Widget::take_native_menu` returns a
`NativeMenuRequest { menu, anchor, avoid }` (`anchor` in client DIPs, `avoid`
the control rectangle the popup must not cover), and `Ui` turns it into the
same `UiHost::show_context_menu` request after the widget handled the input.
`Host::menu_command(widget, command, …)` replays the selection. This is what
the chrome widgets use; an application widget can use it too for a button that
opens a menu declaratively.

### Editing menus at runtime

A stored menu is a value the application owns; edit it instead of rebuilding
it. Item edits address an item by its stable command id, submenus by a
`MenuPath` (index or label steps):

```rust
use bamboo_native_ui::{MenuEntry, MenuPath};

// Item-level edits (search submenus too); false when the id is absent.
menu.set_checked(102, true);
menu.set_label(102, "随机播放（开）");
menu.set_enabled(105, false);
menu.set_command(102, 202);          // menu alias: replay 202 instead
menu.insert(1, MenuEntry::item(110, "新增项"));
menu.remove_command(103);

// A submenu addressed by label, or by position.
let more = MenuPath::root().label("更多");
if let Some(sub) = menu.submenu_at_mut(&more) {
    sub.set_checked(104, false);
}
```

Install the edited model with `Ui::set_menu(id, Some(menu))` (or
`Ui::remove_menu(id)`). `Ui::set_menu` writes both the per-widget registry and
a menu button's/split button's own model, so the click path and a programmatic
open cannot disagree. A popup keeps the snapshot it was built from, so an edit
always applies to the **next** open. `Ui::menu(id)` reads the registry entry.

### Opening menus from code and one activation hook

```rust
// Show the registered menu at a client-DIP point (a shortcut, a gamepad
// button, a command palette). Returns false for no menu / an open editor.
ui.open_menu(WidgetId::new("play"), point, window);

// The imperative equivalent of the widget's keyboard chord: a chrome widget's
// own menu (menu button, menu bar cursor cell, toolbar overflow) or the
// registered menu.
ui.open_menu_for(WidgetId::new("play"), window);

// Reopen a submenu as its own root popup (an explicit reopen, not a hover).
ui.expand_menu(WidgetId::new("play"), &MenuPath::root().label("更多"), window);

// Replay a selection without a popup; it is posted, so it is safe from a
// `Host` callback.
window.menu_click(WidgetId::new("play"), 101);
```

Every native menu — context menus, chrome menus, the tray menu, programmatic
opens and the window menu bar — reports the same lifecycle to one hook:

```rust
use bamboo_native_ui::{MenuDecision, MenuEvent};

fn menu_event(&mut self, event: &mut MenuEvent<'_>, _window: &mut WindowHandle<'_>)
    -> MenuDecision
{
    match event {
        // Rebuild before show; `Handled` (or an empty model) cancels the popup.
        MenuEvent::Opening { widget, source, anchor, menu } => {
            log(format!("open {widget:?} from {}", source.label()));
            menu.set_checked(102, self.shuffle);
        }
        MenuEvent::Opened { menu, .. } => {
            // The popup is up: an edit here closes and rebuilds it, and this
            // event fires again for the rebuilt menu.
            if self.ready { let _ = menu.set_enabled(103, true); }
        }
        // Rewrite a command in place; `Handled` skips the default replay.
        MenuEvent::Selected { command, .. } => {
            if **command == 700 { **command = 701; }
        }
        MenuEvent::Closed { command, .. } => log(format!("closed {command:?}")),
    }
    MenuDecision::Pass
}
```

The order is `Opening` → `Opened` → `Selected` → `Closed` →
`Host::menu_command` (or `Host::tray_event` for the tray). `Opened` is
dispatched from the owner's message loop while the popup is up, so its `menu`
is the live model: editing it (or calling `window.edit_open_menu(|menu| …)`
from `Host::timer`) closes the popup and rebuilds it from the edited model, and
the hook sees another `Opened` for the rebuilt menu (bounded to 8 rebuilds per
open so a hook that edits forever cannot spin). A popup that closes before the
posted message is dispatched still reports `Opened` once. A synthetic
`menu_click` reports only `Selected`. An unchanged model leaves the popup
alone. `window.menu_active()`, `active_menu_widget()` and `dismiss_menu()`
expose the controller state.

### Native window menu bar

The system-drawn bar under the title bar — the one that gives the window
`Alt`/`F10` activation, access-key underlines and free accessibility:

```rust
let bar = MenuBarModel::new()
    .item("程序(&P)", Menu::new().shortcut_item(800, "播放 / 暂停", "Ctrl+P").separator().item(802, "退出"))
    .separator() // a group break (a gap; no vertical rule)
    .item("帮助(&H)", Menu::new().item(830, "关于"));

let window = WindowBuilder::new(host)
    .title("Player")
    .menu_bar(bar)
    .run()?;
```

Commands arrive through `Host::menu_bar_command(command, window)` after the
`menu_event` hook (`MenuSource::MenuBar`, no widget). Labels keep their `&`
access key: the title keeps its label text in the native menu item even though
the framework draws it, so `Alt+P`, a tap of Alt followed by P, and repeated
letters (cycling between titles that share one) are the shell's own behavior.
A chord *held* with Alt (`Alt+P`) is left to the application; only a tap arms
the titles. The host keeps a fallback that opens a matching title's menu from
the message loop (no title highlight first, and no Left/Right traversal between
titles — the shell's own menu mode provides those). The bar belongs to the window frame, so it reduces the client area
and a layout pass follows `set_menu_bar`/`edit_menu_bar`. At runtime:

```rust
window.edit_menu_bar(|bar| bar.set_checked(820, dark));   // rebuilds on change
window.set_menu_bar(Some(other_model));
window.set_menu_bar(None);                                // remove it
```

Top-level titles are painted by the framework with the page palette (the shell
offers no per-app dark menu bar): `panel`/`nav_hover` background, the menu
font, centered labels, and the `&` key underlined only while accelerators are
shown. The divider below the bar comes from `palette.divider` and survives a
hover. The drop-down menus stay native popups and follow the page's dark/light
decision through the shell app mode (D59). Menu-mode keys go to the shell, so
the focused widget never eats the menu's arrow keys; an unconsumed
`WM_SYSKEYDOWN` reaches `DefWindowProc`, which also restores `Alt+F4` and
`Alt+Space`. The painted `MenuBar` widget remains the in-client alternative.

The accelerator text in each drop-down item (`MenuItem::shortcut` /
`Menu::shortcut_item`) is also a live **window** shortcut: while the bar is
installed, the window procedure matches the chord before the page sees the key
and routes the item's command through the same `MenuEvent` `Selected` hook and
`Host::menu_bar_command` a click takes. `MenuBarModel::hotkeys()` returns the
parsed `(Hotkey, command)` pairs (enabled items only, submenus skipped, the
first item wins a repeated chord); it is re-derived on every `set_menu_bar` /
`edit_menu_bar`, so editing a label or shortcut updates the binding with the
menu. Plain letters and digits are rejected by `Hotkey::parse` (a bare `A`
accelerator would swallow typing), while function keys work without a
modifier. The shell owns the keys while its menu is open, and a window
shortcut never fires during `begin_hotkey_capture`. Extra commands without a
menu row use `WindowBuilder::window_hotkey` / `WindowHandle::register_window_hotkey`;
an application binding shadows the menu's chord and both arrive in
`Host::menu_bar_command`.

### Native dialogs

`WindowHandle::show_dialog` opens a platform dialog and returns a `DialogId`
immediately; the answer arrives later in `Host::dialog_finished` on the UI
thread. There are two presentations:

- **modal** (the default): the owner window is disabled while the dialog is
  up, so the user must close it before using the application again;
- **parallel** (`.parallel()` or `.modal(false)`): the dialog has no owner, so
  the window stays enabled and the application remains usable.

```rust
let id = window.show_dialog(
    MessageDialog::new("保存失败，是否重试？")
        .title("出错了")
        .icon(MessageIcon::Error)
        .buttons(MessageButtons::RetryCancel),
);

// Later, outside `event`, with the window back in its normal state:
fn dialog_finished(&mut self, id: DialogId, result: DialogResult, window: &mut WindowHandle<'_>) {
    if let DialogResult::Message(Some(MessageAnswer::Retry)) = result {
        self.retry_save(window);
    }
}
```

Kinds and results:

| Builder | Result | Notes |
|---|---|---|
| `MessageDialog::new(text)` | `DialogResult::Message(Option<MessageAnswer>)` | icons Info/Warning/Error/Question; button sets Ok, OkCancel, YesNo, YesNoCancel, RetryCancel, AbortRetryIgnore; `default_button`. `.button(label, command)` adds application buttons and switches to a task dialog: `MessageAnswer::Custom(command)` comes back, `.instruction(text)` shows a bold main instruction, `.icon_file(path)` sets a custom `.ico` main icon (Esc/close reports `Cancel`; without a comctl32-v6 manifest it falls back to a standard OK box, D94) |
| `FileDialog::open()/save()/folder()` | `DialogResult::File(Option<String>)`, or `DialogResult::Files(Option<Vec<String>>)` with `.multi_select(true)` (open mode) | shell picker; `.filter(label, "*.mp3;*.flac")`, `.initial_dir`, `.initial_name`, `.multi_select(bool)` |
| `FontDialog::new()` | `DialogResult::Font(Option<FontChoice>)` | family, point size, bold, italic; `.initial(FontChoice)` |
| `ColorDialog::new()` | `DialogResult::Color(Option<Color>)` | RGB; `.initial(Color)`; the 16 custom wells persist for the process (`.custom_colors([Color; 16])` seeds them, `dialog::custom_colors()` / `dialog::set_custom_colors(..)` read/restore them), and the shell's mixed custom color is offered again by the next picker |

Date, time and color selection are **in-window components**, not native
dialogs (D115): see `DatePicker`, `TimePicker` and `ColorPicker` below. An
application that needs a full custom dialog window builds it with the normal
widgets.

`None` inside the result means the dialog was dismissed (Cancel, the close
box, or a failure to show it). `show_dialog` never blocks the UI thread: every
dialog runs on its own worker thread, which is what makes a parallel dialog
usable and keeps a modal dialog's nested loop out of the window procedure.
Modal dialogs disable the owner across threads; both message loops must stay
responsive while the dialog is up, which the framework's threads do. Any
number of dialogs may overlap, correlated by `DialogId`.

Limitations: the font and color choosers keep their system captions, and the
color picker's custom wells persist for the process but not across application
restarts (save `dialog::custom_colors()` and restore it). Application-defined
dialog templates are not implemented. The date/time picker is a hand-built dialog
(with an app-controlled caption) around the system control; it carries no
time-zone field and returns local wall-clock numbers.

### Tray icons

A window can install several notification-area icons (D93), described
declaratively:

```rust
use bamboo_native_ui::{Menu, TrayEvent, TrayIcon};

let menu = Menu::new()
    .item(401, "显示主窗口")
    .item(402, "切换主题")
    .separator()
    .item(403, "退出");
window.set_tray_icon(Some(TrayIcon::new("Bamboo").menu(menu)));

// Later: update the tooltip (or menu) in place, or remove the icon.
window.set_tray_icon(Some(TrayIcon::new("正在播放：曲目 007").menu(menu)));
window.set_tray_icon(None);

// A second icon with its own id, menu and balloon target.
window.set_tray_icon_at(1, Some(TrayIcon::new("下载").menu(download_menu)));
window.show_tray_balloon_on(1, "下载完成", "曲目已保存。", MessageIcon::Info);
window.set_tray_icon_at(1, None);
```

`TrayImage::Default` uses the system application icon; `.icon_file("app.ico")`
loads one from disk at the notification-area small-icon size for the current
DPI (a multi-size `.ico` picks the closest image; the icon reloads on a DPI
change, and a load failure falls back to the default). Tooltips are
truncated to the native 127-character limit. Right click or the keyboard menu
key on an icon shows its `Menu` at the cursor; the selected id arrives as
`TrayEvent::Command { icon, command }` in `Host::tray_event`, while mouse and
keyboard actions arrive as `TrayEvent::Action { icon, action }` with
`TrayAction::{Click, DoubleClick, RightClick, MiddleClick, KeySelect}` (the
usual reaction to a click is `window.show()`; a right click also opens the
icon's menu after the callback, so a hook can refresh the model first).
`icon` is the application id passed to `set_tray_icon_at` (0 for the primary
icon). Both run on the UI thread, outside every other callback.

The host owns the `NOTIFYICONDATAW` registrations (version 4, so click and menu
events are distinguished; the id is in `wParam`'s low word), destroys file
icons after a replacement, re-adds every icon when Explorer broadcasts
`TaskbarCreated`, reloads them on a DPI change and removes them when the window
is destroyed. The icon itself is drawn by the shell, so it does not follow the
page palette. `window.show_tray_balloon(title, text, MessageIcon::…)` requests
a notification balloon (the shell owns its look and placement).

### System notifications

A real Windows toast through the notification platform does not need a tray
icon (D135). Give the window an AppUserModelID once and notify from a window
callback:

```rust
use bamboo_native_ui::{Notification, NotificationResult};

WindowBuilder::new(host)
    .title("Player")
    .app_id("MyCompany.Player")
    .run();

let result = window.notify(
    &Notification::new("正在播放", "夜曲 — 周杰伦")
        .tag("now-playing")
        .group("player"),
);
match result {
    NotificationResult::Toast => { /* shown by the shell / Action Center */ }
    NotificationResult::Balloon => { /* fell back to the primary tray balloon */ }
    NotificationResult::Failed => { /* no toast and no tray icon */ }
}
```

`Notification::new(title, body)` is the whole required surface; `.app_id(id)`
overrides the window id for one toast, `.tag`/`.group` replace or remove it in
the Action Center, `.launch(args)` passes arguments to the next launch and
`.silent(true)` suppresses the sound. Text is XML-escaped by the library.
`WindowBuilder::app_id` also calls
`SetCurrentProcessExplicitAppUserModelID` (taskbar grouping). WinRT is
initialized on the window thread next to OLE and uninitialized with the
window. An unpackaged process needs a Start-menu shortcut carrying the same
AppUserModelID for the shell to display a toast; call
`window.register_notification_shortcut("Player")` once (settings/first run) to
write/refresh that `.lnk` under the user's Programs folder. Without it
`ToastNotificationManager` may accept the request while Windows silently drops
it. When the shell refuses, `notify` falls back to the primary tray icon's
balloon and reports `Balloon`. Clicking a toast starts the application through
the shell's activation path — there is no in-process click callback; read
`launch` arguments on the next launch.

### Global hotkeys and shortcut capture

```rust
use bamboo_native_ui::Hotkey;

let hotkey = Hotkey::parse("Ctrl+Alt+Shift+F12").expect("valid spec");
if window.register_hotkey(hotkey) {
    // later: Host::hotkey(hotkey, window) fires system-wide while we exist
}
window.unregister_hotkey(hotkey);

// Shortcut editor: the next chord is consumed before the application sees it.
window.begin_hotkey_capture();
// Host::hotkey_captured(Some(chord), window) or None if Esc/focus loss ended it
```

`Hotkey::parse` accepts `Ctrl`/`Control`, `Shift`, `Alt`, `Win` (any order,
case-insensitive) plus one key: letters, digits, `F1`..`F24`, `Enter`/`Return`,
`Space`, `Tab`, `Esc`/`Escape`, `Backspace`, `Delete`/`Del`, `Insert`/`Ins`,
`Home`, `End`, `PageUp`/`PgUp`, `PageDown`/`PgDn` and the arrows. A shortcut
needs a modifier unless the key is a function key — a global bare `A` would
swallow ordinary typing, so `parse` rejects it (and capture keeps waiting for a
valid chord). `Hotkey::label()` renders `"Ctrl+Alt+Shift+F12"` for menus and
help text, and `Hotkey { modifiers, vk }` is plain data (`vk` matches
`KeyEvent::vk`).

`register_hotkey` returns false when the system refuses the chord because
another window or process owns it; registering the same hotkey twice is a
no-op that returns true, and the host allocates and tracks the ids. All
registrations are released when the window is destroyed. Capture is a modal
input mode owned by the host: modifier-only presses keep waiting, the pressed
chord is built from `GetKeyState`, its translated `WM_CHAR` is dropped, and
Esc, `cancel_hotkey_capture()` or focus loss reports `None`. The captured
chord is not registered automatically — the application decides, and can call
`register_hotkey` from the callback.

**Persistence:** `label()` output parses back (including an unmapped virtual
key, rendered as `VK2C`), and `registered_hotkeys()` /
`registered_window_hotkeys()` return the live tables. Store the labels and
command ids, then restore with `register_hotkey` / `register_window_hotkey`
(check the bool: another process may already own the chord). The menu bar's
derived accelerator labels are not part of `registered_window_hotkeys()`.

Unlike these system-wide registrations, **window shortcuts** only fire while
this window receives the key and never conflict with another process:

```rust
use bamboo_native_ui::{Hotkey, Menu, MenuBarModel, WindowBuilder};

let bar = MenuBarModel::new().item(
    "程序(&P)",
    Menu::new()
        .shortcut_item(800, "播放 / 暂停", "Ctrl+P")   // live while the bar is installed
        .shortcut_item(801, "打开…", "Ctrl+O"),
);
let window = WindowBuilder::new(host)
    .menu_bar(bar)
    .window_hotkey(Hotkey::parse("F5").unwrap(), 824) // no menu row needed
    .build()?;
```

`MenuBarModel::hotkeys()` parses the visible accelerator labels; the host
registers them on install and on every edit and dispatches a match through
`Host::menu_bar_command` (after the `MenuEvent` `Selected` hook, source
`MenuBar`), swallowing the translated character. The application's own chords
(`WindowBuilder::window_hotkey(s)` and
`WindowHandle::register_window_hotkey` / `unregister_window_hotkey` /
`clear_window_hotkeys`) are checked first, so they can override a menu label;
`window_hotkeys()` returns the merged table and `menu_hotkeys()` only the
bar-derived part.

### Container options

Hit-testing walks the widgets top-down (last added wins) and only considers
visible, enabled, *interactive* widgets; each widget's `hit_test(pos)` decides
whether the point is inside its painted shape (default: the layout rectangle,
overridable for round/rounded visuals). Display-only widgets default to
pass-through, so labels, cards and a dismissed toast never block the control
underneath. The container owns these rules:

```rust
ui.set_visible(WidgetId::new("toast"), false, window); // hide, relocate focus, repaint
ui.set_enabled(WidgetId::new("play"), false, window);  // clears hover/press, relocates
ui.set_focusable(WidgetId::new("note"), false, window);// runtime Tab opt-out
ui.set_focus_order(WidgetId::new("play"), 10);         // ascending Tab weight, default 0
ui.set_cursor(WidgetId::new("play"), Some(Cursor::IBeam)); // per-widget override
let cursor = ui.hover_cursor();                        // host applies it on move
ui.set_focus_scope(Some(vec![WidgetId::new("modal_ok")]), window); // Tab trap
ui.set_focus_scope(None, window);                                  // release

// Select through the widget path: a disabled segment or a stale index emits
// nothing; the widget rectangle is invalidated and a real selection returns
// its message.
if let Some(message) = ui.select_child(WidgetId::new("segmented"), 2, window) {
    self.apply(message, window);
}
```

`set_visible` takes the host sink because hiding the focused widget must move
focus to the next eligible widget and invalidate both rings immediately (the
structural change is not a display-property edit). Hidden widgets are also
removed from `Ui::a11y_nodes()`. `set_enabled` takes the sink for the same
focus relocation, but records the widget's disabled look as paint damage.

**Content set through code vs. a user edit.** `Ui::set_text` / `set_number` /
`set_value` / `set_toggle` / `set_selected` / `set_progress` / `set_label` /
`set_rows` never call a widget's change callback: `TextEdit::on_change`,
`NumberInput`/`Slider`'s `on_change` and friends report *user* edits only —
typing, paste, cut, delete, undo, a stepper click, a drag. A setter stores the
value and marks its own widget's rectangle as pending damage;
`Ui::flush_damage(window)` hands the union of every changed widget to the sink
as one rectangle, so syncing N controls in a row costs one repaint request and
the change callbacks stay silent. The same batch covers the widget-specific
setters reached through `Ui::edit_widget`, plus `Ui::set_props`,
`Ui::set_enabled`, `Ui::set_message`, `Ui::set_mnemonic_underlines` and
`Ui::mark_dirty` (an explicit mark for a change the container cannot observe).
`Ui::has_damage()` reports whether something is pending. Call the flush once at
the end of every host callback that can change widget content (the color window
does this for its whole control batch; the showcase flushes in `event`,
`apply`, `timer`, `menu_event`, `a11y_action`, `layout_chrome` and `started`):

```rust
ui.set_text(WidgetId::new("hex"), "#1971C2");
ui.set_number(WidgetId::new("r"), 0x19);
ui.set_value(WidgetId::new("slider"), 0.5);
self.ui.flush_damage(window); // one invalidate covering all three
```

Focus order: ascending `focus_order` weight, insertion order as the tie-break
(default 0). Hidden, disabled, empty and non-focusable widgets are skipped.
A focus scope (`Ui::set_focus_scope`) confines the Tab walk and programmatic
focus to a set of widget ids (an in-window modal panel); `None` releases it.
Cursor precedence: per-widget override, then `Widget::cursor_at(pos)` (a
composite field can report an arrow over a click zone and an I-beam over the
text), then `Widget::cursor()`, then `Hand` for interactive widgets and `Arrow`
otherwise.

### Dynamic widget tree

Widgets are usually built once per page, but a panel of plugin commands, a
runtime toolbar or a small editor can add, remove and reorder entries without a
full rebuild (D62):

```rust
// Remove: drops every per-id table (props, bindings, focus hooks, cursor,
// visibility, focus-scope membership), commits an open editor, relocates focus
// to the same Tab position and repaints the old rectangle. Unknown id → false.
if ui.remove(WidgetId::new("slot_2"), window) {
    window.a11y_refresh();
}

// Insert at a tree position: later = painted on top, hit-tested first and the
// Tab tie-break for equal focus weights. `insert_before(anchor, ..)` is the
// relative form; an unknown anchor appends.
ui.insert(ui.len(), Button::new("slot_2", "动态项 2", Msg::Slot(1)));
ui.set_rect(WidgetId::new("slot_2"), rect);
window.invalidate(rect);

ui.raise(WidgetId::new("slot_2"));  // z-order only; repaint the overlap union
ui.lower(WidgetId::new("slot_2"));
ui.move_rect(WidgetId::new("slot_2"), new_rect, window); // old ∪ new + editor + tooltip
let committed = ui.rebuild(area, window, |ui| { /* build the page again */ });
```

Ids must stay unique in one tree. For a widget created at runtime, intern its
name instead of relying on a static literal (D111):

```rust
let id = WidgetId::intern(&format!("row-{index}")); // stable &'static str
ui.insert(ui.len(), Button::new(id.name(), label, Msg::Row(index)));
```

The text is interned once per process (one leaked string per *distinct* name),
so `WidgetId` keeps its `Copy`/`Eq`/`Hash`/`&'static str` contract. Draw names
from a bounded set (`row-{index}` over a bounded row count or a fixed pool)
rather than a fresh UUID per rebuild, which would grow the interner forever.
`id.name()` reads the string back.

A live framework-painted editing session can survive a remove/re-insert of the
same id (`Ui::remove_preserving_session`, D111):

```rust
// Capture the open draft and remove the widget without committing it.
ui.remove_preserving_session(WidgetId::new("field"), window);
// ...build later: adding a painted TextEdit with the same id restores the
// draft, caret and selection, and re-opens the session.
ui.add(TextEdit::new("field", "", Msg::Field).painted());
if let Some(rect) = field_rect { ui.set_rect(WidgetId::new("field"), rect); }
ui.set_focus(WidgetId::new("field"), window); // if the caret should keep blinking
```

`SessionSnapshot`/`Widget::take_session`/`restore_session` are the widget seam.
Only a painted `TextEdit` supports it; a native `EDIT` and a composite's
embedded field still commit on removal. `has_pending_session(id)` reports a
waiting snapshot and `discard_pending_sessions()` drops them all.

`set_rect` stays deliberately silent for layout passes; `move_rect` is the
single-widget form with invalidation. `rebuild` commits an open painted draft
(its message goes to `drain_messages`), ends a native session, clears the tree
(page images survive) and invalidates `area` once. A structural change is
invisible to a screen reader until the application calls
`WindowHandle::a11y_refresh`.

### Drag and drop

Drags are **system drags** (OLE `DoDragDrop`, D132): the payload leaves the
window and the process, so another window or application can accept it, and
text or files dragged in from anywhere reach the same widget-level targets.
The payload is a `DragData` — `Text(String)` (`CF_UNICODETEXT`) or
`Files(Vec<PathBuf>)` (`CF_HDROP`) — declared in `bamboo_native_ui::drag`.

```rust
ui.set_drag_source(WidgetId::new("queue_row"), || {
    Some(DragData::text(format!("track:{index}")))
});
ui.set_drop_target(WidgetId::new("playlist"), |data, pos| match data {
    DragData::Text(text) => Some(Msg::DropTrack(text, pos.y)),
    DragData::Files(paths) => Some(Msg::DropFiles(paths)),
});
ui.set_drop_hover(WidgetId::new("playlist"), |_data, _pos, over| {
    self.drop_highlight = over;
    true // changed visuals: repaint this widget
});
```

Once the 4 DIP threshold is crossed on a source, the container replaces that
widget's `Trigger::DragStart` path and asks the host to start the drag; OLE owns
the pointer from there. The widget under the pointer becomes the target (its
hover callback runs on each change only), and the release consumes the data
through the drop handler and queues its message. `is_dragging`/`drag_data`/
`drag_source`/`drag_target` are typed access; `drag_source()` is `None` for a
drag that entered from another window. Escape or `Ui::cancel_drag` aborts,
removing the source or target aborts too. A widget with its own internal drag
(`TreeView`, `PlaylistView`, the table header) should not also be a source.

A target receives every kind; the type-specific helpers filter:

```rust
ui.set_text_drop_target(WidgetId::new("note_to"), |text, pos| {
    Some(Msg::Dropped(text, pos))
});
ui.set_files_drop_target(WidgetId::new("import_zone"), |paths, _pos| {
    Some(Msg::Import(paths))
});
ui.set_text_drop_hover(WidgetId::new("note_to"), |text, _pos, over| {
    self.drop_highlight = over; // the app paints the frame
    true
});
```

A painted field registered as a drop target inserts text at the release point
through the normal user path (undo, `max_length`, `on_change`) unless
read-only; the application handler then reports the drop. Pressing inside an
existing text selection keeps it, and crossing the threshold copies it out — a
password field never offers it, and a touch handle drag is an adjustment, not a
drag-out.

The host half: `Host::drag_enter`/`drag_over`/`drag_leave`/`drag_drop` receive
the shell's events in client DIPs and return a `DropEffect`
(`None`/`Copy`/`Move`/`Link`); `Host::drag_finished` follows the source drag
with the effect the receiver reported. The usual implementation forwards to
`Ui::drag_enter`/`drag_over`/`drag_leave`/`drag_drop`/`drag_finished` (the
showcase does this). `WindowHandle::start_drag(data)` starts one directly;
`WindowHandle::set_system_drag_enabled(false)` turns OLE off for automated runs
and leaves the container's in-window fallback (same `DragData` and routing).

### Scroll view

A scroll view scrolls arbitrary child widgets inside a viewport. It is a
container region rather than a widget (D65): the application lays its children
out once in **content** coordinates (the viewport's top-left corner is the
origin) and the container keeps each child's rectangle at
`viewport.origin + content - offset`, so hit-testing, painting, typed access,
the native editor, tooltips and the accessibility tree all read ordinary client
rectangles.

```rust
// Once per layout (a resize re-registers it and keeps the live offset).
ui.set_scroll_view(
    ScrollView::new(WidgetId::new("panel"), viewport, ScrollBars::Both)
        .children([
            ScrollChild::new(WidgetId::new("row_1"), Rect::from_xywh(0.0, 0.0, 300.0, 30.0)),
            ScrollChild::new(WidgetId::new("row_2"), Rect::from_xywh(0.0, 34.0, 300.0, 30.0)),
        ])
        .wheel_step(48.0)       // DIPs per wheel notch (default 48)
        .padding(12.0)          // content margin (uniform; `.padding_by(Insets)`)
        .style(ScrollBarStyle::new().thickness(12.0))
        .name("滚动面板"),      // publishes a UIA Group; children nest under it
    window,
);

ui.scroll_offset(WidgetId::new("panel"));          // Some(Point)
ui.set_scroll_offset(WidgetId::new("panel"), Point::new(0.0, 64.0), window);
ui.scroll_by(WidgetId::new("panel"), 0.0, 48.0, window);
ui.scroll_to_child(WidgetId::new("row_2"), window); // reveal one child
ui.scroll_owner_of(WidgetId::new("row_2"));         // Some(panel)
ui.set_scroll_viewport(WidgetId::new("panel"), rect, window);
ui.remove_scroll_view(WidgetId::new("panel"), window);
```

Behavior:

- Only children intersecting the viewport are painted (clipped to the whole
  viewport) and hit-tested; a scrolled-out child costs nothing. Its rectangle
  is still kept, so the application can read `Ui::rect` for it. The display
  area never shrinks: a bar overlays the content it shares a strip with while
  mid-scroll. `padding` is the content margin — children start at
  `viewport + padding`, and the scroll range ends one padding inside the
  visible bars, so at the end the last row is fully visible with its gap and
  never runs under a scrollbar.
- The bars use the shared scrollbar style/state: thumb drag keeps the grab
  offset, a track click jumps (or pages with
  `ScrollBarStyle::page_on_track_click`), the thumb brightens on hover/active,
  the cursor is the arrow, and a bar press never reaches a widget or moves
  focus. Each axis shows its bar from the content vs the padded viewport (a
  vertical bar never makes a fitting width sprout a horizontal one), and the
  junction square between two shown bars is painted with the track color and
  hit-tests as chrome.
- The wheel first goes to an inner widget that scrolls itself (`ListView`,
  `Table`, `TreeView`, `PropertyGrid`, `MultiSelectList`, a multiline
  `TextEdit`, an open `ComboBox`); otherwise the panel under the pointer scrolls
  by `wheel_step`.
  `Ui::wheel_xy` carries both axes (Shift+wheel horizontal).
- Tab order includes scrolled-out children and a keyboard/a11y/programmatic
  focus move reveals the focused child. A pointer click never scrolls: the
  user clicked the part that was already visible.
- A named panel publishes one `A11yRole::Group` for the viewport with the
  child nodes nested under it; a fully scrolled-out child reports `offscreen`
  (geometric bounds), a partially visible one its clipped on-screen rectangle.
  The group also publishes both scroll axes, so a client can page it through
  the UIA Scroll pattern (`SetScrollPercent`, step by page) exactly like a
  list or table (D67).
- Visibility is derived: the panel shows while at least one of its own
  children or a nested panel's children is visible, so a page switch that
  hides the children hides the bars.
- The content extent is derived from the children unless `ScrollView::content`
  supplies a larger one. Do not move a scrolled child with
  `set_rect`/`move_rect` — re-register the panel instead.
- **Nesting** (D92): `ScrollView::nested_in(parent)` makes the configured
  viewport a rectangle in the parent's *content* coordinates. The container
  resolves the client viewport through the parent's offset and clips it to
  every ancestor viewport, so the inner panel and its content move with the
  page; layout cascades, the wheel scrolls the deepest panel under the pointer
  first and hands the notch to the page once the inner reaches its end, focus
  reveal walks the chain outwards, and removing a parent reparents its panels
  to roots. A named nested panel publishes its own UIA Group and Scroll axes.

```rust
// A small list inside the page's content: viewport in the parent's
// content coordinates, children in the inner panel's own content coordinates.
ui.set_scroll_view(
    ScrollView::new(WidgetId::new("inner"), Rect::from_xywh(10.0, 30.0, 90.0, 84.0), ScrollBars::Vertical)
        .nested_in(WidgetId::new("panel"))
        .children(rows)
        .name("嵌套面板"),
    window,
);
```

### Pointer modifiers and multi-selection

`Event::PointerDown` / `Event::PointerUp` carry a `PointerModifiers`
(`ctrl`, `shift`). `Ui::pointer_down_with` / `pointer_up_with` take the same
values; the plain `pointer_down` / `pointer_up` / `pointer_down_button` /
`pointer_up_button` wrappers pass `PointerModifiers::NONE`.

A release with Ctrl/Shift held is delivered as `WidgetInput::SelectWith(pos,
modifiers)` **only** to a widget whose `Widget::multi_selectable()` is true —
every other control still sees an ordinary click (`SelectWith` fires the
`Click` trigger, so bindings behave the same). `MultiSelectList` is the first
consumer (D66):

```rust
// Plain click: toggle one row, remember it as the anchor.
// Shift+click: select the inclusive range from the anchor (replacing others).
// Ctrl+click:  toggle one row and move the anchor there.
```

The scrollbar helper's hit-test treats a press anywhere across the track within
the thumb's axis span as a thumb grab, so pressing the pill's inset edge starts
a drag instead of jumping the content.

### Touch, pen and long press

`Event::PointerKind(PointerKind::{Mouse, Touch, Pen})` tells the container what
produced the pointer sequence; the host sends it when the device changes
(Windows tags touch/pen promoted to mouse messages in `GetMessageExtraInfo`) and
the application forwards it with `Ui::set_pointer_kind(kind)`. A plain mouse
sends nothing, so desktop behavior is unchanged. `Ui::pointer_kind()` reads it
back.

`Event::LongPress { pos }` is a touch press held in place: the host owns the
~500 ms dwell (it has the timer) and cancels it when the finger moves past
8 DIP or lifts. Forward it to `Ui::long_press(pos, window)`, which asks the
painted editor under the point to select the word there and show its selection
handles. The container then:

- raises the touch keyboard for the session (`UiHost::set_touch_keyboard(true)`,
  which `WindowHandle` maps to the touch layout; a read-only field and a mouse
  press ask for nothing) and dismisses it when the session ends;
- paints two handles (a stem from the text line to a circle, the start above its
  line and the end below) that are clamped inside the field's rectangle;
- lets a press within 20 DIP of a handle drag that selection edge, instead of
  collapsing the selection under the finger.

```rust
match event {
    Event::PointerKind(kind) => self.ui.set_pointer_kind(kind),
    Event::LongPress { pos } => self.ui.long_press(pos, window),
    // ...
}
```

A host implementing its own window supplies `UiHost::set_touch_keyboard`; the
default accepts nothing and the framework then behaves like a mouse-only host.

### Triggers and focus hooks

Bind a high-level interaction instead of inventing a second input path. The
handler may emit a message and/or keep the widget's own behavior two ways:
`ctx.default()` (the original flag form — the default runs *after* the handler
returns) or `ctx.run_default()` (runs the default *now* and returns its message,
so the handler can wrap or post-process it). Leaving both uncalled suppresses
the default:

```rust
ui.on_click(WidgetId::new("play"), |event, ctx| {
    ctx.default();                 // run the button's normal message after this
    log(event.pos);
    Some(Msg::Audit)               // custom message arrives first
});
ui.on_click(WidgetId::new("row"), |_event, ctx| {
    let built_in = ctx.run_default();  // Some(the widget's own message)
    built_in.and_then(|m| rewrap(m))   // the default does not run again
});
ui.bind(WidgetId::new("queue"), Trigger::Wheel, |_event, _ctx| {
    None                           // swallow the wheel: the list stays put
});
ui.on_activate(WidgetId::new("play"), |_event, ctx| { ctx.default(); None });
```

Triggers cover `Click`, `DoubleClick`, `RightClick`, `HoverEnter`,
`HoverLeave`, `Wheel`, `Activate`, `DragStart`, `DragEnd` and `ValueChange`.
`Activate` is the same path for Enter/Space, UIA `Invoke`/`Toggle`
and a text field's UIA focus-then-edit request, so a screen-reader action fires
exactly what the keyboard fires.

- `DoubleClick` comes from the platform (`CS_DBLCLKS`); the ordinary down/up
  pair before it is still a `Click`, and the second click is delivered as a
  normal press/release too, so rapid clicking (a number stepper, a repeat
  button) never drops every other click. Widgets have no default double-click
  behavior.
- `RightClick` fires on a matching secondary-button down+up on the same
  widget. It does not move focus for ordinary controls; a text field is the
  exception — a secondary click focuses it, opens the native editor and shows
  the `EDIT` context menu (paste/undo/select all), and a secondary click inside
  an already-open field keeps the session and shows that menu. For widgets with
  `WidgetProps::context_menu` the binding runs first, then the widget's menu
  opens (see [Context menus](#context-menus)).
- A held press becomes a drag after 4 DIPs of travel: `DragStart` fires once,
  and the release raises `DragEnd` instead of `Click`. Sliders still receive
  their normal `ReleaseAt` and commit their value.
- `ValueChange` runs **after** the widget handled the input, so its custom
  message is queued after the default one. It observes the widget's accessible
  value (checked, value text, selection, range, selected option/row), fires
  only for inputs that can change a value, and only when a binding exists, so
  it costs nothing otherwise. Editor commits and `Ui::set_*` calls are
  application-known and do not fire it.

The custom message is queued before the
widget's own message. `on_click`/`on_activate` compact the common cases; binding
the same (widget, trigger) again replaces the handler, `unbind` removes it.

Focus lifecycle hooks run after a native editor commits and before the host
raises the a11y focus event:

```rust
ui.on_focus_lost(id, |event| {
    match event.reason {
        FocusReason::Keyboard => FocusOutcome::Allow,
        _ => FocusOutcome::Redirect(other),
    }
});
ui.on_focus_gained(id, |_event| FocusOutcome::Allow);
```

Reasons are `Pointer`, `Keyboard`, `Programmatic`, `A11y` and `Relocated`
(hidden/disabled/non-focusable owner). A lost hook can `Cancel` the move or
`Redirect` it once; a gained hook can `Cancel` to revert. Relocation still
clears focus if a hook vetoes it, so a hidden widget can never keep the
keyboard.

**Widget-level observers.** Besides the container seams above, the standard
controls take fluent, additive observers of their own — `Button`, `Toggle`,
`Segmented`, `Tabs`, `Slider`, `Checkbox`, `RadioButton`, `RadioGroup`,
`ListView`, `ComboBox`, `MultiSelectList`, `MenuButton`, `SplitButton`,
`MenuBar` and `Toolbar` (D109). `TextEdit` has its own set (D106).

```rust
Button::new("save", "保存", Msg::Save)
    .on_click(|pos| Some(Msg::Log(format!("click at {}", pos.x))))
    .on_double_click(|_| Some(Msg::QuickSave))
    .on_focus_gained(|event| Some(Msg::Hint(event.reason)))
    .on_key_down(|key| (key.code == KeyCode::Escape).then_some(Msg::Cancel));
```

Each takes `|Point| Option<M>` (click/right/double), `|KeyEvent| Option<M>`
(key down/up) or `|FocusEvent| Option<M>` (focus gained/lost) and returns the
message to deliver, or `None`. They are **additive**: the built-in action still
runs and the observer message is queued *after* the widget's own message. They
never suppress or replace a default, so they take no default handle; use
`Ui::bind` (with `ctx.default()`/`ctx.run_default()`) when the application must
replace an action. `on_key_down` receives every key that reaches the focused
widget before the container interprets it (Tab, arrows outside modify mode,
plain characters), and `on_key_up` every release `Ui::key_up` forwards.

**Surface styling.** The painted components take a shared background surface
(D110): `.corner_radius(dips)`, `.corners(Corners)`,
`.background(color)`, `.background_option(Option<Color>)`,
`.transparent_background(bool)` and `surface()` (a `SurfaceStyle`). Adopted by
`Button`, `Label` (an explicit background paints a chip behind the text),
`Card`, `Toast`, `Banner`, `TextEdit` (native and painted frames),
`NumberInput`, `SearchBox`, `Slider` (track and fill) and `ProgressBar` (track
and fill). A uniform radius also rounds a button/text field's focus ring (and a
field's inner editing ring); `transparent_background` suppresses the widget's
own fill and leaves a real 1 DIP outline — a transparent rounded field is never
painted as a filled pill, and a mixed-corner surface strokes each corner with
its own radius. An opaque rounded frame is one cached two-color mask on GDI
(D133). Hit-testing stays rectangular.

### Keyboard model

- `Tab` / `Shift+Tab` move focus between visible, enabled and focusable
  widgets in focus-weight order.
- `Enter` on a text field opens the native editor; on a *content* control
  (slider, list, segmented control, tabs) it toggles **modify mode**.
- In modify mode arrows operate the control instead of moving focus: slider
  value, list row cursor (+ `PageUp/PageDown/Home/End`), segment/tab selection.
  The ring turns into a solid 2 DIP frame. On a list, `Enter` again activates
  the current row.
- `Esc` leaves modify mode; `Backspace` leaves it too, and outside modify mode
  releases keyboard focus (inside a text field it never reaches the framework
  and still deletes text).
- `Shift+F10` and the menu key open the focused widget's context menu. Inside
  the native editor they belong to the child control's own menu.
- A plain arrow outside modify mode never changes a control's value, so Tab /
  arrows cannot trigger an accidental selection change.
- `Alt+key` runs access keys. A widget may claim one per item
  (`Widget::mnemonic_key`, a menu bar's `Alt+F`); otherwise the first visible
  widget whose `mnemonic()` matches is focused and activated. `MenuButton`,
  `SplitButton` and `MenuBar` labels parse `&` markers through
  `MnemonicLabel` and underline the key character.
- Chrome widgets do not trap the arrows: a focused `Toolbar`/`MenuBar` keeps
  `Left`/`Right` for its item cursor only while the cursor can move that way;
  at the ends and for `Up`/`Down` the key moves focus
  (`Widget::owns_navigation_key`). `Tab` always leaves.

The focus ring is an accessibility indicator, not a click artifact: it appears
only after keyboard navigation (Tab/arrows/Enter, modify mode), never after a
mouse click. `Ui::focus_visible()` reports the state and
`Ui::show_focus_ring(visible, sink)` forces it on (a11y focus events).
`Widget::focus_ring_rect()` places the ring around what the widget actually
paints — the `Toggle` ring hugs the switch track, the list ring hugs the rows
(not the scrollbar), so painted content never shows outside the frame.
`Widget::focus_ring_radius()` follows a non-rectangular shape with the
anti-aliased rounded stroke: the switch ring is a pill and a standalone radio
dot's ring is a circle.

Layout helpers live in `bamboo_native_ui::layout`: `VStack`, `HStack`,
`split_h`, `split_v`, `grid`, `padded`.

### Gamepad as keyboard

`bamboo_native_ui::gamepad` turns an XInput pad into the keys focus navigation
already consumes, so a page written for the keyboard needs no second input
model:

```rust
let mut pad = Gamepad::new(0);            // slot 0..3
pad.set_enabled(true);                    // the application switch
// from the host's animation timer (~30 ms):
if window.is_foreground() {
    for key in pad.poll(Instant::now(), true) {
        ui.pad_key(key, window);          // arrows, Tab, Enter, Escape…
    }
}
```

`Ui::pad_key` is `Ui::key` plus the one platform courtesy a controller needs:
when the key opens a text field, the host raises the Windows on-screen keyboard
and dismisses it when that session ends (see *Soft keyboard* below). Everything
else — focus order, activation, the context-menu chord — is identical.

`PadState` carries `PadButtons` (XInput layout), both sticks (-1..=1) and the
triggers; `read_state(slot)` reads one sample and `Gamepad::update(state, now)`
is the pure translation. D-pad/left stick → arrows (held directions repeat
after 400 ms, then every 90 ms — so the stick walks the same focus order the
keyboard uses), **A**/**Start** → Enter, **B**/**Back** → Escape, **X** →
Space, **Y** → Shift+F10 (context menu), **RB** → Tab, **LB** → Shift+Tab.
`set_enabled(false)` is the manual switch (it drops the held state); `poll`'s
`active` argument should be `WindowHandle::is_foreground()`, so a background
window never navigates from a pad. The showcase watches XInput (33 ms while a
pad is attached, 500 ms otherwise), so a hot-plugged or sleeping controller is
picked up without a restart, shows 手柄导航 in the footer, and exposes 手柄导航
as a checked context-menu item. Poll from the host's animation timer and keep
that timer running while the window is live — a slow watch tick when nothing
animates. Controller input is driven by the timer, so stopping it on idle
silently disables the pad until the next unrelated timer start.

### Soft keyboard (Windows)

A desktop window is not a touch app: the shell keyboard must be asked for. When
a pad session owns the keyboard, `Ui::pad_key` reports it through
`UiHost::set_soft_keyboard(true/false)` and `WindowHandle` maps it to
`bamboo_native_ui::soft_keyboard`:

```rust
// Layout::Gamepad first (the controller layout Windows 11 24H2 exposes to
// handhelds), then the touch keyboard, then TabTip.exe/osk.exe on first use.
soft_keyboard::show(soft_keyboard::Layout::Gamepad);
soft_keyboard::hide();
let open = soft_keyboard::is_open();    // input pane rect + occlusions
```

The functions are standalone and safe to call directly. Visibility is measured,
never assumed: `IFrameworkInputPane::Location` reports the keyboard's screen
rectangle (empty while hidden) and works in an ordinary desktop process; the
input view's occlusions cover a hosted window. That measurement guards the
shell's `ITipInvocation::Toggle`, which always flips the state, so `show`/`hide`
never accidentally invert an already-open keyboard. (Window-class probing does
not work: Windows 11 keeps a visible full-screen `Windows.UI.Core.CoreWindow`
for its input service whether or not the keyboard is up, and the title is
localized.) Pointer and
hardware-keyboard text entry never report to `set_soft_keyboard`, so desktop
users never see a keyboard appear. While a pad-raised keyboard is up, `pad_key`
does not forward keys (on Windows' gamepad layout the user is typing on the
keyboard, and forwarding **A** would commit the field underneath): **A**
re-raises a keyboard the user dismissed with its close button, **B** closes it
and leaves the field (committing, like clicking out). The host also dismisses
it when the native editor closes, the window deactivates or it is destroyed.

## Text editing

`TextEdit` paints its own frame and text, so pages render identically whether
or not an editor is open. Clicking a field (or pressing Enter while it is
focused, or right-clicking it) asks the host for a native borderless `EDIT`
child aligned to the widget rectangle, with the theme font, background, cue
banner and optional password/right-aligned styles — unless the field was built
with `.painted()`, which edits in place. `NumberInput` and `SearchBox` are
always painted sessions (D91): they own their frame and decorations and embed
the painted field without its own chrome, so no native child is requested for
them.

The widget never sees Win32: `Ui` forwards an `EditRequest` through `UiHost`
(implemented by `WindowHandle`), and the application receives callbacks:

| `Host` hook | When | Typical use |
|---|---|---|
| `edit_changed(id, text, window)` | every keystroke (`EN_CHANGE`) | live search |
| `edit_committed(id, text, window)` | focus left, Enter, Tab | `ui.commit_edit(id, text, window)` |
| `edit_canceled(id, window)` | Escape | repaint the widget |
| `ime_compose(text, cursor, window)` | IME composition changed | `ui.ime_compose(text, cursor, window)` |
| `ime_commit(text, window)` | IME produced a result string | `ui.ime_commit(text, window)` |

```rust
fn edit_committed(&mut self, id: WidgetId, text: &str, window: &mut WindowHandle<'_>) {
    if let Some(message) = self.ui.commit_edit(id, text.to_string(), window) {
        self.apply(message, window);
    }
}
```

`Ui::commit_edit` stores the text in the widget, repaints it and returns the
widget's message (`TextEdit`: the new string; `NumberInput`: the clamped
integer, only when it changed). `ui.sync_edit(window)` re-aligns an open editor
after layout; call it at the end of `Host::resize`. Layout and DPI changes are
the only places that need it — a DPI change also recreates the editor font.

While no native editor is open, `TextEdit` still owns a caret model (character
offsets, an optional selection anchor), so typed access can select and replace
text without a session. A keyboard/a11y-focused closed field paints the caret
and selection; pointer focus and an open native session paint neither (the
child `EDIT` owns the caret then), so right-clicking a field cannot leave a
second caret stripe behind.

```rust
ui.edit_widget::<TextEdit<Msg>>(search, |field| {
    field.select_all();
    field.insert_text("竹")
});
self.ui.flush_damage(window);
```

Programmatic edits do not emit a message; call `window.a11y_refresh()` when a
screen reader must see the new selection (see the TextPattern note below).

Behaviour details:

- One session at a time; opening another commits the first.
- Tab commits and then hands focus navigation back to the application (a
  synthetic `KeyCode::Tab` event), so `Ui` moves the ring to the next widget.
- `Esc` discards the edit; the widget keeps its previous text.
- Typing goes to the native control, so `WM_CHAR` never reaches `Host::event`
  for that field; painted fields receive characters through `Ui::char` (see
  “Framework-painted editor”).
- The control is a text-height strip centered vertically in the field (the
  classic `EDIT` class top-aligns its text), so the caret and selection sit in
  the middle. Clicks anywhere in the field — including the padding above and
  below the strip — are forwarded to the editor so the caret lands under the
  pointer; a click outside the field commits.
- The field shows the I-beam cursor over the frame and over the native control
  alike: `WM_SETCURSOR` maps the stored shape only to this window's client
  area and defers to `DefWindowProc` for the child, which keeps the EDIT class
  cursor.
- The control is destroyed after its `WM_KILLFOCUS` handler returns, so it is
  hidden first and cleaned up from a posted message.

### Framework-painted editor (`.painted()`)

A text field can edit in place with framework painting instead of hosting the
native child. This removes the closed/open seam (the field keeps one visual
state), allows single-line text to be centered vertically, and makes every
editor color and measurement configurable:

```rust
TextEdit::new("name", "", Msg::Name)
    .painted()
    .font_size(14.0)
    .max_length(40)
    .cue("输入名称")
    .colors(
        EditColors::new()
            .text(Color::new(58, 46, 24))
            .background(Color::new(255, 250, 235))
            .selection(Color::new(255, 255, 255), Color::new(64, 120, 220))
            .caret(Color::new(210, 60, 60))
            .border(Color::new(226, 168, 58))      // open session
            .border_idle(Color::new(208, 198, 170)) // closed
            .composition_underline(Color::new(120, 90, 30)),
    )
    .caret_width(1.0)               // default: the system caret width
    .padding(Insets::xy(9.0, 2.0))  // default (9,2) / (9,6) multiline
    .line_spacing(2.0)
```

`EditColors` fields are all optional; unset entries come from the theme. The
selection text color defaults to a contrast color for the selection background.
An open session paints the active border plus a second inner ring and a faint
accent tint, and the caret blinks; a focused-but-closed field uses
`border_idle` and paints no caret, so the two states are distinguishable from
the keyboard and from assistive technology. The caret width follows the
Windows accessibility setting (`SPI_GETCARETWIDTH`, in DIPs) and
`TextEdit::caret_width` overrides it.

Input and behavior:

- Click/press opens the session and moves the caret; drag selects; double-click
  selects a word. Right-click only focuses and opens the edit menu — it never
  moves the caret or clears a selection.
- Dragging a selection past the content edge auto-scrolls (faster the farther
  the pointer is outside); the wheel and scrollbar dragging scroll too.
- The caret reveal is persistent: moving or typing scrolls the single-line
  view so the caret is fully inside the content (with a small right margin),
  and the offset survives later repaints (a caret blink, a hover, or a
  keystroke rejected by `max_length` no longer jumps the text back to the
  start). The multiline line offset works the same way.
- Enter commits a single-line field, inserts a newline in a multiline one;
  Tab commits and moves focus; Escape reverts the draft; focus loss commits.
- Ctrl+A/C/X/V and Ctrl+Z/Y (`Ctrl+Shift+Z` also redoes). Typed runs coalesce
  into one undo step; `max_length` applies to typed, pasted and IME text.
- Single-line fields scroll horizontally and honor `.align(Center/Right)`;
  multiline fields wrap only when `.wrap(true)` and stay left-aligned. The cue
  is painted on the first line's rectangle (top-aligned for a multiline field,
  vertically centered for a single line, or `.vertical_center(false)`), so it
  sits where the caret will be instead of in the middle of a tall field.
- `.read_only(true)` (or `WidgetProps::read_only`) only blocks edits: the
  session still opens, the caret moves and blinks, pointer/Shift selection and
  Copy work, and only typing, IME, paste, cut, delete and undo are refused.
  The built-in edit menu disables those items and the a11y element reports
  read-only; programmatic setters still change the value.
- The observers (`on_change`, `on_char`, the three click hooks and
  `on_key_down`/`on_key_up`) run before the built-in action, which still
  executes. They return `Option<M>`; `None` stays silent and at most one
  message is delivered per input (an earlier observer wins over `on_change`;
  `on_char` consumes the change report of the same keystroke). `on_change`
  covers both key/character input and the direct insertions the container
  performs for a painted field (Ctrl+V/menu paste, IME commit, cut/delete/undo
  from the built-in menu). A native `EDIT` session owns its own keys and
  releases, so those observers see nothing there.
- Key releases reach the field through `Event::KeyUp` → `Ui::key_up`; a native
  `EDIT` session owns its own releases.
- Touch input switches the painted field to its touch model (D121): a
  `Ui::long_press` selects the word under the finger and paints the two
  selection handles, which can be dragged to adjust that edge; a touch press
  also raises the shell keyboard through the host. A mouse press goes back to
  the desktop model.
- A painted field is a text drop target: `insert_dropped_text` (used by
  `Ui::set_text_drop_target`) opens a session, places the caret at the release
  point and inserts the dropped text as a user edit. `take_drag_text` supplies
  the selection when the container starts a text drag (D122).
- The drag/caret geometry uses the same measured layout as the paint, so text
  input in the padding lands on the nearest character.

### Composite editors use the same core

`Table` cells, `TreeView` renames, `PropertyGrid` text/number values,
`SearchBox` and `NumberInput` are a painted `TextEdit` embedded behind
`widgets::inline::PaintedField` (D90/D91): the
same buffer, layout cache, caret/selection, word navigation, undo/redo,
clipboard, built-in edit menu, inline IME, caret reveal and caret rectangle as
a standalone field. The composite opens it over the cell/row/value rectangle,
forwards input and applies the committed text through its own callback
(`on_cell_edit`, `on_rename`, `on_change`). Commit contract: single-line →
Enter/Tab/blur commit, Escape reverts; multiline (a value containing a
newline) → Enter inserts a newline, Ctrl+Enter/Tab/blur commit, Escape
reverts; the cell height stays application-owned. While a session is open the
editing cell/row (and every painted field's own a11y value) publishes the live
text to accessibility (a `SetValue` on a table cell closes the session first).
A widget that draws its own frame and decorations passes `chrome = false` to
`PaintedField::begin_styled` (the search box keeps its magnifier/clear, the
number input its prefix/suffix/steppers) and re-paints the field over its own
background.

The window host owns IME for painted fields: it hides the system composition
window, draws the composition inline with an underline, and positions the
candidate list and composition font at the caret. The application must forward
two callbacks (the showcase does):

```rust
fn ime_compose(&mut self, text: &str, cursor: usize, window: &mut WindowHandle<'_>) {
    self.ui.ime_compose(text, cursor, window);
}
fn ime_commit(&mut self, text: &str, window: &mut WindowHandle<'_>) {
    self.ui.ime_commit(text, window);
}
```

Typed characters reach the field through `Ui::char`: forward `Event::Char` from
`Host::event` (the native `EDIT` child receives its own `WM_CHAR` and never
produces this event). Clipboard access is part of `UiHost`
(`clipboard_text` / `copy_to_clipboard`); `WindowHandle` implements both.
Paste is read through that seam, copy/cut write to it.

The built-in context menu follows the native `EDIT` menu's shape: Undo (U),
Redo (R), Cut (T), Copy (C), Paste (P), Delete (D), Select All (A) and an
"插入 Unicode 控制字符(I)" submenu (LRM/RLM/ZWSP/ZWJ/ZWNJ/BOM), all using the
reserved ids in `bamboo_native_ui::widget::edit_command`. Items that cannot
apply are disabled (undo/redo by history, cut/copy/delete without a selection)
and Paste is enabled only when the host clipboard holds text — `Ui` queries
`UiHost::clipboard_text` when it builds the menu, because the widget cannot
reach the clipboard itself. The labels carry real `&` access keys (`撤销(&U)`),
so the shell displays them as `撤销(U)` and activates the item when the letter
is pressed while the popup is open; RTL reading order and the IME items are not
available (they need bidi layout and an HWND-based IMM session).
`Ui` shows the menu only when the application installed no menu of its own, and
the application forwards a selected id before handling its own commands:

```rust
fn menu_command(&mut self, widget: WidgetId, command: u32, window: &mut WindowHandle<'_>) {
    if self.ui.menu_command(widget, command, window) {
        self.drain(window);
        return;
    }
    // ... application commands
}
```

Accessibility stays live while editing: the UIA `Edit` node keeps its value and
`TextPattern` selection in sync, and Invoke on the field opens the session
without a pointer.

## Accessibility (UI Automation)

`Ui` flattens the widget tree into a platform-neutral model and the window
host exposes it to UI Automation on `WM_GETOBJECT` (`UiaRootObjectId` and
`OBJID_CLIENT`).

| Widget | Role | Pattern |
|---|---|---|
| `Button` | Button | Invoke |
| `Toggle` | CheckBox | Toggle (on/off) |
| `Slider` | Slider | RangeValue (min/max/value, SetValue) |
| `TextEdit` / `NumberInput` | Edit | Value (text/number, SetValue), Text |
| `Segmented` | Group | Selection + children: RadioButton + SelectionItem (per-option `IsEnabled`) |
| `Tabs` | Tab | Selection + children: TabItem + SelectionItem |
| `ListView` | List | Selection + children: visible ListItem + SelectionItem; Scroll while the rows overflow (D67) |
| `Table` | Grid | Selection + children: one HeaderItem per column, realized GridCell + SelectionItem, IGridItemProvider (row/column) and Value (`SetValue` commits through `on_cell_edit`); IGridProvider extent and `GetItem`; Scroll while the body or the columns overflow (D63/D84) |
| `TreeView` | Tree | Selection + children: visible TreeItem + SelectionItem and ExpandCollapse (expandable rows only; a leaf reports `LeafNode`); Scroll while the rows overflow (D81) |
| `CoverWall` | List | Selection + children: visible cover ListItem (title — artist) + SelectionItem; Scroll while the grid overflows (D67) |
| `WaveformSeekBar` | Slider | RangeValue (0..=1, `SetValue` seeks; a `%` suffix is accepted) |
| `PlaybackTransport` | ToolBar | children: shuffle/repeat are CheckBox + Toggle (On/Off state), previous/play/next are Button + Invoke (play renames 播放/暂停) |
| `PlaylistView` | List | Selection + children: visible playlist-row ListItem (title — artist) + SelectionItem; Scroll while the tracks overflow (D67) |
| `Pager` | Button | Invoke (next page) |
| `SectionTitle` | Heading (Text) | — (heading level 2) |
| `Card`, `StatCard` | Group / Text | — |
| `EmptyState`, `Toast`, `ColorSwatch` | Text | — |
| `Label`, `Icon`, `Badge`, `Divider`, `Avatar`, `Kbd`, `CopyableText`, `MarqueeText`, `RichText` | Text | — (display-only until the instance opts in; a link label, removable badge, copyable text or linked rich text is interactive and therefore exposed) |
| `Checkbox` | CheckBox | Toggle |
| `RadioButton`, `RadioGroup` children | RadioButton | SelectionItem |
| `RadioGroup` | Group | Selection (single, one selected child) |
| `SearchBox`, `PasswordBox` | Edit | Value, Text |
| `ComboBox` | ComboBox | Value (selected item; SetValue selects by text) |
| `DatePicker`, `TimePicker`, `ColorPicker` | Button | Value (formatted value) + ExpandCollapse (open state) |
| `ProgressRing` | ProgressBar | RangeValue (0..=1) |
| `Banner` | Group | — (name = title, description = body) |
| `Spinner`, `TooltipCard` | Text | — |
| `MultiSelectList` | List | Selection (`CanSelectMultiple`, no required selection) + `ListItem` children; Scroll while the rows overflow (D67) |
| `Accordion`, `Breadcrumb`, `NavRail` | List | Selection + `ListItem` children (the open section / current crumb / selected row is `IsSelected`; SelectionItem selects/toggles) |
| `Popover` | Group | — (a named card; its content widgets are sibling nodes in the tree) |
| `Ribbon` | Tab | `TabItem` headers (one selected) + a `Button` child per command on the selected tab (checked/enabled state) |
| `GroupBox` | Group | — (display-only until exposed) |
| `Splitter` | Separator | — |
| `Sparkline` | Text | — (display-only until exposed) |
| `Gauge` | ProgressBar | RangeValue (display-only until exposed) |
| `RatingStars` | Slider | RangeValue (0..=stars; `SetValue` sets the rating) |
| `PropertyGrid` | List | Selection + `ListItem` children for visible rows (name `key: value`, cursor selected, locked rows disabled, headers as Text); Scroll while the sections overflow. Selecting a row never edits it (UIA has no Invoke on a `ListItem`), but editable rows publish a Value and `SetValue` commits through `on_change` (D103) |
| `ProgressBar` | ProgressBar | RangeValue (0..=1) |
| `MenuButton`, `SplitButton` | Menu | Invoke (asks for the popup) + ExpandCollapse; `MenuItem` children for the entries (checked state, Invoke runs the command) |
| `MenuBar` | MenuBar | ExpandCollapse + `MenuItem` children: top-level items (cursor item selected) and the cursor menu's commands |
| `Toolbar` | ToolBar | `Button` children for the visible cells and `更多`, `MenuItem` children for the hidden overflow commands |
| `StatusBar` | StatusBar | `Text` children, one per segment, plus a `ProgressBar` child for the progress segment (display-only until exposed) |

`A11yChild` carries a per-child `checked` state (menu items), a per-child
`expanded` state (expandable tree rows, `None` for leaves) and an optional
role override, so a toolbar publishes its own buttons and its overflow commands
with the roles a screen reader expects. `A11yInfo::expanded` models an
expandable element (a menu, a menu bar); `Widget::a11y_expand(child, expanded)`
receives the published row index, so one seam serves menus and tree rows.

`A11yInfo::scroll` publishes scroll geometry for the Scroll pattern:
`A11yScroll { horizontal, vertical: Option<A11yScrollAxis> }`, each axis
holding `offset`, `viewport` and `content` in DIPs (`percent()`,
`view_percent()` and `target()` do the percentage math). `ListView`,
`MultiSelectList`, `CoverWall`, `TreeView` and `PropertyGrid` publish their
vertical axis, `Table` both axes (the header follows the horizontal offset but
not the vertical), and a named `ScrollView` pane both axes. The UIA
provider answers `UIA_ScrollPatternId` (and the six scroll property ids) while
an axis overflows, with these rules (D67): a small step is one wheel notch
(48 DIP), a large step 90 % of the viewport, `NoAmount` leaves an axis alone, a
request at the end of the range is a successful no-op, and
`SetScrollPercent` accepts `0..=100` where `-1` (`NoScroll`) means "leave this
axis". A pattern call posts `A11yActionKind::Scroll { horizontal, vertical }`
(absolute DIP offsets) to `Ui::apply_a11y`; the scroll view moves its own
region, and any other widget clamps the request in
`Widget::a11y_scroll(&mut self, horizontal, vertical) -> bool`. Text fields keep
`TextPattern` instead.

Display-only widgets stay out of the tree by default; an instance opts in with
`focusable(true)`, `mouse(true)`, `expose(true)`, narration or a role override
(D27). Once exposed, its role comes from the widget type and can be overridden
with `role(...)`. Only the rows in the list viewport are published, so a
600-row queue stays cheap; a published row keeps its absolute row index
(`A11yChild::index`), so selection and runtime ids still point at the right row
after scrolling. `A11yNode::automation_id` becomes the UIA `AutomationId`;
`A11yNode::description` is exposed as `HelpText`/`FullDescription`.

Application side:

```rust
fn a11y_nodes(&mut self) -> Vec<A11yNode> { self.ui.a11y_nodes() }

fn a11y_action(&mut self, action: A11yAction, window: &mut WindowHandle<'_>) {
    if let Some(message) = self.ui.apply_a11y(&action, window) {
        self.apply(message, window);
    }
}
```

- The provider is a **snapshot**. `WindowHandle::a11y_focus(id)` marks it stale
  and, after the current callback returns, the host rebuilds it in place and
  raises `UIA_AutomationFocusChangedEventId`. Call it after input handling so a
  screen reader follows the keyboard. Repeated calls for the same element are
  deduplicated.
- State changes raise UI Automation notifications after the snapshot refresh:
  `ToggleState` for switches, `RangeValue.Value` for sliders, `Value.Value` for
  edit/number fields, `Name` for renamed controls, and
  `SelectionItem.IsSelected` + `ElementSelected` for a newly selected option or
  row. A screen reader announces a flipped switch or a moved slider from these
  events; the focus event alone does not change when the same element flips.
- A focused composite control (`Segmented`, `Tabs`, `ListView`) publishes
  `HasKeyboardFocus` on its **selected option/row**, not on the container, and
  the focus event is raised for that item. This is what makes Narrator read the
  tab or row and follow arrow-driven selection changes instead of announcing
  only "contains N items". The container's `GetFocus` resolves to the same item.
  Only the viewport rows are ever published; there is no full-list read. A list
  publishes one look-ahead row past the viewport, which reports
  `IsOffscreen=true` with bounds clipped to the viewport.
- `WindowHandle::a11y_refresh()` marks bounds/names stale after a layout or
  page change.
- `WindowHandle::a11y_announce("已加入队列")` raises a UIA notification on the
  root provider (no focus move) for toasts and finished background work.
  `a11y_announce_with(text, AnnouncementPriority::Assertive)` interrupts the
  current utterance (errors); `Polite` (the default) replaces older status
  announcements. Dropped when no client has requested the tree.
- Pattern calls are posted back to the window (`WM_APP_A11Y_ACTION`) and run on
  the UI thread through `Ui::apply_a11y` → the same widget input a user
  produces. The request carries the element's widget id plus option/row index,
  not a snapshot index, so a tree refresh between the client's walk and the
  call cannot redirect the action. `Invoke` on a text field focuses it and
  opens the native editor.
- While a native `EDIT` is open it is a real child window, so screen readers
  use its own UIA implementation for text editing.
- Every element has a runtime id built from `UiaAppendRuntimeId`, a stable hash
  of the widget id and the option/row index; it survives snapshot refreshes
  and list scrolling.
- Closed edit fields expose `TextPattern`: the document range returns the
  widget's text and its screen rectangle, `FindText` searches it, and movement
  is a no-op (`ExpandToEnclosingUnit` covers the whole value). A closed
  `TextEdit` also publishes its caret/selection: `GetSelection` returns one
  range (degenerate at the caret when nothing is selected) and
  `SupportedTextSelection` is `Single`; `NumberInput` reports `None`. Selection
  offsets are byte offsets into the published value (the mask for passwords).
- `cargo run -p bamboo-native-ui-showcase -- --a11y-dump` is the automated
  client check: a worker thread connects through `CUIAutomation`, walks the
  control view, asserts names/roles/patterns/values/selection/text, invokes the
  primary button through UIA, waits for the label to change and for the toast
  notification to reach the client, then verifies focus routing
  (focus lands on the selected tab and follows a `SelectionItem.Select` on a
  list row; the check reads `HasKeyboardFocus` from this window's subtree, not
  the global foreground focus), asserts the ToggleState property event and the
  `ElementSelected` selection event reach a client, and checks that narrated
  row names, the exposed heading description, the clickable swatch and the
  progress bar range all reach the client, before printing the tree.

## Layout skin (M7, `layout-xml`)

Opt-in: `bamboo-native-ui = { path = "native-ui/crates/native-ui", features = ["layout-xml"] }`.
The default build links no XML code and no `quick-xml`.

A skin is an XML-syntax document with the `.buixml` extension. It assigns
rectangles, runtime properties, style tokens and static content initial values
to widget ids the application already created; it never creates, removes or
reorders widgets. Contract: `docs/M7-XML.md`; decisions D123–D130. A
third-party widget's extension guide is `docs/CUSTOM-WIDGETS.md` (the six
`skin_*` methods below — no library change needed).

```rust
use bamboo_native_ui::layout_xml::LayoutDocument;

let text = std::fs::read_to_string("layouts/layout-demo.buixml")?;
let document = LayoutDocument::parse(&text)?; // pure; Err = Vec<LayoutDiagnostic>
let report = ui.apply_layout(&document, &palette, client_size, window)?;
// report.generation / variant_id / applied_entries / content_writes / warnings
```

- `LayoutDocument::parse` — parse and validate; any error rejects the whole
  file. `generation()` is the FNV-1a 64 of the canonical model text;
  `variants()`, `select(client)`, `to_xml()` (normalized, byte-stable) and
  `constants()`.
- `LayoutVariant` — `id()`, `minimums()`, `entries()`, `scrolls()`.
- `LayoutEntry` / `ScrollEntry` — `id()`, `line()`, `path()`,
  `resolve(LayoutSpace)` (so a test or the debug form can resolve without a
  `Ui`). `LayoutSpace::client(size, constants)` /
  `LayoutSpace::content(width, height, constants)`.
- `Ui::apply_layout(doc, palette, client, sink)` — resolves and validates every
  id, rectangle and expression before mutating; returns `LayoutReport` or
  `LayoutFailure` (in which case nothing changed). Content is written only when
  the generation changes; one damage union is flushed.
- `Ui::clear_layout(sink)` — restores every claimed field. Content and
  rectangles are not restored (they are the application's).
- `Ui::installed_layout()` — `AppliedLayout`: `generation()`, `client()`,
  `variant_id()`, `entries()` (`AppliedEntry { id, rect, space }`), `rect(id)`,
  `claimed_ids()`, `baseline_rect(id)`, `content_writes()`, `skin_writes()`,
  `warnings()`.
- `LayoutEntry::skin_attrs()` / `skin_children()` — the raw local attributes
  and data child elements of one entry (`SkinElement { name, attrs, children,
  line, column }`).

### Compiled mode (`layout-compiled`, D128)

Opt-in: `bamboo-native-ui = { …, features = ["layout-compiled"] }`. The build
links no XML parser and no `quick-xml`; the document is Rust the compiler
checks. The dynamic and compiled modes can be enabled together and share every
claim, content and skin rule.

```powershell
# generate once; check the file in and include! it (or run it from a build script)
cargo run -p bamboo-native-ui-layout-codegen -- layouts/layout-demo.buixml -o src/layout_demo.rs
# with a layout context: `install(.., context: &AppLayout)`
cargo run -p bamboo-native-ui-layout-codegen -- layouts/main.buixml --context AppLayout -o src/main.rs
```

```rust
include!("layout_demo.rs"); // defines `pub mod layout_demo`
let report = layout_demo::install(&mut ui, sink, &palette, client)?;
// or: ui.apply_compiled(&layout_demo::LAYOUT, &palette, client, sink)?
// with a context: layout_demo::install(&mut ui, sink, &palette, client, &app)
```

Or expand it at compile time (no checked-in file):

```rust
bamboo_native_ui_layout_macros::layout_xml!("layouts/main.buixml");
// or: layout_xml!("layouts/main.buixml", module = app_layout);
// or: layout_xml!("layouts/main.buixml", context = AppLayout);
```

The macro reads the path from `CARGO_MANIFEST_DIR`, generates the same module
and expands it in place; a document error becomes a `compile_error!` with the
`.buixml` line and column. Stable proc macros cannot register the file as a
dependency, so use the CLI/build script when the document changes without a
rebuild (the codegen golden test is the CI check).

The result is a `LayoutReport` / `LayoutFailure` exactly like `apply_layout`:
every target id is validated before anything mutates, claims capture/restore the
same way, content and local skin data are written only on a generation change,
and one damage union is flushed. Geometry is folded into native `f64`
expressions and the same four `f32` edge casts, so rectangles are bit-identical
to the dynamic path (`tests/layout_parity.rs` asserts it; the golden test in
`crates/layout-codegen` keeps the checked-in module current).

Generated vocabulary (stable; the generator's output shape may grow but these
types are the contract):

- `CompiledLayout { name, generation, variants, targets }` with
  `select(client)` (the same breakpoint rule as `LayoutDocument::select`).
- `CompiledVariant { id, min_width, min_height, entries, scrolls }`.
- `CompiledEntry { id, line, path, type_name, command, rect: RectFn, props,
  style, content, skin_attrs, skin_children, rust_children }`: `props`/`style` are
  `&'static LazyLock<…>` (shared empties when nothing is declared), so an apply
  allocates nothing for them; `content` is `&'static [CompiledContent]` and
  `skin_children` is `&'static [CompiledElement]`, materialized only when a
  generation writes; `rust_children` is a `<rust>` binding evaluated once
  (`None` when the module has a context).
- `CompiledScroll { id, line, path, rect, height: CompiledHeight }` with
  `Keep` / `Exact(fn(Size) -> Option<f32>)` / `Auto`.
- `RectFn = fn(Size) -> Result<Rect, RectError>`; the helpers `rect` / `edges`
  build the f64 geometry. `RectError::{Negative, NotFinite}` becomes
  `xml.geom.negative` / `xml.geom.non-finite` with the entry's `.buixml`
  line and path; `RectError::Context` (`xml.rust.context`) names a rectangle
  that exists only through the context `install`.
- `CompiledValues { variants }` + `CompiledEntryValues { id, rect, props,
  style, content, children }` — the per-apply values a `context = Type` module
  computes (a `Some` replaces that descriptor field). `CompiledValues::default()`
  is the no-context case.
- `CompiledTarget { id, kind: Widget|Scroll, line, path }` — the preflight
  list (an id that is missing or has the wrong kind rejects atomically).
- `CompiledContent` (text/label/value/progress/number/checked/selected) and
  `CompiledElement` + `elements()` for data child lists.

`Ui::apply_compiled(&CompiledLayout, palette, client, sink)` is the runtime
entry; `Ui::apply_compiled_with(&CompiledLayout, &CompiledValues, palette,
client, sink)` takes the context-computed values (the module's `install` calls
it). `Ui::clear_layout` hands claims back in either mode.

**Inline Rust:** a schema scalar value may be `rust:`-prefixed (`x="rust:PAGE_PAD"`,
`label="rust:TITLE"`, `visible="rust:!cfg!(test)"`, `radius="rust:RADIUS"`). The
generator splices the text verbatim: geometry inside the rectangle function with
`cw`/`ch`/`cx`/`cy` as `f64` locals, properties/style in their `LazyLock`
statics, content in the `CompiledContent` table; the generated module starts
with `use super::*`, so the including module's items resolve by name. `rustc`
type-checks the values. The same document still parses in the dynamic mode, but
`apply_layout` refuses it atomically with `xml.rust.unsupported` — a runtime
file is data, never code. Widget-local `x-*` values are not spliced.

**Layout context:** `context = Type` (CLI `--context Type`) makes the generated
`install` take `context: &Type`, and inline Rust may read it — in geometry (a
captured closure per entry, so the rectangle follows the context and the client
size), in properties, style and content (built per apply instead of in a
`LazyLock`). The static `LAYOUT` descriptor keeps the literals; an entry whose
rectangle reads the context reports `RectError::Context` if you call
`apply_compiled` directly. Static properties stay static, so a plain document
still allocates nothing.

**Data bindings:** a `<rust target="id" code="expr"/>` element (a child of
`<layout>`) replaces one widget's declared child list with the `Vec<SkinElement>`
the expression returns. It goes through the same validation and write as
`<item>`/`<row>` elements (children → content → attributes, once per
generation). Bindings are variant-invariant, and a widget has either a static
child list or a binding. Without a context the expression is evaluated once into
a static; with one it is evaluated per apply.

```xml
<layout id="base">
  <widget id="queue" x="0" y="0" w="320" h="240"/>
  <rust target="queue" code="context.rows()"/>
</layout>
```

### Document-created widgets (`crate::creation`, M7.7/D148)

A document can create a widget instead of addressing one the application built:

```xml
<widget id="save" type="button" command="101" x="12" y="8" w="96" h="32" label="保存"/>
```

`type` names a registry type; `command` (a `u32`, only with `type`) is the
application command an interactive built-in reports; `parent` names a scroll
panel the created widget joins (its `x`/`y` are then content coordinates). The
id is created when it does not exist, updated in place while the document stays
the owner, and removed when a reload no longer declares it (or its
`type`/`command`/`parent` changed) — `Ui::clear_layout` removes every created
widget as well.

```rust
use bamboo_native_ui::creation::WidgetRegistry;

let mut registry = WidgetRegistry::<Msg>::standard();
registry.set_commands(|command| match command {
    101 => Some(Msg::Save),
    _ => None,
});
registry.register("vendor.meter", |spec, ctx| { /* Box<dyn Widget<Msg>> */ });
ui.set_widget_registry(registry);
ui.apply_layout(&document, &palette, client, sink)?; // or apply_compiled / install
```

- `WidgetRegistry<M>`: `new` (empty), `standard` (the twelve built-ins:
  `label`, `button`, `toggle`, `slider`, `progress`, `icon`, `badge`, `card`,
  `divider`, `kbd`, `avatar`, `spinner`), `register(type_name, factory)`,
  `set_commands(map)`, `has_type`, `type_names`, `has_commands`. `standard`
  needs `M: Clone` because `toggle`/`slider` report their command on every
  change (read the new state through the typed accessors, D149).
- `WidgetSpec { id, rect, content, attrs, children, command, line, path }` with
  `label()` / `attr(name)` helpers; `FactoryCtx { palette, client, commands }`
  with `ctx.message(command)`.
- `FactoryError::{Missing(field), Rejected(text)}` → `xml.command.missing` /
  `xml.type.create`. A factory must build the entry's id.
- `Ui::set_widget_registry` / `widget_registry` / `widget_registry_mut` /
  `clear_widget_registry`; `Ui::add_boxed`/`insert_boxed` insert a
  `Box<dyn Widget<M>>` (what a factory returns).
- Created widgets are validated, claimed and written exactly like addressed
  ones (the factory only supplies the constructor; content/attributes/surface
  come from the file) and are inserted in the page layer — before the first
  backdrop widget. A `parent` panel membership is appended to the panel's child
  list and positioned in content coordinates; without `parent` the widget is
  client-space. A document reorder restacks the created block by declaration
  order on the next generation.
  `Ui::set_visible` on an id that does not exist yet is remembered, so a page
  pass that hides the page keeps the document's future widgets hidden.
- Every variant must declare every created id with the same `type`/`command`
  (`xml.type.conflict`); an existing id the document does not own is
  `xml.id.duplicate`; a missing registry/type is `xml.type.unknown`.
- `LayoutReport::created` counts the created widgets; `AppliedLayout::created()`
  / `created_ids()` list them (`AppliedCreation { id, type_name, command }`).

Additive `Widget` seams (defaults: `None`, `None`, `ContentCaps::NONE`):

```rust
fn surface_style(&self) -> Option<&SurfaceStyle>;
fn surface_style_mut(&mut self) -> Option<&mut SurfaceStyle>;
fn content_caps(&self) -> ContentCaps; // TEXT|LABEL|VALUE|PROGRESS|NUMBER|TOGGLED|SELECTION
```

A widget publishes which content attributes it accepts, so the applier warns
(`xml.content.unsupported`) instead of silently writing an unsupported value.

The widget-declared skin protocol (`crate::skin`, D127/D137, always compiled —
no feature flag) lets a widget accept local attributes and data child elements
without a registry or a `TypeId` table:

```rust
use bamboo_native_ui::{SkinAttrDecl, SkinElement, SkinValue, SkinValueKind, Widget};

static ATTRS: &[SkinAttrDecl] = &[SkinAttrDecl::new("x-level", SkinValueKind::Number)];
static CHILDREN: &[&str] = &["x-item"];

impl<M> Widget<M> for Meter {
    fn skin_attrs(&self) -> &'static [SkinAttrDecl] { ATTRS }
    fn validate_skin_attr(&self, name: &str, value: &SkinValue<'_>) -> bool {
        // Side-effect free: the applier validates everything first.
        matches!((name, value), ("x-level", SkinValue::Number(v)) if (0.0..=100.0).contains(v))
    }
    fn apply_skin_attr(&mut self, name: &str, value: &SkinValue<'_>) { /* ... */ }
    fn skin_children(&self) -> &'static [&'static str] { CHILDREN }
    fn validate_skin_children(&self, children: &[SkinElement]) -> bool { /* ... */ }
    fn apply_skin_children(&mut self, children: &[SkinElement]) { /* ... */ }
}
```

```xml
<widget id="meter" x="16" y="16" w="200" h="24" x-level="72" x-units="dB">
  <x-item label="低频" weight="3"/>
</widget>
```

Names beginning with `x-` belong to user code; the shared content names are
reserved (`xml.skin.reserved`), an undeclared name is `xml.skin.unknown`, a
value that fails its kind or the widget's `validate_*` is `xml.skin.value`, and
variants that disagree on one id's local data are `xml.skin.conflict`. Local
data is written only when the generation changes, child lists before the shared
content and attributes after it, with one damage mark per widget; an open edit
session skips it (`xml.content.session-open`). `Ui::clear_layout` does not
restore it.

### Built-in local scalars (M7.3)

The library's own widgets declare the state that has no shared name. Every name
below is checked by `skin_attrs()`, validated first and applied silently; a
reload of the same generation writes it once. `x-*` remains user code, and the
shared vocabulary (`text`/`label`/`value`/`progress`/`number`/`checked`/
`selected`) is still written through `content_caps`.

| Widget | Local attributes |
|---|---|
| `Card` | `title`, `body` (text), `emphasis` (bool) |
| `Label` | `color` (color), `text-style` (`body`/`small`/`title`) |
| `SectionTitle` | `color` (color) |
| `CopyableText` | `color` (color), `text-style` (`body`/`small`/`title`) |
| `Icon` | `glyph` (one character), `tint` (color) |
| `Badge` | `tone` (`neutral`/`accent`/`success`/`warning`/`danger`) |
| `Avatar` | `initials` (text), `tint` (color), `ring` (bool), `image` (a whole, non-negative numeric `ImageId`, the `<cover image>` convention) |
| `Kbd` | `shortcut` (text) |
| `ColorSwatch` | `color` (color) |
| `Toast` | `message` (text; does not change visibility), `error` (bool) |
| `Banner` | `body` (text) |
| `Spinner` | `phase` (degrees), `animating` (bool) |
| `TextEdit`, `SearchBox` | `cue` (text), `color` (color: the text, closed or editing) |
| `NumberInput` | `prefix`, `suffix` (text) |
| `Accordion` | `open` (section index) |
| `PlaybackTransport` | `playing`, `shuffle` (bool), `repeat` (`off`/`all`/`one`) |
| `MarqueeText` | `offset` (DIPs), `color` (color), `text-style` (`body`/`small`/`title`) |
| `RichText` | `color` (color: normal/strong runs; muted/link keep their roles) |
| `DatePicker` | `date` (`YYYY-MM-DD`), `open` (bool) |
| `TimePicker` | `time` (`HH:MM`), `open` (bool) |
| `ColorPicker` | `color` (color), `open` (bool) |

Notes: skin colors are **one value type**, `SkinValue::Color(Rgba)`. A
`Color`-kind attribute accepts, in one grammar (whitespace ignored anywhere):

- `#rgb` / `#rgba` / `#rrggbb` / `#rrggbbaa`
- decimal channels `r,g,b` / `r,g,b,a` (commas or spaces; 0..255, a bare
  fractional alpha like `0.5` is 0..1)
- `rgb(r,g,b)` / `rgba(r,g,b,a)` (commas or spaces; functional alpha follows
  CSS: 0..1, or 0..255 above 1)
- a palette token name (`accent`, `card`, `panel`, `text`, `muted`, `divider`,
  `background`, `button-bg`, …) resolved by the applier against the active
  palette

A widget with no transparency support reads the RGB part through
`SkinValue::as_color()` (the alpha is silently dropped); `ColorPicker` reads
`as_rgba()` and seeds both the default color and its alpha. The surface
`background` literal shares the same parser (alpha accepted, dropped — a fill
is opaque); `background="none"` is the transparent-fill spelling.
`RatingStars` takes the shared `value` (stars). `Avatar image` and `<cover
image>` both take the **numeric** `ImageId` the application assigned through
`Ui::set_image`; there is no page image-name registry (the ids are the
application's own keys). Local data is content, so it is never restored by
`clear_layout`.

Text colour and role (M7.8/D150) use the same silent-write rule: `Label`,
`SectionTitle`, `CopyableText`, `MarqueeText`, `RichText`, `TextEdit` and
`SearchBox` declare `color`, and the four label-styled widgets also declare
`text-style`. The setters are `Label::set_color`/`set_text_style`,
`SectionTitle::set_color`, `CopyableText::set_color`/`set_text_style`,
`MarqueeText::set_color`/`set_text_style`, `RichText::set_color`,
`TextEdit::set_text_color` and `SearchBox::set_color`; the getters are
`color_value()`, `text_style()` and `TextEdit::text_color_value()`. A widget
without transparency drops the alpha, and `RichText`'s muted/link runs keep
their palette roles.

### Built-in data child lists (M7.4)

Widgets that own a runtime data setter accept a child list. The list **replaces**
the data model (no widget is created), validates before mutation, and `selected`
(shared content) is applied after it.

```xml
<widget id="list" selected="1">
  <item title="夜曲" subtitle="周杰伦" trailing="3:58"/>
  <item title="晴天" current="true"/>
</widget>
<widget id="combo">
  <option label="低"/><option label="中"/>
</widget>
```

| Widget | Child | Attributes |
|---|---|---|
| `ListView` | `<item>` | `title` (required), `subtitle`, `trailing`, `current`, `disabled` |
| `PlaylistView` | `<track>` | same as `<item>` |
| `ComboBox` | `<option>` | `label` (required) |
| `CoverWall` | `<cover>` | `title` (required), `subtitle`, `image` (raw `ImageId` number), `corner-radius` |
| `StatusBar` | `<segment>` | `text` (required), `icon` (one character), `width`, `align` (`left`/`center`/`right`) |
| `Table` | `<column>` | `title` (required), `width`, `flex`, `align` |
| `Table` | `<row>` | `disabled`; nested `<cell text>` (`text` present, may be empty) |
| `TreeView` | `<node>` | `label` (required), `glyph` (one character), `secondary`, `expanded`, `disabled`; nested `<node>` |
| `PropertyGrid` | `<section>` | `title`; nested `<row>` |
| `PropertyGrid` | `<row>` | `key` (required), `value`, `kind` (`read-only`/`text`/`number`/`choice`/`check`/`rating`), `read-only`, and the kind's parameters: `min`/`max`/`step`/`decimals` (number), `options` (`|`-separated, choice), `on`/`off` (check), `stars` (rating) |
| `Pager` | `<page>` | `label` (required); the page count becomes the list length |
| `Tabs`, `Segmented` | `<option>` | `label` (required) |
| `Breadcrumb` | `<step>` | `label` (required) |
| `NavRail` | `<step>` | `label` (required), `icon` (one character) |
| `RichText` | `<run>` | `text` (required), `style` (`normal`/`strong`/`muted`/`link`), `link` (`u32`, link runs only) |
| `Sparkline` | `<point>` | `value` (required, finite number) |

An unknown child attribute is `xml.skin.value` (a typo guard), a parameter that
does not belong to the row's `kind` is one too, and a missing required
attribute rejects the whole document before mutation. A `Table` child list is
the complete table (columns and rows replace together). Construction-only
chrome (`Toolbar`, `Ribbon`, `MenuBar`) stays Rust-owned and is rejected as an
unknown child. Typed access for the tests: `ListView::rows`, `ComboBox::items`,
`StatusSegment::{icon_glyph,fixed_width,text_align}`, `Table::columns_ref`/
`rows_ref`, `TreeView::items_ref`, `PropertyGrid::sections_ref`/`row_by_key`,
`Pager::page_labels`, `Tabs::labels`/`Segmented::options`,
`Breadcrumb::items`/`NavRail::items`, `RichText::runs_ref` and
`Sparkline::data`.

### Appearance and interaction locals (M7.4 follow-up)

The data/list widgets publish their own appearance and interaction knobs
through the same `skin_attrs` protocol (M7.4/D144). **Event handlers are
deliberately not addressable from a file**: `editable`/`renameable`/
`draggable` only enable the widget-side interaction, and every callback is
still installed in Rust.

| Widget | Attributes |
|---|---|
| `Table` | `row-height`, `header-height`, `cell-padding`, `striped`, `grid-lines`, `editable`, `empty-text` |
| `TreeView` | `row-height`, `indent`, `renameable`, `draggable`, `empty-text` |
| `PropertyGrid` | `row-height`, `header-height`, `key-width`, `grid-lines`, `empty-text` |
| `ListView` | `row-height`, `alternating`, `empty-text` |
| `PlaylistView` | `row-height`, `alternating`, `empty-text` |
| `CoverWall` | `gap`, `corner-radius` (wall default), `caption-height`, `empty-text` |
| `NavRail` | `row-height` |
| `Tabs` | `label-align` (`left`/`center`/`right`) |
| `RichText` | `wrap`, `align` (`left`/`center`/`right`), `text-style` (`body`/`small`/`title`) |
| `Sparkline` | `color` (one color grammar, palette tokens included), `gap` |
| `Breadcrumb` | `separator` (exactly one character) |

Numbers are validated against the widget's real minimum (`row-height` 16 for
table/tree/property, 24 for the lists, 18 for the rail; `key-width` 40;
`indent` 8; others 0), so an undersized value is `xml.skin.value`, not a silent
clamp. `read-only` stays the shared property. The silent runtime setters mirror
the builders and keep derived state correct (a `PropertyGrid` rebuild keeps its
cursor key, lists re-clamp scroll/reveal the selection, a locked table closes
an open cell draft, `RichText`/`Breadcrumb` invalidate their caches); typed
getters are `*_value()`, `is_*()`, `empty_text_ref()`, `label_alignment()`,
`alignment()`, `text_style()`, `separator_char()` and `color_value()`.

### Theme documents (M7.8, `theme-xml`)

A `.buitheme` file declares a whole palette (D150; `docs/M7-XML.md` §18):

```xml
<theme version="1" name="夜蓝" base="dark" accent="#7aa2f7">
  <color name="background" value="#16181d"/>
  <color name="text" value="#e8ebee"/>
</theme>
```

- `base` is required (`light`/`dark`); `accent` and every `<color>` are
  optional. `<color name>` is a palette token in kebab-case, `<color value>` a
  literal from the one colour grammar (alpha dropped — palette colours are
  opaque); duplicates and unknown names are `theme.color.*` errors.
- `bamboo-native-ui` feature `theme-xml` re-exports the model and reader from
  `bamboo_native_ui::theme_xml` (`ThemeDocument`/`ThemeBase`/`ThemeColor`,
  `ThemeDocument::parse`/`to_xml`/`generation`). It enables only
  `layout-core/parse`; `layout-xml` is independent.
- `ThemeDocumentExt::palette(&ThemeSettings) -> Palette` derives
  `Palette::new(settings, base)`, feeds the document accent into that
  derivation and applies the `<color>` overrides in declaration order.
  `set_palette_token(&mut Palette, name, color)` is the public token write;
  read a token back through `Palette`'s fields.
- Apply it by swapping the palette: `WindowHandle::set_theme(settings, palette.dark)`
  (fonts/light-dark decision), `WindowHandle::set_palette(palette)` (whole table
  + one repaint; `WindowHandle::palette()` reads it back), then pass the same
  palette to `Ui::apply_layout` so the layout's colour tokens resolve against
  it. A colour token in a layout resolves against whatever palette the
  application passes; it is content-scoped skin data, not a claim, so loading a
  theme and re-applying the layout are independent steps.

### Custom components (Rust, no layout file)

A widget outside this crate implements `Widget<M>` and is registered with
`Ui::add` / `add_with` / `insert` — no registry, `TypeId` table or library
change. The seams an application widget shares with the built-ins:

- `CustomWidget<M>` — a closure-painted component: `CustomWidget::new(id,
  |ctx, rect, state| …)`, `.rect`, `.preferred_height`, `.focusable`,
  `.interactive`, `.enabled`, `.cursor`, `.input(|rect, input| Option<M>)`,
  `.hit_test`, `.a11y` / `.a11y_name` / `.a11y_fn`, and the surface builders.
  A non-default surface paints before the closure. `Ui::add_custom(id, rect,
  paint)` is the paint-only shorthand. It implements `Widget<M>`, so
  `WidgetProps`, `Ui::with_widget`/`edit_widget`, the focus ring and the
  layout skin's `SurfaceStyle` seam all apply.
- `Observers<M>` (re-exported at the crate root) plus the exported
  `observer_builders!(MyWidget)` / `observer_methods!()` macros: the same
  additive `.on_click` / `.on_right_click` / `.on_double_click` /
  `.on_key_down` / `.on_key_up` / `.on_focus_gained` / `.on_focus_lost`
  surface, for a struct with `observers: Observers<M>`.
- The exported `surface_builders!(MyWidget)` / `surface_builders_plain!(MyWidget)`
  macros: `.corner_radius` / `.corners` / `.background` /
  `.background_option` / `.transparent_background` / `.surface()`, for a struct
  with `surface: SurfaceStyle`. Forward `Widget::surface_style(_mut)` so the
  layout skin can style it.
- The six `skin_*` methods (above) for a layout-file vocabulary.

The macros use `$crate` paths, so they expand correctly in another crate.
Guide: `docs/CUSTOM-WIDGETS.md`; worked examples: `crates/showcase/src/meter.rs`
(full trait + skin) and the API page's `api_custom_draw` (`CustomWidget`).

A widget that does not adopt `SurfaceStyle` gets `xml.style.unsupported` for
`radius`/`corners`/`background`; `font-*` always works (it is container-side).

Diagnostics carry `severity`, a stable `code`, 1-based `line`/`column`, a
best-effort `path` (`layout/wide/m7_action`) and a message; `Display` prints
`line:column: error[code] (path): message`.

### Debug API (`debug-form`)

The library exposes read-only state; the application decides how to show it
(the showcase's `调试` menu formats values into ordinary information dialogs).
Everything lives in `bamboo_native_ui::debug` and is compiled only with
`debug-form` (which implies `layout-xml`), so the default build has no debug
code.

```rust
let snapshot = ui.debug_snapshot();
if let Some(layout) = &snapshot.layout {
    println!("{} gen {} has {} entries", layout.variant, layout.generation, layout.entries.len());
}
for widget in &snapshot.widgets {
    println!("{} focus={}", widget.id.0, widget.focused);
}
```

- `DebugSnapshot` — `layout` (`DebugLayout`: generation, variant, client,
  content/skin write counts, `entries` with id/rect/`EntrySpace`, `claims` with
  the baseline rect and claimed field names, `scrolls` with
  viewport/offset/content, warnings), `widgets` (`DebugWidget`: effective
  visible/enabled/focusable/read-only, focus/hover/press, overlay, scroll
  parent, automation id, `A11yInfo` with narration resolved into the
  name/description and child names, `skin_attrs`/`skin_children` vocabulary),
  `widget_types` (the installed registry's names in registration order; empty
  without one) and `has_commands`,
  `focus`/`hover`/`press` ids, `pending_damage` (waiting for
  `flush_damage`), `last_damage` (the last flushed pieces), `paint`
  (`PaintStats`: frames, last-frame widget count/micros, cumulative per-widget
  paints/micros longest first) and `log` (`DebugEvent` with sequence/kind/
  text).
- `Ui::debug_event(kind, text)` appends an application note to the bounded
  (128-entry) ring; `Ui::clear_debug_log()` empties it. The applier records one
  summary per apply, one event per warning and the first diagnostic of a failed
  apply; `clear_layout` records its own event.
- `Ui::hover_id()` joins `focus_id()`/`press_id()`.
- Paint timing is measured only in `debug-form` builds; `BAMBOO_UI_PAINT_LOG`
  keeps printing slow widgets in every build.

## Not yet available (see TODO.md)

Tracing-event capture into the debug ring (layout applies/warnings/failures are
recorded; the container's tracing uses the process subscriber), a schema-level
container (`<panel>`; a created widget can join an existing scroll panel
through `parent`, but there is no implicit container tree),
the construction-only chrome child lists
(`Toolbar`/`Ribbon`/`MenuBar` may stay Rust-owned; every data/label-owning list
is implemented, M7.4/D140-D142), an image-name registry (skin images take the
application's numeric `ImageId`), and a GPU-present renderer
(the `Canvas` seam is in place; D101). The scroll view has no edge fades or
background fill yet, but it publishes the UIA `Scroll` pattern (D67). A tree
row being renamed publishes an editable `Value` (the draft) and `SetValue`
commits the label like Enter does (D102); every other row is value-less and
read-only, and a screen reader cannot drive a property value editor through UIA
(the row is one ListItem, D78). Composite editors commit into a fixed row
height: a multiline value scrolls inside it rather than growing the row (D90).
