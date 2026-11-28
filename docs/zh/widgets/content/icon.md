# Icon

> 一个居中的字形，按其单元格着色并调整大小。

## 介绍

`Icon` 在其矩形中居中绘制一个字符。字形始终用字形字体（Segoe MDL2 Assets）绘制，
因此请使用诸如 `U+E721` 这样的 MDL2 码位；字体中不包含的字符会显示为缺失字形。
默认大小为单元格高度的 80%，默认色调为调色板的 `muted` 颜色。

图标是装饰：`focusable` 为 `false`，无障碍树看到的是一个名称为空、仅显示的 `Text` 节点，
因此带有含义的图标应通过 `WidgetProps` 获得一个朗读名称。控件从不发送消息，
因此 `M` 只是容器的消息类型。

## 用法（Rust）

```rust
use bamboo_native_ui::widgets::Icon;
use bamboo_native_ui::{Color, Rect, Ui, WidgetId};

enum Msg {}

fn build(ui: &mut Ui<Msg>) {
    ui.add(Icon::new("search_icon", '\u{E721}').size(18.0));
    ui.set_rect(WidgetId::new("search_icon"), Rect::from_xywh(16.0, 16.0, 24.0, 24.0));

    ui.add(Icon::new("star", '\u{E734}').tint(Color::new(226, 168, 58)));
    ui.set_rect(WidgetId::new("star"), Rect::from_xywh(48.0, 16.0, 24.0, 24.0));
}
```

`.size` 的单位为 DIP，且必须至少为 1；若不设置，字形会遵循当前单元格高度。`.tint`
覆盖弱化的调色板颜色。字形在整个矩形中居中。

## 用法（XML）

`icon` 是标准注册表类型。在安装了 `WidgetRegistry::standard()` 时，用 `type="icon"` 创建它：

```xml
<widget id="search_icon" type="icon" x="16" y="16" w="24" h="24"
        glyph="&#xE721;" tint="muted"/>
```

工厂初始为一个空格字形；`glyph` 属性提供真正的字符。在 Rust 中创建的图标仅通过 id 引用：

```xml
<widget id="star" x="48" y="16" w="24" h="24"/>
```

## 属性

| 方法 | 类型 | 默认值 | 描述 |
|---|---|---|---|
| `new(id, glyph)` | `&'static str, char` | — | 创建图标。 |
| `.size(f32)` | `f32` | `None`（单元格高度的 80%） | 字形的 DIP 大小，至少钳制为 1。 |
| `.tint(Color)` | `Color` | `None`（调色板 `muted`） | 字形颜色。 |

Getter 与 setter：`glyph()`、`tint_color()`、`set_glyph(char)` 和
`set_tint(Option<Color>)`。

## 事件

### 消息

无。`Icon` 只绘制字形，从不返回消息。

### 通用观察器

`Icon` 不安装共享观察器构建器。它的 `input` 始终返回 `None`，不可聚焦且
`interactive()` 为 `false`，因此不参与命中测试。要使其可点击，请安装
`WidgetProps::mouse(true)`，并用 `Ui::bind` / `Ui::on_click` 绑定。

### 控件专属回调

无。

### Ui 触发器

任意控件 id 都可以用 `Ui::bind(id, Trigger, handler)`（或 `Ui::on_click` /
`Ui::on_activate` 简写）绑定。返回 `Some(M)` 的处理器会将消息入队；除非处理器调用
`ctx.run_default()`，否则内置操作仍会执行。

| 触发器 | 触发条件 |
|---|---|
| `Click` | 在同一控件上按下并释放。 |
| `DoubleClick` | 平台报告双击。 |
| `RightClick` | 次按键点击，发生在上下文菜单之前。 |
| `HoverEnter` / `HoverLeave` | 指针进入 / 离开控件。 |
| `Wheel` | 控件上发生滚轮事件（`event.wheel_delta`）。 |
| `Activate` | 焦点控件上按 Enter/Space，或 UIA Invoke/Toggle。 |
| `DragStart` / `DragEnd` | 指针越过拖动阈值 / 释放按键。 |
| `ValueChange` | 控件的无障碍值发生变化（在内置更新之后）。 |

默认情况下图标对它们都无反应；只有 `WidgetProps::mouse(true)` 选择启用后，才会将 `Click`
（以及指针触发器）路由到绑定。`ValueChange` 和 `Activate` 从不会产生。

## XML 定义

`icon` 是标准注册表类型，需要在应用文档之前安装 `WidgetRegistry::standard()`。
不适用 `command`。

- 几何：适用常规 `widget` 规则（一种横向形式和一种纵向形式）。
- 共享内容：无。图标没有 `label`/`text` 内容能力。
- 本地属性：`glyph`（恰好一个字符）和 `tint`（颜色语法或调色板 token；会丢弃 alpha）。
- 样式 token：该控件没有 `SurfaceStyle`，因此 `radius`、`corners` 和 `background`
  会被报告为不支持（`xml.style.unsupported`，一个警告）并被跳过。
- 子元素：无。

## 另见

- `../index.md`
- `../display/card.md`
- `../../api/theme-and-palette.md`
- `../../guides/xml-layout-authoring.md`
