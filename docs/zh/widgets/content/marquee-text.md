# MarqueeText

> 放不下时会滚动的单行标题。

## 介绍

`MarqueeText` 绘制一行文本。当测量后的文本适合矩形时，它按左对齐静态绘制。当不适配时，
文本会被裁剪并滚动：在开头停留 `pause` 秒，以 `speed` DIP/秒 移动 `span + gap` DIP，
在末尾停留，然后重复。绘制两份副本，中间间隔 `gap` DIP，使回绕看起来无缝。

该控件不拥有计时器。应用通过 `Ui::edit_widget::<MarqueeText>(id, |m| m.advance(dt))`
从其动画 tick 中推进它，并且仅在 `advance` 返回 `true`（绘制的偏移发生变化）时使其无效。
它从不接收输入，并发布一个由完整字符串命名的仅显示 `Text` 节点。

## 用法（Rust）

```rust
use std::time::Duration;
use bamboo_native_ui::widgets::MarqueeText;
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg {}

fn build(ui: &mut Ui<Msg>) {
    ui.add(
        MarqueeText::new("now_playing", "Now playing: 竹韵 · 夜曲 · Lossless")
            .speed(36.0)
            .gap(40.0)
            .pause(1.2),
    );
    ui.set_rect(WidgetId::new("now_playing"), Rect::from_xywh(16.0, 16.0, 240.0, 22.0));

    // 从宿主的动画 tick 中调用它，约每秒 60 次。
    ui.edit_widget::<MarqueeText>(WidgetId::new("now_playing"), |m| {
        m.advance(Duration::from_millis(16))
    });
}
```

新文本会将动画重置到开头。`is_scrolling()` 报告上一次绘制是否测量到溢出，
因此宿主可以仅在需要时运行计时器。

## 用法（XML）

`MarqueeText` 不在标准注册表中。在 Rust 中创建它，并从文档中通过 `id` 引用：

```xml
<widget id="now_playing" x="16" y="16" w="240" h="22"
        text="Now playing: 竹韵 · 夜曲 · Lossless"
        offset="0" text-style="body"/>
```

`offset` 为绘制的偏移提供初值；应用的下一次 `advance` 会从它保持的时间继续，
因此这是一个初始值，而不是定位跳转。

## 属性

| 方法 | 类型 | 默认值 | 描述 |
|---|---|---|---|
| `new(id, text)` | `&'static str, impl Into<String>` | — | 创建该行。 |
| `.speed(f32)` | `f32` | `40.0` | 滚动速度，DIP/秒（最小 1）。 |
| `.gap(f32)` | `f32` | `48.0` | 结尾与重复开头之间的空白（最小 0）。 |
| `.pause(f32)` | `f32` | `1.5` | 每一端的停留时间，以秒为单位（最小 0）。 |
| `.style(LabelStyle)` | `LabelStyle` | `LabelStyle::Body` | 主题文本角色。 |

Getter 与 setter：`text()`、`offset()`、`is_scrolling()`、
`set_offset(f32)`、`color_value()`、`text_style()`、`set_color(Option<Color>)`
和 `set_text_style(LabelStyle)`。`advance(dt) -> bool` 是动画步进。

## 事件

### 消息

无。`MarqueeText` 只绘制；动画由 `advance(dt)` 驱动，而不是由输入驱动。

### 通用观察器

`MarqueeText` 不安装共享观察器构建器。它从不接收输入，不可聚焦且 `interactive()` 为
`false`，因此没有指针或按键事件到达它。请使用 `Ui::edit_widget::<MarqueeText>`
进行动画 tick，而不是使用事件回调。

### 控件专属回调

无。`advance(Duration) -> bool` 是动画步进，不是回调；容器从自己的计时器调用它。

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

跑马灯对它们都无反应；它根本没有交互路径。

## XML 定义

此控件不由标准注册表创建。在 Rust 中创建它，并从文档中通过 `id` 引用。

- 几何：适用常规 `widget` 规则。使用单行高度，例如 22 DIP。
- 共享内容：`text` 和 `label`。
- 本地属性：`offset`（非负数，单位为 DIP）、`color` 和 `text-style`
  （`body`、`small`、`title`）。
- 样式 token：无 `SurfaceStyle`；`radius`、`corners` 和 `background` 不受支持
  （`xml.style.unsupported`，一个警告）。
- 子元素：无。

## 另见

- `../index.md`
- `label.md`
- `copyable-text.md`
- `../../api/widget-trait.md`
- `../../guides/xml-layout-authoring.md`
