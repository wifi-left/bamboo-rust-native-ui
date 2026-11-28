# Scrim

> 一种变暗、阻止输入的背景，用于窗口内模态外壳。

## 介绍

`Scrim` 在页面之上绘制一个半透明黑色矩形，并吞掉落在其上的每一次指针按下。它是该库窗口内模态模式的背景：将 scrim 添加到页面控件之后、模态面板之前，面板及其控件便保持可点击，而背后的页面无法被触及。

scrim 在视觉上是半透明的，但在输入上是不可穿透的：`interactive()` 为 true，`hit_test` 覆盖其矩形，`input` 消费按下且不返回消息。它不可聚焦，也没有无障碍节点。它的像素从不依赖指针状态（`paints_pointer_state` 为 false），因此在全窗口 scrim 上移动指针不会使页面失效。

`Scrim` 是浮层分隔点。`Widget::is_backdrop` 仅对它为 true：它之前的控件构成页面层，它之后的控件构成浮层。借助 `Host::overlay_paint`，宿主将模态合成到缓存的页面之上，而不是每帧重绘页面（D87）。由文档创建的控件始终被插入到第一个背景之前，因此它留在页面层。

## 用法（Rust）

```rust
use bamboo_native_ui::widgets::{Card, Scrim};
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg { Close }

let mut ui: Ui<Msg> = Ui::new();
ui.add(Card::new("page_card", "Library", "12 tracks"));
ui.add(Scrim::new("modal_scrim"));
ui.add(Card::new("modal_panel", "Delete playlist?", "This cannot be undone."));

ui.set_rect(WidgetId::new("page_card"), Rect::from_xywh(16.0, 16.0, 320.0, 96.0));
ui.set_rect(WidgetId::new("modal_scrim"), Rect::from_xywh(0.0, 0.0, 640.0, 480.0));
ui.set_rect(WidgetId::new("modal_panel"), Rect::from_xywh(160.0, 160.0, 320.0, 120.0));
```

默认矩形为空，因此 scrim 必须始终设置尺寸，通常每次调整大小时都设为整个客户区。显示和隐藏它是通过 `Ui::set_visible` 进行的普通可见性切换。配合 `Ui::set_focus_scope(Some(vec![...]), sink)` 使 Tab 保持在面板内，并在模态关闭时配合 `set_focus_scope(None, sink)`。

## 用法（XML）

该控件不是由标准注册表创建的。请在 Rust 中创建它，并从文档中通过 `id` 寻址。文档可以为它定位并切换通用 `visible` 属性：

```xml
<widget id="modal_scrim" x="0" y="0" w="cw" h="ch"/>
```

alpha 仅在构造时设置，无法在 XML 中声明。

## 属性

| 方法 | 类型 | 默认值 | 描述 |
|---|---|---|---|
| `new(id)` | `&'static str` | — | 创建一个空矩形、alpha 为 0.45 的 scrim。 |
| `.alpha(f32)` | `f32` | `0.45` | 黑色填充的不透明度，钳制到 `0.0..=1.0`。 |

### 行为

- **输入阻止。** 矩形内的按下会被消费；背景后的页面控件永远看不到悬停、按下或点击。
- **绘制。** 填充是以 `alpha` 的黑色；当容器提供脏区域列表时，仅混合相交的部分，因此在模态内悬停不会重新混合整个窗口。
- **光标。** scrim 请求箭头光标。
- **首选高度。** `0.0`；该控件仅绘制你分配的矩形。
- **无障碍。** 不暴露任何节点；模态面板承载语义。
- **顺序。** 在每一个浮层控件之前添加它。将其移到页面中间会使部分页面落入浮层。

## 事件

scrim 接收落在其上的每一次按下并对它们全部返回 `None`，因此它从不发出消息，也没有回调。关闭模态是应用程序的职责：scrim id 上的 `Click` 绑定，或面板内的关闭控件，会派发它自己的消息。

### 消息

无。构造函数不接收回调。

### 通用观察器

该控件不安装任何共享的观察器构建器。它不可聚焦，因此其上不存在 `on_click`、`on_right_click`、`on_double_click`、`on_key_down`、`on_key_up`、`on_focus_gained` 和 `on_focus_lost`。

### 控件专属回调

无。

### Ui 触发器

任何控件 id 都可以用 `Ui::bind(WidgetId::new("modal_scrim"), Trigger::Click, |event, ctx| ...)` 绑定；返回 `Some(Msg::Close)` 会排队一条消息，并且处理器返回后内置行为仍会运行（`ctx.run_default()` 会内联运行它，两者都不抑制它）。scrim 除吞掉按下外没有内置行为，因此该绑定是“点击外部关闭”的模态报告自身的方式。

| 触发器 | 触发时机 |
|---|---|
| `Click` | 在同一控件上按下并释放。 |
| `DoubleClick` | 平台报告双击。 |
| `RightClick` | 次键点击，在上下文菜单之前。 |
| `HoverEnter` / `HoverLeave` | 指针进入 / 离开控件。 |
| `Wheel` | 控件上的滚轮事件（`event.wheel_delta`）。 |
| `Activate` | 聚焦控件上的 Enter/Space，或 UIA Invoke/Toggle。 |
| `DragStart` / `DragEnd` | 指针越过拖动阈值 / 按钮被释放。 |
| `ValueChange` | 控件的无障碍值发生变化（在内置更新之后）。 |

只有 `Click` 有意义——它是唯一能观察到 scrim 按下的触发器。scrim 不可聚焦，也不暴露任何无障碍节点，因此 `Activate`、按键/焦点触发器和 `ValueChange` 永远不会触发。

## XML 定义

该控件不是由标准注册表创建的。请在 Rust 中创建它，并从文档中通过 `id` 寻址。

- **几何：** 适用普通的 `widget` 规则；将其尺寸设为客户区。
- **内容：** 无。不支持 `label` 和 `text`（`xml.content.unsupported`，一条警告），并会被跳过。
- **局部属性：** 无。`skin_attrs()` 为空。
- **样式令牌：** 接受 `font-size` 和 `font-family`，但不绘制任何内容（scrim 不绘制文本）；该控件没有 `SurfaceStyle`，因此 `radius`、`corners` 和 `background` 是 `xml.style.unsupported` 警告，会被跳过。填充始终是内置的黑色混合。
- **子控件：** 无。
- **属性：** 适用通用运行时集合，包括 `visible`，这正是从文档打开和关闭模态的方式。

## 另见

- `../display/card.md`
- `toast.md`
- `../index.md`
- `../../api/widget-trait.md`
- `../../guides/xml-layout-authoring.md`
