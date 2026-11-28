# WaveformSeekBar

> 进度条，轨道按归一化波形采样绘制。

## 介绍

`WaveformSeekBar<M>` 在其矩形范围内为每个槽位绘制一根竖直条。采样是 `0..=1` 范围内的归一化振幅；已播放部分使用强调色，其余部分为柔和色，带有圆形手柄的播放头标记当前位置。悬停在标记上会预览按下时跳转到的位置。

按下会跳转，拖动期间的每次指针移动都会发出一个新位置（因此播放器会随之搓动），释放则提交该位置并停止拖动。设置了 `on_drag_start` / `on_drag` / `on_drag_end` 后，这些消息会与主要的跳转消息并行产生：控件返回跳转结果，并通过 `take_extra_message` 交回额外的消息。应用程序负责播放：在播放定时器中调用 `set_progress`，该调用从不发出消息。

未设置采样时，进度条绘制一条平坦的中心线，因此在解码器提供波形之前，该控件仍会被理解为进度条。

## 用法（Rust）

```rust
use bamboo_native_ui::widgets::WaveformSeekBar;
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg {
    Seek(f32),
}

let mut ui: Ui<Msg> = Ui::new();
ui.add(
    WaveformSeekBar::new("waveform", 0.0, Msg::Seek)
        .samples([0.2, 0.6, 0.35, 0.9, 0.5, 0.75])
        .duration(255.0)
        .bar_width(2.0)
        .gap(1.0),
);
ui.set_rect(WidgetId::new("waveform"), Rect::from_xywh(16.0, 160.0, 520.0, 56.0));
```

`new` 接收初始位置（钳制到 `0..=1`）和跳转处理函数，后者接收新的比例。`duration` 仅影响无障碍值文本：有持续时间时读作 `m:ss`，没有时读作百分比。用类型化访问推送播放更新，不产生消息：

```rust
ui.edit_widget::<WaveformSeekBar<Msg>>(WidgetId::new("waveform"), |bar| {
    bar.set_progress(0.42);
    true
});
```

## 用法（XML）

此控件不由标准注册表创建。请在 Rust 中创建它，并通过文档中的 `id` 引用它。共享的 `value` 内容会静默地设置位置（`0..=1`）；该控件不声明任何控件局部属性，也没有子元素。

```xml
<widget id="waveform" x="16" y="160" w="520" h="56" value="0.32"/>
```

## 属性

| 方法 | 类型 | 默认值 | 描述 |
|---|---|---|---|
| `new(id, progress, on_seek)` | `&'static str, f32, impl Fn(f32) -> M` | — | 创建该进度条；`480 × 56` DIP。 |
| `.samples(iter)` | `impl IntoIterator<Item = f32>` | 空 | 钳制到 `0..=1` 的振幅，每根条对应一个槽位。 |
| `.bar_width(f32)` | `f32` | `2.0` | 单根条的宽度，钳制到 `1..=12`。 |
| `.gap(f32)` | `f32` | `1.0` | 每根条之后的间距，钳制到 `0..=8`。 |
| `.colors(played, unplayed)` | `Color, Color` | 强调色 / 柔和色混合 | 显式的条颜色。 |
| `.duration(f32)` | `f32` | `None` | 轨道秒数（`> 0`）；将无障碍值格式化为 `m:ss`。 |
| `.on_drag_start(handler)` | `Fn(f32) -> M` | 无 | 附加型：搓动开始时以按下位置触发。 |
| `.on_drag(handler)` | `Fn(f32) -> M` | 无 | 附加型：每次搓动移动时触发。 |
| `.on_drag_end(handler)` | `Fn(f32) -> M` | 无 | 在释放时以提交的位置触发。 |

类型化访问和运行时 setter：

| 方法 | 类型 | 描述 |
|---|---|---|
| `.set_samples(iter)` / `.samples_ref()` | `impl IntoIterator` / `&[f32]` | 替换或读取振幅。 |
| `.set_duration(Option<f32>)` / `.duration_value()` | `Option<f32>` | 用于值文本的持续时间。 |
| `.progress()` / `.set_progress(f32)` | `f32` | 位置 `0..=1`；setter 从不发出消息。 |
| `.is_dragging()` / `.drag_preview()` | `bool` / `Option<f32>` | 是否正在进行搓动以及所绘制的预览。 |

### 行为

- **默认值。** 已播放颜色为 `palette.accent`；未播放颜色为 `palette.muted`，向 `palette.background` 混合 35%。默认矩形为 `480 × 56` DIP；`preferred_height` 为 56。
- **指针。** 按下会跳转并开始拖动；每次拖动移动都会发出消息。在两端之外的按下会钳制到 `0.0` / `1.0`。释放会提交预览，设置了 `on_drag_end` 时它是唯一的消息。
- **键盘。** `WaveformSeekBar::STEP` 为 `0.02`。左/右方向键调整一个步长，Home/End 跳到两端，在边缘处步进不会发出任何消息。
- **无障碍。** 角色为 `Slider`，范围为 `0..=1`，值文本为 `m:ss` 或百分比。`a11y_set_value` 接受 `"45%"` 或 `"0.45"` 并对其钳制。

## 事件

### 消息

| 消息 | 发出条件 |
|---|---|
| 作为 `on_seek` 传给 `new` 的 `impl Fn(f32) -> M` | 按下开始搓动、每次搓动移动以及键盘步进（`Left`/`Right`/`Home`/`End`/`Activate`）；`f32` 是新的 `0..=1` 位置。 |
| 传给 `.on_drag_start` 的 `impl Fn(f32) -> M` | 搓动开始，携带按下位置。 |
| 传给 `.on_drag` 的 `impl Fn(f32) -> M` | 搓动期间的每次指针移动。 |
| 传给 `.on_drag_end` 的 `impl Fn(f32) -> M` | 释放时提交位置。 |

拖动回调是附加型的：控件返回跳转结果，并通过 `take_extra_message` 交回拖动消息。在释放时，拖动结束消息成为该输入的主要消息。`set_progress` 和 `a11y_set_value` 更新位置而不发出消息。

### 通用观察器

此控件不安装任何共享的观察器构建器。请使用构造函数中的跳转处理函数、拖动回调或 `Ui::bind`。

### 控件专属回调

| 回调 | 签名 | 触发条件 |
|---|---|---|
| `new(id, progress, on_seek)` | `&'static str, f32, impl Fn(f32) -> M` | 按下、搓动移动或键盘步进。 |
| `.on_drag_start(handler)` | `impl Fn(f32) -> M` | 一次，携带按下时的初始位置。 |
| `.on_drag(handler)` | `impl Fn(f32) -> M` | 在按住按钮期间每次拖动移动时。 |
| `.on_drag_end(handler)` | `impl Fn(f32) -> M` | 释放时，携带提交的位置。 |

### Ui 触发器

除上述构建器外，任何控件 id 都可以通过 `Ui::bind` 绑定：

```rust
ui.bind(WidgetId::new("waveform"), Trigger::Click, |event, ctx| {
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

该进度条响应 `Click`、`HoverEnter`/`HoverLeave`、`DragStart`/`DragEnd`、`Activate` 和 `ValueChange`；在其上的滚轮事件或右键点击不产生任何效果。

## XML 定义

此控件不由标准注册表创建。请在 Rust 中创建它，并通过文档中的 `id` 引用它。

几何遵循常规的 `<widget>` 规则（水平形式和垂直形式各一种）。通用属性（`visible`、`tooltip`、`name`、`description`、`automation-id`、`role`、`expose`、焦点策略）适用。内容支持仅为共享的 `value` 属性；`text`、`label`、`progress`、`number`、`checked` 和 `selected` 会被拒绝为 `xml.content.unsupported`。进度条从调色板绘制，没有 `SurfaceStyle`，因此 `radius`、`corners` 和 `background` 会被报告为 `xml.style.unsupported` 并被跳过。

控件局部属性：无。子元素：无。

## 另见

- `./playback-transport.md`
- `./playlist-view.md`
- `../index.md`
- `../../api/widget-trait.md`
- `../../guides/xml-layout-authoring.md`
