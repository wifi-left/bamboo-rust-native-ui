# NumberInput

> 带范围、可选步进区、固定小数位数和自定义格式化的整数字段。

## 介绍

`NumberInput<M>` 绘制一个带边框的数字字段，数字前后有前缀和后缀，并且使用 `.steppers()`
时，在右边缘绘制一对上/下区域。点击文本区域会打开一个绘制的输入会话，接受数字、小数点
以及（对于最小值为负的范围）负号。提交时会将文本解析为整数，钳制到构造函数范围，
并在值发生变化时发出 `on_change(value)`；无法解析的文本保留旧值。点击步进区会按
`step_size` 递增/递减并立即发出；步进区是点击目标，因此在其上按下永远不会打开编辑器。

值始终保持为整数。`.decimals(n)` 只改变显示和解析格式（`value = 250` 且 2 位小数绘制为
`2.50`，输入 `2.75` 提交 `275`），因此步进器和无障碍范围模式仍以原始单位工作。
`.format`/`.parse` 用应用自己的文本转换替换该定点形式。

别名 `NumberStepper<M>` 是同一类型（见 `bamboo_native_ui::widgets::NumberStepper`）。
无障碍为 `A11yRole::NumberField`，带当前值、范围和 `read_only` 标志。`M` 是
`on_change` 返回的消息类型。

## 用法（Rust）

```rust
use bamboo_native_ui::widgets::NumberInput;
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg { Volume(i64) }

fn build(ui: &mut Ui<Msg>) {
    ui.add(
        NumberInput::new("volume", 60, 0..=100, Msg::Volume)
            .suffix("%")
            .step_size(5)
            .steppers(),
    );
    ui.set_rect(WidgetId::new("volume"), Rect::from_xywh(16.0, 16.0, 120.0, 34.0));

    ui.add(NumberInput::new("price", 250, 0..=100_000, Msg::Volume).decimals(2).prefix("¥"));
}
```

构造函数会将初始值钳制到范围内。将 `.format` 与理解相同文本的 `.parse` 配对；没有自定义
解析器时则使用定点形式。`set_value` 和 `step` 静默改变值；只有步进器和提交的编辑会发出消息。

## 用法（XML）

`NumberInput` 不在标准注册表中。在 Rust 中创建它，并从文档中通过 `id` 引用：

```xml
<widget id="volume" x="16" y="16" w="120" h="34"
        number="60" suffix="%"/>
```

`number` 是共享内容（`content_caps` 公开 `NUMBER`），通过 `Ui::set_number` 写入，
并钳制到范围内。`prefix` 和 `suffix` 是本地属性。字段采用 `SurfaceStyle`，因此
`radius`/`corners` 和 `background` 适用。

## 属性

| 方法 | 类型 | 默认值 | 描述 |
|---|---|---|---|
| `new(id, value, range, on_change)` | `&'static str, i64, RangeInclusive<i64>, impl Fn(i64) -> M + 'static` | — | 创建字段（值钳制到范围内）。 |
| `.steppers()` | — | 关闭 | 在右边缘显示上/下区域。 |
| `.step_size(i64)` | `i64` | `1` | 步进器使用的增量（最小 1）。 |
| `.decimals(u8)` | `u8` | `0` | 显示和解析的固定小数位数（最大 9）。 |
| `.prefix(impl Into<String>)` | `impl Into<String>` | `""` | 绘制在数字之前的文本。 |
| `.suffix(impl Into<String>)` | `impl Into<String>` | `""` | 绘制在数字之后的文本。 |
| `.format(Fn(i64) -> String)` | `impl Fn(i64) -> String + 'static` | `None` | 自定义显示文本。 |
| `.parse(Fn(&str) -> Option<i64>)` | `impl Fn(&str) -> Option<i64> + 'static` | `None` | 对提交文本的自定义解析器。 |
| `.align(TextAlign)` | `TextAlign` | `TextAlign::Right` | 数字对齐方式。 |
| `.font_size(f32)` | `f32` | `None`（主题正文） | 每个字段的文本大小，单位为 DIP。 |
| `.read_only(bool)` | `bool` | `false` | 拒绝输入和步进器；保留焦点/选择/复制。 |
| `.disabled()` | — | 启用 | 禁用字段。 |
| `.corner_radius(f32)` | `f32` | `0.0` | 字段框角半径。 |
| `.corners(Corners)` | `Corners` | 零 | 逐角的框半径。 |
| `.background(Color)` | `Color` | `None`（调色板 `search_bg`） | 框填充覆盖。 |
| `.background_option(Option<Color>)` | `Option<Color>` | `None` | 设置或清除填充覆盖。 |
| `.transparent_background(bool)` | `bool` | `false` | 抑制填充，保留边框。 |

Getter 与 setter：`value()`、`display()`、`is_editing()`、`is_read_only()`、
`prefix_text()`、`suffix_text()`、`set_prefix`、`set_suffix`、
`set_value(i64) -> bool`（静默，钳制）、`step(i64) -> bool`（静默，
在范围内按原始单位步进）和 `surface()`。

## 事件

### 消息

| 消息 | 发出时机 |
|---|---|
| 传给 `new`（`on_change`）的 `Fn(i64) -> M` | 步进器点击或提交的编辑改变了值；参数是新的钳制整数。无法解析的文本保留旧值且不发出任何消息。 |

### 通用观察器

`NumberInput` 不安装共享观察器构建器。其机制是构造函数的 `on_change` 回调和内联绘制会话；
使用 `Ui::bind` 添加额外触发器。

### 控件专属回调

除 `on_change` 外无。`.format`/`.parse` 是值显示/解析钩子，`.set_value`/`.step` 是静默
设置器，不是事件回调。

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

字段对步进区上的 `Click` 和 `Activate`（打开输入会话）做出反应；
`HoverEnter`/`HoverLeave` 高亮指针下的步进器。滚轮不被消费。`ValueChange` 跟随值变化。

## XML 定义

此控件不由标准注册表创建。在 Rust 中创建它，并从文档中通过 `id` 引用。

- 几何：适用常规 `widget` 规则；默认高度为 34 DIP。
- 共享内容：`number`。
- 本地属性：`prefix` 和 `suffix`（文本）。
- 样式 token：通过 `SurfaceStyle` 支持 `radius`、`corners` 和 `background`；
  `font-size` 和 `font-family` 适用。
- 子元素：无。范围、步进器、小数位数和格式化/解析钩子只能在构造函数中设置。

## 另见

- `../index.md`
- `text-edit.md`
- `../controls/slider.md`
- `../../api/theme-and-palette.md`
- `../../guides/xml-layout-authoring.md`
