# TextEdit

> 支持原生或框架绘制编辑的单行或多行文本字段。

## 介绍

`TextEdit<M>` 是框架的文本字段。它在关闭状态下自行绘制边框和文本，因此页面在开始编辑
之前看起来就是完整的。默认情况下，点击（或 Enter）会请求平台宿主打开一个原生 `EDIT`
会话（Windows）；提交的文本通过 `Ui::commit_edit` 返回，并成为 `on_commit` 返回的消息。
`.painted()` 则改用框架绘制就地编辑：控件在获得焦点期间拥有输入、选择、IME 组字、
剪贴板、撤销/重做及其上下文菜单。

两个别名以不同的词汇命名同一个控件，因此没有第二份实现需要保持同步：

- `PasswordBox<M>` = `TextEdit<M>`；构建时调用 `.password()` 以 `•` 圆点遮盖值。
- `TextArea<M>` = `TextEdit<M>`；调用 `.multiline()` 获得可换行、垂直滚动的字段。
  Enter 插入换行；Tab/Escape/焦点丢失像单行字段一样提交或取消。

关闭状态的字段以字符为单位保持自己的插入符和选择，因此编程式的 `set_caret`、
`set_selection`、`select_all`、`insert_text` 和 `clear` 无需打开会话即可工作。
无障碍为 `A11yRole::TextField`，带实时值（密码则遮盖）、`read_only` 标志和选择范围。
`M` 是 `on_commit` 和可选回调返回的消息类型。

## 用法（Rust）

```rust
use bamboo_native_ui::widgets::{EditColors, PasswordBox, TextEdit};
use bamboo_native_ui::{Color, Rect, Ui, WidgetId};

enum Msg { Name(String), Pin(String) }

fn build(ui: &mut Ui<Msg>) {
    ui.add(
        TextEdit::new("name", "Bamboo Native UI", Msg::Name)
            .cue("Track name")
            .max_length(64)
            .painted()
            .colors(
                EditColors::new()
                    .background(Color::new(255, 250, 235))
                    .text(Color::new(58, 46, 24))
                    .caret(Color::new(210, 60, 60)),
            ),
    );
    ui.set_rect(WidgetId::new("name"), Rect::from_xywh(16.0, 16.0, 280.0, 34.0));

    ui.add(PasswordBox::new("pin", "", Msg::Pin).password());
    ui.set_rect(WidgetId::new("pin"), Rect::from_xywh(16.0, 60.0, 200.0, 34.0));
}
```

默认矩形为 280×34（多行时首选高度为 84）。没有 `.painted()` 时，编辑会话由平台宿主处理；
`.painted()` 才能使 `EditColors`、内联 IME 组字和绘制的插入符生效。

## 用法（XML）

`TextEdit` 不在标准注册表中（其别名也不在）。在 Rust 中创建它，并从文档中通过 `id`
引用：

```xml
<widget id="name" x="16" y="16" w="280" h="34"
        text="Bamboo Native UI" cue="Track name" color="text" radius="medium"/>
```

`text` 是共享内容（`content_caps` 公开 `TEXT`），每个文档世代静默写入一次；
如果该 id 上有编辑会话打开，则跳过写入（`xml.content.session-open`，一个警告）。
`cue` 和 `color` 是本地属性，`radius`/`corners`/`background` 通过 `SurfaceStyle` 生效。

## 属性

| 方法 | 类型 | 默认值 | 描述 |
|---|---|---|---|
| `new(id, text, on_commit)` | `&'static str, impl Into<String>, impl Fn(String) -> M + 'static` | — | 创建字段。 |
| `.painted()` | — | 关闭（原生 `EDIT` 会话） | 使用框架绘制就地编辑。 |
| `.password()` | — | 关闭 | 用圆点遮盖值。 |
| `.multiline()` | — | 关闭 | 换行、垂直滚动并在焦点丢失时提交。 |
| `.wrap(bool)` | `bool` | `true` | 在单词边界处换行长行（多行）。 |
| `.scrollbars(ScrollBars)` | `ScrollBars` | `ScrollBars::Vertical` | 多行字段显示哪些滚动条。 |
| `.scrollbar_style(ScrollBarStyle)` | `ScrollBarStyle` | 7 DIP 轨道，最小 12 DIP 滑块 | 滚动条粗细和颜色。 |
| `.align(TextAlign)` | `TextAlign` | `TextAlign::Left` | 水平文本对齐。 |
| `.font_size(f32)` | `f32` | `None`（主题正文） | 每个字段的文本大小，单位为 DIP。 |
| `.cue(impl Into<String>)` | `impl Into<String>` | `""` | 值为空时的占位提示。 |
| `.padding(Insets)` | `Insets` | 单行 `(9, 2)`，多行 `(9, 6)` | 内容内边距。 |
| `.line_spacing(f32)` | `f32` | `2.0` | 行间的额外间距，单位为 DIP。 |
| `.vertical_center(bool)` | `bool` | 单行开启 | 将单行垂直居中。 |
| `.caret_width(f32)` | `f32` | `None`（系统无障碍宽度） | 插入符条宽度，钳制在 0.5..6.0。 |
| `.max_length(u32)` | `u32` | `0`（无限制） | 输入时接受的最大字符数。 |
| `.read_only(bool)` | `bool` | `false` | 拒绝变更；保留焦点、选择和复制。 |
| `.disabled()` | — | 启用 | 禁用字段。 |
| `.corner_radius(f32)` | `f32` | `0.0` | 字段框角半径。 |
| `.corners(Corners)` | `Corners` | 零 | 逐角的框半径。 |
| `.background(Color)` | `Color` | `None`（调色板 `search_bg`） | 框填充覆盖。 |
| `.background_option(Option<Color>)` | `Option<Color>` | `None` | 设置或清除填充覆盖。 |
| `.transparent_background(bool)` | `bool` | `false` | 抑制填充，保留边框。 |

颜色（全部可选；`None` 保持主题）：

| 方法 | 类型 | 描述 |
|---|---|---|
| `.colors(EditColors)` | `EditColors` | 替换整套颜色。 |
| `.text_color(Color)` | `Color` | 普通文本（以及插入符默认值）。 |
| `.edit_background(Color)` | `Color` | 字段填充。 |
| `.selection_colors(text, background)` | `Color, Color` | 选中的文本和高亮。 |
| `.caret_color(Color)` | `Color` | 插入符颜色。 |

`EditColors`（`bamboo_native_ui::widgets::EditColors`）公开公共 `Option<Color>` 字段
`text`、`background`、`selection_text`、`selection_background`、`caret`、`cue`、
`border`、`border_idle` 和 `composition_underline`，以及每个字段的构建器和
`selection(text, bg)`。

回调：`.on_change(Fn(&str) -> Option<M>)` 在每次改变了文本的用户编辑之后；
`.on_char(Fn(char) -> Option<M>)` 在插入之前（一个观察钩子）；`.on_click`/
`.on_right_click`/`.on_double_click` 使用 `Fn(Point) -> Option<M>`；
`.on_key_down`/`.on_key_up` 使用 `Fn(KeyEvent) -> Option<M>`（原生会话的按键释放永远
不会到达控件）。

访问器：`text()`、`caret()`、`selection() -> Option<(usize, usize)>`、
`is_editing()`、`composition()`、`is_painted()`、`cue_text()`、
`text_color_value()`、`scrollbar()`、`surface()`。设置器：`set_cue`、
`set_text_color(Option<Color>)`、`set_caret(usize)`、`set_selection(anchor,
caret)`、`select_all()`、`clear()` 和 `insert_text(&str)`。

## 事件

### 消息

| 消息 | 发出时机 |
|---|---|
| 传给 `new`（`on_commit`）的 `Fn(String) -> M` | 编辑会话提交（单行按 Enter，Tab 或焦点丢失，或 `Ui::commit_edit`）；仅当文本发生变化时。 |
| 观察器处理器（见下文） | 已安装的回调为对应输入返回 `Some(M)`。 |

### 通用观察器

`TextEdit` **不**使用共享观察器宏。它用相同的累加规则（内置行为仍会运行）声明自己的回调
方法，但没有 `on_focus_gained`/`on_focus_lost`：

| 方法 | 处理器 |
|---|---|
| `.on_change(f)` | `Fn(&str) -> Option<M>` |
| `.on_char(f)` | `Fn(char) -> Option<M>` |
| `.on_click(f)` | `Fn(Point) -> Option<M>` |
| `.on_right_click(f)` | `Fn(Point) -> Option<M>` |
| `.on_double_click(f)` | `Fn(Point) -> Option<M>` |
| `.on_key_down(f)` | `Fn(KeyEvent) -> Option<M>` |
| `.on_key_up(f)` | `Fn(KeyEvent) -> Option<M>` |

### 控件专属回调

| 回调 | 处理器 | 触发条件 |
|---|---|---|
| `.on_change(f)` | `Fn(&str) -> Option<M>` | 在每次改变了文本的用户编辑（输入、粘贴、剪切、删除、撤销/重做、提交的 IME）之后；编程式设置器不触发它。 |
| `.on_char(f)` | `Fn(char) -> Option<M>` | 每个输入的/IME 字符在插入之前；插入仍会发生。 |
| `.on_click(f)` | `Fn(Point) -> Option<M>` | 在字段上左键按下并释放。 |
| `.on_right_click(f)` | `Fn(Point) -> Option<M>` | 右键点击；如果应用未安装编辑菜单，编辑菜单仍会打开。 |
| `.on_double_click(f)` | `Fn(Point) -> Option<M>` | 平台双击；内置的单词选择仍会运行。 |
| `.on_key_down(f)` | `Fn(KeyEvent) -> Option<M>` | 每次按键（包括自动重复），在内置编辑之前。 |
| `.on_key_up(f)` | `Fn(KeyEvent) -> Option<M>` | 每次按键释放；原生会话的释放永远不会到达控件。 |

这些是 TextEdit 专属的，而不是共享观察器构建器。

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

字段对指针单击/双击/右键点击和拖动选择、`Activate`（打开绘制的会话）以及 `Wheel`
（多行滚动）做出反应；`ValueChange` 跟随文本变化。

## XML 定义

此控件不由标准注册表创建。在 Rust 中创建它，并从文档中通过 `id` 引用。

- 几何：适用常规 `widget` 规则。
- 共享内容：`text`。此控件不接受 `label`。
- 本地属性：`cue`（文本）和 `color`（文本颜色）。
- 样式 token：通过 `SurfaceStyle` 支持 `radius`、`corners` 和 `background`；
  `font-size` 和 `font-family` 适用于关闭状态的绘制和编辑器。
- 子元素：无。`password`、`multiline`、`painted`、`scrollbars` 和回调只能在构造函数中设置。

## 另见

- `../index.md`
- `search-box.md`
- `number-input.md`
- `../controls/button.md`
- `../../api/theme-and-palette.md`
- `../../guides/xml-layout-authoring.md`
