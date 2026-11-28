# CopyableText

> 带复制提示和应用复制消息的只读文本。

## 介绍

`CopyableText<M>` 绘制一行文本，并且在设置 `.copyable(handler)` 时，在右侧绘制
复制字形。点击控件、激活它，或在它聚焦时按 Ctrl+C，会发出 `handler()`。该控件从不接触
平台剪贴板：由应用根据消息写入剪贴板（例如使用 `WindowHandle::copy_text`），
这使控件保持可测试且平台中立。

没有处理器时，文本是普通装饰，且不绘制字形。有处理器时，实例可接受指针交互并显示手形
光标；悬停时字形变为强调色。`M` 是复制处理器返回的消息类型。

## 用法（Rust）

```rust
use bamboo_native_ui::widgets::CopyableText;
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg { CopyDeviceId }

fn build(ui: &mut Ui<Msg>) {
    ui.add(
        CopyableText::new("device_id", "Bamboo-7F3A")
            .copyable(|| Msg::CopyDeviceId),
    );
    ui.set_rect(WidgetId::new("device_id"), Rect::from_xywh(16.0, 16.0, 200.0, 22.0));
}
```

把相同字符串写入剪贴板来处理该消息。希望键盘 Ctrl+C 生效时，把实例设为
`focusable`（通过 `WidgetProps`）；指针点击无需此项即可工作。

## 用法（XML）

`CopyableText` 不在标准注册表中。在 Rust 中创建它，并从文档中通过 `id` 引用：

```xml
<widget id="device_id" x="16" y="16" w="200" h="22" text="Bamboo-7F3A"/>
```

`text` 和 `label` 都被接受为共享内容。复制处理器只能在构造函数中设置；`type` 条目无法
创建此控件。

## 属性

| 方法 | 类型 | 默认值 | 描述 |
|---|---|---|---|
| `new(id, text)` | `&'static str, impl Into<String>` | — | 创建文本行。 |
| `.copyable(handler)` | `impl Fn() -> M + 'static` | `None` | 绘制字形并在点击/激活/Ctrl+C 时发出消息。 |
| `.style(LabelStyle)` | `LabelStyle` | `LabelStyle::Small` | 主题文本角色。 |

Getter 与 setter：`text()`、`color_value()`、`text_style()`、
`set_color(Option<Color>)` 和 `set_text_style(LabelStyle)`。

行为说明：

- 复制字形为 `U+E8C8`，右侧占用 `min(height, 18)` DIP；文本框在其前 4 DIP 处结束。
- 仅在附加了处理器时设置 `interactive()` 和 `cursor()`。
- 无障碍：`A11yRole::Text`，仅显示，名称来自文本。

## 事件

### 消息

| 消息 | 发出时机 |
|---|---|
| 传给 `.copyable(handler)` 的 `Fn() -> M` | 点击复制字形、激活文本（Enter/Space），或在文本拥有焦点时按 Ctrl+C。 |

### 通用观察器

`CopyableText` 不安装共享观察器构建器。它唯一的回调是控件专属的 `.copyable(handler)`；
没有它时 `input` 返回 `None`、`interactive()` 为 `false`，且不绘制字形。

### 控件专属回调

| 回调 | 处理器 | 触发条件 |
|---|---|---|
| `.copyable(handler)` | `impl Fn() -> M + 'static` | 点击该行、`Activate`（Enter/Space），或在聚焦时按 Ctrl+C。控件自身从不写入剪贴板。 |

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

附加处理器后，`Click` 和 `Activate` 会发出复制消息，`HoverEnter`/`HoverLeave` 为字形着色；
Ctrl+C 是普通按键事件，不是触发器。没有处理器时，不会传递任何指针触发器。

## XML 定义

此控件不由标准注册表创建。在 Rust 中创建它，并从文档中通过 `id` 引用。

- 几何：适用常规 `widget` 规则。
- 共享内容：`text` 和 `label`。
- 本地属性：`color` 和 `text-style`（`body`、`small`、`title`）。
- 样式 token：无 `SurfaceStyle`；`radius`、`corners` 和 `background` 不受支持
  （`xml.style.unsupported`，一个警告）。
- 子元素：无。

## 另见

- `../index.md`
- `label.md`
- `marquee-text.md`
- `../../guides/xml-layout-authoring.md`
