# 游戏手柄

> 将 XInput 控制器转换为焦点导航本已消费的键盘事件。

## 概述

手柄由宿主轮询（XInput；对菜单来说 33 ms 已足够），其变化成为普通的 `KeyEvent`：方向键
（D-pad）或左摇杆按下方向键，**A** 是 Enter，**B** 是 Escape，**RB** 是 Tab，**LB** 是
Shift+Tab，**Y** 打开聚焦控件的上下文菜单（`Shift+F10`），**X** 是 Space。应用将这些按键
转发给 `Ui::pad_key`（或 `Ui::key`），因此为键盘编写的页面无需第二套输入模型即可工作。

手柄打开文本字段时，`pad_key` 还会唤起 Windows 屏幕键盘，并在键盘弹出时将该手柄路由到
它。按住的方向会重复（延迟 400 ms，之后每 90 ms 重复一次），因此长列表可以滚动而无需反复
点按。

`Gamepad`、`PadButtons`、`PadState` 模型和 `update` 的纯函数部分跨平台，不需要硬件即可
测试；`read_state` 是唯一的平台调用，在非 Windows 上返回 `None`。软键盘仅限 Windows。

## 关键类型

### `PadButtons`

```rust
pub struct PadButtons(pub u16);
```

位常量：`DPAD_UP`、`DPAD_DOWN`、`DPAD_LEFT`、`DPAD_RIGHT`、`START`、`BACK`、
`LEFT_THUMB`、`RIGHT_THUMB`、`LEFT_SHOULDER`、`RIGHT_SHOULDER`、`A`、`B`、`X`、`Y`。
方法：`contains(bit) -> bool`、`pressed_since(previous) -> PadButtons`（新按下的位）。

### `PadState`

```rust
pub struct PadState {
    pub buttons: PadButtons,
    pub left_stick: (f32, f32),
    pub right_stick: (f32, f32),
    pub left_trigger: u8,
    pub right_trigger: u8,
}
```

摇杆为 `-1.0..=1.0`，y 轴向上；扳机为 `0..=255`。

### `Gamepad`

```rust
pub struct Gamepad { /* private */ }

impl Gamepad {
    pub fn new(slot: u32) -> Self; // 钳制到 0..=3
}
```

| 方法 | 签名 | 描述 |
|---|---|---|
| `poll` | `poll(&mut self, now: Instant, active: bool) -> Vec<KeyEvent>` | 读取手柄并转换变化；不活动/禁用时重置。 |
| `update` | `update(&mut self, state: PadState, now: Instant) -> Vec<KeyEvent>` | `poll` 的纯函数部分。 |
| `slot` | `slot(&self) -> u32` | XInput 槽位。 |
| `set_enabled` | `set_enabled(&mut self, enabled: bool)` | 关闭该层；丢弃按住状态。 |
| `is_enabled` | `is_enabled(&self) -> bool` | 当前开关。 |
| `reset` | `reset(&mut self)` | 忘记按住状态（停用、拔出）。 |

`read_state(slot: u32) -> Option<PadState>` 读取一个槽位（未连接手柄时返回 `None`）。它在
crate 根处重新导出为 `read_gamepad`。

### 软键盘

`bamboo_native_ui::soft_keyboard` 在 Windows 上重新导出 `platform::keyboard`：
`show(layout: Layout) -> bool`、`hide() -> bool`、`is_open() -> bool`，以及
`Layout { Gamepad, Touch }`。`Ui::pad_key` 会调用接收器的软键盘钩子；应用很少直接调用该模块。

### `Ui` 上的方法

`Ui::pad_key(&mut self, key: KeyEvent, sink: &mut impl UiHost) -> bool` 路由一个合成按键。
它返回该按键是否被消费。

## 示例

在计时器上轮询并转发按键：

```rust
use std::time::{Duration, Instant};
use bamboo_native_ui::{read_gamepad, Gamepad, WindowHandle};

fn attach_pad(&mut self, window: &mut bamboo_native_ui::WindowHandle<'_>) {
    if read_gamepad(0).is_some() {
        self.pad = Some(Gamepad::new(0));
        window.set_animation_timer(Some(Duration::from_millis(33)));
    }
}

fn timer(&mut self, window: &mut WindowHandle<'_>) {
    let Some(pad) = self.pad.as_mut() else {
        return;
    };
    for key in pad.poll(Instant::now(), window.is_foreground()) {
        if self.ui.pad_key(key, window) {
            self.drain(window);
        }
    }
}
```

`window.is_foreground()` 即 `active` 标志：后台窗口永不接收手柄导航，与游戏主机仪表盘的
约定相同。手柄离开时，丢弃它，稍后慢速监视 tick 可重新附加它。

## 说明与限制

- `poll` 返回 `Vec<KeyEvent>`；空向量表示无变化。正面和肩部按钮是边沿触发的（按住 A 不会
  重复）；方向键会保持按住，在 `400 ms` 后重复，之后每 `90 ms` 重复一次。
- 左摇杆在其主导轴超过 `0.55` 死区后计为一个方向；对角线摇杆时方向键（D-pad）优先。XInput
  的 y 轴向上增长，因此负 y 映射为 Down。
- 合成按键携带 `vk: 0`；请与 `KeyEvent::code` 比较，而非 `vk`。
- 手柄打开文本字段时，`pad_key` 会唤起屏幕键盘。键盘弹出期间它接管手柄，按键不会被转发；
  **A** 重新唤起被关闭的键盘，**B** 关闭它并离开该字段（提交）。
- 没有 XInput 的平台从 `read_state` 返回 `None`；纯函数的 `update` 路径仍可用于测试和
  合成输入。
- 轮询和软键盘仅限 Windows；模型在所有平台都能编译。

## 另见

- `hotkeys.md`（将组合键绑定到动作）
- `menus.md`（Y 打开聚焦的上下文菜单）
- `accessibility.md`（朗读焦点移动）
- `../widgets/controls/toggle.md`
