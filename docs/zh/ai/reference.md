# Bamboo Native UI — AI 参考

一份面向 AI 助手的、自包含的单文件上下文。它镜像了位于 `F:\codes\rust\mp3player\native-ui` 的工作区的公开接口面。当本文件与源码不一致时，以源码为准。

## 心智模型

- **应用**负责计算每个控件的矩形并持有其状态。
- 框架（`Ui<M>`）持有保留式控件树、命中测试、焦点、悬停/按下状态以及脏区追踪。它只绘制脏矩形。
- 每个窗口一个持久的后台缓冲；只块传送脏包围盒。**空闲即零绘制。** 没有定时器就不进行周期性重绘。
- 绘制路径上禁止分配：借用文本/脏矩形，按窗口生命周期缓存字体、画刷和转换后的图像。
- 公开类型与平台无关。Win32/GDI/D2D 宿主位于 `#[cfg(windows)]` 之后。框架在宿主边界上是**仅限 Windows** 的。
- 消息：控件从 `input` 返回 `Option<M>`，其中 `M` 是应用的消息枚举。`ui.drain_messages()` 产出 `Vec<M>`。

## Crate 与依赖

| Crate | 路径 | 角色 |
|---|---|---|
| `bamboo-native-ui` | `crates/native-ui` | 框架：`Ui`、控件、`Host`、后端 |
| `bamboo-native-ui-layout-core` | `crates/layout-core` | `.buixml`/`.buitheme` 模型、解析器、写出器、值类型 |
| `bamboo-native-ui-layout-codegen` | `crates/layout-codegen` | `.buixml` → Rust（库 + CLI 二进制） |
| `bamboo-native-ui-layout-macros` | `crates/layout-macros` | `layout_xml!` 过程宏 |
| `bamboo-native-ui-showcase` | `crates/showcase` | 全控件演示二进制（`showcase`） |

```toml
[dependencies]
bamboo-native-ui = { path = "native-ui/crates/native-ui" }
# 可选：
bamboo-native-ui-layout-macros = { path = "native-ui/crates/layout-macros" }
```

二进制需要 `windows` crate 来提供 `run()` 的结果类型以及 user32/Win32 调用；框架重导出了平台接口面。

## Feature 标志（`bamboo-native-ui`）

| Feature | 默认 | 效果 |
|---|---|---|
| `gdi` | 是 | GDI/Windows 后端；公开 API 保持平台无关 |
| `layout-xml` | 否 | `.buixml` 解析器/模型/应用器；启用 `layout-core/parse`（quick-xml） |
| `theme-xml` | 否 | `.buitheme` → `Palette`；启用 `layout-core/parse` |
| `layout-compiled` | 否 | 代码生成输出的运行时；不链接 XML 解析器 |
| `debug-form` | 否 | 调试快照 + 热重载；隐含 `layout-xml` |

`layout-core` 单独带有一个 `parse` feature（quick-xml），默认关闭。

## 最小应用

```rust
use bamboo_native_ui::{
    Host, PaintCtx, Rect, Size, ThemeSettings, Ui, WidgetId, WindowBuilder, WindowHandle,
};
use bamboo_native_ui::widgets::{Button, Label};

enum Msg { Play }

struct App { ui: Ui<Msg> }

impl App {
    fn new() -> Self {
        let mut ui = Ui::new();
        ui.add(Label::new("title", "Now playing").wrap(true));
        ui.add(Button::new("play", "Play", Msg::Play).primary());
        Self { ui }
    }
}

impl Host for App {
    fn paint(&mut self, ctx: &mut PaintCtx<'_>) {
        for rect in ctx.dirty {
            ctx.canvas.fill_rect(*rect, ctx.painter.palette().background);
        }
        self.ui.paint(ctx);
    }

    fn resize(&mut self, size: Size, _dpi: f32, window: &mut WindowHandle<'_>) {
        self.ui.set_rect(WidgetId::new("title"), Rect::from_xywh(16.0, 16.0, size.width - 32.0, 24.0));
        self.ui.set_rect(WidgetId::new("play"), Rect::from_xywh(16.0, 56.0, 120.0, 36.0));
        window.invalidate_all();
    }

    fn event(&mut self, _event: &bamboo_native_ui::Event, window: &mut WindowHandle<'_>) -> bool {
        for _msg in self.ui.drain_messages() { /* 处理 */ }
        self.ui.flush_damage(window);
        false
    }
}

fn main() -> windows::core::Result<()> {
    WindowBuilder::new(App::new())
        .title("Demo")
        .size(800.0, 600.0)
        .theme(ThemeSettings::default(), true)
        .run()
}
```

`run()` 返回 `windows::core::Result<i32>`。`WindowBuilder` 方法：`app_id`、`owner`、`menu_bar`、`renderer`、`window_hotkey(s)`、`title`、`size`、`min_size`、`theme(settings, dark)`、`run`。

## 核心类型（签名）

```rust
// 几何（DIP；Rect::to_px(scale) 向外取整）
pub struct Point { pub x: f32, pub y: f32 }
pub struct Size  { pub width: f32, pub height: f32 }
pub struct Insets { pub left: f32, pub top: f32, pub right: f32, pub bottom: f32 }
pub struct Corners {
    pub top_left: f32, pub top_right: f32, pub bottom_right: f32, pub bottom_left: f32,
}
// Rect 存储边，而非宽/高。
pub struct Rect { pub left: f32, pub top: f32, pub right: f32, pub bottom: f32 }
impl Rect {
    pub fn new(left: f32, top: f32, right: f32, bottom: f32) -> Self;
    pub fn from_xywh(x: f32, y: f32, width: f32, height: f32) -> Self;
    pub fn from_size(size: Size) -> Self;
    pub fn width(self) -> f32; pub fn height(self) -> f32; pub fn size(self) -> Size;
    pub fn contains(self, x: f32, y: f32) -> bool;
    pub fn intersects(self, other: Rect) -> bool;
    pub fn union(self, other: Rect) -> Rect;
    pub fn inset(self, dx: f32, dy: f32) -> Rect;
    pub fn inflate(self, dx: f32, dy: f32) -> Rect;
    pub fn translate(self, dx: f32, dy: f32) -> Rect;
}

pub struct Color { pub r: u8, pub g: u8, pub b: u8 }   // 不透明 RGB
impl Color {
    pub fn new(r: u8, g: u8, b: u8) -> Self;
    pub fn rgb(rgb: [u8; 3]) -> Self;
    pub fn from_hex(hex: u32) -> Self;                 // 0xRRGGBB
    pub fn to_hex(self) -> u32;
}
pub struct Rgba { pub color: Color, pub alpha: u8 }    // alpha 0..=255

pub struct ThemeSettings {
    pub accent: [u8; 3],
    pub font_family: Option<String>,
    pub font_size: f32,
    pub list_density: ListDensity, // Compact | Comfortable | Spacious
}
impl Palette { pub fn new(settings: &ThemeSettings, dark: bool) -> Palette; }
```

`Palette` 有约 32 个公开字段：`dark, background, sidebar, player, cover,
nav_active, accent, text, muted, divider, row_selected, scroll_track,
scroll_thumb, accent_text, search_bg, button_bg, button_hover, button_pressed,
button_active, button_active_hover, explicit_border, nav_hover, row_hover,
nav_pressed, row_pressed, scroll_thumb_hover, card, panel, button_disabled,
text_disabled, play_button, play_hover`。

`WidgetId` 是 `pub struct WidgetId(pub &'static str)`；`WidgetId::new("id")`。

## `Ui<M>` — 最常用的方法

```rust
// 树
Ui::new(); ui.add(widget); ui.add_with(widget, WidgetProps::new());
ui.insert(at, widget); ui.remove(id, &mut sink); ui.clear();
ui.set_rect(id, rect); ui.rect(id); ui.ids(); ui.with_widget::<W, R>(id, f);
// 绑定 / 焦点
ui.bind(id, Trigger::Click, |event, ctx| Some(Msg::X));
ui.on_click(id, f); ui.on_activate(id, f); ui.set_focus(id, &mut sink);
// 输入（由宿主调用）
ui.pointer_move(p); ui.pointer_down(p, &mut sink); ui.pointer_up(p, &mut sink);
ui.pointer_down_button(p, MouseButton::Left, &mut sink);
ui.wheel(p, delta, &mut sink); ui.key(KeyEvent, &mut sink); ui.key_up(..);
// 消息 / 脏区
ui.drain_messages() -> Vec<M>; ui.flush_damage(&mut sink);
// setter（静默；记录脏区）
ui.set_enabled(id, bool); ui.set_visible(id, bool); ui.set_text(id, &str);
ui.set_label(id, &str); ui.set_value(id, f32); ui.set_toggle(id, bool);
ui.set_selected(id, usize); ui.set_rows(id, rows); ui.set_progress(id, f32);
// 滚动 / 图像 / 菜单 / 无障碍
ui.set_scroll_view(view); ui.set_scroll_offset(id, offset);
ui.set_image(UiImage::gradient(id, w, h, top, bottom)); ui.image(id);
ui.set_context_menu(id, menu); ui.open_menu(id, &mut sink);
ui.a11y_nodes(); ui.apply_a11y(...);
// 布局皮肤
ui.apply_layout(&document, &palette, size, &mut sink); // feature layout-xml
ui.apply_compiled(&LAYOUT, &palette, size, &mut sink); // feature layout-compiled
ui.clear_layout(&mut sink); ui.installed_layout();
```

`UiHost` 是 sink trait（`Invalidator` 加原生编辑、工具提示、上下文菜单、剪贴板、软键盘、拖拽）。`WindowHandle` 实现了它。

## `Widget<M>` 契约

必需：

```rust
fn id(&self) -> WidgetId;
fn rect(&self) -> Rect;
fn set_rect(&mut self, rect: Rect);
fn as_any(&self) -> &dyn Any;
fn as_any_mut(&mut self) -> &mut dyn Any;
fn paint(&self, paint: &WidgetPaint<'_>, state: &WidgetState);
fn input(&mut self, input: WidgetInput) -> Option<M>;
```

提供（按需重写）：`preferred_height`、`enabled`、`focusable`、
`visible`、`interactive`、`hit_test`、`cursor`、`adjustable`、`paint_bounds`、
`overlay`、`focus_inset`、`focus_ring_rect/radius`、原生编辑方法
（`wants_edit`、`edit_request`、`commit_text`）、内联编辑方法、观察者、
`a11y`、`read_only`、状态 setter（`set_enabled`、`set_value`、`set_toggle`、
`set_selected`、`set_rows`、`set_label`、`set_progress`、`set_text`、
`set_number`、`set_visible`），以及皮肤协议：

```rust
fn skin_attrs(&self) -> &'static [SkinAttrDecl];
fn validate_skin_attr(&self, name: &str, value: &SkinValue<'_>) -> bool;
fn apply_skin_attr(&mut self, name: &str, value: &SkinValue<'_>);
fn skin_children(&self) -> &'static [&'static str];
fn validate_skin_children(&self, children: &[SkinElement]) -> bool;
fn apply_skin_children(&mut self, children: &[SkinElement]);
```

`WidgetState`：`hovered, pressed, focused, adjusting, enabled, focus_visible,
editing, caret_on`。
`WidgetPaint`：`painter, canvas, palette, font, images, viewport, dirty` 外加
`text`、`measure_text`、`style`、`image`、`enabled_color` 辅助方法。
`WidgetInput`：`HoverEnter/Leave/At, PressAt, DragTo, ReleaseAt, ClickAt,
SelectWith, DoubleClickAt, RightClickAt, DragStart/End, Wheel, Step, Activate,
Char, Key, KeyUp`。

## `Host` 与 `PaintCtx`

必需：`paint(&mut self, ctx: &mut PaintCtx<'_>)`。
提供的钩子：`overlay_paint`、`event`、`timer`、`resize`、`dpi_changed`、
`started`、`close_requested`、`loaded`、`visibility_changed`、`activated`、
`visual_state_changed`、`moved`、`files_dropped`、拖拽回调、`destroying`、
`edit_changed/committed/canceled`、`ime_compose/commit`、`a11y_nodes`、
`a11y_action`、`menu_command`、`menu_event`、`menu_bar_command`、
`dialog_finished`、`tray_event`、`hotkey`、`hotkey_captured`、`destroy`。

`PaintCtx` 字段：`painter`、`canvas`、`dirty: &[Rect]`、`size`、`dpi`、
`editing: Option<WidgetId>`、`caret_on`、`overlay`、`overlay_first`、
`caret: Option<Rect>`（输出）。辅助方法 `scale()`、`hits(rect)`、`full()`。

## 事件与触发器

`Event`：`PointerMove, PointerDown, PointerUp, PointerDoubleClick,
PointerLeave, PointerKind, LongPress, Wheel, Key(KeyEvent), KeyUp(KeyEvent),
Alt(bool), Char(char), Focus(bool), Resize(Size), DpiChanged(f32)`。
`KeyCode`：`Backspace, Tab, Enter, Escape, Space, Left, Right, Up, Down, Home,
End, PageUp, PageDown, Insert, Delete, Letter(u8), Digit(u8), Function(u8),
Other(u16)`。

某些控件安装的共享观察者构造器（叠加式；内建动作仍会运行）：`.on_click(Fn(Point) -> Option<M>)`、
`.on_right_click(Fn(Point) -> Option<M>)`、
`.on_double_click(Fn(Point) -> Option<M>)`、
`.on_key_down(Fn(KeyEvent) -> Option<M>)`、
`.on_key_up(Fn(KeyEvent) -> Option<M>)`、
`.on_focus_gained(Fn(FocusEvent) -> Option<M>)`、
`.on_focus_lost(Fn(FocusEvent) -> Option<M>)`。

带有共享观察者的控件：Button、Toggle、Segmented、Tabs、Slider、
Checkbox、RadioButton、RadioGroup、ComboBox、MultiSelectList、ListView、
ColorPicker、DatePicker、TimePicker、MenuBar、MenuButton、SplitButton、Toolbar。
`TextEdit` 定义了自己的叠加式 `on_change`、`on_char`、`on_click`、
`on_key_down`、`on_key_up`。

`Trigger`（用于 `Ui::bind`）：`Click, DoubleClick, RightClick, HoverEnter,
HoverLeave, Wheel, Activate, DragStart, DragEnd, ValueChange`。
`TriggerEvent { trigger, pos: Option<Point>, wheel_delta: f32 }`。

## 控件目录

所有控件都从 `bamboo_native_ui::widgets`（而非 crate 根）重导出。构造器消息使用应用类型 `M`。

| 类别 | 类型 |
|---|---|
| 文本与内容 | `Label`、`Icon`、`Badge`、`Divider`、`Avatar`、`Kbd`、`CopyableText`、`MarqueeText`、`RichText` |
| 输入 | `Checkbox`、`RadioButton`、`RadioGroup`、`SearchBox`、`TextEdit`（`PasswordBox`、`TextArea`）、`NumberInput`（`NumberStepper`）、`ComboBox`、`MultiSelectList` |
| 控件 | `Button`、`Toggle`（`Switch`）、`Segmented`、`Tabs`、`Slider`、`ProgressBar`、`Pager`、`ColorSwatch` |
| 选择器 | `ColorPicker`、`DatePicker`、`TimePicker` |
| 数据与集合 | `Sparkline`、`Gauge`、`RatingStars`、`ListView`、`Table`、`TreeView`、`PropertyGrid` |
| 导航 | `Accordion`（`Collapsible`）、`Breadcrumb`、`NavRail`、`Splitter` |
| 反馈 | `Spinner`、`ProgressRing`、`Banner`、`TooltipCard`、`Scrim`、`Toast`（`Snackbar`） |
| 显示 | `Card`、`GroupBox`、`StatCard`、`EmptyState`、`SectionTitle` |
| 播放器与媒体 | `WaveformSeekBar`、`PlaybackTransport`、`CoverWall`、`PlaylistView` |
| 窗口装饰 | `MenuBar`、`MenuButton`、`SplitButton`、`Toolbar`、`StatusBar`、`Ribbon` |
| 覆盖层与自定义 | `Popover`、`CustomWidget` |

关键构造器：

```rust
Label::new("id", "text")
Icon::new("id", '\u{E768}')
Badge::new("id", "text").tone(BadgeTone::Accent)
Avatar::new("id", "Name")
Kbd::new("id", "Ctrl+K")
Button::new("id", "Label", Msg::Action).primary()
Toggle::new("id", "Label", true, Msg::Toggled)
Checkbox::new("id", "Label", true, Msg::Checked)
Slider::new("id", 0.5, 0.0..=1.0, Msg::Volume)
ProgressBar::new("id", 0.42)
Segmented::new("id", &["A", "B"], 0, Msg::Segment)
Tabs::new("id", &["One", "Two"], 0, Msg::Tab)
ComboBox::new("id", &["A", "B"], 0, Msg::Choice)
TextEdit::new("id", "text", Msg::Commit)
NumberInput::new("id", 10, 0..=100, Msg::Number)
ListView::new("id").rows(rows)             // Row::new("title")
Table::new("id").columns(cols).rows(rows)  // TableColumn::new("Title", 120.0)
TreeView::new("id").items(items)           // TreeItem::new("Label")
PropertyGrid::new("id").sections(sections) // PropertySection::new("Section")
Card::new("id", "Title", "Body")
Popover::new("id").anchor(rect)
CustomWidget::new("id", |paint, rect, state| { /* 绘制 */ })
```

showcase（`crates/showcase/src/main.rs`）演示了所有这些控件。

## `.buixml` 布局文档

皮肤会寻址 Rust 端创建的控件：它移动、设置样式并填充这些控件的 id。它也能**创建**控件：在安装了 `WidgetRegistry` 的情况下，带 `type="..."` 的条目会创建控件（标准集合覆盖 12 种类型；第三方类型从 Rust 注册）。被创建的控件归文档所有 —— 重新加载会移除或替换新文档不再声明的控件 —— 而交互式创建器需要一个映射到 `M` 的 `command`。任意的内建控件和自定义控件仍需要 Rust 工厂。

```xml
<?xml version="1.0" encoding="utf-8"?>
<layouts version="1">
  <const name="pad" value="16"/>
  <layout id="base" min-width="0" min-height="0">
    <widget id="title" x="pad" y="16" w="cw-32" h="24"/>
    <widget id="play" type="button" command="101"
            x="cx-22" y="96" w="44" h="44" background="accent" radius="pill"/>
    <widget id="list" x="pad" y="160" w="cw-2*pad" h="180" row-height="36">
      <item title="First" subtitle="Sub" trailing="A"/>
    </widget>
  </layout>
</layouts>
```

几何：水平方向 —— `x+w`、`right+w`、`x+right`（拉伸）中恰好一个；
垂直方向 —— `y+h`、`bottom+h`、`y+bottom` 中恰好一个。
表达式文法：`expr := term (('+'|'-') term)*`、
`term := factor (('*'|'/') factor)*`、
`factor := number | number '%' | variable | '(' expr ')' | '-' factor`。
空间变量：`left/top` = 0、`right/bottom` = 边缘、`cw/ch` = 尺寸、
`cx/cy` = 一半。`N%` 是该轴范围的 N/100。常量按顺序用
`<const>` 声明；只能引用更早声明的常量。深度限制为 32。

控件属性：`type`、`command`（u32，仅与 `type` 一起使用）、`parent`、
`visible`、`enabled`、`interactive`、`focusable`、`focus-order`、`read-only`、
`tooltip`、`name`、`description`、`automation-id`、`role`、`expose`、`cursor`、
`radius`（`0..N`、`small=4`、`medium=6`、`large=10`、`pill`）、`corners`
（`"tl tr br bl"`）、`background`（调色板 token、`#rgb`/`#rrggbb`、`none`）、
`font-size`、`font-family`，以及内容属性（`text`、`label`、`value`、
`progress`、`number`、`checked`、`selected`）。

变体：每个断点一个 `<layout>`，配以 `min-width`/`min-height`；选择是纯函数的（取可容纳的最大最小值，否则用 `(0,0)` 基准）。

子列表（每个控件）：`<item>`、`<option>`、`<step>`、`<segment>`、
`<node>`、`<column>/<row>/<cell>`、`<section>`、`<track>`、`<cover>`、
`<point>`、`<run>`、`<page>`。第三方控件使用通过皮肤协议声明的 `x-*` 名称。

运行时 API（`feature = "layout-xml"`）：

```rust
use bamboo_native_ui::layout_xml::LayoutDocument;
let document = LayoutDocument::parse(text)?;
ui.apply_layout(&document, &palette, size, &mut sink)?; // 出错时原子回滚
ui.clear_layout(&mut sink);
```

XML 不会绑定任何事件处理器。`command` 通过 `CommandMap<M>` 将被创建的交互式控件映射到应用消息。内容在每次文档生成时写入一次；它不是实时绑定的，也不会被 `clear_layout` 恢复。

## 编译式布局

代码生成 CLI：

```text
cargo run -p bamboo-native-ui-layout-codegen -- layouts/main.buixml \
    -o src/main_layout.rs --module main_layout [--context AppLayout]
```

生成的模块：`pub static LAYOUT: CompiledLayout` 以及
`pub fn install<M, H: UiHost>(ui, sink, palette, client) -> Result<LayoutReport, LayoutFailure>`。
用 `include!` 或构建脚本使用它，然后：

```rust
ui.apply_compiled(&main_layout::LAYOUT, &palette, size, &mut sink)?;
// 或 main_layout::install(&mut ui, &mut sink, &palette, size)?;
```

过程宏：

```rust
bamboo_native_ui_layout_macros::layout_xml!("layouts/main.buixml");
bamboo_native_ui_layout_macros::layout_xml!("layouts/main.buixml", module = app_layout);
bamboo_native_ui_layout_macros::layout_xml!("layouts/main.buixml", context = AppLayout);
```

该宏就地展开同一个模块，并且消费方需要 `layout-compiled`。稳定的过程宏无法注册文件依赖，因此只修改 `.buixml` 不会触发重建；为此请使用 CLI/构建脚本。

编译模式还接受内联 Rust：`x="rust:EXPR"`（几何、visible/enabled/radius/content）以及用于行的 `<rust target="id" code="..."/>`。

## `.buitheme` 主题文档（`feature = "theme-xml"`）

```xml
<?xml version="1.0" encoding="utf-8"?>
<theme version="1" name="Night" base="dark" accent="#7aa2f7">
  <color name="background" value="#16181d"/>
  <color name="text" value="#e8ebee"/>
</theme>
```

`base` 是 `light` 或 `dark`；`<color name value>` 在推导后覆盖一个调色板 token。API：

```rust
use bamboo_native_ui::theme_xml::{ThemeDocument, ThemeDocumentExt};
let document = ThemeDocument::parse(text)?;
let palette = document.palette(&settings);
window.set_theme(settings.clone(), palette.dark);
window.set_palette(palette);
```

调色板 token 是 `background=` 使用的 31 个 kebab-case 名称：
`background, sidebar, player, cover, nav-active, accent, text, muted, divider,
row-selected, scroll-track, scroll-thumb, accent-text, search-bg, button-bg,
button-hover, button-pressed, button-active, button-active-hover,
explicit-border, nav-hover, row-hover, nav-pressed, row-pressed,
scroll-thumb-hover, card, panel, button-disabled, text-disabled, play-button,
play-hover`。

## 注册表与标准 `type=` 名称

`WidgetRegistry::standard()` 创建：
`label, button, toggle, slider, progress, icon, badge, card, divider, kbd,
avatar, spinner`。

```rust
let mut registry = WidgetRegistry::<Msg>::standard();
registry.set_commands(|id| match id { 101 => Some(Msg::Save), _ => None });
registry.register("x-meter", |spec, ctx| { /* 返回 Box<dyn Widget<Msg>> */ });
ui.set_widget_registry(registry);
```

交互式创建器（`button`、`toggle`、`slider`）需要 `command`。

## 常见陷阱

1. 控件位于 `bamboo_native_ui::widgets::*`，而非 crate 根（只有
   `CustomWidget` 在根处重导出）。
2. 控件 id 是 `&'static str`；`WidgetId::new("id")`。
3. 在 `Host::resize` 中设置矩形，然后调用 `window.invalidate_all()`。
4. 只绘制与 `ctx.dirty` 相交的内容；最后调用 `ui.paint(ctx)`。
5. `Ui` 的 setter 是静默的并记录脏区；调用 `ui.flush_damage(window)`。
6. XML 从不绑定事件处理器；在 Rust 中接好回调。
7. 动态 XML 路径拒绝 `rust:` 值；编译模式接受它们。
8. `clear_layout` 不会恢复矩形、滚动视口或内容。
9. 滚动是由 `Ui` 管理的 `ScrollView`，而非一个控件。
10. 宿主/平台层仅限 Windows；中性类型可在其他平台编译。

## 接下来看哪里

- `docs/en/index.md` — 文档首页。
- `docs/en/quickstart/` — 十三个教程（六个核心，七个实战）。
- `docs/en/widgets/` — 每个控件一页（用法、属性、事件、XML）。
- `docs/en/api/` — 每个 API 领域一页。
- `docs/en/guides/xml-layout-authoring.md` — 实用的 `.buixml` 指南。
- `docs/en/reference/xml-elements.md` — 每个 XML 属性/子元素。
- `docs/archive/` — 设计记录与里程碑契约。
