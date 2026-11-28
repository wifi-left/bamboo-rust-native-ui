# Breadcrumb

> 一条可点击的路径，除当前页面外的每个面包屑都会导航。

## 介绍

`Breadcrumb<M>` 绘制一行水平排列的标签，以箭头分隔。较早的面包屑是链接：点击其中一个，或在键盘光标上按下 Enter，会通过 `on_navigate` 报告其索引。最后一个面包屑是当前页面：它以常规文本色绘制，且从不导航。

标签在绘制时测量并被缓存，因此命中测试和绘制共享同一套布局。该控件按显示尺寸确定：它在自身矩形内绘制，对于过长的路径不滚动也不换行。

## 用法（Rust）

```rust
use bamboo_native_ui::{Rect, Ui, WidgetId};
use bamboo_native_ui::widgets::Breadcrumb;

enum Msg {
    Navigate(usize),
}

let mut ui: Ui<Msg> = Ui::new();
ui.add(
    Breadcrumb::new("location", &["Home", "Library", "Nocturne"], Msg::Navigate)
        .separator('/')
        .label("Location"),
);
ui.set_rect(WidgetId::new("location"), Rect::from_xywh(16.0, 16.0, 260.0, 24.0));
```

`new` 将最后一个标签视为当前页面，并将光标停在那里。`separator` 接受一个字符；私用区码点使用字形字体绘制，其他任何字符使用 UI 字体绘制。`set_items` 静默替换路径，并将光标移动到新的最后一个面包屑。

## 用法（XML）

该控件不由标准注册表创建。请在 Rust 中创建它，并在文档中通过 `id` 引用。`<step>` 子元素会替换路径；最后一步是当前页面：

```xml
<widget id="location" x="16" y="16" w="260" h="24" separator="/" name="Location">
  <step label="Home"/>
  <step label="Library"/>
  <step label="Nocturne"/>
</widget>
```

## 属性

| 方法 | 类型 | 默认值 | 描述 |
|---|---|---|---|
| `new(id, items, on_navigate)` | `&'static str, &[&str], impl Fn(usize) -> M` | — | 创建路径；最后一项为当前项。 |
| `.separator(char)` | `char` | MDL2 `U+E76C` | 在面包屑之间绘制的字符。 |
| `.label(impl Into<String>)` | text | widget id | 路径的无障碍名称。 |
| `.disabled(bool)` | `bool` | `false` | 绘制为禁用并忽略输入。 |
| `.items()` | `&[String]` | — | 标签。 |
| `.current_index()` | `Option<usize>` | last | 当前（不导航的）面包屑的索引。 |
| `.set_items(Vec<String>)` | silent | — | 替换路径；光标移动到最后一个面包屑。 |
| `.navigate(index)` | `Option<M>` | — | 通过常规路径导航；当前面包屑和越界索引返回 `None`。 |
| `.set_separator(char)` | `bool` | — | 运行时替换分隔符；会使已测量的路径失效。 |
| `.separator_char()` | `char` | — | 当前分隔符。 |

行为：只有最后一个之前的面包屑是导航目标；悬停其中一个会绘制强调色下划线和手形光标，而当前面包屑使用箭头光标。在调整模式下，Left/Right 移动光标（循环），Home/End 跳到第一个/最后一个面包屑，Enter/空格导航。首选高度为 24 DIP。无障碍角色为 `List`，当前面包屑处于选中状态。

## 事件

### 消息

| 消息 | 发出时机 |
|---|---|
| 作为 `on_navigate` 传给 `new` 的 `Fn(usize) -> M` | 较早的面包屑被点击，或 Enter/空格导航光标；参数为面包屑索引。当前（最后）面包屑从不导航。 |

`set_items` 和 `selected` 静默移动光标，不发出任何内容。

### 通用观察器

该控件不安装共享的观察器构建器（`on_click`、`on_right_click`、`on_double_click`、`on_key_down`、`on_key_up`、`on_focus_gained`、`on_focus_lost`）。构造函数的 `on_navigate` 就是它的事件接缝；其他任何内容都通过 `Ui::bind` 处理。

### 控件专属回调

| 方法 | 处理器 | 触发时机 |
|---|---|---|
| `new(id, items, on_navigate)` | `&'static str, &[&str], impl Fn(usize) -> M + 'static` | 存储导航回调；它是该控件声明的唯一回调。 |
| `.navigate(index)` | 返回 `Option<M>` | 运行时形式：正常导航并返回消息，或在当前面包屑、越界索引或禁用路径时返回 `None`。 |

### Ui 触发器

任何控件 id 都可以通过 `Ui::bind` 进行绑定：

```rust
ui.bind(WidgetId::new("location"), Trigger::Click, |event, ctx| {
    // 返回 Some(Msg::...) 以入队一条消息；除非显式调用 ctx.run_default()，
    // 内置操作仍会执行。
    None
});
```

| 触发器 | 触发时机 |
|---|---|
| `Click` | 在同一控件上按下并释放。 |
| `DoubleClick` | 平台报告双击。 |
| `RightClick` | 次级按钮点击，在上下文菜单之前。 |
| `HoverEnter` / `HoverLeave` | 指针进入 / 离开该控件。 |
| `Wheel` | 控件上的滚轮事件（`event.wheel_delta`）。 |
| `Activate` | 聚焦控件上按下 Enter/空格，或 UIA Invoke/Toggle。 |
| `DragStart` / `DragEnd` | 指针越过拖动阈值 / 释放按钮。 |
| `ValueChange` | 控件的无障碍值发生变化（在内置更新之后）。 |

它响应 `Click` 和 `Activate`（导航）以及 `HoverEnter`/`HoverLeave`（链接下划线）。`Wheel`、`RightClick`、拖动触发器以及 `ValueChange` 没有内置效果——所发布的选择是当前页面，只有路径本身能改变它。

## XML 定义

该控件不由标准注册表创建。请在 Rust 中创建它，并在文档中通过 `id` 引用。

几何布局遵循常规的 `<widget>` 规则。通用属性适用，包括 `enabled` 和 `name`。`font-size` 和 `font-family` 属于绘制层。路径从调色板绘制，没有 `SurfaceStyle`，因此 `radius`、`corners` 和 `background` 会被报告为 `xml.style.unsupported` 并跳过。

局部属性：

| 属性 | 类型 | 描述 |
|---|---|---|
| `separator` | text | 恰好一个字符。 |

子元素：

| 元素 | 属性 | 描述 |
|---|---|---|
| `<step>` | `label`（必需，非空） | 一个面包屑；路径会被替换。 |

子元素列表属于内容：仅在文档生成变化时写入，与变体无关且静默。共享的 `selected` 属性会被接受并移动键盘光标；所绘制的当前页面仍是最后一步。

## 另见

- `nav-rail.md`
- `accordion.md`
- `../index.md`
- `../../guides/xml-layout-authoring.md`
