# Bamboo Native UI — widget lookup table

Compact index for AI assistants. `M` is the application message type. All types
are imported from `bamboo_native_ui::widgets`. "addr" means the widget is
created in Rust and addressed by `id` in a `.buixml` document (no `type=`).

## Content

| Widget | Constructor | Messages | XML |
|---|---|---|---|
| [`Label`](../widgets/content/label.md) | `Label::new(id, text)` | — | `type="label"`; `color`, `text-style` |
| [`Icon`](../widgets/content/icon.md) | `Icon::new(id, glyph: char)` | — | `type="icon"`; `glyph`, `tint` |
| [`Badge`](../widgets/content/badge.md) | `Badge::new(id, text)` | `.removable(Fn() -> M)` | `type="badge"`; `tone` |
| [`Divider`](../widgets/content/divider.md) | `Divider::new(id)` | — | `type="divider"` |
| [`Avatar`](../widgets/content/avatar.md) | `Avatar::new(id, name)` | — | `type="avatar"`; `initials`, `tint`, `ring`, `image` |
| [`Kbd`](../widgets/content/kbd.md) | `Kbd::new(id, text)` | — | `type="kbd"`; `shortcut` |
| [`CopyableText`](../widgets/content/copyable-text.md) | `CopyableText::new(id, text)` | `.copyable(Fn() -> M)` | addr |
| [`MarqueeText`](../widgets/content/marquee-text.md) | `MarqueeText::new(id, text)` | — (app calls `advance(dt)`) | addr |
| [`RichText`](../widgets/content/rich-text.md) | `RichText::new(id).run(..)` | `.on_link(Fn(id) -> M)` | addr; `<run>` |

## Input

| Widget | Constructor | Messages | XML |
|---|---|---|---|
| [`Checkbox`](../widgets/inputs/checkbox.md) | `Checkbox::new(id, label, checked, Fn(bool) -> M)` | `Fn(bool) -> M` | addr |
| [`RadioButton`](../widgets/inputs/radio-button.md) | `RadioButton::new(id, label, selected, Fn() -> M)` | `Fn() -> M` | addr |
| [`RadioGroup`](../widgets/inputs/radio-group.md) | `RadioGroup::new(id, &[&str], selected, Fn(usize) -> M)` | `Fn(usize) -> M` | addr |
| [`SearchBox`](../widgets/inputs/search-box.md) | `SearchBox::new(id, text, Fn(String) -> M)` | `Fn(String) -> M` | addr |
| [`TextEdit`](../widgets/inputs/text-edit.md) | `TextEdit::new(id, text, Fn(String) -> M)` | `Fn(String) -> M` + own `on_*` | addr; aliases `PasswordBox`, `TextArea` |
| [`NumberInput`](../widgets/inputs/number-input.md) | `NumberInput::new(id, i64, RangeInclusive<i64>, Fn(i64) -> M)` | `Fn(i64) -> M` | addr; alias `NumberStepper` |
| [`ComboBox`](../widgets/inputs/combo-box.md) | `ComboBox::new(id, &[&str], selected, Fn(usize) -> M)` | `Fn(usize) -> M` | addr; `<option>` |
| [`MultiSelectList`](../widgets/inputs/multi-select-list.md) | `MultiSelectList::new(id, &[&str], Fn(Vec<usize>) -> M)` | `Fn(Vec<usize>) -> M` | addr |

## Controls

| Widget | Constructor | Messages | XML |
|---|---|---|---|
| [`Button`](../widgets/controls/button.md) | `Button::new(id, label, M)` | `M` | `type="button"`, `command` required |
| [`Toggle`](../widgets/controls/toggle.md) | `Toggle::new(id, label, on, Fn(bool) -> M)` | `Fn(bool) -> M` | `type="toggle"`, `command` required; alias `Switch` |
| [`Segmented`](../widgets/controls/segmented.md) | `Segmented::new(id, &[&str], selected, Fn(usize) -> M)` | `Fn(usize) -> M` | addr; `<option>` |
| [`Tabs`](../widgets/controls/tabs.md) | `Tabs::new(id, &[&str], selected, Fn(usize) -> M)` | `Fn(usize) -> M`; `.closable(Fn(usize) -> M)` | addr; `<option>` |
| [`Slider`](../widgets/controls/slider.md) | `Slider::new(id, f32, RangeInclusive<f32>, Fn(f32) -> M)` | `Fn(f32) -> M`; drag callbacks | `type="slider"`, `command` required |
| [`ProgressBar`](../widgets/controls/progress-bar.md) | `ProgressBar::new(id, value: f32)` | — | `type="progress"` |
| [`Pager`](../widgets/controls/pager.md) | `Pager::new(id, page, pages, Fn(i32) -> M)` | `Fn(i32) -> M` (`-1`/`+1`) | addr; `<page>` |
| [`ColorSwatch`](../widgets/controls/color-swatch.md) | `ColorSwatch::new(id, Color, label)` | — | addr |

## Pickers

| Widget | Constructor | Messages | XML |
|---|---|---|---|
| [`ColorPicker`](../widgets/pickers/color-picker.md) | `ColorPicker::new(id, Color, Fn(Color) -> M)` / `new_rgba` | `Fn(Color) -> M`, `Fn(Rgba) -> M` | addr |
| [`DatePicker`](../widgets/pickers/date-picker.md) | `DatePicker::new(id, Date, Fn(Date) -> M)` | `Fn(Date) -> M` | addr |
| [`TimePicker`](../widgets/pickers/time-picker.md) | `TimePicker::new(id, Time, Fn(Time) -> M)` | `Fn(Time) -> M` | addr |

## Data

| Widget | Constructor | Messages | XML |
|---|---|---|---|
| [`Sparkline`](../widgets/data/sparkline.md) | `Sparkline::new(id, &[f32])` | — | addr; `<point value>` |
| [`Gauge`](../widgets/data/gauge.md) | `Gauge::new(id, value: f32)` | — | addr |
| [`RatingStars`](../widgets/data/rating-stars.md) | `RatingStars::new(id, stars, value, Fn(u8) -> M)` | `Fn(u8) -> M` | addr |
| [`ListView`](../widgets/data/list-view.md) | `ListView::new(id).rows(Vec<Row>)` | `.on_activate(Fn(usize) -> M)` | addr; `<item>` |
| [`Table`](../widgets/data/table.md) | `Table::new(id).columns(..).rows(..)` | `on_select/activate/header_click/column_resize/column_reorder/cell_edit` | addr; `<column>/<row>/<cell>` |
| [`TreeView`](../widgets/data/tree-view.md) | `TreeView::new(id).items(Vec<TreeItem>)` | `on_select/activate/toggle/rename/move/drag_*` | addr; `<node>` |
| [`PropertyGrid`](../widgets/data/property-grid.md) | `PropertyGrid::new(id).sections(Vec<PropertySection>)` | `on_select/activate/change` | addr; `<section>` |

## Navigation

| Widget | Constructor | Messages | XML |
|---|---|---|---|
| [`Accordion`](../widgets/navigation/accordion.md) | `Accordion::new(id, &[&str], Fn(usize, bool) -> M)` | `Fn(usize, bool) -> M` | addr; alias `Collapsible` |
| [`Breadcrumb`](../widgets/navigation/breadcrumb.md) | `Breadcrumb::new(id, &[&str], Fn(usize) -> M)` | `Fn(usize) -> M` | addr; `<step>` |
| [`NavRail`](../widgets/navigation/nav-rail.md) | `NavRail::new(id, &[&str], selected, Fn(usize) -> M)` | `Fn(usize) -> M` | addr; `<step>` |
| [`Splitter`](../widgets/navigation/splitter.md) | `Splitter::vertical/horizontal(id, Fn(f32) -> M)` | `Fn(f32) -> M` | addr |

## Feedback

| Widget | Constructor | Messages | XML |
|---|---|---|---|
| [`Spinner`](../widgets/feedback/spinner.md) | `Spinner::new(id)` | — (`advance(dt)`) | `type="spinner"` |
| [`ProgressRing`](../widgets/feedback/progress-ring.md) | `ProgressRing::new(id, value: f32)` | — | addr |
| [`Banner`](../widgets/feedback/banner.md) | `Banner::new(id, BannerKind, title)` | `.action(..)`, `.closable(..)` | addr; `body` |
| [`TooltipCard`](../widgets/feedback/tooltip-card.md) | `TooltipCard::new(id, title)` | — | addr |
| [`Scrim`](../widgets/feedback/scrim.md) | `Scrim::new(id)` | — | addr |
| [`Toast`](../widgets/feedback/toast.md) | `Toast::new(id)` + `.show(text, error)` | — (app timer) | addr; `message`, `error`; alias `Snackbar` |

## Display

| Widget | Constructor | Messages | XML |
|---|---|---|---|
| [`Card`](../widgets/display/card.md) | `Card::new(id, title, body)` | — | `type="card"`; `title`, `body`, `emphasis` |
| [`GroupBox`](../widgets/display/group-box.md) | `GroupBox::new(id, title)` | — | addr |
| [`StatCard`](../widgets/display/stat-card.md) | `StatCard::new(id, value, label)` | — | addr |
| [`EmptyState`](../widgets/display/empty-state.md) | `EmptyState::new(id, title, hint)` | — | addr |
| [`SectionTitle`](../widgets/display/section-title.md) | `SectionTitle::new(id, label)` | — | addr; `color` |

## Player

| Widget | Constructor | Messages | XML |
|---|---|---|---|
| [`WaveformSeekBar`](../widgets/player/waveform-seek-bar.md) | `WaveformSeekBar::new(id, progress, Fn(f32) -> M)` | seek + drag callbacks | addr |
| [`PlaybackTransport`](../widgets/player/playback-transport.md) | `PlaybackTransport::new(id, Fn(TransportAction) -> M)` | `TransportAction` | addr |
| [`CoverWall`](../widgets/player/cover-wall.md) | `CoverWall::new(id).covers(Vec<Cover>)` | `on_open`, `on_select` | addr; `<cover>` |
| [`PlaylistView`](../widgets/player/playlist-view.md) | `PlaylistView::new(id).rows(Vec<Row>)` | `on_select/activate/reorder` | addr; `<track>` |

## Chrome

| Widget | Constructor | Messages | XML |
|---|---|---|---|
| [`MenuBar`](../widgets/chrome/menu-bar.md) | `MenuBar::new(id, Fn(u32) -> M)` | `Fn(u32) -> M` | addr |
| [`MenuButton`](../widgets/chrome/menu-button.md) | `MenuButton::new(id, label, Menu, Fn(u32) -> M)` | `Fn(u32) -> M` | addr |
| [`SplitButton`](../widgets/chrome/split-button.md) | `SplitButton::new(id, label, Menu, Fn() -> M, Fn(u32) -> M)` | primary + command | addr |
| [`Toolbar`](../widgets/chrome/toolbar.md) | `Toolbar::new(id, Vec<ToolbarItem>, Fn(u32) -> M)` | `Fn(u32) -> M` | addr |
| [`StatusBar`](../widgets/chrome/status-bar.md) | `StatusBar::new(id).segment(..)` | — | addr; `<segment>` |
| [`Ribbon`](../widgets/chrome/ribbon.md) | `Ribbon::new(id, Vec<RibbonTab>, Fn(u32) -> M)` | command + `on_tab` | addr |

## Overlay and custom

| Widget | Constructor | Messages | XML |
|---|---|---|---|
| [`Popover`](../widgets/overlay/popover.md) | `Popover::new(id).anchor(rect)` | `.on_dismiss(Fn() -> M)` | addr |
| [`CustomWidget`](../widgets/custom/custom-widget.md) | `CustomWidget::new(id, paint_closure)` | `.input(FnMut(Rect, WidgetInput) -> Option<M>)` | addr; `x-*` via skin protocol |

## Shared observer builders

Available on Button, Toggle, Segmented, Tabs, Slider, Checkbox, RadioButton,
RadioGroup, ComboBox, MultiSelectList, ListView, ColorPicker, DatePicker,
TimePicker, MenuBar, MenuButton, SplitButton, Toolbar:

`.on_click(Fn(Point) -> Option<M>)`, `.on_right_click(..)`,
`.on_double_click(..)`, `.on_key_down(Fn(KeyEvent) -> Option<M>)`,
`.on_key_up(..)`, `.on_focus_gained(Fn(FocusEvent) -> Option<M>)`,
`.on_focus_lost(..)`.

`TextEdit` defines its own `on_change`, `on_char`, `on_click`, `on_key_down`,
`on_key_up`. Any widget id can also be bound with `Ui::bind(id, Trigger, ..)`;
triggers are `Click, DoubleClick, RightClick, HoverEnter, HoverLeave, Wheel,
Activate, DragStart, DragEnd, ValueChange`.
