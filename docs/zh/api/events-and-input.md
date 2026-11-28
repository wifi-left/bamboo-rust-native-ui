# 事件与输入

> 规范化的、平台无关的输入事件，以及 Windows 宿主将 Win32 消息转换成它们时使用的名称。

## 概述

`bamboo_native_ui::event` 定义了宿主交给应用的词汇表。后端会将 `WM_*`
消息转换成这些值；控件永远看不到 Win32 常量，并且每个位置都以 DIP 为单位。流程是：
Windows 宿主产生一个 `Event`，`Host::event` 路由它（基于 `Ui` 的应用
将其映射到对应的 `Ui` 方法），`Ui` 在调用控件之前解析命中测试和焦点，并以一个
`WidgetInput` 传入（见
[控件 trait](./widget-trait.md)）。

这些类型是 `Copy` 值类型，没有事件对象身份，也没有
分配。只有 `KeyCode::from_vk`/`to_vk`、`MouseButton::from_wparam` 和
`Color` 的 `COLORREF` 辅助方法带有 Windows 形状的数值，并且它们是
如此文档化的。

## 关键类型

### `Event`

```rust
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    PointerMove { pos: Point, buttons: u32 },          // -> pointer_move
    PointerDown { pos: Point, button: MouseButton, modifiers: PointerModifiers },
    PointerUp { pos: Point, button: MouseButton, modifiers: PointerModifiers },
    PointerDoubleClick { pos: Point, button: MouseButton }, // -> pointer_double_click
    PointerLeave,                                      // -> pointer_leave
    PointerKind(PointerKind),                          // -> set_pointer_kind
    LongPress { pos: Point },                          // -> long_press
    Wheel { pos: Point, delta_x: f32, delta_y: f32 },  // -> wheel_xy
    Key(KeyEvent),                                     // -> key
    KeyUp(KeyEvent),                                   // -> key_up（观察）
    Alt(bool),                                         // -> set_mnemonic_underlines
    Char(char),                                        // -> char
    Focus(bool),                                       // 窗口获得/失去焦点
    Resize(Size),                                      // -> Host::resize
    DpiChanged(f32),                                   // -> Host::dpi_changed
}
```

`PointerDown`/`PointerUp` 经由 `pointer_down_with` /
`pointer_up_with` 路由（按下时和释放时的修饰键）。`Wheel` 向上/向左为正。`Alt(true)`
跟在单独轻按 Alt 之后；组合键或焦点变化会发送
`false`。内置的 Windows 宿主直接通过 `Host::resize` /
`Host::dpi_changed` 报告尺寸和 DPI，因此 `Resize`/`DpiChanged` 是供转发它们的宿主使用。
一次双击不会替换按下/释放对。

### `KeyCode`

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeyCode {
    Backspace, Tab, Enter, Escape, Space,
    Left, Right, Up, Down, Home, End, PageUp, PageDown, Insert, Delete,
    /// `A`..`Z` 以 0..=25 表示。
    Letter(u8),
    /// `0`..`9` 以 0..=9 表示。
    Digit(u8),
    /// `F1`..`F24` 以 1..=24 表示。
    Function(u8),
    /// 任何其他虚拟键码。
    Other(u16),
}

impl KeyCode {
    pub fn is_letter(self, letter: u8) -> bool;
    pub fn from_vk(vk: u16) -> Self;
    pub const fn to_vk(self) -> Option<u16>;
}
```

`from_vk`/`to_vk` 映射 Win32 虚拟键码；这些数值碰巧与 VK
常量相匹配，但不涉及任何 Win32 类型。对 `Other(0)` 和
超出范围的 `Function`，`to_vk` 返回 `None`。

### `KeyEvent`、`MouseButton`、`Cursor`

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyEvent {
    pub code: KeyCode, pub vk: u16,
    pub ctrl: bool, pub shift: bool, pub alt: bool,
    /// 自动重复（该键此前已按下）。
    pub repeat: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MouseButton { Left, Right, Middle, Other(u16) }
impl MouseButton { pub fn from_wparam(wparam: u32) -> Option<Self>; }

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Cursor { #[default] Arrow, Hand, IBeam, SizeWE, SizeNS }
```

`Cursor` 告诉宿主显示哪个系统光标；控件通过
`Widget::cursor` 建议一个光标，并由 `Ui::hover_cursor` 报告给应用。

### `PointerModifiers` 与 `PointerKind`

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PointerModifiers { pub ctrl: bool, pub shift: bool }
impl PointerModifiers {
    pub const NONE: Self;
    pub const fn ctrl(ctrl: bool) -> Self;
    pub const fn shift(shift: bool) -> Self;
    pub fn any(self) -> bool;
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PointerKind { #[default] Mouse, Touch, Pen }
```

当设备发生变化时，`PointerKind` 在一个序列的第一个指针事件之前发送；
已绘制的编辑器用它切换到触摸模型（长按、手柄、屏幕键盘），而无需询问用户。
无法辨别设备的宿主会将其保持在 `Mouse`。

## 示例

```rust
use bamboo_native_ui::{Event, Ui, UiHost};

enum Msg {
    Dismiss,
}

fn route(ui: &mut Ui<Msg>, event: &Event, sink: &mut impl UiHost) {
    match *event {
        Event::PointerMove { pos, .. } => {
            ui.pointer_move(pos, sink);
        }
        Event::PointerDown { pos, button, modifiers } => {
            ui.pointer_down_with(pos, button, modifiers, sink);
        }
        Event::PointerUp { pos, button, modifiers } => {
            ui.pointer_up_with(pos, button, modifiers, sink);
        }
        Event::Wheel { pos, delta_x, delta_y } => {
            ui.wheel_xy(pos, delta_x, delta_y, sink);
        }
        Event::Key(key) => {
            ui.key(key, sink);
        }
        Event::Char(ch) => {
            ui.char(ch, sink);
        }
        Event::Alt(show) => {
            ui.set_mnemonic_underlines(show);
        }
        _ => {}
    }
}
```

## 说明与限制

- 位置是客户区 DIP；宿主将物理坐标除以
  `dpi / 96`。
- `PointerMove::buttons` 是已按下按钮的位掩码
  （`MOUSE_BUTTON_*` 风格的位），而不是 `MouseButton`。
- 修饰键采样两次：`PointerDown` 报告按下时的状态，`PointerUp`
  报告其自身的状态。多选列表读取释放时的状态，但两端都会被
  报告。
- `Key::repeat` 标记自动重复；每次按下只作用一次的处理器会检查它。
- `Char` 排除控制字符：Enter、Tab 和 Backspace 会以 `Key` 到达。
- `LongPress` 需要带有触摸和计时器的宿主；仅鼠标的宿主永远不会发送
  它。

## 另见

- [控件 trait](./widget-trait.md)
- [Ui 容器](./ui-container.md)
- [宿主与窗口](./host-and-window.md)
- [游戏手柄](./gamepad.md)
- [文本编辑](../widgets/inputs/text-edit.md)
- [第一个窗口快速上手](../quickstart/03-first-window.md)
