# XML 元素

> `.buixml` 文档中的每个元素、属性和接受值，外加
> 逐控件的皮肤词汇和稳定的诊断代码。

## 文档骨架

```xml
<?xml version="1.0" encoding="utf-8"?>
<layouts version="1">
  <const name="pad" value="16"/>
  <layout id="base" min-width="0" min-height="0">
    <scroll id="page" x="0" y="0" w="cw" h="ch" content-height="auto"/>
    <widget id="title" x="pad" y="pad" w="cw-2*pad" h="28" label="Title"/>
  </layout>
</layouts>
```

| 元素 | 属性 | 必需 | 备注 |
|---|---|---|---|
| `<layouts>` | `version` | `version` = `1` | 根；不支持 DTD、CDATA、命名空间或处理指令。 |
| `<layout>` | `id`、`min-width`、`min-height` | `id` | 必须有一个变体为 `(0, 0)`；最小值对唯一。 |
| `<const>` | `name`、`value` | 二者 | `name` 匹配 `[a-z][a-z0-9_]*`；`value` 是一个常量表达式。 |
| `<widget>` | 见下文 | `id` | 每个变体中每个 id 一个条目。 |
| `<scroll>` | `id`、`x`/`right`/`w`、`y`/`bottom`/`h`、`content-height` | `id` | 寻址一个通过 `Ui::set_scroll_view` 注册的面板。 |
| `<rust>` | `target`、`code` | 二者 | 仅编译模式；`<rust>` 必须为空。 |

`<scroll content-height>` 接受一个数字/表达式（精确高度）、`auto`
（增长以适配；从不收缩），或省略（保持）。

### 变体

选择：在 `min-width` 和 `min-height` 都 `<=` 客户端尺寸（DIP）的
`<layout>` 元素中，最大的 `min-width`，然后最大的
`min-height` 胜出；否则使用 `(0, 0)` 变体。几何、属性和
样式可以在变体之间不同。内容和局部皮肤数据在每个声明它们的
变体中必须相等。

## `<widget>` 属性

### 标识与创建

| 属性 | 接受的值 | 备注 |
|---|---|---|
| `id` | 非空字符串 | `WidgetId` 名称。必需。 |
| `type` | `[a-z][a-z0-9_.-]*` | 当 id 不存在时创建它；需要一个 `WidgetRegistry`。 |
| `command` | 非负整数（`u32`） | 仅与 `type` 一起使用；由注册表的 `CommandMap` 解析。 |
| `parent` | 一个滚动面板 id | 对创建的控件而言是成员关系；对被寻址的控件而言是断言。 |

### 几何

恰好给出 `x`、`right`、`w` 中的两个，以及 `y`、`bottom`、`h` 中的两个：

| 轴 | 接受的组合 | 结果 |
|---|---|---|
| 横向 | `x` + `w` | 左边缘和宽度 |
| 横向 | `right` + `w` | `x = cw - right - w` |
| 横向 | `x` + `right` | 拉伸宽度 |
| 纵向 | `y` + `h` | 上边缘和高度 |
| 纵向 | `bottom` + `h` | `y = ch - bottom - h` |
| 纵向 | `y` + `bottom` | 拉伸高度 |

值是表达式。`w`/`h` 必须 `>= 0`；`right`/`bottom` 必须
`>= 0`；`x`/`y` 可以为负。零尺寸是警告。

```text
expr   := term (('+' | '-') term)*
term   := factor (('*' | '/') factor)*
factor := number | number '%' | variable | '(' expr ')' | '-' factor
```

| 变量 | 值 |
|---|---|
| `left`、`top` | `0` |
| `right`、`bottom` | 空间宽度 / 高度 |
| `cw`、`ch` | 空间宽度 / 高度 |
| `cx`、`cy` | 空间中心 |
| 一个 `<const>` 名称 | 它的值 |

`N%` 是该属性轴向的百分比（`x`/`w`/`right` 使用 `cw`；
`y`/`h`/`bottom` 使用 `ch`）。`rust:` 为仅编译模式的内联 Rust 表达式
加上前缀。

### 属性

| 属性 | 接受的值 |
|---|---|
| `visible`、`enabled`、`interactive`、`focusable`、`read-only`、`expose` | `true`、`false`、`1`、`0` |
| `focus-order` | 整数（较小的在前） |
| `tooltip`、`name`、`description`、`automation-id` | 字符串 |
| `role` | 下面的一个角色 token |
| `cursor` | `arrow`、`hand`、`i-beam`、`size-we`、`size-ns` |

角色 token：`button`、`toggle`、`slider`、`text-field`、`number-field`、
`radio-group`、`tab-list`、`list`、`multi-select-list`、`radio-item`、
`tab-item`、`list-item`、`tree`、`tree-item`、`grid`、`grid-cell`、
`column-header`、`heading`、`group`、`separator`、`text`、`combo-box`、
`progress-bar`、`menu-item`、`menu`、`menu-bar`、`tool-bar`、`status-bar`。

### 样式 token

| 属性 | 接受的值 |
|---|---|
| `radius` | 数字 `>= 0`、`small`、`medium`、`large`、`pill` |
| `corners` | 四个非负数 `tl tr br bl`（空格或逗号） |
| `background` | 调色板 token、颜色字面量、`none` |
| `font-size` | 数字 `> 0`（DIP） |
| `font-family` | 非空字符串 |

`radius` 和 `corners` 互斥。没有表面的控件
会报告 `xml.style.unsupported`（警告）并跳过表面 token。

### 内容

| 属性 | 值 | 能力 |
|---|---|---|
| `text` | 字符串 | `TEXT` |
| `label` | 字符串 | `LABEL` |
| `value` | 有限数字 | `VALUE` |
| `progress` | 数字，钳制到 `0..=1` | `PROGRESS` |
| `number` | 整数 | `NUMBER` |
| `checked` | bool | `TOGGLED` |
| `selected` | 非负整数 | `SELECTION` |

`text` 和 `label` 在一个条目上冲突；`value`、`progress` 和 `number`
彼此冲突。内容只在 generation 改变时写入。

### 内联 Rust（`rust:`）

编译模式接受在 `scalar_attr_name` 中命名的每个标量属性
（属性、样式、内容）以及几何值上的 `rust:`。动态
应用器以 `xml.rust.unsupported` 拒绝它。`<rust target="..."
code="..."/>` 仅在编译模式下用一个 `Vec<SkinElement>` 替换一个控件的子列表。

## 各控件的内容能力

| 能力 | 控件 |
|---|---|
| `TEXT` | `TextEdit`、`SearchBox`、`CopyableText`、`MarqueeText` |
| `LABEL` | `Label`、`Button`、`Badge`、`Divider`、`Avatar`、`Kbd`、`SectionTitle`、`GroupBox`、`StatCard`、`Banner`、`Checkbox`、`RadioButton`、`ColorSwatch`、`RichText`、`CopyableText`、`MarqueeText` |
| `VALUE` | `Slider`、`RatingStars`、`WaveformSeekBar` |
| `PROGRESS` | `ProgressBar`、`StatusBar` |
| `NUMBER` | `NumberInput` |
| `TOGGLED` | `Toggle`、`Checkbox` |
| `SELECTION` | `ComboBox`、`Segmented`、`Tabs`、`Pager`、`CoverWall`、`RadioGroup`、`ListView`、`MultiSelectList`、`Accordion`、`Breadcrumb`、`NavRail`、`PlaylistView`、`Table`、`TreeView` |

未列入某项能力的控件会报告 `xml.content.unsupported`
（警告）并跳过该属性。`Checkbox` 同时接受 `label` 和
`checked`；`StatusBar` 接受 `text` 和 `progress`。每项能力都由控件
发布（`Widget::content_caps`），因此别名继承相同的
集合。

## 逐控件局部属性

控件通过皮肤协议声明这些。任何其他非 schema 名称在应用时都是
`xml.skin.unknown`。以 `x-` 开头的名称属于
应用控件。

| 控件 | 属性 | 类型 / 接受的值 |
|---|---|---|
| `Accordion` | `open` | 数字 `>= 0`（节索引） |
| `Avatar` | `initials` | 文本 |
| `Avatar` | `tint` | 颜色 |
| `Avatar` | `ring` | bool |
| `Avatar` | `image` | 非负整数 `ImageId` |
| `Badge` | `tone` | `neutral`、`accent`、`success`、`warning`、`danger`（不区分大小写） |
| `Banner` | `body` | 文本 |
| `Breadcrumb` | `separator` | 单个字符 |
| `Card` | `title`、`body` | 文本 |
| `Card` | `emphasis` | bool |
| `ColorPicker` | `color` | 颜色（保留 alpha） |
| `ColorPicker` | `open` | bool |
| `ColorSwatch` | `color` | 颜色 |
| `CopyableText` | `color` | 颜色 |
| `CopyableText` | `text-style` | `body`、`small`、`title` |
| `CoverWall` | `gap`、`corner-radius`、`caption-height` | 数字 `>= 0` |
| `CoverWall` | `empty-text` | 文本 |
| `DatePicker` | `date` | 文本 `YYYY-MM-DD`，月 `1..=12`，日 `>= 1` |
| `DatePicker` | `open` | bool |
| `Icon` | `glyph` | 恰好一个字符 |
| `Icon` | `tint` | 颜色 |
| `Kbd` | `shortcut` | 文本 |
| `Label` | `color` | 颜色 |
| `Label` | `text-style` | `body`、`small`、`title` |
| `ListView` | `row-height` | 数字 `>= 24` |
| `ListView` | `alternating` | bool |
| `ListView` | `empty-text` | 文本 |
| `MarqueeText` | `offset` | 数字 `>= 0` |
| `MarqueeText` | `color` | 颜色 |
| `MarqueeText` | `text-style` | `body`、`small`、`title` |
| `NavRail` | `row-height` | 数字 `>= 18` |
| `NumberInput` | `prefix`、`suffix` | 文本 |
| `PlaybackTransport` | `playing`、`shuffle` | bool |
| `PlaybackTransport` | `repeat` | `off`/`none`、`all`/`list`、`one`/`single` |
| `PlaylistView` | `row-height` | 数字 `>= 24` |
| `PlaylistView` | `alternating` | bool |
| `PlaylistView` | `empty-text` | 文本 |
| `PropertyGrid` | `row-height` | 数字 `>= 16` |
| `PropertyGrid` | `header-height` | 数字 `>= 0` |
| `PropertyGrid` | `key-width` | 数字 `>= 40` |
| `PropertyGrid` | `grid-lines` | bool |
| `PropertyGrid` | `empty-text` | 文本 |
| `RichText` | `wrap` | bool |
| `RichText` | `align` | `left`、`center`、`right` |
| `RichText` | `text-style` | `body`、`small`、`title` |
| `RichText` | `color` | 颜色 |
| `SearchBox` | `cue` | 文本 |
| `SearchBox` | `color` | 颜色 |
| `SectionTitle` | `color` | 颜色 |
| `Sparkline` | `color` | 颜色 |
| `Sparkline` | `gap` | 数字 `>= 0` |
| `Spinner` | `phase` | 数字 |
| `Spinner` | `animating` | bool |
| `Table` | `row-height` | 数字 `>= 16` |
| `Table` | `header-height`、`cell-padding` | 数字 `>= 0` |
| `Table` | `striped`、`grid-lines`、`editable` | bool |
| `Table` | `empty-text` | 文本 |
| `Tabs` | `label-align` | `left`、`center`、`right` |
| `TextEdit` | `cue` | 文本 |
| `TextEdit` | `color` | 颜色 |
| `TimePicker` | `time` | 文本 `HH:MM`，时 `<= 23`，分 `<= 59` |
| `TimePicker` | `open` | bool |
| `Toast` | `message` | 文本 |
| `Toast` | `error` | bool |
| `TreeView` | `row-height` | 数字 `>= 16` |
| `TreeView` | `indent` | 数字 `>= 8` |
| `TreeView` | `renameable`、`draggable` | bool |
| `TreeView` | `empty-text` | 文本 |

这些控件不声明任何局部属性，也不声明任何子元素：`Button`、
`Toggle`、`Slider`、`ProgressBar`、`Divider`、`Checkbox`、`RadioButton`、
`RadioGroup`、`MultiSelectList`、`GroupBox`、`StatCard`、`EmptyState`、`Gauge`、
`RatingStars`、`Splitter`、`WaveformSeekBar`、`Toolbar`、`Ribbon`、`MenuBar`、
`MenuButton`、`SplitButton`、`Popover`、`CustomWidget`、`ProgressRing`、
`Scrim`、`Snackbar`、`TooltipCard`。`ComboBox`、`Segmented`、`Pager` 和
`StatusBar` 不声明局部属性，但确实接受上面的子列表。以下所有
控件仍然接受几何、通用属性、样式 token、字体 token
以及它们的内容能力。

别名共享基础控件的声明：`Collapsible` = `Accordion`、
`PasswordBox`/`TextArea` = `TextEdit`、`Switch` = `Toggle`。

## 子元素

子列表替换控件的数据模型；它不是实时绑定。每个
子属性都是一个由控件验证的原始字符串。

| 父级 | 子元素 | 属性 |
|---|---|---|
| `ListView` | `<item>` | `title`（必需，非空）、`subtitle`、`trailing`、`current`（bool）、`disabled`（bool） |
| `PlaylistView` | `<track>` | 与 `<item>` 相同 |
| `ComboBox` | `<option>` | `label`（必需，非空） |
| `Tabs`、`Segmented` | `<option>` | `label`（必需，非空） |
| `CoverWall` | `<cover>` | `title`（必需，非空）、`subtitle`、`image`（整数 `ImageId`）、`corner-radius`（数字） |
| `StatusBar` | `<segment>` | `text`（必需，非空）、`icon`（单个字符）、`width`（数字，钳制 `>= 20`）、`align`（`left`/`center`/`right`） |
| `Table` | `<column>` | `title`（必需，非空）、`width`（数字 `>= 0`）、`flex`（bool）、`align`（`left`/`center`/`right`） |
| `Table` | `<row>` | `disabled`（bool）；子元素 `<cell text="..."/>` |
| `Table` | `<cell>` | `text`（必需；可为空） |
| `TreeView` | `<node>` | `label`（必需，非空）、`glyph`（单个字符）、`secondary`、`expanded`（bool）、`disabled`（bool）；嵌套 `<node>` |
| `PropertyGrid` | `<section>` | `title`（文本）；子元素 `<row>` |
| `PropertyGrid` | `<row>` | `key`（必需）、`value`、`kind`、`read-only`（bool），外加该 kind 的参数 |
| `Pager` | `<page>` | `label`（必需，非空）；页面计数变为列表长度 |
| `Breadcrumb` | `<step>` | `label`（必需，非空） |
| `NavRail` | `<step>` | `label`（必需，非空）、`icon`（单个字符） |
| `RichText` | `<run>` | `text`（必需，非空）、`style`（`normal`/`strong`/`muted`/`link`）、`link`（`u32`，仅链接 run） |
| `Sparkline` | `<point>` | `value`（必需，有限数字） |

`<row kind="...">` 选择其编辑器及允许的参数：

| `kind` | 参数 |
|---|---|
| `read-only`（默认）、`text` | `key`、`value`、`kind`、`read-only` |
| `number` | 外加 `min`、`max`、`step`（`> 0`）、`decimals`（整数 `0..=10`） |
| `choice` | 外加 `options`（以竖线分隔，每个选项非空） |
| `check` | 外加 `on`、`off`（二者均非空） |
| `rating` | 外加 `stars`（整数 `1..=10`） |

## 已注册的创建类型

`WidgetRegistry::standard()` 注册这些。交互类型
需要 `command`。

| `type` | 需要 `command` | 构造函数 |
|---|---|---|
| `label` | 否 | `Label` |
| `button` | 是 | `Button` |
| `toggle` | 是 | `Toggle`（每次变化都报告；`M: Clone`） |
| `slider` | 是 | `Slider`（每次变化都报告；`M: Clone`） |
| `progress` | 否 | `ProgressBar` |
| `icon` | 否 | `Icon` |
| `badge` | 否 | `Badge` |
| `card` | 否 | `Card` |
| `divider` | 否 | `Divider` |
| `kbd` | 否 | `Kbd` |
| `avatar` | 否 | `Avatar` |
| `spinner` | 否 | `Spinner` |

创建的控件是页面层的，属于文档，并且必须在每个变体中以相同的
`type`/`command`/`parent` 声明。

## 限制

| 限制 | 值 |
|---|---|
| `.buixml` 文件大小 | 4 MiB |
| 每份文档的条目数 | 50 000 |
| 每个控件的子元素数 | 10 000 |
| 每次解析的诊断数 | 100 |
| 表达式嵌套 | 32 |
| `.buitheme` 文件大小 | 1 MiB |
| 每个主题的 `<color>` 覆盖数 | 512 |

不支持 DTD、CDATA、命名空间、include、文件监视器或事件绑定。样式和
内容值是字面量，绝不是表达式。子属性是字面量
字符串，几何和常量除外。

## 诊断

错误拒绝整个文档；警告会被报告并应用其余部分。
代码是稳定的。

| 代码 | 严重程度 | 原因 |
|---|---|---|
| `xml.syntax` | 错误 | XML 格式错误。 |
| `xml.version` | 错误 | `version` 缺失或不受支持。 |
| `xml.structure` | 错误 | 嵌套错误、杂散文本、DTD、大小/条目限制。 |
| `xml.unknown-element` | 错误 | schema 之外的元素。 |
| `xml.unknown-attribute` | 错误 | schema 之外的属性（非控件元素）。 |
| `xml.missing-attribute` | 错误 | 缺少必需属性。 |
| `xml.attr.value` | 错误 | 格式错误的数字/bool/token/颜色/名称。 |
| `xml.attr.conflict` | 错误 | 互斥属性（`radius`+`corners`、内容对等）。 |
| `xml.duplicate-layout-id` | 错误 | 两个变体共享 `id`。 |
| `xml.duplicate-widget-id` | 错误 | 一个变体中的两个条目共享 `id`。 |
| `xml.layout.base` | 错误 | 没有 `(0,0)` 变体，或最小值重复。 |
| `xml.const.invalid` | 错误 | 常量名称超出 `[a-z][a-z0-9_]*`。 |
| `xml.const.reserved` | 错误 | 常量遮蔽了一个空间变量。 |
| `xml.const.duplicate` | 错误 | 常量被声明两次。 |
| `xml.const.undefined` | 错误 | 未知常量名称。 |
| `xml.const.vars` | 错误 | 常量使用了空间变量或 `%`。 |
| `xml.expr.syntax` | 错误 | 表达式格式错误。 |
| `xml.expr.div-zero` | 错误 | 除以零。 |
| `xml.expr.non-finite` | 错误 | 非有限结果或字面量。 |
| `xml.expr.depth` | 错误 | 表达式深度超过 32 层。 |
| `xml.geom.underdetermined` | 错误 | 几何为某轴提供的值少于两个。 |
| `xml.geom.overdetermined` | 错误 | 几何为某轴提供了全部三个值。 |
| `xml.geom.negative` | 错误 | 负的 `w`/`h`/拉伸边、负的 `right`/`bottom`。 |
| `xml.geom.invalid` | 错误 | 矩形在当前尺寸下无效。 |
| `xml.geom.non-finite` | 错误 | 某矩形边缘不是有限值。 |
| `xml.id.unknown-widget` | 错误 | 被寻址的控件 id 不存在。 |
| `xml.id.unknown-scroll` | 错误 | `<scroll>` id 不是已注册的面板。 |
| `xml.id.ambiguous` | 错误 | 一个 id 解析到多个目标。 |
| `xml.id.duplicate` | 错误 | 某创建条目的 id 存在但不属于文档。 |
| `xml.parent.missing` | 错误 | `parent` 不是滚动面板，或该 id 不是它的子项。 |
| `xml.content.conflict` | 错误 | 变体对一个 id 的内容不一致。 |
| `xml.content.unsupported` | 警告 | 控件不发布该内容能力。 |
| `xml.content.session-open` | 警告 | 目标正在被编辑；内容写入被跳过。 |
| `xml.skin.conflict` | 错误 | 变体对一个 id 的局部皮肤数据不一致。 |
| `xml.skin.reserved` | 错误 | 控件声明复用了共享名称。 |
| `xml.skin.unknown` | 错误 | 控件未声明的局部属性/子元素。 |
| `xml.skin.value` | 错误 | 声明的值未通过其类型或控件的验证。 |
| `xml.style.unsupported` | 警告 | 在没有表面的控件上使用表面 token。 |
| `xml.value.clamped` | 警告 | 值被钳制到控件的范围。 |
| `xml.value.zero-size` | 警告 | `w` 或 `h` 求值为零。 |
| `xml.rust.unsupported` | 错误 | 动态模式中的内联 Rust。 |
| `xml.rust.context` | 错误 | 通过静态描述符应用的上下文矩形。 |
| `xml.rust.duplicate` | 错误 | 两个 `<rust>` 绑定指向同一个控件。 |
| `xml.rust.target` | 错误 | `<rust>` 指向该变体未声明的 id。 |
| `xml.rust.conflict` | 错误 | `<rust>` 与静态子列表不一致。 |
| `xml.type.unknown` | 错误 | `type` 没有注册表，或注册表中没有该类型。 |
| `xml.type.create` | 错误 | 工厂拒绝该条目或返回了另一个 id。 |
| `xml.type.conflict` | 错误 | 变体对一个创建的 id 的 `type`/`command`/`parent` 不一致。 |
| `xml.command.missing` | 错误 | 交互类型没有可解析的命令。 |

## 另请参阅

- [XML 布局编写](../guides/xml-layout-authoring.md)
- [调色板 token](palette-tokens.md)
- [运行时 XML 快速上手](../quickstart/05-xml-dynamic.md)
- [编译期 XML 快速上手](../quickstart/06-xml-compiled.md)
- [Ui 容器](../api/ui-container.md)
- [调试](../guides/debugging.md)
