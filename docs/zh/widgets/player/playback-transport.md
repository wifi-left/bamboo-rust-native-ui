# PlaybackTransport

> 五单元格播放器传输条带：随机播放、上一曲、播放/暂停、下一曲和循环播放。

## 介绍

`PlaybackTransport<M>` 在一个矩形范围内绘制五个图标按钮：随机播放、上一曲、播放/暂停、下一曲和循环播放。点击某个单元格（或在键盘光标所在单元格上按 Enter/Space）会通过构造函数处理函数上报对应的 `TransportAction`。播放/暂停是主要动作，绘制为一个实心圆；当 `playing` 为 true 时，它显示暂停字形。

该控件从不决定应该发生什么。`playing`、`shuffle` 和 `repeat` 是应用状态，通过 `set_playing`、`set_shuffle` 和 `set_repeat` 推回，每个都返回是否有任何变化，因此一次失败的播放命令会让该条带如实显示真实状态。

键盘模型是光标而非选择：左/右/Home/End 移动光标，Enter/Space 运行光标所在单元格，上/下则离开前往相邻控件。辅助技术获得一个包含五个子项的 `ToolBar`：三个 Button 和两个 Toggle（随机播放和循环播放）。

## 用法（Rust）

```rust
use bamboo_native_ui::widgets::{PlaybackTransport, RepeatMode, TransportAction};
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg {
    Transport(TransportAction),
}

let mut ui: Ui<Msg> = Ui::new();
ui.add(
    PlaybackTransport::new("transport", Msg::Transport)
        .playing(false)
        .shuffle(false)
        .repeat(RepeatMode::Off),
);
ui.set_rect(WidgetId::new("transport"), Rect::from_xywh(120.0, 232.0, 300.0, 44.0));
```

`TransportAction::ALL` 是该条带的顺序。`RepeatMode::next` 按 Off → All → One → Off 递进，`RepeatMode::active` 对两种循环类型都为 true。一个典型的处理函数会推进模式并绘制新状态：

```rust
ui.edit_widget::<PlaybackTransport<Msg>>(WidgetId::new("transport"), |strip| {
    if strip.repeat_mode() == RepeatMode::Off {
        strip.set_repeat(RepeatMode::All)
    } else {
        false
    }
});
```

## 用法（XML）

此控件不由标准注册表创建。请在 Rust 中创建它，并通过文档中的 `id` 引用它。它接受三个控件局部属性，没有子元素：

```xml
<widget id="transport" x="120" y="232" w="300" h="44"
        playing="false" shuffle="true" repeat="all"/>
```

`repeat` 以不区分大小写的方式解析：接受 `off`/`none`、`all`/`list` 和 `one`/`single`；任何其他值都会被拒绝为 `xml.skin.value`。这些属性静默地写入状态，与 setter 完全一致。

## 属性

| 方法 | 类型 | 默认值 | 描述 |
|---|---|---|---|
| `new(id, on_action)` | `&'static str, impl Fn(TransportAction) -> M` | — | 创建该条带；`300 × 44` DIP，已启用。 |
| `.playing(bool)` | `bool` | `false` | 初始播放状态（为 true 时显示暂停字形）。 |
| `.shuffle(bool)` | `bool` | `false` | 初始随机播放高亮。 |
| `.repeat(RepeatMode)` | `RepeatMode` | `Off` | 初始循环状态。 |
| `.set_playing(bool)` | `bool` | — | 应用更新；返回其是否发生变化。 |
| `.set_shuffle(bool)` | `bool` | — | 应用更新；返回其是否发生变化。 |
| `.set_repeat(RepeatMode)` | `bool` | — | 应用更新；返回其是否发生变化。 |
| `.is_playing()` / `.shuffle_enabled()` / `.repeat_mode()` | `bool` / `bool` / `RepeatMode` | — | 读取当前状态。 |
| `.cursor()` / `.set_cursor(usize)` | `usize` / `bool` | `2` | 键盘光标所在单元格；setter 会钳制且从不发出消息。 |

`TransportAction` 变体：`Shuffle`、`Previous`、`PlayPause`、`Next`、`Repeat`。`RepeatMode` 变体：`Off`（默认）、`All`、`One`。它们的 `label()` 方法返回控件使用的无障碍名称（源字符串为中文：`随机播放`、`上一曲`、`播放或暂停`、`下一曲`、`循环播放`，以及循环的 `关` / `列表` / `单曲`）。

### 行为

- **默认矩形。** `300 × 44` DIP；`preferred_height` 为 44，光标从中间单元格（播放/暂停）开始。
- **指针。** 按下会把光标移动到按下的单元格；在同一单元格上释放会运行它。在一个单元格上按下并在另一个单元格上释放属于拖动，不发出任何消息。该条带会吞掉释放之后的点击，因此动作只会运行一次。
- **键盘。** 左/右/Home/End 移动光标并进行钳制（不循环）；Enter/Space、`Activate` 和无障碍子动作会运行光标所在单元格。`owns_navigation_key` 仅保留左/右/Home/End/Enter/Space，因此上/下和 Tab 会将焦点移走。
- **禁用。** 禁用的条带忽略所有输入；悬停和按下状态会被清除。

## 事件

### 消息

| 消息 | 发出条件 |
|---|---|
| 作为 `on_action` 传给 `new` 的 `impl Fn(TransportAction) -> M` | 运行某个单元格：在按下的单元格上释放点击、在光标所在单元格上按 `Enter`/`Space`、`Activate`，或一次 UIA 子调用。 |

`TransportAction` 是 `Shuffle`、`Previous`、`PlayPause`、`Next` 或 `Repeat` 之一。移动光标（方向键、`Step`、按下）不发出任何消息；在一个单元格上按下并在另一个单元格上释放也不发出任何消息。

### 通用观察器

此控件不安装任何共享的观察器构建器。请使用构造函数的 `on_action` 处理函数或 `Ui::bind`。

### 控件专属回调

| 回调 | 签名 | 触发条件 |
|---|---|---|
| `new(id, on_action)` | `&'static str, impl Fn(TransportAction) -> M` | 通过指针、键盘、无障碍或 UIA 运行某个单元格。 |

### Ui 触发器

除上述构建器外，任何控件 id 都可以通过 `Ui::bind` 绑定：

```rust
ui.bind(WidgetId::new("transport"), Trigger::Click, |event, ctx| {
    // 返回 Some(Msg::...) 以排队一条消息；调用 ctx.default() 以保留
    // 内置动作，随后会在处理函数之后运行。
    None
});
```

| 触发器 | 触发条件 |
|---|---|
| `Click` | 在同一控件上按下并释放。 |
| `DoubleClick` | 平台报告一次双击。 |
| `RightClick` | 次键点击，在上下文菜单之前。 |
| `HoverEnter` / `HoverLeave` | 指针进入 / 离开该控件。 |
| `Wheel` | 控件上方的滚轮事件（`event.wheel_delta`）。 |
| `Activate` | 在获得焦点的控件上按 Enter/Space，或一次 UIA Invoke/Toggle。 |
| `DragStart` / `DragEnd` | 指针越过拖动阈值 / 按钮被释放。 |
| `ValueChange` | 控件的无障碍值发生变化（在内置更新之后）。 |

该条带响应 `Click`、`HoverEnter`/`HoverLeave`、`Activate` 和 `ValueChange`（随机播放/循环播放开关）；它没有滚轮、右键或双击行为。

## XML 定义

此控件不由标准注册表创建。请在 Rust 中创建它，并通过文档中的 `id` 引用它。

几何遵循常规的 `<widget>` 规则。通用属性（`visible`、`tooltip`、`name`、`description`、`automation-id`、`role`、`expose`、enabled、焦点策略）适用。该条带从调色板绘制，没有 `SurfaceStyle`，因此 `radius`、`corners` 和 `background` 会被报告为 `xml.style.unsupported`。它不声明任何共享内容（`text`、`label`、`value`、`progress`、`selected` 会被拒绝），因此 `playing` / `shuffle` / `repeat` 是它唯一的局部词汇。

| 属性 | 类型 | 描述 |
|---|---|---|
| `playing` | bool | 绘制暂停字形和强调色播放圆。 |
| `shuffle` | bool | 随机播放单元格上的强调色高亮和开关状态。 |
| `repeat` | text | `off`、`all` 或 `one`（以及上述别名）。 |

子元素：无。

## 另见

- `./waveform-seek-bar.md`
- `./playlist-view.md`
- `../chrome/toolbar.md`
- `../index.md`
- `../../api/widget-trait.md`
