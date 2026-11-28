# 测试

> 运行工作区测试矩阵、保持生成的布局模块为最新，并
> 在不打开窗口的情况下对控件进行单元测试。

## 不开窗口能测试什么

`Painter` 和 `Canvas` 之上的一切都是平台中立的，因此控件逻辑、
布局解析、布局应用、辅助功能快照和编译后
布局描述符都在普通的 `cargo test` 中运行。只有 `#[cfg(windows)]` 宿主
（`Host`、`WindowBuilder`、`WindowHandle`、GDI/Direct2D 后端以及 UIA
provider）需要 showcase 风格的探针。

| 层 | 测试风格 |
|---|---|
| 控件的绘制/输入/辅助功能 | 一个假 `Canvas` 加上 `WidgetPaint` |
| 解析器和模型 | `bamboo-native-ui-layout-core` 单元测试 |
| 动态应用 | 用 `TestHost` 接收端调用 `Ui::apply_layout` |
| 编译应用 | 检入的生成模块加上同一个 `TestHost` |
| 宿主、UIA、IME | showcase 的 `--self-test` 和 `--a11y-dump` |

## 步骤

### 1. 运行矩阵

```powershell
cargo build
cargo test -p bamboo-native-ui
cargo test -p bamboo-native-ui-layout-core --features parse
cargo test -p bamboo-native-ui --features layout-xml
cargo test -p bamboo-native-ui --features theme-xml
cargo test -p bamboo-native-ui --features debug-form
cargo test -p bamboo-native-ui --features layout-compiled
cargo test -p bamboo-native-ui --features layout-xml,layout-compiled
cargo test -p bamboo-native-ui-layout-codegen
cargo test -p bamboo-native-ui-layout-macros
```

默认套件覆盖控件库和容器。每个特性套件添加
自己的测试：

- `layout-xml` 添加解析器/应用器测试，以及集成测试
  `crates/native-ui/tests/layout_parity.rs`（动态和编译模式产生
  相同结果）、`layout_rust.rs`（编译模式中的内联 `rust:` 值、
  动态模式拒绝）、`layout_context.rs`（`context = Type` 模块）以及
  `layout_create.rs`（文档创建的控件）。
- `theme-xml` 添加 `.buitheme` 往返、base/accent 派生以及
  token 覆盖测试。
- `debug-form` 添加快照和诊断环形缓冲区测试。
- `layout-compiled` 添加描述符和编译应用测试；它不链接任何
  XML 解析器。
- codegen crate 固定检入的生成模块；宏 crate
  检查 `layout_xml!` 展开错误。

在非 Windows 的 CI 目标上，还要用 `--no-default-features` 构建：它会移除
GDI 后端（`feature = "gdi"`），且必须仍能编译可移植层。
在 Windows 上平台层需要 `gdi`，因此在那里保留默认特性。

### 2. 保持生成的模块为最新

当检入的生成文件过期时，codegen 基准测试会失败。
修改 fixture 之后重新生成：

```powershell
cargo run -p bamboo-native-ui-layout-codegen -- crates/native-ui/tests/fixtures/parity.buixml -o crates/native-ui/tests/fixtures/parity_generated.rs
cargo run -p bamboo-native-ui-layout-codegen -- crates/native-ui/tests/fixtures/rust.buixml --module rust_layout -o crates/native-ui/tests/fixtures/rust_generated.rs
cargo run -p bamboo-native-ui-layout-codegen -- crates/native-ui/tests/fixtures/context.buixml --module context_layout --context AppLayout -o crates/native-ui/tests/fixtures/context_generated.rs
cargo run -p bamboo-native-ui-layout-codegen -- crates/native-ui/tests/fixtures/binding.buixml --module binding_layout -o crates/native-ui/tests/fixtures/binding_generated.rs
cargo run -p bamboo-native-ui-layout-codegen -- crates/native-ui/tests/fixtures/create.buixml --module create_layout -o crates/native-ui/tests/fixtures/create_generated.rs
cargo test -p bamboo-native-ui-layout-codegen
```

文档错误会使 CLI 以 1 退出，打印带有其 `.buixml`
行和列的诊断，并且不写入任何内容。

### 3. 测试控件的绘制和输入

后端 trait 足够小，可以打桩。下面的桩每个字符测量 8 DIP，
且不记录任何内容。

```rust
use bamboo_native_ui::backend::{Canvas, ImageData, ImageId, TextAlign, TextStyle};
use bamboo_native_ui::geometry::{Rect, Size};
use bamboo_native_ui::{Color, ImageStore, Painter, Widget, WidgetInput, WidgetPaint};

#[derive(Default)]
struct BlankCanvas;

impl Canvas for BlankCanvas {
    fn scale(&self) -> f32 { 1.0 }
    fn dpi(&self) -> f32 { 96.0 }
    fn fill_rect(&mut self, _rect: Rect, _color: Color) {}
    fn text(
        &mut self,
        _rect: Rect,
        _text: &str,
        _style: &TextStyle,
        _align: TextAlign,
        _color: Color,
    ) {
    }
    fn measure_text(&mut self, text: &str, _style: &TextStyle) -> Size {
        Size::new(text.chars().count() as f32 * 8.0, 16.0)
    }
    fn ellipse(&mut self, _rect: Rect, _color: Color) {}
    fn vertical_gradient(&mut self, _rect: Rect, _top: Color, _bottom: Color) {}
    fn image(&mut self, _rect: Rect, _crop: bool, _id: ImageId, _data: ImageData<'_>) {}
    fn push_clip(&mut self, _rect: Rect) {}
    fn pop_clip(&mut self) {}
}

#[test]
fn a_click_flips_the_checkbox() {
    use bamboo_native_ui::widgets::Checkbox;
    use bamboo_native_ui::{Point, WidgetState};

    #[derive(Debug, PartialEq)]
    enum Msg {
        Remember(bool),
    }

    let mut check = Checkbox::new("remember", "Remember me", true, Msg::Remember);
    check.set_rect(Rect::from_xywh(0.0, 0.0, 180.0, 28.0));

    let painter = Painter::new();
    let palette = *painter.palette();
    let images = ImageStore::new();
    let mut canvas = BlankCanvas;
    let mut paint = WidgetPaint {
        painter: &painter,
        canvas: &mut canvas,
        palette,
        font: None,
        images: &images,
        viewport: Rect::from_xywh(0.0, 0.0, 400.0, 300.0),
        dirty: &[Rect::from_xywh(0.0, 0.0, 180.0, 28.0)],
    };
    check.paint(&mut paint, &WidgetState { enabled: true, ..Default::default() });

    let message = check.input(WidgetInput::ClickAt(Point::new(8.0, 14.0)));
    assert_eq!(message, Some(Msg::Remember(false)));
    assert!(!check.checked());
}
```

相同的模式测试 `Activate`、`Key`、`Wheel`、`Step` 以及悬停输入。
`WidgetState` 携带 `hovered`、`pressed`、`focused`、`enabled`、`editing` 和
`caret_on`；设置你的绘制路径读取的那些。

对于绘制断言，将 `BlankCanvas` 替换为一个记录器，它在 `fill_rect` 中将
`(Rect, Color)` 推入 `Vec`、在 `text` 中推入 `(Rect, String)`，就像
内置控件测试那样，然后断言记录的几何和颜色。

### 4. 测试容器和布局

`Ui` 测试需要一个实现 `Invalidator`（必需的 `invalidate`）和
`UiHost`（所有方法均有默认值）的接收端。

```rust
use bamboo_native_ui::geometry::Rect;
use bamboo_native_ui::{Invalidator, Ui, UiHost, WidgetId};

#[derive(Default)]
struct TestHost {
    invalidated: Vec<Rect>,
}

impl Invalidator for TestHost {
    fn invalidate(&mut self, rect: Rect) {
        self.invalidated.push(rect);
    }
}

impl UiHost for TestHost {}

#[test]
fn a_label_change_records_one_damage_rect() {
    let mut ui: Ui<()> = Ui::new();
    ui.add(bamboo_native_ui::widgets::Label::new("title", "Old"));
    ui.set_rect(WidgetId::new("title"), Rect::from_xywh(0.0, 0.0, 100.0, 20.0));
    ui.set_label(WidgetId::new("title"), "New");

    let mut host = TestHost::default();
    assert!(ui.flush_damage(&mut host));
    assert_eq!(host.invalidated.len(), 1);
}
```

对于布局测试，解析一份文档，调用 `ui.apply_layout(&document, &palette,
Size::new(...), &mut host)`，并断言解析后的矩形、警告、
`ui.installed_layout()`、辅助功能树（`ui.a11y_nodes()`）或调试快照。
现有的 `layout_parity.rs` 展示了完整模式：一个 fixture 通过两种模式
应用，然后对矩形、属性、内容和节点进行结构比较。

### 5. 测试皮肤协议

如果控件声明了局部属性或子列表，请测试两个阶段：

```rust
use bamboo_native_ui::widgets::Card;
use bamboo_native_ui::{SkinValue, Widget};

#[test]
fn a_skin_attribute_reaches_the_widget() {
    let mut card = Card::new("card", "Old title", "Body");
    assert!(<Card as Widget<()>>::validate_skin_attr(
        &card,
        "title",
        &SkinValue::Text("New title"),
    ));
    <Card as Widget<()>>::apply_skin_attr(&mut card, "title", &SkinValue::Text("New title"));
    assert_eq!(card.title(), "New title");
}
```

相同的形态测试自定义控件：验证一个值、应用它，并读取
类型化的 getter。`validate_skin_attr` 必须无副作用，并且必须与
`apply_skin_attr` 一致：能通过验证的值必须能应用。应用器依赖
这一点来保持文档的原子性。

### 6. 运行宿主探针

可移植测试无法覆盖窗口过程、原生编辑控件、
IME、菜单、托盘或 UIA。这正是 showcase 探针的用途：

```powershell
cargo run -p bamboo-native-ui-showcase -- --self-test
cargo run -p bamboo-native-ui-showcase -- --a11y-dump
cargo run -p bamboo-native-ui-showcase --no-default-features -- --self-test
cargo run -p bamboo-native-ui-showcase --no-default-features -- --a11y-dump
```

`--self-test` 绘制 showcase、驱动一次原生编辑会话，并在
成功时以 0 退出。`--a11y-dump` 附加一个真实的 UIA 客户端并打印树；
非零退出意味着探针失败。在发布前以及
平台层更改之后都要运行两者。对于启动崩溃，添加 `BAMBOO_UI_DEBUG=1`（参见
[调试](debugging.md)）。

### 7. 在 bug 所在之处添加测试

| Bug | 测试位置 |
|---|---|
| 控件绘制错误 / 消息错误 | 控件自身的 `#[cfg(test)] mod tests`，使用假 canvas |
| 容器损坏/焦点/滚动 | `ui.rs` 中带 `TestHost` 的单元测试 |
| 解析器接受/拒绝了某内容 | `bamboo-native-ui-layout-core` 解析器测试 |
| 出错时应用不改变任何内容 | `layout_*` 集成测试 |
| 生成的代码过期 | `crates/layout-codegen/tests/golden.rs` |
| 仅 Windows 的回调或 UIA | showcase 探针 |

让解析器测试针对诊断代码，而不是消息文本：代码是
稳定的，消息面向开发者且可能变化。

## 另请参阅

- [调试](debugging.md)
- [特性开关](feature-flags.md)
- [XML 布局编写](xml-layout-authoring.md)
- [Widget trait](../api/widget-trait.md)
- [绘制与 Canvas](../api/painting-and-canvas.md)
- [Host 与窗口](../api/host-and-window.md)
