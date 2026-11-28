# Bamboo Native UI — 控件速查表

面向 AI 助手的紧凑索引。`M` 是应用的消息类型。所有类型都从 `bamboo_native_ui::widgets` 导入。"addr" 表示该控件在 Rust 中创建，并在 `.buixml` 文档中通过 `id` 寻址（不带 `type=`）。

## 文本与内容

| 控件 | 构造器 | 消息 | XML |
|---|---|---|---|
| [`Label`](../widgets/content/label.md) | `Label::new(id, text)` | — | `type="label"`；`color`、`text-style` |
| [`Icon`](../widgets/content/icon.md) | `Icon::new(id, glyph: char)` | — | `type="icon"`；`glyph`、`tint` |
| [`Badge`](../widgets/content/badge.md) | `Badge::new(id, text)` | `.removable(Fn() -> M)` | `type="badge"`；`tone` |
| [`Divider`](../widgets/content/divider.md) | `Divider::new(id)` | — | `type="divider"` |
| [`Avatar`](../widgets/content/avatar.md) | `Avatar::new(id, name)` | — | `type="avatar"`；`initials`、`tint`、`ring`、`image` |
| [`Kbd`](../widgets/content/kbd.md) | `Kbd::new(id, text)` | — | `type="kbd"`；`shortcut` |
| [`CopyableText`](../widgets/content/copyable-text.md) | `CopyableText::new(id, text)` | `.copyable(Fn() -> M)` | addr |
| [`MarqueeText`](../widgets/content/marquee-text.md) | `MarqueeText::new(id, text)` | —（应用调用 `advance(dt)`） | addr |
| [`RichText`](../widgets/content/rich-text.md) | `RichText::new(id).run(..)` | `.on_link(Fn(id) -> M)` | addr；`<run>` |

## 输入

| 控件 | 构造器 | 消息 | XML |
|---|---|---|---|
| [`Checkbox`](../widgets/inputs/checkbox.md) | `Checkbox::new(id, label, checked, Fn(bool) -> M)` | `Fn(bool) -> M` | addr |
| [`RadioButton`](../widgets/inputs/radio-button.md) | `RadioButton::new(id, label, selected, Fn() -> M)` | `Fn() -> M` | addr |
| [`RadioGroup`](../widgets/inputs/radio-group.md) | `RadioGroup::new(id, &[&str], selected, Fn(usize) -> M)` | `Fn(usize) -> M` | addr |
| [`SearchBox`](../widgets/inputs/search-box.md) | `SearchBox::new(id, text, Fn(String) -> M)` | `Fn(String) -> M` | addr |
| [`TextEdit`](../widgets/inputs/text-edit.md) | `TextEdit::new(id, text, Fn(String) -> M)` | `Fn(String) -> M` + 自有的 `on_*` | addr；别名 `PasswordBox`、`TextArea` |
| [`NumberInput`](../widgets/inputs/number-input.md) | `NumberInput::new(id, i64, RangeInclusive<i64>, Fn(i64) -> M)` | `Fn(i64) -> M` | addr；别名 `NumberStepper` |
| [`ComboBox`](../widgets/inputs/combo-box.md) | `ComboBox::new(id, &[&str], selected, Fn(usize) -> M)` | `Fn(usize) -> M` | addr；`<option>` |
| [`MultiSelectList`](../widgets/inputs/multi-select-list.md) | `MultiSelectList::new(id, &[&str], Fn(Vec<usize>) -> M)` | `Fn(Vec<usize>) -> M` | addr |

## 控件

| 控件 | 构造器 | 消息 | XML |
|---|---|---|---|
| [`Button`](../widgets/controls/button.md) | `Button::new(id, label, M)` | `M` | `type="button"`，`command` 必需 |
| [`Toggle`](../widgets/controls/toggle.md) | `Toggle::new(id, label, on, Fn(bool) -> M)` | `Fn(bool) -> M` | `type="toggle"`，`command` 必需；别名 `Switch` |
| [`Segmented`](../widgets/controls/segmented.md) | `Segmented::new(id, &[&str], selected, Fn(usize) -> M)` | `Fn(usize) -> M` | addr；`<option>` |
| [`Tabs`](../widgets/controls/tabs.md) | `Tabs::new(id, &[&str], selected, Fn(usize) -> M)` | `Fn(usize) -> M`；`.closable(Fn(usize) -> M)` | addr；`<option>` |
| [`Slider`](../widgets/controls/slider.md) | `Slider::new(id, f32, RangeInclusive<f32>, Fn(f32) -> M)` | `Fn(f32) -> M`；拖拽回调 | `type="slider"`，`command` 必需 |
| [`ProgressBar`](../widgets/controls/progress-bar.md) | `ProgressBar::new(id, value: f32)` | — | `type="progress"` |
| [`Pager`](../widgets/controls/pager.md) | `Pager::new(id, page, pages, Fn(i32) -> M)` | `Fn(i32) -> M`（`-1`/`+1`） | addr；`<page>` |
| [`ColorSwatch`](../widgets/controls/color-swatch.md) | `ColorSwatch::new(id, Color, label)` | — | addr |

## 选择器

| 控件 | 构造器 | 消息 | XML |
|---|---|---|---|
| [`ColorPicker`](../widgets/pickers/color-picker.md) | `ColorPicker::new(id, Color, Fn(Color) -> M)` / `new_rgba` | `Fn(Color) -> M`、`Fn(Rgba) -> M` | addr |
| [`DatePicker`](../widgets/pickers/date-picker.md) | `DatePicker::new(id, Date, Fn(Date) -> M)` | `Fn(Date) -> M` | addr |
| [`TimePicker`](../widgets/pickers/time-picker.md) | `TimePicker::new(id, Time, Fn(Time) -> M)` | `Fn(Time) -> M` | addr |

## 数据与集合

| 控件 | 构造器 | 消息 | XML |
|---|---|---|---|
| [`Sparkline`](../widgets/data/sparkline.md) | `Sparkline::new(id, &[f32])` | — | addr；`<point value>` |
| [`Gauge`](../widgets/data/gauge.md) | `Gauge::new(id, value: f32)` | — | addr |
| [`RatingStars`](../widgets/data/rating-stars.md) | `RatingStars::new(id, stars, value, Fn(u8) -> M)` | `Fn(u8) -> M` | addr |
| [`ListView`](../widgets/data/list-view.md) | `ListView::new(id).rows(Vec<Row>)` | `.on_activate(Fn(usize) -> M)` | addr；`<item>` |
| [`Table`](../widgets/data/table.md) | `Table::new(id).columns(..).rows(..)` | `on_select/activate/header_click/column_resize/column_reorder/cell_edit` | addr；`<column>/<row>/<cell>` |
| [`TreeView`](../widgets/data/tree-view.md) | `TreeView::new(id).items(Vec<TreeItem>)` | `on_select/activate/toggle/rename/move/drag_*` | addr；`<node>` |
| [`PropertyGrid`](../widgets/data/property-grid.md) | `PropertyGrid::new(id).sections(Vec<PropertySection>)` | `on_select/activate/change` | addr；`<section>` |

## 导航

| 控件 | 构造器 | 消息 | XML |
|---|---|---|---|
| [`Accordion`](../widgets/navigation/accordion.md) | `Accordion::new(id, &[&str], Fn(usize, bool) -> M)` | `Fn(usize, bool) -> M` | addr；别名 `Collapsible` |
| [`Breadcrumb`](../widgets/navigation/breadcrumb.md) | `Breadcrumb::new(id, &[&str], Fn(usize) -> M)` | `Fn(usize) -> M` | addr；`<step>` |
| [`NavRail`](../widgets/navigation/nav-rail.md) | `NavRail::new(id, &[&str], selected, Fn(usize) -> M)` | `Fn(usize) -> M` | addr；`<step>` |
| [`Splitter`](../widgets/navigation/splitter.md) | `Splitter::vertical/horizontal(id, Fn(f32) -> M)` | `Fn(f32) -> M` | addr |

## 反馈

| 控件 | 构造器 | 消息 | XML |
|---|---|---|---|
| [`Spinner`](../widgets/feedback/spinner.md) | `Spinner::new(id)` | —（`advance(dt)`） | `type="spinner"` |
| [`ProgressRing`](../widgets/feedback/progress-ring.md) | `ProgressRing::new(id, value: f32)` | — | addr |
| [`Banner`](../widgets/feedback/banner.md) | `Banner::new(id, BannerKind, title)` | `.action(..)`、`.closable(..)` | addr；`body` |
| [`TooltipCard`](../widgets/feedback/tooltip-card.md) | `TooltipCard::new(id, title)` | — | addr |
| [`Scrim`](../widgets/feedback/scrim.md) | `Scrim::new(id)` | — | addr |
| [`Toast`](../widgets/feedback/toast.md) | `Toast::new(id)` + `.show(text, error)` | —（应用定时器） | addr；`message`、`error`；别名 `Snackbar` |

## 显示

| 控件 | 构造器 | 消息 | XML |
|---|---|---|---|
| [`Card`](../widgets/display/card.md) | `Card::new(id, title, body)` | — | `type="card"`；`title`、`body`、`emphasis` |
| [`GroupBox`](../widgets/display/group-box.md) | `GroupBox::new(id, title)` | — | addr |
| [`StatCard`](../widgets/display/stat-card.md) | `StatCard::new(id, value, label)` | — | addr |
| [`EmptyState`](../widgets/display/empty-state.md) | `EmptyState::new(id, title, hint)` | — | addr |
| [`SectionTitle`](../widgets/display/section-title.md) | `SectionTitle::new(id, label)` | — | addr；`color` |

## 播放器与媒体

| 控件 | 构造器 | 消息 | XML |
|---|---|---|---|
| [`WaveformSeekBar`](../widgets/player/waveform-seek-bar.md) | `WaveformSeekBar::new(id, progress, Fn(f32) -> M)` | 跳转 + 拖拽回调 | addr |
| [`PlaybackTransport`](../widgets/player/playback-transport.md) | `PlaybackTransport::new(id, Fn(TransportAction) -> M)` | `TransportAction` | addr |
| [`CoverWall`](../widgets/player/cover-wall.md) | `CoverWall::new(id).covers(Vec<Cover>)` | `on_open`、`on_select` | addr；`<cover>` |
| [`PlaylistView`](../widgets/player/playlist-view.md) | `PlaylistView::new(id).rows(Vec<Row>)` | `on_select/activate/reorder` | addr；`<track>` |

## 窗口装饰

| 控件 | 构造器 | 消息 | XML |
|---|---|---|---|
| [`MenuBar`](../widgets/chrome/menu-bar.md) | `MenuBar::new(id, Fn(u32) -> M)` | `Fn(u32) -> M` | addr |
| [`MenuButton`](../widgets/chrome/menu-button.md) | `MenuButton::new(id, label, Menu, Fn(u32) -> M)` | `Fn(u32) -> M` | addr |
| [`SplitButton`](../widgets/chrome/split-button.md) | `SplitButton::new(id, label, Menu, Fn() -> M, Fn(u32) -> M)` | 主操作 + command | addr |
| [`Toolbar`](../widgets/chrome/toolbar.md) | `Toolbar::new(id, Vec<ToolbarItem>, Fn(u32) -> M)` | `Fn(u32) -> M` | addr |
| [`StatusBar`](../widgets/chrome/status-bar.md) | `StatusBar::new(id).segment(..)` | — | addr；`<segment>` |
| [`Ribbon`](../widgets/chrome/ribbon.md) | `Ribbon::new(id, Vec<RibbonTab>, Fn(u32) -> M)` | command + `on_tab` | addr |

## 覆盖层与自定义

| 控件 | 构造器 | 消息 | XML |
|---|---|---|---|
| [`Popover`](../widgets/overlay/popover.md) | `Popover::new(id).anchor(rect)` | `.on_dismiss(Fn() -> M)` | addr |
| [`CustomWidget`](../widgets/custom/custom-widget.md) | `CustomWidget::new(id, paint_closure)` | `.input(FnMut(Rect, WidgetInput) -> Option<M>)` | addr；通过皮肤协议支持的 `x-*` |

## 共享观察者构造器

可用于 Button、Toggle、Segmented、Tabs、Slider、Checkbox、RadioButton、
RadioGroup、ComboBox、MultiSelectList、ListView、ColorPicker、DatePicker、
TimePicker、MenuBar、MenuButton、SplitButton、Toolbar：

`.on_click(Fn(Point) -> Option<M>)`、`.on_right_click(..)`、
`.on_double_click(..)`、`.on_key_down(Fn(KeyEvent) -> Option<M>)`、
`.on_key_up(..)`、`.on_focus_gained(Fn(FocusEvent) -> Option<M>)`、
`.on_focus_lost(..)`。

`TextEdit` 定义了自己的 `on_change`、`on_char`、`on_click`、`on_key_down`、
`on_key_up`。任意控件 id 也可以用 `Ui::bind(id, Trigger, ..)` 绑定；
触发器为 `Click, DoubleClick, RightClick, HoverEnter, HoverLeave, Wheel,
Activate, DragStart, DragEnd, ValueChange`。
