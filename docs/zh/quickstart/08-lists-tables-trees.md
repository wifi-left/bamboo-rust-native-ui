# 08 — 列表、表格与树

> 用三个虚拟化数据视图填充页面：一个 640 行的列表、一个可排序且
> 可编辑的表格，以及一棵可展开的树 —— 既用 Rust 也用 `.buixml` 子列表。

## 你将构建

一个曲目浏览器：[`ListView`](../widgets/data/list-view.md) 是带
副标题和尾部文本的平面列表；[`Table`](../widgets/data/table.md) 添加排序、
原地编辑和选择；[`TreeView`](../widgets/data/tree-view.md)
展开、重命名并接受拖拽移动。三者都只绘制其
视口中的行，因此数千行每帧只耗费一个视口。

## 前置条件

- [03](03-first-window.md) 中的窗口可以构建并运行。
- 第 4 步像 [05](05-xml-dynamic.md) 那样启用 `layout-xml`。
- 三个控件本身无需特性开关。

## 步骤

### 1. 一个虚拟化列表

`ListView` 拥有其行、滚动偏移、滚轮处理、悬停/按下状态和
集成的滚动条。先创建空的，然后用 `Ui::set_rows` 把行交给它。

```rust
use bamboo_native_ui::widgets::ListView;
use bamboo_native_ui::{Row, Ui, WidgetId, WindowHandle};

enum Msg {
    PlayRow(usize),
}

fn build_list(ui: &mut Ui<Msg>) {
    ui.add(
        ListView::new("tracks")
            .row_height(44.0)
            .on_activate(Msg::PlayRow),
    );
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
    ui.edit_widget::<ListView<Msg>>(WidgetId::new("tracks"), |list| list.scroll_to_row(320));
    ui.flush_damage(window);
}
```

`Row::new(title)` 接受任何可转换为 `String` 的内容；`.subtitle`、
`.trailing`、`.current(true)` 和 `.disabled(true)` 是可选的。`row_height`
有 24 DIP 的下限，`on_activate` 在行被按下并释放时触发，
列表有焦点时按 Enter 也触发。`scroll_to_row` 将一行放到顶部边缘；
`ensure_row_visible` 移动最小距离。`edit_widget` 记录待处理
损坏，`flush_damage` 将其变成一次失效。

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
                TableColumn::new("#", 56.0).align(TextAlign::Right),
                TableColumn::flex("Title"),
                TableColumn::new("Length", 76.0).align(TextAlign::Right),
            ])
            .rows(rows)
            .sort(1, true)
            .on_select(Msg::Select)
            .on_activate(Msg::Open)
            .on_header_click(Msg::Sort)
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

### 3. 一棵可展开的树

`TreeView` 在结构变化时将可见节点展平为缓存的行列表 ——
绝不在绘制期间。行携带路径而不是标签，因此消息
廉价且在树形状不变时保持有效。

```rust
use bamboo_native_ui::widgets::{TreeItem, TreeView};
use bamboo_native_ui::Ui;

enum Msg {
    Toggle(Vec<usize>, bool),
    Rename(Vec<usize>, String),
    Move(Vec<usize>, Vec<usize>),
}

fn build_tree(ui: &mut Ui<Msg>) {
    ui.add(
        TreeView::new("tree")
            .items(library())
            .row_height(26.0)
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

拖拽默认开启。用来自
`bamboo_native_ui::widgets::tree` 的
`TreeDragRules::new().reorder(false).nest(true)` 限制允许的放置，并启用 `drag_rules(rules)`。树
自行应用有效的移动并通过 `on_move` 报告它，因此应用
只需在自己的模型中镜像新的层级。

### 4. 在 `.buixml` 中描述数据

文档为声明了子列表的控件提供其子列表。三个
视图接受 `<item>`、`<column>`/`<row>`/`<cell>` 和嵌套的 `<node>`。
控件必须先在 Rust 中存在：三者都不是注册表类型。

```xml
<?xml version="1.0" encoding="utf-8"?>
<layouts version="1">
  <layout id="base" min-width="0" min-height="0">
    <widget id="tracks" x="12" y="12" w="cw-24" h="200"
            row-height="40" alternating="true" empty-text="No tracks">
      <item title="Nocturne" subtitle="Chopin" trailing="3:58" current="true"/>
    </widget>

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

完全像 [05](05-xml-dynamic.md) 那样应用它。子列表是**数据，而不是
绑定**：它们在文档生成变化时替换控件的模型，
从不在尺寸改变时。如果编辑会话打开时生成发生变化，会跳过该
控件（`xml.content.session-open`），因此重新加载无法覆盖正在输入的
单元格。

## 预期结果

- 列表滚动浏览 640 行；只有视口中的行被绘制。
- 标题点击排序并移动指示器；双击编辑单元格。
- 树展开、在 `F2` 时重命名，并接受规则内的拖拽移动。
- 加载文档会一次性替换每个视图的行；调整尺寸不会。

## 故障排查

- **列表为空** —— 它被创建但从未被赋予行。调用
  `ui.set_rows(WidgetId::new("tracks"), rows)`；没有 rows 构建器。
- **标题点击不排序** —— 控件只报告列。对数据排序
  并调用 `Table::set_sort`；单独的 `.sort` 只绘制指示器。
- **原地编辑不启动** —— `editable(true)` 关闭，或单元格
  只读。用 Enter、`F2` 或双击启动。
- **重新加载后树的路径指向错误的节点** —— 路径是索引
  链。顺序变化时重建应用的路径映射。

## 下一步

[09 — 菜单、工具栏与状态栏](09-menus-toolbars-status.md)：将这些视图
包裹在应用界面框架中并路由命令 id。
