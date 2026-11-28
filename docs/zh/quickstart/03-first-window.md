# 03 — 第一个窗口

> 编写一个完整的 `Host` 实现：一个标签、一个按钮、一次绘制过程和
> 一个事件循环。

## 你将构建

一个 520×320 的深色窗口，标题为 "Bamboo Hello"。它显示一个标签和一个按钮；
每次点击将标签更新为 `Clicked N time(s)`。该程序是后续每个快速上手所修改的基础。

## 前置条件

- 第 [01](01-create-project.md) 和 [02](02-add-the-crate.md) 页已完成。
- `bamboo-native-ui` 已以默认特性链接。

## 步骤

### 1. 理解两个类型

- `WindowBuilder` 创建原生窗口、运行消息循环并拥有
  后台缓冲。`WindowBuilder::new(host)` 按值接收你的 `Host`。
- `Host` 是窗口的应用侧。只有 `paint` 是必需的；
  其余（`resize`、`event`、`started`……）都有空默认实现。
- `Ui<M>` 是拥有控件的容器。`M` 是你的消息类型；一个
  控件从其输入处理器返回 `Option<M>`。

框架将 Win32 消息规范化为 `Event` 值。你的 `event`
方法将它们转发给 `Ui`，并取出控件产生的消息。

### 2. 替换 `src/main.rs`

```rust
use bamboo_native_ui::widgets::{Button, Label};
use bamboo_native_ui::{
    Event, Host, PaintCtx, Rect, Size, ThemeSettings, Ui, WidgetId, WindowBuilder, WindowHandle,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Msg {
    Greet,
}

struct App {
    ui: Ui<Msg>,
    clicks: u32,
}

impl App {
    fn new() -> Self {
        let mut ui = Ui::new();
        ui.add(Label::new("status", "No clicks yet"));
        ui.add(Button::new("greet", "Say hello", Msg::Greet));
        Self { ui, clicks: 0 }
    }

    fn apply(&mut self, message: Msg) {
        match message {
            Msg::Greet => {
                self.clicks += 1;
                self.ui.set_label(
                    WidgetId::new("status"),
                    format!("Clicked {} time(s)", self.clicks),
                );
            }
        }
    }
}

impl Host for App {
    fn paint(&mut self, ctx: &mut PaintCtx<'_>) {
        // The back buffer keeps the page between paints: fill exactly the
        // dirty pieces, then let the container repaint the widgets on them.
        let background = ctx.painter.palette().background;
        for rect in ctx.dirty {
            ctx.canvas.fill_rect(*rect, background);
        }
        self.ui.paint(ctx);
    }

    fn resize(&mut self, size: Size, _dpi: f32, window: &mut WindowHandle<'_>) {
        self.ui.set_rect(
            WidgetId::new("status"),
            Rect::from_xywh(24.0, 24.0, size.width - 48.0, 28.0),
        );
        self.ui.set_rect(
            WidgetId::new("greet"),
            Rect::from_xywh(24.0, 64.0, 140.0, 36.0),
        );
        window.invalidate_all();
    }

    fn event(&mut self, event: &Event, window: &mut WindowHandle<'_>) -> bool {
        let handled = match *event {
            Event::PointerMove { pos, .. } => self.ui.pointer_move(pos, window),
            Event::PointerDown {
                pos,
                button,
                modifiers,
            } => self.ui.pointer_down_with(pos, button, modifiers, window),
            Event::PointerUp {
                pos,
                button,
                modifiers,
            } => self.ui.pointer_up_with(pos, button, modifiers, window),
            Event::Key(key) => self.ui.key(key, window),
            _ => false,
        };
        for message in self.ui.drain_messages() {
            self.apply(message);
        }
        // A burst of setters becomes one repaint request.
        self.ui.flush_damage(window);
        handled
    }
}

fn main() -> std::process::ExitCode {
    let result = WindowBuilder::new(App::new())
        .title("Bamboo Hello")
        .size(520.0, 320.0)
        .min_size(320.0, 200.0)
        .theme(ThemeSettings::default(), true)
        .run();
    match result {
        Ok(_) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("bamboo: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
```

### 3. 各部分的作用

- **`paint`** 接收一个借用的脏列表（`ctx.dirty`）。它在每个片段下填充页面
  背景，然后调用 `ui.paint(ctx)`，后者绘制与脏片段相交的每个
  控件。在片段外绘制是浪费工作：宿主永远不会复制那些像素。
- **`resize`** 是该程序重新计算布局的唯一地方。它设置
  每个控件的矩形，并使整个客户区失效一次。尺寸以
  DIP 为单位；宿主应用 DPI 缩放。
- **`event`** 将框架规范化后的事件转换为 `Ui` 调用。
  按钮上的指针按下/抬起对产生 `Msg::Greet`；`drain_messages`
  将其交给应用。
- **`apply`** 更新应用状态，然后调用 `Ui::set_label`，后者
  将标签的矩形记录为待处理损坏。`flush_damage(window)` 将
  其变成一次失效。
- **`theme`** 接收 `ThemeSettings` 加上深色/浅色决定（此处为 `true`）。

### 4. 运行它

```powershell
cargo run
```

出现一个标题为 "Bamboo Hello" 的窗口。点击按钮：标签
变为 `Clicked 1 time(s)`，然后 `Clicked 2 time(s)`，依此类推。

要观察绘制路径，设置调试变量：

```powershell
$env:BAMBOO_UI_DEBUG = "1"
cargo run
```

宿主将绘制和失效诊断记录到 stderr。窗口空闲时，不会出现绘制行；
悬停或点击只产生受影响的矩形。

## 预期结果

- 一个 520×320 的深色窗口，带一个标签和一个 `Say hello` 按钮。
- 点击使标签中的计数器递增。
- 空闲时窗口不重绘。

## 故障排查

- **窗口空白或灰色** —— `resize` 没有运行，或矩形
  从未被设置。检查 `WindowBuilder::new(App::new())` 是否接收到你的 app，
  以及 `resize` 是否为每个 id 调用了 `ui.set_rect`。
- **按钮没有反应** —— 指针事件没有到达 `Ui`。确保
  `event` 用 `pointer_down_with`/`pointer_up_with` 转发 `PointerDown`/`PointerUp`，
  并且按钮的矩形
  非空。
- **点击后标签从不改变** —— 消息被取出了但
  更改没有被刷新。在处理消息后调用 `ui.flush_damage(window)`。
- **`run` 立即返回错误** —— `WindowBuilder::run` 必须在
  主线程上调用；Win32 要求消息循环在那里。不要从工作线程
  生成窗口。

## 下一步

[04 — 自定义控件](04-custom-widgets.md)：绘制你自己的组件并从中发出
消息。
