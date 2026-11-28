# Avatar

> 圆形首字母头像，或裁剪为圆形显示的正方形图片。

## 介绍

`Avatar` 绘制一个带最多两个首字母的填充圆形，或者当通过 `Ui::set_image` 注册了图片时绘制
图片。首字母由名称推导而来：两个单词取其首字母，一个 ASCII 单词取其前两个字符，
一个非 ASCII 单词取其第一个字符。`.initials` 覆盖该推导结果。

没有显式 `.tint` 时，圆形颜色会从六个固定色调中基于名称确定性地选择，因此同一个艺术家在
不同页面上保持同一种颜色。`.ring(true)` 为重叠头像绘制一个 1 DIP 的圆环。已注册的图片会
中心裁剪为正方形并抗锯齿到圆形；未注册的 id 回退到首字母。命中测试遵循圆形，
而不是矩形的角。

该控件是装饰（`focusable` 为 `false`），并发布一个由全名命名的仅显示 `Text` 节点
（当名称为空时则用首字母）。它不发送消息，因此 `M` 只是容器的消息类型。

## 用法（Rust）

```rust
use bamboo_native_ui::widgets::Avatar;
use bamboo_native_ui::{Color, ImageId, Rect, Ui, WidgetId};

enum Msg {}

fn build(ui: &mut Ui<Msg>) {
    ui.add(Avatar::new("artist", "Annie Lennox"));
    ui.set_rect(WidgetId::new("artist"), Rect::from_xywh(16.0, 16.0, 40.0, 40.0));

    ui.add(
        Avatar::new("me", "竹韵")
            .initials("ZY")
            .tint(Color::new(96, 134, 208))
            .ring(true),
    );
    ui.set_rect(WidgetId::new("me"), Rect::from_xywh(64.0, 16.0, 40.0, 40.0));

    ui.add(Avatar::new("cover", "Nocturne").image(ImageId(1)));
    ui.set_rect(WidgetId::new("cover"), Rect::from_xywh(112.0, 16.0, 40.0, 40.0));
}
```

矩形应为正方形；圆形使用宽度和高度中较小的那个。`ImageId(1)` 必须引用通过
`Ui::set_image` 注册的图片（例如 `UiImage::gradient`）；未知 id 会静默回退到首字母。

## 用法（XML）

`avatar` 是标准注册表类型。在安装了 `WidgetRegistry::standard()` 时，用 `type="avatar"`
创建它：

```xml
<widget id="artist" type="avatar" x="16" y="16" w="40" h="40"
        label="Annie Lennox"/>
```

本地属性覆盖推导出的外观：

```xml
<widget id="cover" type="avatar" x="64" y="16" w="40" h="40"
        initials="ZY" tint="#6090d0" ring="true" image="1"/>
```

`image` 是应用注册的图片的数字 `ImageId`；页面中没有图片名称注册表。在 Rust 中创建的
头像通过 id 引用：

```xml
<widget id="me" x="112" y="16" w="40" h="40"/>
```

## 属性

| 方法 | 类型 | 默认值 | 描述 |
|---|---|---|---|
| `new(id, name)` | `&'static str, impl Into<String>` | — | 创建头像并推导首字母。 |
| `.initials(impl Into<String>)` | `impl Into<String>` | 从 `name` 推导 | 显式的圆形文本。 |
| `.image(ImageId)` | `ImageId` | `None` | 显示已注册的图片而非首字母。 |
| `.tint(Color)` | `Color` | `None`（基于名称的固定色调） | 圆形填充颜色。 |
| `.ring(bool)` | `bool` | `false` | 在圆形周围绘制一个 1 DIP 的圆环。 |

Getter 与 setter：`name()`、`initials_text()`、`set_initials`、
`tint_color()`、`set_tint(Option<Color>)`、`has_ring()`、`set_ring(bool)`、
`image_id()` 和 `set_image_id(Option<ImageId>)`。

首字母上的文本颜色根据圆形的亮度选择（深色填充上接近白色，浅色填充上接近黑色）。

## 事件

### 消息

无。`Avatar` 绘制首字母或图片，从不返回消息。

### 通用观察器

`Avatar` 不安装共享观察器构建器。它不可聚焦，`interactive()` 为 `false`；其圆形
`hit_test` 只有在实例通过 `WidgetProps::mouse(true)` 选择启用指针输入后才重要，
此后 `Ui::bind` / `Ui::on_click` 可以接收点击。

### 控件专属回调

无。

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

默认情况下头像对它们都无反应；使用 `WidgetProps::mouse(true)` 后，圆形内部的 `Click`
会到达绑定。其他触发器没有意义。

## XML 定义

`avatar` 是标准注册表类型，需要 `WidgetRegistry::standard()`。不适用 `command`。

- 几何：适用常规 `widget` 规则；正方形矩形得到圆形。
- 共享内容：仅 `label`。它会替换名称并重新推导首字母。
- 本地属性：`initials`（文本）、`tint`（颜色语法或调色板 token）、`ring`（bool）
  和 `image`（用作 `ImageId` 的完整非负数）。
- 样式 token：无 `SurfaceStyle`；`radius`、`corners` 和 `background` 不受支持
  （`xml.style.unsupported`，一个警告）。
- 子元素：无。

## 另见

- `../index.md`
- `label.md`
- `../display/card.md`
- `../../guides/xml-layout-authoring.md`
