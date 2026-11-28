# 03 — 第一个窗口

> 编写一个完整的 `Host` 实现：一个标签、一个按钮、一次绘制过程和
> 一个事件循环。

## 你将构建

一个 520×320 的深色窗口，标题为 "Bamboo Hello"。它显示一个标签和一个按钮；
每次点击都把标签更新为 `Clicked N time(s)`。该程序是后续每个快速上手修改的基础。

## 本节新术语

- **`Host`** —— 你的应用实现的 trait（接口）；窗口通过它回调绘制、尺寸改变和输入。只有 `paint` 是必需的。
- **trait** —— Rust 的接口：一个类型承诺提供的一组具名方法。
- **`impl`** —— 一个代码块，用于给类型添加方法（`impl App`）或为它实现 trait（`impl Host for App`）。
- **`Ui<M>`** —— 拥有一个页面全部控件的容器，并把输入路由给它们；`M` 是你的消息类型。
- **控件（widget）** —— 一个元素，带有稳定 id、矩形、绘制代码和输入处理器。
- **消息（message，`M`）** —— 控件返回的、用来说明发生了什么的值；`enum` 表示这样一组固定的值。
- **`Option<M>`** —— 零个或一个消息：发生事情时为 `Some(Msg::Greet)`，没有发生时为 `None`。
- **事件（event）** —— 一种规范化后的输入或窗口事件（指针移动、点击、按键），交给 `Host`；事件描述输入，消息描述意图。
- **DIP** —— 设备无关像素：1/96 英寸。矩形以 DIP 书写；宿主应用显示器的 DPI 缩放。
- **绘制（paint）** —— 绘制过程。`Host::paint` 绘制受损的片段，并请求 `Ui` 在这些片段上绘制控件。
- **脏矩形（dirty rectangle）** —— 像素发生变化、必须重绘的矩形；宿主会把它们合并，只复制这些片段。
- **后台缓冲（back buffer）** —— 一块离屏表面，在两次绘制之间保留页面，因此下一次绘制只需重绘变化的部分。
- **失效 / 损坏（invalidate / damage）** —— 把一个矩形标记为需要重绘。`Ui` 的 setter 记录损坏；`flush_damage` 把它变成一次失效。
- **`WindowBuilder`** —— 创建原生窗口、拥有后台缓冲并运行消息循环。
- **`WindowHandle`** —— 每次回调中使用的句柄，用于使窗口失效并执行窗口操作。
- **`PaintCtx`** —— 绘制上下文：画布、绘制器、脏列表、尺寸和 DPI。

## 前置条件

- 第 [01](01-create-project.md) 和 [02](02-add-the-crate.md) 页已完成。
- `bamboo-native-ui` 已以默认特性链接。

## 步骤

### 1. 理解各个部件

- **`WindowBuilder`** 创建原生窗口、运行消息循环并拥有后台缓冲。`WindowBuilder::new(host)` 按值接收你的 `Host`；调用链以 `run()` 结束，它会阻塞直到窗口关闭。这里必须使用 Windows：窗口宿主只在 Windows 上编译。
- **`Host`** 是窗口的应用侧。它是一个 trait，在 Rust 中就是一个接口：一组带签名的方法。你写 `impl Host for App` 并填充回调。只有 `paint` 是必需的；其余（`resize`、`event`、`started`……）都有空默认实现。`resize` 在窗口首次显示时和每次尺寸改变时运行，`event` 在每个输入或窗口事件上运行。
- **`Ui<M>`** 是拥有控件的容器。`M` 是你的消息类型，这里用 `enum` 定义 —— 一组固定的值。控件的输入处理器返回 `Option<M>`：`Some(message)` 报告一件事，`None` 表示什么也不报告。容器把消息排队，直到你的代码取走它们。
- **事件不是消息。** 事件是用户或 Windows 做了什么（在某个位置的 `PointerDown`）。消息是控件判断这意味什么（`Msg::Greet`）。`Host::event` 接收事件，把它们转发给 `Ui`，然后取走控件产生的消息。

框架把 Win32 消息规范化为 `Event` 值，因此你的代码永远看不到原始的 `WM_*` 消息。矩形和指针位置都以 DIP（设备无关像素）表示，宿主在绘制时应用显示器的 DPI 缩放。

### 2. 替换 `src/main.rs`

打开 crate 的源码 `bamboo-hello/bamboo-hello/src/main.rs`（不是外层工作区文件夹里的文件），用下面的程序替换生成的 `Hello, world!` 程序：

```rust
// 本页用到的两个内置控件。
use bamboo_native_ui::widgets::{Button, Label};
// 窗口宿主、绘制上下文、几何、主题和控件标识。
use bamboo_native_ui::{
    Event, Host, PaintCtx, Rect, Size, ThemeSettings, Ui, WidgetId, WindowBuilder, WindowHandle,
};

// 本应用能接收的每条消息。控件产生这些值。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Msg {
    Greet, // 按钮被点击时发出。
}

struct App {
    ui: Ui<Msg>, // 拥有控件；Msg 是本应用的消息类型。
    clicks: u32, // 应用状态，在消息到达时更新。
}

impl App {
    // 只构建一次控件树；矩形稍后在 resize 中分配。
    fn new() -> Self {
        let mut ui = Ui::new();
        // add() 取得控件的所有权，并按它的 id 索引。
        ui.add(Label::new("status", "No clicks yet"));
        // 第三个参数是按钮在点击时发出的消息。
        ui.add(Button::new("greet", "Say hello", Msg::Greet));
        Self { ui, clicks: 0 }
    }

    // 把一条消息变成应用状态和一次控件更新。
    fn apply(&mut self, message: Msg) {
        match message {
            Msg::Greet => {
                self.clicks += 1;
                // setter 记录损坏；像素在下一次绘制时改变。
                self.ui.set_label(
                    WidgetId::new("status"),
                    format!("Clicked {} time(s)", self.clicks),
                );
            }
        }
    }
}

// 实现框架的接口：窗口会调用这些方法。
impl Host for App {
    // 必需：绘制页面上受损的部分。
    fn paint(&mut self, ctx: &mut PaintCtx<'_>) {
        // 后台缓冲在两次绘制之间保留页面：只填充脏片段，
        // 然后让容器在这些片段上重绘控件。
        let background = ctx.painter.palette().background;
        for rect in ctx.dirty {
            // 每个片段都是一块合并后的、发生变化的 DIP 矩形。
            ctx.canvas.fill_rect(*rect, background);
        }
        // 重绘与脏片段相交的控件。
        self.ui.paint(ctx);
    }

    // 可选：在首次显示和每次尺寸改变时调用。
    fn resize(&mut self, size: Size, _dpi: f32, window: &mut WindowHandle<'_>) {
        // 应用负责布局：每个控件 id 一个矩形。
        self.ui.set_rect(
            WidgetId::new("status"),
            // 全宽状态行，左右各留 24 DIP 边距。
            Rect::from_xywh(24.0, 24.0, size.width - 48.0, 28.0),
        );
        self.ui.set_rect(
            WidgetId::new("greet"),
            // 标签下方固定尺寸的按钮。
            Rect::from_xywh(24.0, 64.0, 140.0, 36.0),
        );
        // 缓冲里仍是旧布局，因此先整体重绘一次。
        window.invalidate_all();
    }

    // 可选：在每个规范化后的输入或窗口事件上调用。
    fn event(&mut self, event: &Event, window: &mut WindowHandle<'_>) -> bool {
        // 只转发控件需要的事件；每次调用在 Ui 用到它时返回 true。
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
            _ => false, // 本页不需要的事件。
        };
        // 控件把消息排队；取走每一条并应用。
        for message in self.ui.drain_messages() {
            self.apply(message);
        }
        // 一串 setter 合并成一次重绘请求。
        self.ui.flush_damage(window);
        handled
    }
}

// 进程入口；返回 ExitCode 以报告成功或失败。
fn main() -> std::process::ExitCode {
    // 构建原生窗口，然后在这个线程上运行它的消息循环。
    let result = WindowBuilder::new(App::new())
        .title("Bamboo Hello")
        .size(520.0, 320.0) // 客户区，单位为 DIP。
        .min_size(320.0, 200.0)
        .theme(ThemeSettings::default(), true) // true 选择深色。
        .run(); // 阻塞直到窗口关闭。
    match result {
        Ok(_) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            // 错误写到终端，不弹对话框。
            eprintln!("bamboo: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
```

**刚才发生了什么。** 你把生成的程序换成了一个应用：它构建两个控件、实现 `Host` 回调并打开窗口。还没有运行任何东西。下一条命令会让 Rust 编译器检查整个文件；id 拼错不是编译错误，因此 `"status"` 和 `"greet"` 在运行时同样重要。

### 3. 各部分的作用

- **`paint`** 接收一个借用的脏列表（`ctx.dirty`）：自上次绘制以来发生变化的片段，单位为 DIP。后台缓冲仍持有页面其余部分，因此该方法在每个片段下填充背景，并调用 `ui.paint(ctx)`，后者绘制与脏片段相交的每个控件。在片段外绘制是浪费工作：宿主永远不会复制那些像素。这也是同一份代码在悬停一像素后和整体改变尺寸后都能工作的原因 —— 决定绘制多少的是列表，而不是控件代码。
- **`resize`** 是该程序重新计算布局的唯一地方。它在窗口首次显示时和每次尺寸改变时运行。它给每个控件分配矩形 —— 应用负责布局；没有测量阶段 —— 并使整个客户区失效一次，因为后台缓冲里仍是旧布局。尺寸以 DIP 为单位；宿主应用 DPI 缩放。
- **`event`** 将框架规范化后的事件转换为 `Ui` 调用。每次调用返回 `Ui` 是否用到了该事件。按钮上的指针按下/抬起对让按钮的输入处理器返回 `Some(Msg::Greet)`，容器把该消息排队。`drain_messages` 把排队的消息交给应用。
- **`apply`** 更新应用状态，然后调用 `Ui::set_label`，后者把标签的矩形记录为待处理的损坏；它本身不绘制。在 `event` 末尾，`flush_damage(window)` 把所有待处理损坏变成一次失效，因此一串 setter 仍然只触发一次重绘请求。
- **`theme`** 接收 `ThemeSettings` 和深色/浅色决定（此处为 `true`）。
- **`main`** 返回 `std::process::ExitCode`，因此 shell 能判断程序是否正常关闭。本页能报告的唯一错误是创建或运行窗口失败，错误会打印到终端。

#### 一次点击的完整流程

1. Windows 送来一条鼠标消息；宿主把它规范化为
   `Event::PointerDown`，随后是 `Event::PointerUp`。
2. `Host::event` 把每个事件转发给 `ui.pointer_down_with` /
   `pointer_up_with`。
3. `Ui` 更新按钮的按下状态并调用它的输入处理器，后者返回 `Some(Msg::Greet)`。
4. `drain_messages` 产出 `Msg::Greet`；`apply` 递增 `clicks` 并调用
   `set_label`，后者为标签矩形记录损坏。
5. `flush_damage` 使该矩形失效。宿主合并损坏，稍后带着它调用 `paint`，放在 `ctx.dirty` 中。
6. `paint` 用背景填充该片段，`ui.paint` 在上面绘制标签。

### 4. 运行它

在外层 `bamboo-hello` 文件夹（含工作区 `Cargo.toml` 的那个）中运行：

```powershell
# 必要时构建，然后打开窗口。命令会阻塞，直到你关闭窗口。
cargo run
```

出现一个标题为 "Bamboo Hello" 的窗口。点击按钮：标签
变为 `Clicked 1 time(s)`，然后 `Clicked 2 time(s)`，依此类推。关闭窗口即结束程序并返回提示符（以及程序打印的错误文本，如果有）。

程序运行期间占用该终端。窗口内容不会打印在那里；终端里只会出现来自 `eprintln!` 的诊断和错误。

要观察绘制路径，为当前终端会话设置调试变量，并在同一个外层文件夹中运行：

```powershell
# 打开绘制/失效诊断（PowerShell 语法）。
$env:BAMBOO_UI_DEBUG = "1"
# 该变量不需要重新构建；再次启动程序即可。
cargo run
```

宿主将绘制和失效诊断记录到 stderr。窗口空闲时，不会出现绘制行；
悬停或点击只产生受影响的矩形。在 `cmd.exe` 中改用 `set BAMBOO_UI_DEBUG=1`；无论哪种 shell，该值都持续到你关闭那个终端。

## 预期结果

- 一个 520×320 的深色窗口，带一个标签和一个 `Say hello` 按钮。
- 点击使标签中的计数器递增。
- 空闲时窗口不重绘；设置 `BAMBOO_UI_DEBUG=1` 后，终端在你悬停或点击之前保持安静。
- 关闭窗口后提示符返回，通常退出码为 0。

## 故障排查

- **窗口空白或灰色** —— `resize` 没有运行，或矩形
  从未被设置。检查 `WindowBuilder::new(App::new())` 是否收到你的 app，
  以及 `resize` 是否为每个 id 调用了 `ui.set_rect`。传给 `set_rect` 的 id 必须与传给 `Label::new` 和 `Button::new` 的 id 完全一致。
- **按钮没有反应** —— 指针事件没有到达 `Ui`。确保
  `event` 用 `pointer_down_with`/`pointer_up_with` 转发 `PointerDown`/`PointerUp`，
  并且按钮的矩形
  非空。点击 (24, 64) 处 140×36 矩形内部；点击标签区域不会有任何效果。
- **点击后标签从不改变** —— 消息取出来了，但
  更改没有刷新。在处理消息后调用 `ui.flush_damage(window)`。
  同时检查 `Msg::Greet` 分支仍在用 id `"status"` 调用 `set_label`。
- **`run` 立即返回错误** —— `WindowBuilder::run` 必须在
  主线程上调用；Win32 要求消息循环在那里。不要从工作线程
  生成窗口。
- **`error: could not find \`Cargo.toml\``** —— 你在错误的文件夹中。请从包含工作区清单的外层 `bamboo-hello` 文件夹运行
  `cargo run`。
- **`cargo` 不被识别** —— 没有安装 Rust，或这个终端在安装之前就已打开；从 <https://rustup.rs> 安装并打开一个新的
  PowerShell 窗口。
- **链接步骤失败并提示 "link.exe not found"** —— 缺少 C++ 构建工具或
  Windows SDK。安装 **使用 C++ 的桌面开发**
  工作负载并打开一个新终端；第 01 页有详细说明。
- **窗口一打开就关闭** —— `run()` 返回了错误，或程序 panic 了。请查看运行 `cargo run` 的终端，包括
  `bamboo: ...` 行。从资源管理器启动 `.exe` 会隐藏这些输出。
- **修改后出现的仍是旧窗口** —— 编辑器没有保存
  `main.rs`，或你运行了另一份检出。按 Ctrl+S 保存，停止旧程序，然后再次运行 `cargo run`。
- **`unresolved import bamboo_native_ui`** —— 缺少框架依赖，或它被加到了错误的清单；请先完成 [第 02 页](02-add-the-crate.md)。
- **`$env:BAMBOO_UI_DEBUG` 不被识别** —— 你在 `cmd.exe` 而不是
  PowerShell 中。在那里改用 `set BAMBOO_UI_DEBUG=1`。该变量必须在同一个终端窗口中、`cargo run` 之前设置。
- **设置调试变量后仍没有绘制诊断** —— 确认变量设置在运行程序的终端里，而不是另一个窗口，并且程序是在设置之后从该终端启动的。在 PowerShell 中用 `$env:BAMBOO_UI_DEBUG` 验证。

## 下一步

[04 — 自定义控件](04-custom-widgets.md)：绘制你自己的组件并从中发出
消息。
