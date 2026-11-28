# 12 — 文本编辑与输入法

> 原生和框架绘制的文本字段、数字和搜索输入，以及
> 中文/日文输入。

## 你将构建

一个带五个字段的表单。**Name** 是一个原生 `EDIT` 子控件；**Notes** 是一个
框架绘制的多行编辑器，带自己的颜色；**PIN** 是掩码的；
**Search** 是一个 `SearchBox`；**Count** 是一个带步进器的 `NumberInput`。一个
状态标签报告提交消息、观察到的字符以及通过 `Ui::with_widget` 读回的
插入符。最后一步将输入法组字转发到
获得焦点的绘制编辑器。

## 前置条件

- [03](03-first-window.md) 中的项目可以构建并运行。
- 无需额外特性。原生编辑器和输入法回调仅限 Windows；
  绘制编辑器可在任何能绘制的目标上工作。

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
    Name(String), Notes(String), Pin(String), Search(String),
    Count(i64), Price(i64), Duration(i64), Char(char), Clicked, Shortcut,
}

struct App { ui: Ui<Msg> }

impl App {
    fn apply(&mut self, message: Msg, window: &mut WindowHandle<'_>) {
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
            TextEdit::new("name", "Bamboo Native UI", Msg::Name)
                .cue("Track name")
                .max_length(64),
        );
        ui.add(
            TextArea::new("notes", "Line one\nLine two", Msg::Notes)
                .multiline().wrap(true).scrollbars(ScrollBars::Vertical)
                .painted()
                .colors(EditColors::new()
                    .background(Color::new(255, 250, 235))
                    .text(Color::new(58, 46, 24))
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
选择和复制，但拒绝修改。

### 3. 观察编辑和按键

构造函数处理器只是提交。添加控件自己的回调
以查看每次编辑、每个输入的字符以及永远不进入提交的输入。
这替换上面较短的 `notes` 构建器。

```rust
ui.add(
    TextArea::new("notes", "", Msg::Notes)
        .painted().multiline()
        .on_change(|text| Some(Msg::Notes(text.to_string())))
        .on_char(|ch| (!ch.is_control()).then_some(Msg::Char(ch)))
        .on_click(|_| Some(Msg::Clicked))
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

### 4. 读取和设置插入符与选择

```rust
let _state = self.ui.with_widget::<TextEdit<Msg>, _>(WidgetId::new("notes"),
    |edit| (edit.caret(), edit.selection()));
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
        .debounce(Duration::from_millis(250)),
);
```

`.debounce` 是一个**提示**：控件存储它（`debounce_value()`）且从不
启动定时器；消息每次提交到达一次，而不是每次击键。

### 6. 使用数字输入

`NumberInput::new(id, value, range, on_change)` 将初始值钳制到
包含的范围内。值保持为 `i64`。

```rust
ui.add(NumberInput::new("count", 8, 1..=500, Msg::Count)
    .steppers().step_size(5).suffix(" px"));
ui.add(NumberInput::new("price", 250, 0..=100_000, Msg::Price).decimals(2));
ui.add(
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

宿主为绘制编辑器隐藏系统组字窗口，因此
应用必须转发组字和提交。

```rust
impl Host for App {
    fn ime_compose(&mut self, text: &str, cursor: usize, window: &mut WindowHandle<'_>) {
        if self.ui.ime_compose(text, cursor, window) {
            for message in self.ui.drain_messages() {
                self.apply(message, window);
            }
        }
    }

    fn ime_commit(&mut self, text: &str, window: &mut WindowHandle<'_>) {
        self.ui.ime_commit(text, window);
        for message in self.ui.drain_messages() {
            self.apply(message, window);
        }
    }

    fn edit_committed(&mut self, id: WidgetId, text: &str, window: &mut WindowHandle<'_>) {
        if let Some(message) = self.ui.commit_edit(id, text.to_string(), window) {
            self.apply(message, window);
        }
    }

    fn close_requested(&mut self, _reason: CloseReason, window: &mut WindowHandle<'_>) -> bool {
        if window.edit_active() {
            window.end_edit(); // commit the open editor before closing
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

`Ui` 通过 `UiHost` 向宿主报告获得焦点的绘制编辑器的插入符矩形。
`WindowHandle` 实现了它，因此输入法候选窗口
跟随插入符。自定义绘制编辑器显式地做同样的事：

```rust
window.set_caret_rect(Some(Rect::from_xywh(120.0, 40.0, 2.0, 18.0))); // caret
window.set_caret_rect(None); // stop the blink timer
```

## 预期结果

运行 `cargo run`。点击 **Notes**，切换到中文输入法并输入：
组字以内联方式带下划线绘制，然后提交的字符
替换它。`Ctrl+S` 报告快捷键；步进器发出 `Msg::Count`。

- **Name** 和 **PIN** 打开一个 `EDIT` 子控件（PIN 显示圆点）。
- **Notes** 原地编辑、换行、滚动并使用沙色的颜色。
- 搜索框绘制其字形和清除按钮；数字字段遵循范围、
  步进器、小数和自定义格式。
- 输入法组字可见地内联显示，候选窗口跟随插入符。

## 故障排查

- **原生字段在输入时看起来冻结** —— 它在提交时更新。使用
  `edit_changed` 获取实时状态，或 `.painted()` 原地编辑。
- **输入法不显示组字** —— `Host::ime_compose` 没有转发到
  `Ui::ime_compose`；宿主隐藏了系统组字窗口。
- **候选窗口在角落打开** —— 缺少插入符矩形。
  内置绘制字段报告它；自定义编辑器必须调用
  `window.set_caret_rect`。
- **输入后数字回退** —— 文本没有解析。将自定义
  `.format` 与匹配的 `.parse` 配对，或保留定点形式。

## 下一步

[13 — 播放器 UI](13-player-ui.md)：将封面墙、播放列表、传输
带和波形寻址条组合成一个媒体播放器页面。
