# 12 — 文本编辑与输入法

> 原生和框架绘制的文本字段、数字和搜索输入，以及
> 中文/日文输入。

## 你将构建

一个带五个字段的表单。**Name** 是一个原生 `EDIT` 子控件；**Notes** 是一个
框架绘制的多行编辑器，带自己的颜色；**PIN** 带掩码；
**Search** 是一个 `SearchBox`；**Count** 是一个带步进器的 `NumberInput`。一个
状态标签报告提交消息、观察到的字符以及通过 `Ui::with_widget` 读回的
光标。最后一步把输入法组字转发给
获得焦点的框架绘制编辑器。

## 前置条件

- [03](03-first-window.md) 中的项目可以构建并运行。
- 无需额外特性。原生编辑器和输入法回调仅限 Windows；
  框架绘制编辑器可在任何能绘制的目标上工作。

## 本节新术语

- **原生编辑器** —— 字段把输入交给真正的 Windows `EDIT` 控件，因此由
  Windows 绘制文本、选区和光标，并负责键盘处理。
- **框架绘制编辑器** —— 框架把文本画进自己的窗口（`.painted()`），这样
  应用可以设置每一种颜色，并把值保留在框架的布局内部。
- **提交** —— 把完成的文本作为消息交回应用。单行字段上按 Enter、按 Tab
  或失去焦点都会提交。
- **光标** —— 闪烁的插入点，用字符偏移量度量。
- **选区** —— 被高亮的字符范围；它可以是空的。
- **提示文本** —— 字段为空时显示的灰色提示。
- **输入法** —— Input Method Editor，Windows 用来输入中文、日文和韩文的
  服务。**组字**是它在用户确认候选词之前就地显示、尚未转换的文本。
- **步进器** —— 调节数字字段的小上下箭头。
- **焦点** —— 决定哪个控件接收输入和键盘快捷键。
- **防抖** —— 对快速输入作出反应之前施加的延迟；这里它只作为提示
  存储，并不是一个运行中的定时器。

本页所有 Rust 代码都写入 crate 的 `src/main.rs`
（`bamboo-hello/bamboo-hello/src/main.rs`）。

## 步骤

### 1. 定义消息和状态

```rust
use std::time::Duration;

use bamboo_native_ui::widgets::{EditColors, Label, NumberInput, SearchBox, TextArea, TextEdit};
use bamboo_native_ui::{
    Color, CloseReason, Host, KeyCode, Rect, ScrollBars, Ui, UiHost, WidgetId, WindowHandle,
};

#[derive(Clone, Debug, PartialEq)]
enum Msg {
    // 每个字段一个变体：各自携带它提交的文本或数字。
    Name(String), Notes(String), Pin(String), Search(String),
    Count(i64), Price(i64), Duration(i64), Char(char), Clicked, Shortcut,
}

struct App { ui: Ui<Msg> }

impl App {
    fn apply(&mut self, message: Msg, window: &mut WindowHandle<'_>) {
        // 在状态标签中显示最后一条消息，然后重绘它。
        self.ui.set_label(WidgetId::new("status"), format!("{message:?}"));
        self.ui.flush_damage(window);
    }
}
```

`TextEdit<M>` 是字段类型；`PasswordBox<M>` 和 `TextArea<M>` 是同一控件的
别名。`M` 是提交消息和回调类型。

### 2. 添加一个原生和一个绘制字段

对于默认的原生字段，点击或 Enter 向宿主请求一个 `EDIT`
会话。在关闭时，该字段绘制自己的边框和值。构造函数
处理器是**提交**：单行上的 Enter、Tab 或焦点丢失。

```rust
impl App {
    fn new() -> Self {
        let mut ui = Ui::new();
        ui.add(Label::new("status", "Edit a field"));
        ui.add(
            // 默认为原生：点击/Enter 时 Windows 会打开一个 `EDIT` 子控件。
            TextEdit::new("name", "Bamboo Native UI", Msg::Name)
                .cue("Track name") // 字段为空时显示的灰色提示
                .max_length(64),   // 64 个字符；0 表示不限制
        );
        ui.add(
            // `.painted()` 把文本留在我们自己的窗口内，颜色才能生效。
            TextArea::new("notes", "Line one\nLine two", Msg::Notes)
                .multiline().wrap(true).scrollbars(ScrollBars::Vertical)
                .painted()
                .colors(EditColors::new()
                    .background(Color::new(255, 250, 235))
                    .text(Color::new(58, 46, 24))
                    // 先是选中文本的颜色，然后是它背后的高亮色。
                    .selection(Color::new(255, 255, 255), Color::new(64, 120, 220))
                    .caret(Color::new(210, 60, 60))
                    .composition_underline(Color::new(226, 168, 58))),
        );
        ui.add(TextEdit::new("pin", "", Msg::Pin).password().max_length(8));
        Self { ui }
    }
}
```

构建器：`.cue(text)` 显示提示文本；`.password()` 掩码值；
`.multiline()` 使 Enter 插入换行；`.wrap(bool)`；
`.scrollbars(ScrollBars)` 是 `None`、`Vertical`（默认）、`Horizontal` 或
`Both`；`.max_length(u32)`（`0` = 无限制）；`.read_only(bool)` 保留焦点、
选区和复制，但拒绝修改。

**刚才发生了什么** —— `new` 创建了三个看起来相似、行为却不同的字段。
**Name** 和 **PIN** 会在被点击时打开一个 Windows `EDIT` 子控件，
所以关闭时框架只绘制它们的值和边框。
**Notes** 是 `.painted()` 的，因此它原地编辑并使用沙色配色；它的
`EditColors` 还决定了选区高亮、光标和输入法下划线的颜色。

### 3. 观察编辑和按键

构造函数处理器只是提交。添加控件自己的回调
以查看每次编辑、每个输入的字符以及不会进入提交的输入。
这替换上面较短的 `notes` 构建器。

```rust
ui.add(
    TextArea::new("notes", "", Msg::Notes)
        .painted().multiline()
        // 在改变了文本的任何用户编辑之后触发（setter 不算）。
        .on_change(|text| Some(Msg::Notes(text.to_string())))
        // 观察钩子：在插入之前看到每个字符。
        .on_char(|ch| (!ch.is_control()).then_some(Msg::Char(ch)))
        .on_click(|_| Some(Msg::Clicked))
        // Ctrl+S：`Letter(18)` 就是 S。
        .on_key_down(|key| (key.ctrl && key.code == KeyCode::Letter(18))
            .then_some(Msg::Shortcut))
        .on_key_up(|_| None),
);
```

`on_change` 在改变了文本的用户编辑（输入、粘贴、剪切、
删除、撤销、已提交的输入法）之后运行；程序化 setter 不会触发它。`on_char`
在插入**之前**观察每个输入的字符或输入法结果 —— 一个
观察钩子，而不是过滤器。`on_click`、`on_right_click` 和
`on_double_click` 保留内置行为；`on_key_down`/`on_key_up` 接收
`KeyEvent`（原生会话的按键释放永远不会到达控件）。每次输入最多传递一条
消息，观察者优先于 `on_change`。

### 4. 读取和设置光标与选区

```rust
// 读取光标偏移和选中范围（偏移量按字符计）。
let _state = self.ui.with_widget::<TextEdit<Msg>, _>(WidgetId::new("notes"),
    |edit| (edit.caret(), edit.selection()));
// 程序化 setter：它绕过 `max_length`，后者只限制输入。
self.ui.edit_widget::<TextEdit<Msg>>(WidgetId::new("notes"), |edit| edit.select_all());
```

`caret()` 是字符偏移；`selection()` 是一个规范化的 `(start, end)`
对，或在折叠时为 `None`。`set_caret`、`set_selection`、`select_all`、
`insert_text` 和 `clear` 是程序化 setter；它们不强制执行
`max_length`，后者适用于用户输入。

### 5. 使用搜索框

`SearchBox` 围绕一个内嵌的绘制字段绘制一个前导放大镜
和一个清除按钮。它在 Enter、Tab 或焦点丢失时提交。

```rust
ui.add(
    SearchBox::new("search", "", Msg::Search)
        .cue("Search tracks")
        .max_length(64)
        // 只作为提示存储；控件不会启动任何定时器。
        .debounce(Duration::from_millis(250)),
);
```

`.debounce` 是一个**提示**：控件存储它（`debounce_value()`）且从不
启动定时器；消息每次提交到达一次，而不是每次击键。

### 6. 使用数字输入

`NumberInput::new(id, value, range, on_change)` 将初始值钳制到
包含的范围内。值保持为 `i64`。

```rust
// 步进器钳制到包含端点的 1..=500 范围；值保持为 i64。
ui.add(NumberInput::new("count", 8, 1..=500, Msg::Count)
    .steppers().step_size(5).suffix(" px"));
// `decimals(2)` 会把原始值 250 绘制成 2.50。
ui.add(NumberInput::new("price", 250, 0..=100_000, Msg::Price).decimals(2));
ui.add(
    // 自定义的显示/解析组合：显示 m:ss，并接受把 m:ss 输回来。
    NumberInput::new("duration", 95, 0..=3600, Msg::Duration)
        .format(|seconds| format!("{}:{:02}", seconds / 60, seconds % 60))
        .parse(|text| {
            let (m, s) = text.split_once(':')?;
            Some(m.trim().parse::<i64>().ok()? * 60 + s.trim().parse::<i64>().ok()?)
        }),
);
```

`decimals(2)` 为原始 `250` 绘制 `2.50`，并将输入解析回原始单位，
因此步进器和无障碍 RangeValue 保持基于整数。`format` 和
`parse` 是一对：返回 `None` 的解析器保留当前值。

### 7. 转发输入法并管理原生编辑器

宿主为框架绘制编辑器隐藏系统组字窗口，因此
应用必须转发组字和提交。

```rust
impl Host for App {
    fn ime_compose(&mut self, text: &str, cursor: usize, window: &mut WindowHandle<'_>) {
        // 在获得焦点的绘制字段内显示尚未提交的组字。
        if self.ui.ime_compose(text, cursor, window) {
            for message in self.ui.drain_messages() {
                self.apply(message, window);
            }
        }
    }

    fn ime_commit(&mut self, text: &str, window: &mut WindowHandle<'_>) {
        // 输入法转换完成：插入文本并清除组字。
        self.ui.ime_commit(text, window);
        for message in self.ui.drain_messages() {
            self.apply(message, window);
        }
    }

    fn edit_committed(&mut self, id: WidgetId, text: &str, window: &mut WindowHandle<'_>) {
        // 一个原生 `EDIT` 会话结束：路由它的提交消息。
        if let Some(message) = self.ui.commit_edit(id, text.to_string(), window) {
            self.apply(message, window);
        }
    }

    fn close_requested(&mut self, _reason: CloseReason, window: &mut WindowHandle<'_>) -> bool {
        if window.edit_active() {
            window.end_edit(); // 关闭前先提交打开的编辑器
        }
        true
    }
}
```

`Ui::ime_compose` 将未提交的字符串写入获得焦点的绘制字段；
`Ui::ime_commit` 插入转换后的结果，触发更改消息并
清除组字。`commit_edit` 存储原生文本并返回提交
消息；`window.end_edit()` 关闭原生会话（在页面
切换之前调用它），`window.edit_active()` 回答是否有会话打开。

`Ui` 通过 `UiHost` 向宿主报告获得焦点的框架绘制编辑器的光标矩形。
`WindowHandle` 实现了它，因此输入法候选窗口
跟随光标。自定义绘制编辑器显式地做同样的事：

```rust
// 用客户区坐标告诉输入法光标在哪里。
window.set_caret_rect(Some(Rect::from_xywh(120.0, 40.0, 2.0, 18.0))); // 光标
window.set_caret_rect(None); // 停止闪烁定时器
```

### 8. 运行它

从 crate 文件夹运行（`bamboo-hello/bamboo-hello`，也就是存放
`src/main.rs` 的那个）。

```powershell
# 工作目录：bamboo-hello/bamboo-hello
cargo run
```

窗口带着五个字段和状态标签出现。点击一个字段并
开始输入；状态标签会显示第 1 步中的某条消息。构建
错误会指出出错的文件和行号，且不会打开窗口。

**刚才发生了什么** —— 这次运行编译了 `main.rs` 并打开了窗口。
击键现在走两条路之一：原生 `EDIT` 会话只在提交时报告，或者
框架绘制编辑器的回调 —— 这就是为什么不同字段的状态标签
会在不同时刻更新。

## 预期结果

运行 `cargo run`。点击 **Notes**，切换到中文输入法并输入：
组字以内联方式带下划线绘制，然后提交的字符
替换它。`Ctrl+S` 报告快捷键；步进器发出 `Msg::Count`。

- **Name** 和 **PIN** 打开一个 `EDIT` 子控件（PIN 显示圆点）。
- **Notes** 原地编辑、换行、滚动并使用沙色配色。
- 搜索框绘制其字形和清除按钮；数字字段遵循范围、
  步进器、小数和自定义格式。
- 输入法组字以内联方式可见，候选窗口跟随光标。

## 故障排查

- **字段不接受输入** —— 点击没有打开原生
  `EDIT` 会话，或绘制字段从未获得焦点。点击字段矩形内部，
  并确认 `Ui` 像第 03 页那样接收指针和按键事件。
- **原生字段在输入时看起来冻结** —— 它在提交时更新。使用
  `edit_changed` 获取实时状态，或 `.painted()` 原地编辑。
- **在原生字段中输入时 `apply` 什么也没收到** —— 原生字段
  只在提交时报告（Enter、Tab、焦点丢失），而不是每次击键。添加第 3 步的
  控件回调，或把字段切换为 `.painted()`。
- **光标缺失或不闪烁** —— 框架绘制编辑器需要焦点和
  光标颜色；`.colors(EditColors::new().caret(...))` 可以设置它。自定义
  编辑器还必须报告光标矩形。
- **选区没有高亮** —— 在绘制字段上设置 `EditColors::selection(text, background)`；
  两个参数分别是选中文本的颜色和它背后的高亮色。
- **密码字段显示明文** —— 构建器链里缺少 `.password()`。
- **输入法不显示组字** —— `Host::ime_compose` 没有转发到
  `Ui::ime_compose`；宿主隐藏了系统组字窗口。
- **输入法文本进了错误的字段** —— 组字被路由到获得焦点的框架绘制编辑器。
  点击你想要的字段，并确认焦点没有留在仍然打开的原生字段上。
- **候选窗口在角落打开** —— 缺少光标矩形。
  内置绘制字段会报告它；自定义编辑器必须调用
  `window.set_caret_rect`。
- **搜索框只在提交时才发出消息** —— `.debounce` 是一个存储的提示，
  消息每次提交到达一次（Enter、Tab 或焦点丢失）；没有
  按击键运行的定时器。
- **输入后数字回退** —— 文本没有解析。将自定义
  `.format` 与匹配的 `.parse` 配对，或保留定点形式。
- **步进器不再改变值** —— 值到达了包含范围的端点；`1..=500` 在 500 处钳制。

## 下一步

[13 — 播放器 UI](13-player-ui.md)：将封面墙、播放列表、播放控制条
和波形拖动条组合成一个媒体播放器页面。
