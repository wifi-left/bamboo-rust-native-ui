# 控件注册表与创建

> 让布局文档按类型名创建控件，并将其 `command` id 映射到应用的消息。

## 概述

在 M7.7 之前，`.buixml` 文档只能寻址应用已构建的控件。注册表改变了这一点：
`<widget type="button" …>` 在其 id 不存在时，用为该类型名注册的工厂创建一个 `Button`。
模块 `bamboo_native_ui::creation` 总是会被编译；*创建*需要 `feature = "layout-xml"` 或
`feature = "layout-compiled"`，因为它在一次应用内部运行。

被创建的控件就是普通控件。几何、共享内容、表面/字体令牌和控件本地的皮肤数据，都由处理
寻址 id 的同一个应用器处理；工厂只提供构造函数。这正是工厂可以用空文本构建的原因——文件中的
`label`/`text`/`selected` 值会在插入后立即到达。

文档拥有它所创建的内容：不再声明某个 id 的重载会移除它，`Ui::clear_layout` 会移除所有被
创建的控件。被创建的控件会插入到**页面层**——位于第一个背景控件（`Scrim`）之前，若没有则
位于末尾——因此模态遮罩会继续绘制在它们之上。

## 关键类型

### `WidgetRegistry`

```rust
pub struct WidgetRegistry<M> { /* private */ }

impl<M: 'static> WidgetRegistry<M> {
    pub fn new() -> Self;
    pub fn standard() -> Self where M: Clone;
    pub fn register(
        &mut self,
        type_name: &'static str,
        factory: impl Fn(&WidgetSpec<'_>, &FactoryCtx<'_, M>)
                -> Result<Box<dyn Widget<M>>, FactoryError>
            + 'static,
    );
    pub fn set_commands(&mut self, commands: impl CommandMap<M> + 'static);
    pub fn has_type(&self, type_name: &str) -> bool;
}

impl<M> WidgetRegistry<M> {
    pub fn type_names(&self) -> impl Iterator<Item = &'static str> + '_;
    pub fn has_commands(&self) -> bool;
}
```

`new()` 是空的：此时文档中的 `type` 条目会变成 `xml.type.unknown`。`register` 会替换同名
工厂。`standard()` 安装 `STANDARD_TYPES` 中的十二个内置项；`toggle` 和 `slider` 在每次
变化时都会报告其命令，因此 `standard()` 要求 `M: Clone`。

```rust
pub const STANDARD_TYPES: &[&str] = &[
    "label", "button", "toggle", "slider", "progress", "icon", "badge",
    "card", "divider", "kbd", "avatar", "spinner",
];
```

### `Factory`、`WidgetSpec`、`FactoryCtx`

```rust
pub type Factory<M> =
    dyn Fn(&WidgetSpec<'_>, &FactoryCtx<'_, M>) -> Result<Box<dyn Widget<M>>, FactoryError>;

pub struct WidgetSpec<'a> {
    pub id: WidgetId,
    pub rect: Rect,
    pub content: &'a [ContentAttr],
    pub attrs: &'a [(String, String)],
    pub children: &'a [SkinElement],
    pub command: Option<u32>,
    pub line: usize,
    pub path: &'a str,
}

impl WidgetSpec<'_> {
    pub fn id(&self) -> WidgetId;
    pub fn label(&self) -> Option<&str>;       // 第一个 label/text 值
    pub fn attr(&self, name: &str) -> Option<&str>;
}

pub struct FactoryCtx<'a, M> {
    pub palette: &'a Palette,
    pub client: Size,
    pub commands: Option<&'a dyn CommandMap<M>>,
}

impl<M> FactoryCtx<'_, M> {
    pub fn message(&self, command: Option<u32>) -> Option<M>;
}
```

`rect` 已针对当前断点解析。`attrs` 和 `children` 是文件的原始本地数据（值从不是表达式）；
应用器在插入后写入它们，因此工厂通常只需要命令。

### `CommandMap`、`FactoryError`

```rust
pub trait CommandMap<M> {
    fn message(&self, command: u32) -> Option<M>;
}

impl<M, F> CommandMap<M> for F where F: Fn(u32) -> Option<M> { /* … */ }

pub enum FactoryError {
    Missing(&'static str),   // 报告为 xml.command.missing
    Rejected(String),        // 报告为 xml.type.create
}

impl FactoryError {
    pub fn rejected(message: impl Into<String>) -> Self;
}
```

普通闭包就是一个 `CommandMap`，因此 `set_commands(|id| …)` 是常用形式。一个命令 id 映射到
一个消息值。

### 安装到容器上

```rust
impl<M> Ui<M> {
    pub fn set_widget_registry(&mut self, registry: WidgetRegistry<M>);
    pub fn widget_registry(&self) -> Option<&WidgetRegistry<M>>;
    pub fn widget_registry_mut(&mut self) -> Option<&mut WidgetRegistry<M>>;
    pub fn clear_widget_registry(&mut self);
}
```

替换注册表会影响下一次应用；屏幕上已有的控件会保留，直到重载移除它们。

## 文档属性

| 属性 | 含义 |
|---|---|
| `type="name"` | 当该 id 不存在时从注册表创建它。名称匹配 `[a-z][a-z0-9_.-]*`；`x-*`/`vendor.type` 名称归属于应用。 |
| `command="101"` | 被创建的交互式控件通过 `CommandMap` 解析的 `u32` 命令 id。仅可与 `type` 一起使用。 |
| `parent="panel"` | 该条目所属的滚动画布。在被创建的条目上它是安装的成员关系；在寻址条目上它断言应用的成员关系（不匹配时为 `xml.parent.missing`）。 |

缺少 `type` 时保持仅寻址的行为。generation 变化时，`type` 条目会更新本文档已创建的 id
（相同的 `type`/`command`），或移除并重新创建它（不同的 `type`/`command`）。已存在且非
文档所有的 `type` id 会报 `xml.id.duplicate`；未注册的名称会报 `xml.type.unknown`。

```xml
<layouts version="1">
  <layout id="base">
    <widget id="hello" type="label" x="10" y="10" w="100" h="20" label="你好"/>
    <widget id="go" type="button" command="101" x="10" y="40" w="80" h="30" label="出发"/>
    <widget id="row" type="label" parent="panel" x="0" y="250" w="80" h="20" label="行"/>
  </layout>
</layouts>
```

## 示例

```rust
use bamboo_native_ui::creation::WidgetRegistry;
use bamboo_native_ui::Ui;

#[derive(Clone)]
enum Msg {
    Save,
    Volume(f32),
}

fn registry() -> WidgetRegistry<Msg> {
    let mut registry = WidgetRegistry::<Msg>::standard();
    registry.set_commands(|command| match command {
        101 => Some(Msg::Save),
        102 => Some(Msg::Volume(0.0)),
        _ => None,
    });
    registry
}

fn install(ui: &mut Ui<Msg>) {
    ui.set_widget_registry(registry());
}
```

随后 `<widget id="go" type="button" command="101" …/>` 会创建一个 `Button`，其点击会报告
`Msg::Save`。

要添加私有类型，请在您自己的代码中注册它（此处的 `Meter` 是任意 `Widget<Msg>` 实现）：

```rust
let mut registry = WidgetRegistry::<Msg>::new();
registry.register("x-meter", |spec, _ctx| {
    Ok(Box::new(Meter::new(spec.id.name(), spec.rect)))
});
```

工厂必须返回 `id()` 恰好为 `spec.id` 的控件；应用器会以 `xml.type.create` 拒绝其他任何
情况。参见
[XML layout authoring](../guides/xml-layout-authoring.md#custom-widgets)。

## 说明与限制

- 被创建的控件属于内容：其皮肤数据和内容在 generation 变化时写入，而非在调整尺寸时。
- 交互式标准类型需要命令表和 `command` 值；缺少任一者时工厂返回
  `FactoryError::Missing("command")`（`xml.command.missing`）。
- 对于仅点击的控件，`M` 无需为 `Clone`。只有 `toggle` 和 `slider` 需要它（它们的回调在
  每次变化时产生相同的消息；可通过类型化访问器读回状态）。
- 在一个变体中为某 id 声明 `type` 的文档，必须在每个变体中都声明它，且类型/命令相同
  （`xml.type.conflict`）。
- 工厂错误是原子的：一个失败的条目会拒绝整个应用，并保留之前的布局不变。
- 注册表以 `widget_types` 和 `has_commands` 暴露给调试表单；参见
  [diagnostics-and-debug.md](diagnostics-and-debug.md)。
- 编译文档携带相同的 `type`/`command`/`parent` 数据
  （`CompiledEntry::type_name`/`command`/`parent`）；其他一切不变。

## 另见

- [layout-documents.md](layout-documents.md) — `LayoutEntry::type_name`、`command`、`parent`
- [layout-codegen.md](layout-codegen.md) — 编译文档中的创建
- [props-and-narration.md](props-and-narration.md) — 工厂可能遵循的 `WidgetProps`
- [XML layout authoring](../guides/xml-layout-authoring.md#custom-widgets)
