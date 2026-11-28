# 热键

> 解析并标注键盘组合键，将其注册为系统级或按窗口生效，并为设置界面捕获下一个组合键。

## 概述

`Hotkey` 由一组修饰键和一个虚拟键码组成，虚拟键码与 `KeyEvent::vk` 处于同一数值空间，
因此公开 API 中不会出现任何 Win32 类型。宿主使用 `RegisterHotKey` 注册它（系统级），
或将其注册为窗口快捷键（不做系统注册）。可打印的普通按键必须带修饰键：`parse` 会拒绝裸 `A`，因为全局
`A` 快捷键会在任何地方吞掉普通输入。

`Hotkey`、`HotkeyModifiers` 模型和 `parse`/`label` 跨平台。注册、捕获和
`Host`/`WindowHandle` 回调仅限 Windows。

## 关键类型

### `HotkeyModifiers`

```rust
pub struct HotkeyModifiers {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub win: bool,
}
```

常量：`NONE`、`CTRL`、`SHIFT`、`ALT`、`WIN`。方法：`is_empty()`、
`with_ctrl()`、`with_shift()`、`with_alt()`、`with_win()`。

### `Hotkey`

```rust
pub struct Hotkey {
    pub modifiers: HotkeyModifiers,
    pub vk: u16,
}
```

| 方法 | 签名 | 描述 |
|---|---|---|
| `new` | `new(modifiers, key: KeyCode) -> Option<Self>` | 从命名按键构建；若其没有稳定的虚拟键则返回 `None`。 |
| `from_vk` | `from_vk(modifiers, vk: u16) -> Self` | 从原始虚拟键码构建。 |
| `parse` | `parse(spec: &str) -> Option<Self>` | 解析 `"Ctrl+Shift+M"`、`"Alt+F10"` 等。 |
| `label` | `label(&self) -> String` | `"Ctrl+Shift+M"` 风格的文本；可经由 `parse` 往返转换。 |
| `key` | `key(&self) -> KeyCode` | 该键作为命名 `KeyCode`（或 `Other(vk)`）。 |
| `is_global_safe` | `is_global_safe(&self) -> bool` | 带有修饰键，或是一个功能键。 |
| `from_key_event` | `from_key_event(&KeyEvent) -> Option<Self>` | 从键盘事件的状态构建。 |

`hotkey::is_modifier_vk(vk) -> bool` 报告某个键码是否仅对组合键有贡献。

`parse` 不区分大小写，接受任意顺序的修饰键（`ctrl`/`control`、`shift`、`alt`、
`win`/`windows`/`meta`/`super`）、命名按键（`enter`、`tab`、`escape`/`esc`、
`delete`/`del`、`pageup`/`pgup`、方向键、功能键 `F1`–`F24`）以及持久化的 `VKxx` 键码。
`label` 按 Ctrl、Alt、Shift、Win 的顺序打印修饰键，然后打印按键。

### 注册与捕获（`WindowHandle`）

| API | 签名 |
|---|---|
| `register_hotkey` | `register_hotkey(&mut self, hotkey: Hotkey) -> bool` |
| `unregister_hotkey` | `unregister_hotkey(&mut self, hotkey: Hotkey) -> bool` |
| `registered_hotkeys` | `registered_hotkeys(&self) -> Vec<Hotkey>` |
| `register_window_hotkey` | `register_window_hotkey(&mut self, hotkey, command: u32) -> bool` |
| `unregister_window_hotkey` | `unregister_window_hotkey(&mut self, hotkey) -> bool` |
| `clear_window_hotkeys` | `clear_window_hotkeys(&mut self)` |
| `window_hotkeys` | `window_hotkeys(&self) -> Vec<(Hotkey, u32)>` |
| `begin_hotkey_capture` | `begin_hotkey_capture(&mut self)` |
| `cancel_hotkey_capture` | `cancel_hotkey_capture(&mut self)` |
| `hotkey_capture_active` | `hotkey_capture_active(&self) -> bool` |

`Host::hotkey(hotkey, window)` 接收触发的全局热键；
`Host::hotkey_captured(hotkey: Option<Hotkey>, window)` 接收捕获结果。

## 示例

解析与标注：

```rust
use bamboo_native_ui::Hotkey;

let play = Hotkey::parse("ctrl+alt+p").expect("valid chord");
assert_eq!(play.label(), "Ctrl+Alt+P");

let help = Hotkey::parse("F1").expect("function keys need no modifier");
assert_eq!(help.label(), "F1");

// 裸的可打印按键会被拒绝：它会捕获普通输入。
assert!(Hotkey::parse("A").is_none());
```

全局注册并捕获一个新的组合键：

```rust
use bamboo_native_ui::{Hotkey, WindowHandle};

fn install(window: &mut WindowHandle<'_>) {
    if let Some(hotkey) = Hotkey::parse("Ctrl+Alt+P") {
        let _ = window.register_hotkey(hotkey);
    }
}

fn hotkey(&mut self, hotkey: Hotkey, window: &mut WindowHandle<'_>) {
    window.show();
    self.status = format!("{} pressed", hotkey.label());
}

fn hotkey_captured(&mut self, hotkey: Option<Hotkey>, window: &mut WindowHandle<'_>) {
    if let Some(hotkey) = hotkey {
        // 由应用决定捕获到的组合键是否成为全局热键。
        let _ = window.register_hotkey(hotkey);
    }
}
```

## 说明与限制

- `register_hotkey` 在 Windows 上是系统级的，另一个进程已拥有该组合键时返回 `false`；
  重复注册同一热键是无操作，返回 `true`。窗口销毁时注册会被释放。
- `register_window_hotkey` 不需要系统注册，因此不会与另一个进程冲突。窗口拥有焦点时，
  它会以 `command` 触发 `Host::menu_bar_command`（在 `MenuEvent` 钩子之后），页面永远不会
  看到该组合键。对于命令 `0`、不带修饰键且非功能键的组合键，或重复项，它返回 `false`。
- 原生菜单栏的加速键标签会自动注册为窗口快捷键；为同一组合键显式调用
  `register_window_hotkey` 会覆盖它们。
- 捕获会在完成前消耗按键，让它们在应用看到之前就被拦截（包括转换后的字符）。仅按修饰键
  会继续等待；`Some` 表示带修饰键或功能键的组合键，`None` 表示 Esc、焦点丢失或
  `cancel_hotkey_capture`。该结果**不会**自动注册。
- 请持久化 `hotkey.label()` 字符串；在检查 `bool` 结果后（系统可能已拥有该组合键）用
  `register_hotkey` 恢复。
- 所有注册与捕获仅限 Windows；解析/标注模型跨平台。

## 另见

- `menus.md`（菜单项上的加速键标签）
- `accessibility.md`（朗读状态）
- `gamepad.md`（映射到按键的手柄按钮）
- `../quickstart/03-first-window.md`
