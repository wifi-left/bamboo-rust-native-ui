# 属性与叙述

> 按实例的呈现与辅助功能覆盖：`WidgetProps`、`FontSpec`、`Narration` 与 `ContentCaps`。

## 概述

控件只构建一次并复用，因此同一类型的两个实例之间不同的细节并不存放在控件结构体中。
`bamboo_native_ui::props` 拥有它们：

- `WidgetProps` — 可见性、启用、可聚焦性、Tab 权重、光标、命中测试、只读、工具提示、
  上下文菜单、自动化 id、调色板、字体、辅助功能角色、叙述以及辅助功能公开性。
- `FontSpec` — 按控件的 UI 字体覆盖。
- `Narration` — 应用拥有的可访问名称、描述以及逐行格式化器。
- `ContentCaps` — 某控件接受哪些内容设置器（由控件发布，由布局应用器读取）。

在创建时使用 `Ui::add_with(widget, props)` 应用属性，或在运行时使用
`Ui::set_props(id, props, sink)`。每个字段都是可选的：未设置的字段保留控件自身的默认值，
因此现有页面的叙述和绘制与之前完全相同。

属性不是布局。边距、增长和对齐仍由应用计算（框架没有测量阶段）；属性只携带容器无需重建树
即可遵循的按实例状态。

## 关键类型

### `WidgetProps`

```rust
pub struct WidgetProps { /* 所有字段私有；每个设置器都是可选的 */ }

impl WidgetProps {
    pub fn new() -> Self;
    pub fn visible(self, visible: bool) -> Self;
    pub fn enabled(self, enabled: bool) -> Self;
    pub fn focusable(self, focusable: bool) -> Self;
    pub fn focus_order(self, weight: i32) -> Self;
    pub fn cursor(self, cursor: Cursor) -> Self;
    pub fn mouse(self, interactive: bool) -> Self;
    pub fn read_only(self, read_only: bool) -> Self;
    pub fn tooltip(self, text: impl Into<String>) -> Self;
    pub fn tooltip_at(self, index: usize, text: impl Into<String>) -> Self;
    pub fn context_menu(self, menu: Menu) -> Self;
    pub fn automation_id(self, id: impl Into<String>) -> Self;
    pub fn palette(self, palette: Palette) -> Self;
    pub fn font(self, spec: FontSpec) -> Self;
    pub fn font_size(self, size: f32) -> Self;
    pub fn font_family(self, family: impl Into<String>) -> Self;
    pub fn role(self, role: A11yRole) -> Self;
    pub fn narrate(self, narration: Narration) -> Self;
    pub fn name(self, name: impl Into<String>) -> Self;
    pub fn describe(self, description: impl Into<String>) -> Self;
    pub fn expose(self, exposed: bool) -> Self;
}
```

| 构建方法 | 作用范围 | 说明 |
|---|---|---|
| `visible` | 容器可见性 | 结构性：输入、Tab、a11y 和滚动条都随之变化。 |
| `enabled` | 控件输入 | 禁用的控件会丢弃指针状态。 |
| `focusable` | 焦点策略 | 覆盖控件类型的默认值。 |
| `focus_order` | Tab 顺序 | 权重较小者优先；权重相等时保持插入顺序。默认 `0`。 |
| `cursor` | 指针 | 悬停时的形状。 |
| `mouse` | 命中测试 | 显示控件默认为穿透；`mouse(true)` 使卡片/Toast 可点击并启用触发器绑定。 |
| `read_only` | 编辑器 + a11y | 无原生编辑会话；与控件自身的 `read_only` 按位或。 |
| `tooltip` / `tooltip_at` | 宿主工具提示 | `tooltip_at` 针对某一行/选项；空字符串会抑制该项。 |
| `context_menu` | 宿主菜单 | 右键、Menu 键或 Shift+F10。 |
| `automation_id` | UIA | 默认使用控件 id。 |
| `palette` | 绘制 | 此控件使用不同的调色板绘制。 |
| `font`/`font_size`/`font_family` | UI 文本 | 参见 `FontSpec`；图标字形保留其自身字体。 |
| `role` | a11y | 覆盖由控件类型推导出的角色。 |
| `narrate`/`name`/`describe` | a11y | 仅用于叙述；绝不更改绘制的文本。 |
| `expose` | a11y 树 | 仅显示的控件即使在可聚焦时也会被排除。 |

### `FontSpec`

```rust
pub struct FontSpec { /* private */ }

impl FontSpec {
    pub fn new() -> Self;
    pub fn size(self, size: f32) -> Self;          // 绝对 DIP，下限为 1.0
    pub fn family(self, family: impl Into<String>) -> Self;
    pub fn is_empty(&self) -> bool;
    pub fn apply(&self, base: &TextStyle) -> TextStyle;
}
```

`apply` 合并到绘制器的角色样式上：尺寸替换比例，字面替换字面。字形字体（`FontFace::Glyph`）
会原样返回，因此图标永远不会变成豆腐块。`WidgetProps::font(FontSpec::new())` 会丢弃覆盖，
而不是存储一个空的覆盖。

### `Narration`

```rust
pub struct Narration { /* private */ }

impl Narration {
    pub fn new() -> Self;
    pub fn name(self, name: impl Into<String>) -> Self;
    pub fn name_with(self, format: impl Fn(&A11yInfo) -> String + 'static) -> Self;
    pub fn description(self, description: impl Into<String>) -> Self;
    pub fn description_with(self, format: impl Fn(&A11yInfo) -> String + 'static) -> Self;
    pub fn row(self, format: impl Fn(usize, &A11yChild) -> String + 'static) -> Self;
}
```

固定 vs 惰性：`name`/`description` 存储一个字符串；`name_with`/`description_with` 在每次
快照时根据控件当前的 `A11yInfo` 构建它，因此名称可以跟随播放进度或计数而无需重建树。
`row` 重命名复合控件的每个已发布子项；索引是控件级索引（滚动列表的行保持其绝对索引）。

叙述供给屏幕阅读器、`--a11y-dump` 和 `--self-test` 所观察的同一个 `A11yInfo`，因此三者
看到相同的值。

### `ContentCaps`

```rust
pub struct ContentCaps(u16);

impl ContentCaps {
    pub const NONE: Self;
    pub const TEXT: Self;       // Widget::set_text
    pub const LABEL: Self;      // Widget::set_label
    pub const VALUE: Self;      // Widget::set_value
    pub const PROGRESS: Self;   // Widget::set_progress
    pub const NUMBER: Self;     // Widget::set_number
    pub const TOGGLED: Self;    // Widget::set_toggle
    pub const SELECTION: Self;  // Widget::set_selected

    pub const fn union(self, other: Self) -> Self;
    pub const fn contains(self, other: Self) -> bool;
    pub const fn is_empty(self) -> bool;
    pub const fn bits(self) -> u16;
}
```

控件通过 `Widget::content_caps` 发布其集合。布局应用器会查询它，并警告
`xml.content.unsupported`，而不是静默写入控件无法接受的值。

## 属性如何到达运行时

- **绘制** — `palette` 替换交给控件的调色板；字体覆盖合并进 `TextStyle`；可见性/启用状态
  改变容器要求控件绘制和命中测试的内容。
- **辅助功能** — `role` 覆盖推导出的角色；`narrate`/`name`/`describe` 替换推导出的名称和
  描述；`expose` 强制控件进入或移出树；这一切都会落入 `A11yInfo`。
- **工具提示** — 宿主在其停留后解析 `tooltip_for(item)`：逐项条目优先（空条目会抑制），
  否则使用控件默认值。
- **菜单** — `context_menu` 是一个静态 `Menu`；需要反映实时状态的菜单由应用重建。

## 示例

```rust
use bamboo_native_ui::widgets::Card;
use bamboo_native_ui::{A11yRole, FontSpec, Narration, Ui, WidgetProps};

enum Msg {
    OpenAlbum,
}

fn add_card(ui: &mut Ui<Msg>) {
    ui.add_with(
        Card::new("details", "Album", "Night pieces"),
        WidgetProps::new()
            .mouse(true)
            .focus_order(2)
            .tooltip("Opens the album")
            .font(FontSpec::new().size(15.0))
            .role(A11yRole::Group)
            .narrate(Narration::new().name("Album details")),
    );
}
```

在运行时，相同字段通过 `set_props` 设置；使聚焦控件不可见或不可聚焦会重新定位焦点并重绘
两个矩形。

## 说明与限制

- 特性门控：`props`（以及 `creation`）在每种特性组合下都会编译；只有应用文档才需要
  `layout-xml`/`layout-compiled`。
- `Ui::add_with` 在首次布局前记录焦点/可见性/启用状态，因此它从不需要重新定位焦点。
  `Ui::set_props` 在活动页面上运行，因此需要。
- 叙述仅替换可访问字符串；绘制的 label/text 是内容，通过控件自身的设置器或布局文档设置。
- `tooltip_at` 的索引是绝对列表行 / 分段选项 / 选项卡，与辅助功能树发布的索引相同。
- `FontSpec` 影响 UI 文本，而非图标字形。尺寸是叠加在主题比例之上的绝对 DIP；后端负责应用
  DPI。
- 布局文档的属性能映射到相同字段：`visible`、`enabled`、`interactive`、`focusable`、
  `focus-order`、`read-only`、`tooltip`、`name`、`description`、`automation-id`、`role`、
  `expose`、`cursor`，以及 `font-size`/`font-family`。参见
  [layout-documents.md](layout-documents.md)。
- 在文档中，`name`/`description` 是叙述；`label`/`text` 是内容。

## 另见

- [accessibility.md](accessibility.md) — `A11yInfo`、角色、提供者树
- [widget-trait.md](widget-trait.md) — `content_caps` 与皮肤协议
- [layout-documents.md](layout-documents.md) — XML 属性词汇表
- [Theming guide](../guides/theming.md)
