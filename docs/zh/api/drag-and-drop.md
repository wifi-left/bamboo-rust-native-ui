# 拖放

> 系统级拖放：文本和文件列表负载、放置所报告的效果，以及容器拖拽方法。

## 概述

拖拽是**系统拖拽**：它会离开窗口和进程（OLE `DoDragDrop`），因此负载仅限于 Shell
能够传输的内容——Unicode 文本和文件列表。需要更丰富负载的应用应将其编码进文本（JSON、
URL、id）中，而不是使用进程本地的值。

保留模式的控件树对 COM 一无所知。宿主将 `IDropTarget` 回调转换为 `Host` 方法
（`drag_enter` / `drag_over` / `drag_leave` / `drag_drop`），并附带客户区 DIP 坐标，
和指针事件完全一样。容器将它们路由到控件注册的源、目标和悬停回调。

`DragData` 和 `DropEffect` 是跨平台模型；OLE 传输、`WindowHandle::start_drag` 以及宿主的
系统拖拽回调仅限 Windows。没有 OLE 时，容器会保留一个窗口内回退拖拽会话。

## 关键类型

### `DragData`

```rust
pub enum DragData {
    Text(String),
    Files(Vec<std::path::PathBuf>),
}
```

`Text` 是 `CF_UNICODETEXT`（一个字段选区、一个路径、一个 URL 或一个编码对象）；`Files` 是
`CF_HDROP`，正如资源管理器所拖放的那样。构造函数：`text`、`files`。读取方法：
`as_text() -> Option<&str>`、`as_files() -> Option<&[PathBuf]>`、`is_empty()`。当对象同时
携带两者时（资源管理器的拖拽通常如此），系统拖拽优先选择文件列表。

### `DropEffect`

```rust
pub enum DropEffect { None, Copy, Move, Link }
```

`None` 是默认值（没有目标接受）。源在拖拽循环结束后读取该效果：`Move` 告知它移除原始行/
文件。除 `None` 外，`accepted()` 对所有变体都为 `true`。

### `Ui` 拖拽方法

| 方法 | 签名 | 描述 |
|---|---|---|
| `set_drag_source` | `set_drag_source(&mut self, id, source: impl Fn() -> Option<DragData> + 'static) -> &mut Self` | 手势离开此控件时生成负载。 |
| `clear_drag_source` | `clear_drag_source(&mut self, id)` | 移除一个源。 |
| `set_drop_target` | `set_drop_target(&mut self, id, drop: impl Fn(DragData, Point) -> Option<M> + 'static) -> &mut Self` | 接受任何负载。 |
| `set_text_drop_target` | `set_text_drop_target(&mut self, id, drop: impl Fn(String, Point) -> Option<M> + 'static) -> &mut Self` | 仅接受文本。 |
| `set_files_drop_target` | `set_files_drop_target(&mut self, id, drop: impl Fn(Vec<PathBuf>, Point) -> Option<M> + 'static) -> &mut Self` | 仅接受文件列表。 |
| `set_drop_hover` | `set_drop_hover(&mut self, id, hover: impl Fn(&DragData, Point, bool) -> bool + 'static) -> &mut Self` | 进入/离开反馈；当视觉发生变化时返回 `true`。 |
| `set_text_drag_source` | `set_text_drag_source(&mut self, id) -> &mut Self` | 拖拽字段的选中文本（D122）。 |
| `drag_data` / `drag_text` | `drag_data() -> Option<&DragData>`, `drag_text() -> Option<&str>` | 窥视传输中的负载。 |
| `drag_source` / `drag_target` | `drag_source() -> Option<WidgetId>`, `drag_target() -> Option<WidgetId>` | 当前拖拽的端点。 |
| `cancel_drag` | `cancel_drag(&mut self, sink)` | 中止在此窗口中开始的拖拽。 |

### 宿主回调

| 方法 | 签名 |
|---|---|
| `drag_enter` | `fn drag_enter(&mut self, data: &DragData, pos: Point, window) -> DropEffect` |
| `drag_over` | `fn drag_over(&mut self, data: &DragData, pos: Point, window) -> DropEffect` |
| `drag_leave` | `fn drag_leave(&mut self, window)` |
| `drag_drop` | `fn drag_drop(&mut self, data: DragData, pos: Point, window) -> DropEffect` |
| `drag_finished` | `fn drag_finished(&mut self, effect: DropEffect, window)` |
| `files_dropped` | `fn files_dropped(&mut self, paths: Vec<String>, window)`（`WM_DROPFILES`） |

## 示例

一个在两个控件之间拖拽的文本负载，带有一个文本放置目标：

```rust
use bamboo_native_ui::{DragData, Point, Ui, WidgetId};

fn wire_drag(ui: &mut Ui<Msg>, title: &str) -> &mut Ui<Msg> {
    let source = WidgetId::new("now-playing-title");
    let target = WidgetId::new("playlist");

    let payload = title.to_string();
    ui.set_drag_source(source, move || Some(DragData::text(payload.clone())));
    ui.set_text_drop_target(target, |text, _pos: Point| {
        Some(Msg::AppendToPlaylist(text))
    });
    ui.set_text_drop_hover(target, |_text, _pos, _over| true);
    ui
}
```

转发宿主的系统拖拽回调，才能使同一手势从另一个窗口或应用中也有效：

```rust
use bamboo_native_ui::{DragData, DropEffect, Point, WindowHandle};

fn drag_enter(&mut self, data: &DragData, pos: Point, window: &mut WindowHandle<'_>) -> DropEffect {
    self.ui.drag_enter(data, pos, window)
}

fn drag_drop(&mut self, data: DragData, pos: Point, window: &mut WindowHandle<'_>) -> DropEffect {
    self.ui.drag_drop(data, pos, window)
}
```

从应用代码开始拖拽（例如一个工具栏按钮）：

```rust
use bamboo_native_ui::{DragData, WindowHandle};

fn export(window: &mut WindowHandle<'_>, path: std::path::PathBuf) -> bool {
    window.start_drag(DragData::files([path]))
}
```

## 说明与限制

- 系统拖拽仅限 Windows（OLE）。`DragData` 和 `DropEffect` 在所有平台都能编译，OLE 不可
  用时（测试、自动化运行），容器会保留一个窗口内回退拖拽会话。
- 负载只能是文本或文件列表；它必须能跨越到另一个进程后仍然存活。请将更丰富的数据编码进
  文本。
- 用 `set_text_drag_source` 注册的绘制文本字段，仅当按下发生在未折叠选区内部时才启动拖拽；
  密码字段和触摸选择手柄永远不会启动它。
- 用作文本放置目标的绘制字段会先在释放点插入文本（一次普通的用户编辑，因此其 `on_change`
  也会报告），除非它为只读。
- `drag_over` 会被持续调用；只对悬停状态确实发生变化的控件重新造成损伤。
- `set_system_drag_enabled(false)` 会关闭 OLE 拖拽（用于合成指针没有真实按钮释放的自动化
  运行）；`system_drag_enabled()` 报告该状态。当 OLE 关闭时，`WindowHandle::start_drag` 返回
  `false`。
- `drag_finished` 会清除源控件的拖拽状态；OLE 已消耗了指针释放，因此不会跟随 `PointerUp`。

## 另见

- `scrolling.md`（滚动列表内的放置目标）
- `hotkeys.md`（拖拽手势的键盘等效项）
- `../widgets/data/list-view.md`
- `../widgets/content/copyable-text.md`
