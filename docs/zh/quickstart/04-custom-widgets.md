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

## 前置条件

- [03](03-first-window.md) 中的窗口可以构建并运行。
- 无需额外特性开关；`CustomWidget` 和 `Widget` 总是
  被编译。

## 步骤

### 1. 扩展消息类型

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Msg {
    Greet,
    BarClicked,
}
```

### 2. 路径 A —— `CustomWidget`

`CustomWidget::new(id, paint)` 接收一个绘制闭包；`.input(handler)` 添加一个
返回 `Option<M>` 的输入闭包。像任何控件一样注册它：

```rust
use bamboo_native_ui::widgets::CustomWidget;
use bamboo_native_ui::{Rect, WidgetId, WidgetInput};

fn add_level_bar(ui: &mut bamboo_native_ui::Ui<Msg>) {
    ui.add(
        CustomWidget::new("level", |ctx, rect, state| {
            let palette = ctx.palette;
            let radius = rect.height() / 2.0;
            ctx.painter
                .round_rect(ctx.canvas, rect, radius, palette.scroll_track);
            let filled = Rect::from_xywh(rect.left, rect.top, rect.width() * 0.6, rect.height());
            let color = if state.hovered {
                palette.accent
            } else {
                palette.scroll_thumb
            };
            ctx.painter.round_rect(ctx.canvas, filled, radius, color);
        })
        .rect(Rect::from_xywh(24.0, 120.0, 240.0, 12.0))
        .interactive(true)
        .input(|_rect, input| match input {
            WidgetInput::ClickAt(_) => Some(Msg::BarClicked),
            _ => None,
        }),
    );
}
```

注意：

- `CustomWidget` 默认为 `focusable(false)` 和 `interactive(false)`，因此只用于显示的组件永远不会抢占点击。调用 `.interactive(true)` 以
  接收指针输入，调用 `.focusable(true)` 以加入 Tab 顺序（
  容器随后绘制焦点环）。
- 绘制闭包接收控件矩形和一个 `WidgetState`，包含
  `hovered`、`pressed`、`focused`、`enabled`、`editing` 和 `caret_on`。
- 其他构建器是 `preferred_height`、`enabled`、`cursor`、`hit_test`
  （一个非矩形命中区域）、`a11y` 以及表面构建器
  （`.corner_radius`、`.background`）。

从 `App::new` 调用 `add_level_bar(&mut self.ui)` 并处理消息：

```rust
Msg::BarClicked => {
    self.ui.set_label(WidgetId::new("status"), "Bar clicked");
}
```

### 3. 路径 B —— 实现 `Widget<M>`

该 trait 的必需方法是 `id`、`rect`、`set_rect`、`as_any`、
`as_any_mut`、`paint` 和 `input`。其余都有默认实现。

```rust
use bamboo_native_ui::{Rect, Widget, WidgetId, WidgetInput, WidgetPaint, WidgetState};

struct LevelBar {
    id: WidgetId,
    rect: Rect,
    level: f32,
}

impl LevelBar {
    fn new(id: &'static str, level: f32) -> Self {
        Self {
            id: WidgetId::new(id),
            rect: Rect::from_xywh(0.0, 0.0, 240.0, 12.0),
            level,
        }
    }
}

impl Widget<Msg> for LevelBar {
    fn id(&self) -> WidgetId {
        self.id
    }

    fn rect(&self) -> Rect {
        self.rect
    }

    fn set_rect(&mut self, rect: Rect) {
        self.rect = rect;
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn paint(&self, ctx: &mut WidgetPaint<'_>, state: &WidgetState) {
        let palette = ctx.palette;
        let radius = self.rect.height() / 2.0;
        ctx.painter
            .round_rect(ctx.canvas, self.rect, radius, palette.scroll_track);
        let width = self.rect.width() * self.level.clamp(0.0, 1.0);
        let filled = Rect::from_xywh(self.rect.left, self.rect.top, width, self.rect.height());
        let color = if state.hovered {
            palette.accent
        } else {
            palette.scroll_thumb
        };
        ctx.painter.round_rect(ctx.canvas, filled, radius, color);
    }

    fn input(&mut self, input: WidgetInput) -> Option<Msg> {
        match input {
            WidgetInput::ClickAt(_) => Some(Msg::BarClicked),
            _ => None,
        }
    }
}
```

像内置控件一样注册并定位它：

```rust
self.ui.add(LevelBar::new("level", 0.6));
// in resize:
self.ui.set_rect(
    WidgetId::new("level"),
    Rect::from_xywh(24.0, 120.0, size.width - 48.0, 12.0),
);
```

`as_any`/`as_any_mut` 是 `Ui::with_widget` 和 `Ui::edit_widget` 用来
到达具体类型的途径，因此返回 `self`。

### 4. 可选 —— 声明 XML 皮肤属性

控件可以接受来自 `.buixml` 文档的局部属性。用
`skin_attrs` 发布名称，验证每个值，并应用它。默认实现
不声明任何内容，因此这是可选的，并且无需
`feature = "layout-xml"` 即可工作。

```rust
use bamboo_native_ui::{SkinAttrDecl, SkinValue, SkinValueKind};

static LEVEL_ATTRS: &[SkinAttrDecl] = &[SkinAttrDecl::new("level", SkinValueKind::Number)];

// These methods go inside the `impl Widget<Msg> for LevelBar` block:
fn skin_attrs(&self) -> &'static [SkinAttrDecl] {
    LEVEL_ATTRS
}

fn apply_skin_attr(&mut self, name: &str, value: &SkinValue<'_>) {
    if name != "level" {
        return;
    }
    if let SkinValue::Number(level) = value {
        self.level = (*level as f32).clamp(0.0, 1.0);
    }
}

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

```powershell
cargo run
```

悬停在条上：填充部分切换为强调色。点击它：
状态标签变为 `Bar clicked`。

## 预期结果

按钮下方出现一个圆角条。悬停只改变该条的矩形；
点击发出消息，应用把它变成标签修改。标签
重绘和条重绘是两个独立的脏矩形。

## 故障排查

- **条不可见** —— 绘制闭包在 `rect` 外绘制，或
  矩形为空。控件必须在其自己的矩形内绘制；宿主
  只 blit 该区域。
- **点击无效** —— `CustomWidget` 默认为非交互。添加
  `.interactive(true)`。对于手动实现，除非你自己处理输入，
  不要将 `focusable`/`interactive` 重写为 `false`。
- **`with_widget` 返回 `None`** —— 控件以错误的类型添加，或
  `as_any` 返回的不是 `self`。
- **指针离开后悬停高亮仍保持** —— 容器
  不知道控件移动了。从 `rect()` 返回正确的矩形，并在它变化时调用
  `set_rect`。

## 下一步

[05 — 动态 XML](05-xml-dynamic.md)：将矩形和一些属性
从 Rust 中移出，放入 `.buixml` 文件。
