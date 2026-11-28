# SearchBox

> 带前置字形、占位提示和清除按钮的搜索框。

## 介绍

`SearchBox<M>` 绘制一个带边框的框，包含搜索字形、其文本（为空时显示提示），以及右侧的
清除按钮（`×`）。编辑发生在控件内部绘制的内联会话中，而不是在原生子控件中：点击文本区域
放置插入符，然后输入、选择并使用常规编辑键。会话在 Enter、Tab 或焦点丢失时提交，
并且仅当提交的文本与当前值不同时才发出 `on_commit(text)`。

清除按钮位于编辑会话之外。点击它会清除值并发出 `on_commit(String::new())`；这是一次变更，
因此只读框会拒绝它。`.read_only(true)` 保留焦点、选择和复制功能，但拒绝输入、粘贴、剪切、
删除、撤销和清除按钮。`.debounce` 只为应用自己的计时器存储一个提示；控件每次提交发出一条
消息，自身从不启动计时器。

无障碍为 `A11yRole::TextField`，带实时值和 `read_only` 标志。`M` 是 `on_commit`
返回的消息类型。

## 用法（Rust）

```rust
use std::time::Duration;
use bamboo_native_ui::widgets::SearchBox;
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg { Search(String) }

fn build(ui: &mut Ui<Msg>) {
    ui.add(
        SearchBox::new("search", "", Msg::Search)
            .cue("Search tracks")
            .debounce(Duration::from_millis(250))
            .max_length(64),
    );
    ui.set_rect(WidgetId::new("search"), Rect::from_xywh(16.0, 16.0, 240.0, 34.0));
}
```

默认提示为 `"搜索"`（中文的 "search"）；如上所示设置英文提示。`debounce_value()`
让应用读取其配置的提示。`is_editing()` 报告是否有绘制的会话处于打开状态。

## 用法（XML）

`SearchBox` 不在标准注册表中。在 Rust 中创建它，并从文档中通过 `id` 引用：

```xml
<widget id="search" x="16" y="16" w="240" h="34"
        text="" cue="Search tracks" color="text" radius="medium"/>
```

`text` 是共享内容（`content_caps` 公开 `TEXT`）。`cue` 和 `color` 是本地属性。
该框采用 `SurfaceStyle`，因此 `radius`/`corners` 和 `background` 也适用。

## 属性

| 方法 | 类型 | 默认值 | 描述 |
|---|---|---|---|
| `new(id, text, on_commit)` | `&'static str, impl Into<String>, impl Fn(String) -> M + 'static` | — | 创建搜索框。 |
| `.cue(impl Into<String>)` | `impl Into<String>` | `"搜索"` | 值为空时显示的占位提示。 |
| `.read_only(bool)` | `bool` | `false` | 拒绝变更，但保留焦点、选择和复制。 |
| `.max_length(u32)` | `u32` | `0`（无限制） | 编辑器接受的最大字符数。 |
| `.debounce(Duration)` | `Duration` | `None` | 供应用自己的防抖计时器使用的提示。 |
| `.corner_radius(f32)` | `f32` | `0.0` | 统一的框圆角半径。 |
| `.corners(Corners)` | `Corners` | 零 | 逐角的框半径。 |
| `.background(Color)` | `Color` | `None`（调色板 `search_bg`） | 框填充覆盖。 |
| `.background_option(Option<Color>)` | `Option<Color>` | `None` | 设置或清除填充覆盖。 |
| `.transparent_background(bool)` | `bool` | `false` | 抑制填充，保留边框。 |

Getter 与 setter：`text()`、`is_read_only()`、`is_editing()`、
`cue_text()`、`debounce_value()`、`color_value()`、`surface()`、`set_cue`
和 `set_color(Option<Color>)`。

行为说明：

- 会话打开时控件拥有键盘（方向键、Home/End、Delete/Backspace、撤销/重做、复制/剪切）；
  容器拦截 Tab、Escape、Enter 和粘贴组合键。
- 清除按钮是靠近右边缘的 22 DIP 目标，仅在值非空时绘制；在其上按下永远不会打开编辑会话。
- 光标：文本上为 I 形光标，清除目标上为箭头。

## 事件

### 消息

| 消息 | 发出时机 |
|---|---|
| 传给 `new`（`on_commit`）的 `Fn(String) -> M` | 绘制的会话在 Enter/Tab/焦点丢失时提交，或点击清除目标；仅当提交的文本与存储的值不同时。 |

### 通用观察器

`SearchBox` 不安装共享观察器构建器。其机制是构造函数的 `on_commit` 回调加上容器驱动的
编辑会话；使用 `Ui::bind` 添加额外触发器。`.debounce(Duration)` 只是应用自己计时器的
提示，自身不发出任何消息。

### 控件专属回调

除 `on_commit` 外无。`.debounce(Duration)` 存储防抖提示，`.max_length(u32)` 限制输入；
两者都不是事件回调。

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

该框对 `Click`（仅清除目标）和 `Activate`（打开内联会话）做出反应；
`HoverEnter`/`HoverLeave` 高亮清除目标。编辑时，按键、滚轮和拖动会送到绘制的字段。
`ValueChange` 跟随已提交的值变化。

## XML 定义

此控件不由标准注册表创建。在 Rust 中创建它，并从文档中通过 `id` 引用。

- 几何：适用常规 `widget` 规则；默认高度为 34 DIP。
- 共享内容：`text`。
- 本地属性：`cue`（文本）和 `color`（文本颜色）。
- 样式 token：通过 `SurfaceStyle` 支持 `radius`、`corners` 和 `background`；
  `font-size` 和 `font-family` 适用。
- 子元素：无。`read_only`、`max_length` 和 `debounce` 只能在构造函数中设置。

## 另见

- `../index.md`
- `text-edit.md`
- `../../api/theme-and-palette.md`
- `../../guides/xml-layout-authoring.md`
