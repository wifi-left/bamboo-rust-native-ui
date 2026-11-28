# 08 — 列表、表格与树

> 用三个虚拟化数据视图填充页面：一个 640 行的列表、一个可排序且
> 可编辑的表格，以及一棵可展开的树 —— 既用 Rust 也用 `.buixml` 子列表。

## 你将构建

一个曲目浏览器：[`ListView`](../widgets/data/list-view.md) 是带
副标题和尾部文本的平面列表；[`Table`](../widgets/data/table.md) 添加排序、
原地编辑和选中；[`TreeView`](../widgets/data/tree-view.md)
展开、重命名并接受拖拽移动。三者都只绘制其
视口中的行，因此数千行每帧只耗费一个视口。

**本节新术语**

- **虚拟化** —— 控件持有每一行，但只绘制、命中测试和描述
  可见矩形内的行。每帧开销跟随视口，而不是行数。这就是
  640 行或 1,000 行感觉和十行一样的原因。
- **视口** —— 可滚动内容上的可见窗口。滚动改变
  哪些行位于其中；行本身不会被重建。
- **行** —— 列表数据的一行。`Row::new(title)` 可以带一个副标题和
  尾部文本。
- **列** —— 仅表格几何：固定宽度，或弹性以共享剩余
  宽度。列不知道如何比较它的值。
- **单元格** —— 行/列交点上的一个值；编辑改变的就是单元格。
- **选中** —— 哪一行是键盘游标（`selected_row`）。
- **激活** —— “执行操作”的信号：`on_activate` 在行被点击或
  用 Enter 激活时触发。选中和激活是相互独立的概念，
  即使一个手势同时产生两者。
- **模型** —— 控件拥有的数据（它的行或树项）。用 `Ui::set_rows`、
  `.items(...)` 或 XML 子列表替换它。
- **数据子元素** —— 像 `<item>`、`<column>`、`<row>`、
  `<cell>` 和 `<node>` 这样的 XML 元素，从文档为控件提供模型。

## 前置条件

- [03](03-first-window.md) 中的窗口可以构建并运行。
- 第 4 步像 [05](05-xml-dynamic.md) 那样启用 `layout-xml`。
- 三个控件本身无需特性开关。

这里的一切都放进 crate 文件夹 `bamboo-hello/bamboo-hello/`：
Rust 片段扩展 `src/main.rs`，第 4 步的 XML 是 `src` 旁边
`layouts/` 下的新文件（例如 `layouts/views.buixml`）。从该 crate 文件夹
或上一级的工作区文件夹运行 `cargo run`。

## 步骤

### 1. 一个虚拟化列表

`ListView` 拥有其行、滚动偏移、滚轮处理、悬停/按下状态和
集成的滚动条。先创建空的，然后用 `Ui::set_rows` 把行交给它。

本页三个构建器都是你在 `App::new` 中调用一次的函数。
没有调用，视图就不存在；没有矩形（来自 `resize`
或第 4 步的文档），它会保留构造函数的矩形，可能重叠页面
或坐在你不想放的位置。

```rust
use bamboo_native_ui::widgets::ListView;
use bamboo_native_ui::{Row, Ui, WidgetId, WindowHandle};

enum Msg {
    PlayRow(usize),
}

fn build_list(ui: &mut Ui<Msg>) {
    ui.add(
        ListView::new("tracks")
            // 固定行高：第一个可见行是 scroll / height。
            .row_height(44.0)
            // 对被点击的行和键盘游标上的 Enter 调用。
            .on_activate(Msg::PlayRow),
    );
    // 模型持有全部 640 行；控件只绘制视口。
    let rows = (0..640)
        .map(|index| {
            Row::new(format!("Track {:03}", index + 1))
                .subtitle(format!("Artist {} · Album {}", index % 17, index % 9))
                .trailing(format!("{}:{:02}", 2 + index % 3, (index * 7) % 60))
        })
        .collect();
    ui.set_rows(WidgetId::new("tracks"), rows);
}

fn scroll_to_track(ui: &mut Ui<Msg>, window: &mut WindowHandle<'_>) {
    // edit_widget 按 id 访问具体的 ListView。
    ui.edit_widget::<ListView<Msg>>(WidgetId::new("tracks"), |list| list.scroll_to_row(320));
    // 滚动改变了损坏；把它变成一次失效。
    ui.flush_damage(window);
}
```

`Row::new(title)` 接受任何可转换为 `String` 的内容；`.subtitle`、
`.trailing`、`.current(true)` 和 `.disabled(true)` 是可选的。`row_height`
有 24 DIP 的下限，`on_activate` 在行被按下并释放时触发，
列表有焦点时按 Enter 也触发。`scroll_to_row` 将一行放到顶部边缘；
`ensure_row_visible` 移动最小距离。`edit_widget` 记录待处理
损坏，`flush_damage` 将其变成一次失效。

`selected_row` 是键盘游标，由 Up/Down、PageUp/PageDown、
Home/End 移动。`on_activate` 是动作信号，携带被激活的行
索引；应用决定“播放第 320 行”意味着什么。点击会激活
指针下的行，无需先有选中。

**刚才发生了什么。** 模型有 640 行，但控件只触碰
与它视口相交的那几行（外加至多一个部分可见的行）。
滚动改变一个偏移量，并绘制移入视口的行；
其余保持为数据。`scroll_to_row(320)` 直接跳到 640 行深处，
`flush_damage` 恰好调度一次重绘。

虚拟化是关键：绘制、命中测试和无障碍树都使用
`visible_rows()`，因此工作量与屏幕上的行数成正比。列表周围的
[`ScrollView`](../api/scrolling.md) 把滚轮刻度留给它，
因为 `scrolls_with_wheel()` 为 `true`。

### 2. 一个可排序、可编辑的表格

`Table` 是一个固定标题带，加上按数据绘制的行。列只是
几何：一个固定宽度，或一个共享剩余宽度的弹性列。
排序由应用拥有；控件从不比较它无法知道的值。

```rust
use bamboo_native_ui::widgets::{Table, TableColumn, TableRow};
use bamboo_native_ui::{TextAlign, Ui, WidgetId};

enum Msg {
    Select(usize),
    Open(usize),
    Sort(usize),
    Edit(usize, usize, String),
}

fn build_table(ui: &mut Ui<Msg>) {
    // 1,000 行：表格像列表一样对它们虚拟化。
    let rows = (0..1_000)
        .map(|index| {
            TableRow::new([
                format!("{:04}", index + 1),
                format!("Track {:03}", index + 1),
                format!("{}:{:02}", 2 + index % 3, (index * 11) % 60),
            ])
        })
        .collect::<Vec<_>>();
    ui.add(
        Table::new("library")
            .columns(vec![
                // 固定宽度，数字右对齐。
                TableColumn::new("#", 56.0).align(TextAlign::Right),
                // 弹性：占用固定列留下的宽度。
                TableColumn::flex("Title"),
                TableColumn::new("Length", 76.0).align(TextAlign::Right),
            ])
            .rows(rows)
            // 只设置初始箭头；控件不对任何内容排序。
            .sort(1, true)
            .on_select(Msg::Select)
            .on_activate(Msg::Open)
            .on_header_click(Msg::Sort)
            // Enter、F2 或双击打开原地编辑器。
            .editable(true)
            .on_cell_edit(Msg::Edit),
    );
}
```

`.sort(1, true)` 只绘制初始指示器。标题点击发出
列索引；应用负责排序，然后调用 `set_sort` 使指示器
跟随。`editable(true)` 在 Enter、`F2` 或双击时启动原地
会话；控件在 `on_cell_edit` 运行之前就已应用提交的文本，因此
处理器只存储它。`on_select` 在点击的行被选中后触发；
`on_activate` 在 Enter 或双击时触发。如果持久化布局，
`on_column_resize` 和 `on_column_reorder` 存在。

列承载的是几何，不是含义：`TableColumn::flex("Title")` 没有
比较器，因此控件无法排序。标题点击把索引交给你，你的
代码对自己的 `Vec<TableRow>` 排序，调用 `set_rows`，并用
`set_sort(Some((column, ascending)))` 告诉控件绘制哪个
箭头。同样的分工也适用于编辑：提交的字符串存储在控件中并
被报告；在你的模型中持久化它是你的工作。

**刚才发生了什么。** 表格从三个列定义填充它的标题带，
只绘制视野中的行。点击一行会选中它并
报告索引；双击（或 Enter）激活它；编辑会话
在文本已本地应用之后报告新文本。

### 3. 一棵可展开的树

`TreeView` 在结构变化时将可见节点展平为缓存的行列表 ——
绝不在绘制期间。行携带路径而不是标签，因此消息
廉价且在树形状不变时保持有效。

```rust
use bamboo_native_ui::widgets::{TreeItem, TreeView};
use bamboo_native_ui::Ui;

enum Msg {
    // 路径是从根开始的索引链，而不是标签：廉价且在
    // 树形状不变时保持稳定。
    Toggle(Vec<usize>, bool),
    Rename(Vec<usize>, String),
    Move(Vec<usize>, Vec<usize>),
}

fn build_tree(ui: &mut Ui<Msg>) {
    ui.add(
        TreeView::new("tree")
            .items(library())
            .row_height(26.0)
            // 每个嵌套层级的间距，以 DIP 为单位。
            .indent(16.0)
            .renameable(true)
            .draggable(true)
            .on_toggle(|path, expanded| Msg::Toggle(path.to_vec(), expanded))
            .on_rename(|path, label| Msg::Rename(path.to_vec(), label.to_string()))
            .on_move(|from, to| Msg::Move(from.to_vec(), to.to_vec())),
    );
}

fn library() -> Vec<TreeItem> {
    vec![
        TreeItem::new("Library")
            .glyph('♪')
            .secondary("3 albums")
            // 仅初始状态；之后标志由控件拥有。
            .expanded(true)
            .child(
                TreeItem::new("Night")
                    .expanded(true)
                    .child(TreeItem::new("Nocturne").glyph('♫'))
                    .child(TreeItem::new("Sunny Day").glyph('♫')),
            )
            .child(TreeItem::new("Favorites").secondary("12").disabled(true)),
    ]
}
```

`TreeItem::new(label)` 加 `.child(...)`（或 `.children(vec)`）构建
层级；`.expanded(true)` 是初始状态，`.glyph` 添加图标，
`.secondary` 添加一个柔和的右对齐注释。点击山形符、Right/Left 或 Space
展开和折叠；控件更新自己的标志并报告 `on_toggle`。
`renameable(true)` 允许 `F2` 重命名：Enter/Tab 提交，Escape 还原，空
标签被忽略，`on_rename` 在新标签应用后运行。

像 `[0, 1]` 这样的路径表示“第一个根节点的第二个子节点”。它在结构不变时保持
有效，这就是回调交给你索引而不是字符串的原因。重命名不改变
路径；移动节点则会。

拖拽默认开启。用来自
`bamboo_native_ui::widgets::tree` 的
`TreeDragRules::new().reorder(false).nest(true)` 限制允许的放置，并启用 `drag_rules(rules)`。树
自行应用有效的移动并通过 `on_move` 报告它，因此应用
只需在自己的模型中镜像新的层级。

**刚才发生了什么。** 树把初始层级展平成一次
行列表，绘制可见的行，现在在输入时变更自己的展开状态。
`on_toggle`、`on_rename` 和 `on_move` 告诉应用发生了什么变化，
因此它可以更新自己的模型，而无需驱动控件的重绘。

### 4. 在 `.buixml` 中描述数据

文档为声明了子列表的控件提供其子列表。三个
视图接受 `<item>`、`<column>`/`<row>`/`<cell>` 和嵌套的 `<node>`。
控件必须先在 Rust 中存在：三者都不是注册表类型。

```xml
<?xml version="1.0" encoding="utf-8"?>
<layouts version="1">
  <layout id="base" min-width="0" min-height="0">
    <!-- id 必须已在 Rust 中存在；<item> 提供行。 -->
    <widget id="tracks" x="12" y="12" w="cw-24" h="200"
            row-height="40" alternating="true" empty-text="No tracks">
      <item title="Nocturne" subtitle="Chopin" trailing="3:58" current="true"/>
    </widget>

    <!-- <column> 子元素定义几何；<row>/<cell> 提供数据。 -->
    <widget id="library" x="12" y="224" w="cw-24" h="200"
            row-height="26" header-height="26" striped="true" editable="true">
      <column title="#" width="56" align="right"/>
      <column title="Title" flex="true"/>
      <column title="Length" width="76" align="right"/>
      <row disabled="true">
        <cell text="01"/>
        <cell text="Nocturne"/>
        <cell text="3:58"/>
      </row>
    </widget>

    <!-- 嵌套的 <node> 元素镜像 TreeItem 层级。 -->
    <widget id="tree" x="12" y="436" w="cw-24" h="220"
            row-height="26" indent="16" renameable="true">
      <node label="Library" glyph="♪" secondary="3 albums" expanded="true">
        <node label="Night" expanded="true">
          <node label="Nocturne" glyph="♫"/>
          <node label="Sunny Day" glyph="♫"/>
        </node>
        <node label="Favorites" secondary="12" disabled="true"/>
      </node>
    </widget>
  </layout>
</layouts>
```

将其保存为 `layouts/views.buixml`，与 `main.buixml` 相邻，并用
[05](05-xml-dynamic.md) 中的同一模式加载它
（`LayoutDocument::parse(include_str!("../layouts/views.buixml"))`），再调用
`apply_layout`。子列表是**数据，而不是绑定**：它们在文档生成变化时
替换控件的模型，从不在尺寸改变时。如果编辑会话打开时生成
发生变化，会跳过该控件
（`xml.content.session-open`），因此重新加载无法覆盖正在输入的
单元格。

三个视图都不是标准注册表类型，因此文档无法
创建它们：每个 `id` 都需要一个匹配的 `ListView`/`Table`/`TreeView`。文档仍然提供它们的矩形和
所示的外观属性；它无法做的是创建控件对象本身。

**刚才发生了什么。** 文档应用了一个变体，然后替换了每个
视图的模型：一个 `<item>` 变成一行列表行，三个 `<column>` 加一行
`<cell>` 的 `<row>` 成为表格的标题和数据，嵌套的
`<node>` 树成为树的层级。调整窗口尺寸会保留模型；
只有生成变化（不同的文档）才会重新加载它们。

## 预期结果

- 列表滚动浏览 640 行；只有视口中的行被绘制。
- 标题点击排序并移动指示器；双击编辑单元格。
- 树展开、在 `F2` 时重命名，并接受规则内的拖拽移动。
- 加载文档会一次性替换每个视图的行；调整尺寸不会。

**运行它。** 从工作区文件夹 `bamboo-hello/` 或 crate 文件夹
`bamboo-hello/bamboo-hello/` 运行：

```powershell
cargo run
```

内容区中会堆叠出现三个视图：列表、表格和
树。滚动列表，观察滚动条滑块移动，而窗口只
重绘视口。点击表格标题：在你的处理器排序后箭头会变化。
在树中按 `F2` 并输入一个新名字。

## 故障排查

- **`LayoutDocument` 无法解析** —— 第 4 步需要 `layout-xml` 特性；
  三个控件本身不需要它即可编译。
- **列表为空** —— 它被创建但从未被赋予行。调用
  `ui.set_rows(WidgetId::new("tracks"), rows)`；没有 rows 构建器。id
  必须匹配已添加的控件，而对缺失 id 调用 `set_rows`
  什么都不做，也不报告任何内容。XML `<item>` 列表在其
  `<widget>` id 不存在时也不会显示。
- **标题点击不排序** —— 控件只报告列。对数据排序，
  通过 `edit_widget` 用 `set_rows` 安装新的行顺序，
  并在同一个表格上调用 `set_sort`，使箭头跟随。单独的 `.sort` 只绘制
  指示器。
- **原地编辑不启动** —— `editable(true)` 关闭，或单元格
  只读。用 Enter、`F2` 或双击启动。使用 XML 时，`editable="true"`
  必须放在 `<widget>` 上，而不是 `<row>` 上。
- **编辑在重新加载后不保留** —— 控件存储并报告
  提交的文本；在 `on_cell_edit` 中把它持久化到你的模型。否则，
  下一次文档生成或重建的行列表会恢复旧文本。
- **重新加载后树的路径指向错误的节点** —— 路径是索引
  链。顺序变化时重建应用的路径映射。
- **树拖拽没有效果** —— 放置超出了配置的
  `TreeDragRules`（例如只允许嵌套时做重排），或
  目标节点被禁用。树会校验并且不应用任何内容，因此没有消息
  到达。
- **只有视图的一部分可见，或一个视图重叠另一个** —— 矩形
  来自 `set_rect` 或文档。最后运行的那个胜出；如果
  `apply_layout` 失败，控件保留它们之前拥有的矩形，可能
  是构造函数的默认值。检查第一条
  `layout rejected:` 行。
- **XML 数据不出现** —— 文档被解析但应用被
  拒绝，因此没有安装子列表。读第一条 `layout rejected:`
  行：`xml.syntax` 表示 XML 格式错误，`xml.id.unknown-widget` 表示某个 id 没有
  Rust 控件使用。

## 下一步

[09 — 菜单、工具栏与状态栏](09-menus-toolbars-status.md)：将这些视图
包裹在应用界面框架中并路由命令 id。
