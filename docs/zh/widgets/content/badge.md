# Badge

> 带色调颜色和可选移除目标的小型状态 chip。

## 介绍

`Badge<M>` 绘制一个胶囊形状的状态 chip：着色填充上的小号文本，右侧可选一个关闭字形
（`×`）。提供五种色调。`Neutral` 和 `Accent` 遵循调色板；`Success`、`Warning` 和
`Danger` 在浅色和深色调色板中都使用固定颜色，因此无论强调色如何设置，它们的显示效果
都相同。

普通徽标是装饰（`focusable` 为 `false`，无障碍节点是仅显示的 `Text`）。
`.removable(handler)` 因拥有一个关闭目标而使实例变为可交互：点击 `×` 或在实例聚焦时
按 Delete/Backspace 会发出处理器的消息。悬停着色仅限于关闭目标，而不是整个 chip。
`M` 是移除处理器返回的消息类型。

## 用法（Rust）

```rust
use bamboo_native_ui::widgets::{Badge, BadgeTone};
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg { RemoveTag }

fn build(ui: &mut Ui<Msg>) {
    ui.add(Badge::new("lossless", "Lossless").tone(BadgeTone::Success));
    ui.set_rect(WidgetId::new("lossless"), Rect::from_xywh(16.0, 16.0, 80.0, 22.0));

    ui.add(
        Badge::new("flac", "FLAC")
            .tone(BadgeTone::Neutral)
            .removable(|| Msg::RemoveTag),
    );
    ui.set_rect(WidgetId::new("flac"), Rect::from_xywh(16.0, 46.0, 72.0, 22.0));
}
```

可移除徽标为其关闭目标预留空间，因此请给它足够的宽度以容纳文本加上一个框高度。
chip 半径始终为高度的一半。

## 用法（XML）

`badge` 是标准注册表类型。在安装了 `WidgetRegistry::standard()` 时，用 `type="badge"` 创建它：

```xml
<widget id="lossless" type="badge" x="16" y="16" w="80" h="22"
        label="Lossless" tone="success"/>
```

工厂初始为空文本；`label` 内容会填充它。在 Rust 中创建的徽标通过 id 引用：

```xml
<widget id="flac" x="16" y="46" w="72" h="22" label="FLAC"/>
```

`tone` 接受 `neutral`、`accent`、`success`、`warning` 或 `danger`。可移除处理器只能在
构造函数中设置，无法在 XML 中声明。

## 属性

| 方法 | 类型 | 默认值 | 描述 |
|---|---|---|---|
| `new(id, text)` | `&'static str, impl Into<String>` | — | 创建 chip。 |
| `.tone(BadgeTone)` | `BadgeTone` | `BadgeTone::Neutral` | 强调色。 |
| `.removable(handler)` | `impl Fn() -> M + 'static` | `None` | 绘制关闭目标并在移除时发出消息。 |

Getter 与 setter：`text()`、`is_removable()`、`tone_value()` 和
`set_tone(BadgeTone)`。`BadgeTone::parse(&str)` 解析皮肤名称。

行为说明：

- 关闭目标是 chip 末尾 `min(height, 24)` DIP 的区域。
- 当实例接收键盘输入时，`Delete`/`Backspace` 会移除徽标；为此需要将其设为 `focusable`。
- `interactive()` 和 `repaint_on_move()` 仅在可移除徽标上为 true。
- 无障碍：`A11yRole::Text`，仅显示，名称来自徽标文本。

## 事件

### 消息

| 消息 | 发出时机 |
|---|---|
| 传给 `.removable(handler)` 的 `Fn() -> M` | 点击关闭目标，或在徽标拥有键盘输入时 Delete/Backspace 到达徽标。 |

### 通用观察器

`Badge` 不安装共享观察器构建器。它唯一的回调是控件专属的 `.removable(handler)`；
仅当附加了该处理器时徽标才是可交互的（此时 `interactive()` 和 `repaint_on_move()` 为
true）。如有需要，用 `Ui::bind` 绑定额外的指针触发器。

### 控件专属回调

| 回调 | 处理器 | 触发条件 |
|---|---|---|
| `.removable(handler)` | `impl Fn() -> M + 'static` | 点击关闭目标，或在徽标接收键盘输入时按下 Delete/Backspace。 |

### Ui 触发器

任意控件 id 都可以用 `Ui::bind(id, Trigger, handler)`（或 `Ui::on_click` /
`Ui::on_activate` 简写）进行绑定。返回 `Some(M)` 的处理器会将消息入队；除非处理器调用
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

可移除徽标会对 `Click`（仅限关闭目标内部）以及 `HoverEnter`/`HoverLeave`
（关闭目标着色）做出反应；Delete/Backspace 是普通按键事件，不处理 `Activate`。

## XML 定义

`badge` 是标准注册表类型，需要 `WidgetRegistry::standard()`。不适用 `command`，
因为标准工厂没有消息。

- 几何：适用常规 `widget` 规则。
- 共享内容：仅 `label`。
- 本地属性：`tone`（上述五个名称）。未知色调会报 `xml.skin.value`。
- 样式 token：该控件没有 `SurfaceStyle`；`radius`、`corners` 和 `background`
  不受支持（`xml.style.unsupported`，一个警告）并被跳过。
- 子元素：无。

## 另见

- `../index.md`
- `kbd.md`
- `../controls/button.md`
- `../../api/theme-and-palette.md`
- `../../guides/xml-layout-authoring.md`
