# Kbd

> 一个显示诸如 `Ctrl+K` 之类键盘快捷键的带边框 chip。

## 介绍

`Kbd` 把文本绘制在小型圆角 chip 内：一个 1 DIP 的边框、调色板 `button_bg` 填充、
小号文本角色和弱化前景色。圆角使用主题的小号圆角。它是一种固定呈现；没有大小或颜色构建器，
更大的矩形只是把相同的 chip 文本居中。

该控件是装饰（`focusable` 为 `false`），并发布一个由快捷键命名的仅显示 `Text` 节点。
它不发送消息，因此 `M` 只是容器的消息类型。

## 用法（Rust）

```rust
use bamboo_native_ui::widgets::Kbd;
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg {}

fn build(ui: &mut Ui<Msg>) {
    ui.add(Kbd::new("kbd_search", "Ctrl+K"));
    ui.set_rect(WidgetId::new("kbd_search"), Rect::from_xywh(16.0, 16.0, 56.0, 22.0));
}
```

使用 `Ui::set_label` 接缝（或 `set_text`）在运行时更新快捷键；`set_text` 是 XML
`shortcut` 属性使用的静默皮肤路径。

## 用法（XML）

`kbd` 是标准注册表类型。在安装了 `WidgetRegistry::standard()` 时，用 `type="kbd"` 创建它：

```xml
<widget id="kbd_search" type="kbd" x="16" y="16" w="56" h="22"
        shortcut="Ctrl+K"/>
```

工厂初始为空文本；`shortcut` 提供文本。在 Rust 中创建的 `Kbd` 通过 id 引用：

```xml
<widget id="kbd_save" x="16" y="46" w="56" h="22" label="Ctrl+S"/>
```

## 属性

| 方法 | 类型 | 默认值 | 描述 |
|---|---|---|---|
| `new(id, text)` | `&'static str, impl Into<String>` | — | 创建提示 chip。 |

Getter 与 setter：`text()` 和 `set_text(impl Into<String>)`（静默）。

## 事件

### 消息

无。`Kbd` 只绘制快捷键 chip，从不返回消息。

### 通用观察器

`Kbd` 不安装共享观察器构建器。它不可聚焦，`interactive()` 为 `false`，也不接收输入，
因此没有指针事件到达它。提示必须可点击时，用 `WidgetProps::mouse(true)` 选择启用，
并用 `Ui::bind` / `Ui::on_click` 绑定。

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

默认情况下 chip 对它们都无反应；只有 `WidgetProps::mouse(true)` 选择启用才会将 `Click`
和指针触发器传递到绑定。

## XML 定义

`kbd` 是标准注册表类型，需要 `WidgetRegistry::standard()`。不适用 `command`。

- 几何：适用常规 `widget` 规则。chip 填满矩形；56×22 DIP 可舒适地容纳三到四个字符。
- 共享内容：仅 `label`。
- 本地属性：`shortcut`（文本）。`shortcut` 和 `label` 都设置同一个显示字符串。
- 样式 token：无 `SurfaceStyle`；`radius`、`corners` 和 `background` 不受支持
  （`xml.style.unsupported`，一个警告）。
- 子元素：无。

## 另见

- `../index.md`
- `badge.md`
- `../controls/button.md`
- `../../guides/xml-layout-authoring.md`
