# 04 — 自定义控件

> 以两种方式绘制应用专属组件：只用闭包的
> `CustomWidget`，或带自持状态的手动 `Widget<M>` 实现。

## 你将构建

一个 `LevelBar`：一条带填充部分的圆角轨道，悬停时变为强调色，
点击时发出 `Msg::BarClicked`。把它加进
[03 — 第一个窗口](03-first-window.md) 的窗口；程序其余部分
不变。

展示两条路径，因为它们适合不同的需求：

- `CustomWidget` 适合由矩形、一些绘制和一个输入
  处理器组成的组件。
- 在组件拥有绘制
  和输入必须共享的状态、或需要发布 XML 皮肤属性时，用手动 `Widget<M>` 实现。

两条路径都用 `ui.add` 注册控件，并给它一个稍后由 `resize` 和
`set_rect` 使用的 id。区别在于状态放在哪里：`CustomWidget`
闭包没有自己的字段，因此路径 A 的填充固定为 60%；手动控件把
`level` 保存在结构体字段里，这正是皮肤属性可以更新的东西。除非你需要
自持状态或 XML 属性，否则从路径 A 开始。

两条路径是互斥的：它们各自用 id `"level"` 注册一个控件，
并且各自有导入同名项的 `use` 行。二选一；把两者都粘进一个文件会产生
重复导入和两个条。

## 本节新术语

- **控件（widget）** —— 这里指框架需要的四样东西：稳定的 `WidgetId`、
  以 DIP 表示的矩形、`paint` 方法和 `input` 方法。
- **`CustomWidget`** —— 把一个绘制闭包（以及可选的输入闭包）变成控件的
  构建器；适合不拥有可变状态的简单组件。
- **`Widget<M>`** —— 手动实现控件所用的 trait；类型拥有自己的字段，
  因此绘制和输入可以共享状态。
- **闭包（closure）** —— 在使用处就地编写的内联函数，例如
  `|ctx, rect, state| { ... }`；它可以读取附近的变量。
- **`Option<M>`** —— 零个或一个消息：`Some(message)` 报告一件事，
  `None` 表示什么也不报告。
- **`WidgetInput`** —— 交付给控件的规范化输入，例如
  `ClickAt(position)` 或 `Activate`。
- **`WidgetState`** —— 容器传给绘制过程的交互状态：
  `hovered`、`pressed`、`focused`、`enabled`、`editing` 和 `caret_on`。
- **命中测试（hit test）** —— 判断一个点是否属于某个控件；默认按矩形判断，
  可用 `.hit_test` 替换。
- **a11y** —— 无障碍：发布给屏幕阅读器的角色和名称。
- **`WidgetId`** —— 由 `set_rect`、`with_widget` 和焦点使用的稳定 id。
- **皮肤属性（skin attribute）** —— 控件声明的、每个实例可选的值，
  以便 `.buixml` 文档设置它；控件负责校验并应用该值。
- **`.buixml`** —— 布局文档（第 05 页），可以按 id 定位控件并
  设置它们声明的属性。

## 前置条件

- [03](03-first-window.md) 中的窗口可以构建并运行。
- 无需额外特性开关；`CustomWidget` 和 `Widget` 总是
  被编译。

## 步骤

### 1. 扩展消息类型

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Msg {
    Greet,      // 第 03 页已有的按钮消息。
    BarClicked, // 水平条被点击时发出。
}
```

让 `apply` 中的 `match` 保持穷尽：加上第 2 步的新分支，否则编译器会报告缺少分支。

### 2. 路径 A —— `CustomWidget`

`CustomWidget::new(id, paint)` 接收一个绘制闭包；`.input(handler)` 添加一个
返回 `Option<M>` 的输入闭包。闭包是就在使用处编写的内联函数；这里第一个闭包接收绘制上下文、控件
矩形和容器的 `WidgetState`，第二个接收矩形和一个 `WidgetInput`。像任何控件一样注册结果。把这个
辅助函数放在 `main.rs` 底部、`main` 旁边：

```rust
// 用闭包绘制的控件类型。
use bamboo_native_ui::widgets::CustomWidget;
// 下面两个闭包用到的几何和输入类型。
use bamboo_native_ui::{Rect, WidgetId, WidgetInput};

// 模块级辅助函数：从 App::new 调用一次，添加这个条。
fn add_level_bar(ui: &mut bamboo_native_ui::Ui<Msg>) {
    ui.add(
        // 绘制闭包：ctx 含画布/绘制器，rect 是控件边界，
        // state 携带来自容器的悬停/按下/焦点状态。
        CustomWidget::new("level", |ctx, rect, state| {
            let palette = ctx.palette;
            let radius = rect.height() / 2.0; // 胶囊形状。
            ctx.painter
                // 填充背后的轨道。
                .round_rect(ctx.canvas, rect, radius, palette.scroll_track);
            // 本演示固定填充 60%；绘制在 rect 内。
            let filled = Rect::from_xywh(rect.left, rect.top, rect.width() * 0.6, rect.height());
            let color = if state.hovered {
                palette.accent // 悬停由容器报告，不涉及本地状态。
            } else {
                palette.scroll_thumb
            };
            ctx.painter.round_rect(ctx.canvas, filled, radius, color);
        })
        // 初始矩形；稍后的 set_rect 可以替换它。
        .rect(Rect::from_xywh(24.0, 120.0, 240.0, 12.0))
        // 指针输入必需；CustomWidget 默认是 false。
        .interactive(true)
        // 输入闭包：返回零个或一个消息。
        .input(|_rect, input| match input {
            WidgetInput::ClickAt(_) => Some(Msg::BarClicked),
            _ => None, // 忽略其他输入。
        }),
    );
}
```

`Rect` 和 `WidgetInput` 供两个闭包使用。`WidgetId` 在同一个
导入里，是因为路径 B 会用到它；如果你只做到路径 A，编译器可能警告
该导入未使用。

注意：

- `CustomWidget` 默认为 `focusable(false)` 和 `interactive(false)`，因此只用于显示的组件永远不会抢占点击。调用 `.interactive(true)` 以
  接收指针输入，调用 `.focusable(true)` 以加入 Tab 顺序（
  容器随后绘制焦点环）。
- 绘制闭包接收控件矩形和一个 `WidgetState`，包含
  `hovered`、`pressed`、`focused`、`enabled`、`editing` 和 `caret_on`。
- 其他构建器是 `preferred_height`、`enabled`、`cursor`、`hit_test`
  （一个非矩形命中区域）、`a11y` 以及表面构建器
  （`.corner_radius`、`.background`）。

在 `App::new` 中、标签和按钮添加之后调用 `add_level_bar(&mut ui)`；
`App::new` 没有 `self`，因此传入它局部的 `ui`。然后在 `App::apply` 中处理该消息：

```rust
// 放在 App::apply 的 match 中，Msg::Greet 分支旁边。
Msg::BarClicked => {
    // 复用第 03 页的状态标签。
    self.ui.set_label(WidgetId::new("status"), "Bar clicked");
}
```

### 3. 路径 B —— 实现 `Widget<M>`

该 trait 的必需方法是 `id`、`rect`、`set_rect`、`as_any`、
`as_any_mut`、`paint` 和 `input`。其余都有默认实现。

`id` 标识控件；`rect`/`set_rect` 把它的边界暴露给容器；
`paint` 在这些边界内绘制；`input` 返回 `Option<Msg>`。两个
`as_any` 方法支持向下转型，`Ui::with_widget` 和
`Ui::edit_widget` 正是这样从 `dyn Widget<Msg>` 取回你的具体 `LevelBar` 类型。
这里类型拥有 `level`，因此绘制和输入通过 `&self` 和
`&mut self` 共享它。

```rust
// 控件 trait 及其方法用到的类型。
use bamboo_native_ui::{Rect, Widget, WidgetId, WidgetInput, WidgetPaint, WidgetState};

struct LevelBar {
    id: WidgetId, // 用于 set_rect、焦点和无障碍的稳定标识。
    rect: Rect,   // 当前边界，单位为 DIP。
    level: f32,   // 绘制和输入共享的自持状态（0.0..=1.0）。
}

impl LevelBar {
    fn new(id: &'static str, level: f32) -> Self {
        Self {
            id: WidgetId::new(id),
            // 标称尺寸；resize() 会替换这个矩形。
            rect: Rect::from_xywh(0.0, 0.0, 240.0, 12.0),
            level,
        }
    }
}

// 实现框架的控件接口；其余由 trait 默认实现。
impl Widget<Msg> for LevelBar {
    fn id(&self) -> WidgetId {
        self.id
    }

    // 容器用这个矩形做命中测试。
    fn rect(&self) -> Rect {
        self.rect
    }

    // 由应用调用，通常来自 resize。
    fn set_rect(&mut self, rect: Rect) {
        self.rect = rect;
    }

    // 供 Ui::with_widget 向下转型。
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    // 供 Ui::edit_widget 向下转型。
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    // 只在 self.rect 内绘制；宿主只 blit 该区域。
    fn paint(&self, ctx: &mut WidgetPaint<'_>, state: &WidgetState) {
        let palette = ctx.palette;
        let radius = self.rect.height() / 2.0; // 胶囊形状。
        ctx.painter
            // 填充背后的轨道。
            .round_rect(ctx.canvas, self.rect, radius, palette.scroll_track);
        // 做钳制，让越界的 level 仍留在轨道内。
        let width = self.rect.width() * self.level.clamp(0.0, 1.0);
        let filled = Rect::from_xywh(self.rect.left, self.rect.top, width, self.rect.height());
        let color = if state.hovered {
            palette.accent // 悬停状态来自容器。
        } else {
            palette.scroll_thumb
        };
        ctx.painter.round_rect(ctx.canvas, filled, radius, color);
    }

    // 点击返回一个消息，其余情况返回 None。
    fn input(&mut self, input: WidgetInput) -> Option<Msg> {
        match input {
            WidgetInput::ClickAt(_) => Some(Msg::BarClicked),
            _ => None,
        }
    }
}
```

在 `App::new` 中注册它，在 `App::resize` 中定位它，就像内置控件一样：

```rust
// 在 App::new 中，标签和按钮旁边。
self.ui.add(LevelBar::new("level", 0.6)); // 填充 60%。
// 在 App::resize 中，其他 set_rect 调用旁边：
self.ui.set_rect(
    WidgetId::new("level"), // 与 LevelBar::new 相同的 id。
    Rect::from_xywh(24.0, 120.0, size.width - 48.0, 12.0),
);
```

`as_any`/`as_any_mut` 是 `Ui::with_widget` 和 `Ui::edit_widget` 用来
到达具体类型的途径，因此返回 `self`。

### 4. 可选 —— 声明 XML 皮肤属性

控件可以接受来自 `.buixml` 文档的局部属性。皮肤协议就是让这件事安全的约定：控件发布它理解的
属性名和值类型，解析器用这份列表校验文档，只有通过后控件才应用该值。这样，文档作者无需修改 Rust 代码或
重新编译，就能调整某个实例（`level="0.8"`）。默认实现
不声明任何内容，因此这是可选的，并且控件从 Rust 创建时仍然正常工作；声明本身在
没有 `feature = "layout-xml"` 的情况下也能编译。

```rust
// 用于声明和接收 .buixml 属性的类型。
use bamboo_native_ui::{SkinAttrDecl, SkinValue, SkinValueKind};

// 把 "level" 声明为数值。解析器会拒绝这个名字的其他类型。
static LEVEL_ATTRS: &[SkinAttrDecl] = &[SkinAttrDecl::new("level", SkinValueKind::Number)];

// 这些方法放在 `impl Widget<Msg> for LevelBar` 块内：
// 发布本控件接受的属性名。
fn skin_attrs(&self) -> &'static [SkinAttrDecl] {
    LEVEL_ATTRS
}

fn apply_skin_attr(&mut self, name: &str, value: &SkinValue<'_>) {
    // 忽略本控件未声明的名字。
    if name != "level" {
        return;
    }
    // 使用之前先检查值的种类。
    if let SkinValue::Number(level) = value {
        self.level = (*level as f32).clamp(0.0, 1.0);
    }
}

// 没有数据子元素。
fn skin_children(&self) -> &'static [&'static str] {
    &[]
}
```

然后文档可以写
`<widget id="level" x="16" y="120" w="240" h="12" level="0.8"/>`。相关
方法：`validate_skin_attr`（在应用任何内容之前做无副作用检查）、`skin_children`、`validate_skin_children` 和 `apply_skin_children`
（用于数据子元素）。以 `x-` 开头的属性名保留给
应用代码。完整协议见
[自定义控件](../widgets/custom/custom-widget.md)。

### 5. 运行它

在外层 `bamboo-hello` 文件夹（含工作区 `Cargo.toml` 的那个）中运行：

```powershell
# 必要时构建并打开窗口；关闭窗口即返回提示符。
cargo run
```

悬停在条上：填充部分切换为强调色。点击它：
状态标签变为 `Bar clicked`。再次点击：它仍停留在 `Bar clicked`，
因为本页把这个消息映射为固定字符串，而不是计数器。

如果窗口没有出现，编译错误或 `bamboo: ...` 行就在这个终端里；
窗口无法显示错误。

**刚才发生了什么。** 控件被加入与标签和按钮相同的 `Ui`，因此它像内置控件一样
参与命中测试、脏矩形和绘制。它的输入处理器返回了一个消息，`apply` 修改了标签。

## 预期结果

按钮下方出现一个圆角条，路径 A 中宽 240 DIP、高 12 DIP。悬停只改变该条的矩形；
点击发出消息，应用把它变成标签修改。标签
重绘和条重绘是两个独立的脏矩形，因此悬停不会重绘按钮。

## 故障排查

- **条不可见** —— 绘制闭包在 `rect` 外绘制，或
  矩形为空。控件必须在其自己的矩形内绘制；宿主
  只 blit 该区域。检查 `set_rect` 是否为 id `"level"` 运行过，
  以及矩形是否位于 y = 120、处在高 320 DIP 的客户区内。
- **点击无效** —— `CustomWidget` 默认非交互。添加
  `.interactive(true)`。对于手动实现，除非你自己处理输入，
  不要将 `focusable`/`interactive` 重写为 `false`。该条
  只有 12 DIP 高，所以要瞄准条内；点击周围的背景属于控件之外。
- **`with_widget` 返回 `None`** —— 控件以错误的类型添加，或
  `as_any` 返回的不是 `self`。
- **指针离开后悬停高亮仍保持** —— 容器
  不知道控件移动了。从 `rect()` 返回正确的矩形，并在它变化时调用
  `set_rect`。
- **粘贴后出现诸如 "expected one of ..." 的编译错误** —— 这些方法
  应属于 `impl` 块内。皮肤辅助方法（`skin_attrs`、
  `apply_skin_attr`、`skin_children`）放进 `impl Widget<Msg> for LevelBar`，
  而 `add_level_bar` 是模块级的自由函数，不是 `App` 的方法。
- **`cannot find function add_level_bar`** —— 辅助函数不在当前
  作用域中，或它被放进了 `impl App`。把它放在 `main` 旁边，并从 `App::new` 用局部的 `ui` 调用它：`add_level_bar(&mut ui);`。
- **"non-exhaustive patterns: `Msg::BarClicked` not covered"** —— `App::apply` 中的 `match` 仍只有旧分支。加上 `Msg::BarClicked` 分支。
- **窗口打开了，但条从未出现** —— 定义了 `add_level_bar`
  却没有调用，或没有矩形到达控件。控件由 `add` 注册；它的矩形来自 `CustomWidget` 的 `.rect(...)` 或稍后的 `set_rect`。确保其中一个提供了非空矩形。
- **悬停有效，但点击不改变标签** —— 输入闭包
  缺失或返回 `None`，缺少消息分支，或 `flush_damage` 被删掉了。每一步都必需：闭包 → 排队的消息 → `apply` → 弄脏标签 → flush。
- **`unused import: WidgetId` 警告** —— 如果你只实现了
  路径 A，这是预期。要么从那个导入中删掉 `WidgetId`，要么保留使用它的路径 B。
- **XML 的 `level` 属性没有效果** —— 文档需要
  `feature = "layout-xml"`（第 05 页），并且名字必须在
  `skin_attrs` 中声明。以 `x-` 开头的名字属于应用代码，而不是控件。
- **修改后出现的仍是旧窗口** —— 编辑器没有保存
  `main.rs`，或上一个程序仍然开着。按 Ctrl+S 保存，关闭上一次运行留下的窗口，然后再次启动 `cargo run`。
- **链接步骤失败并提示 "link.exe not found"** —— 缺少 C++ 构建工具或
  Windows SDK。安装 **使用 C++ 的桌面开发**
  工作负载并打开一个新终端；第 01 页有详细说明。
- **`cargo` 不被识别，或 `could not find Cargo.toml`** —— 使用外层
  `bamboo-hello` 文件夹，以及在安装 Rust 之后打开的终端；第 01 页的检查在这里同样适用。

## 下一步

[05 — 动态 XML](05-xml-dynamic.md)：将矩形和一些属性
从 Rust 中移出，放入 `.buixml` 文件。
