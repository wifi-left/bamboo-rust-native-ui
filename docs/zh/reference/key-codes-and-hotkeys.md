# 键码与热键

> `KeyCode`/`KeyEvent` 词汇、虚拟键映射、`Hotkey` 语法和
> 标签，以及手柄到按键的转换。

## `KeyCode`

```rust
pub enum KeyCode {
    Backspace, Tab, Enter, Escape, Space,
    Left, Right, Up, Down, Home, End, PageUp, PageDown, Insert, Delete,
    Letter(u8),    // A..Z 对应 0..=25
    Digit(u8),     // 0..9 对应 0..=9
    Function(u8),  // F1..F24 对应 1..=24
    Other(u16),    // 任意其他虚拟键码
}
```

方法：

| 方法 | 返回 |
|---|---|
| `is_letter(u8)` | 是否为等于给定从 0 开始索引的 `Letter`。 |
| `from_vk(u16)` | 当虚拟键码已知时返回一个具名变体，否则返回 `Other(vk)`。 |
| `to_vk()` | `Option<u16>`；对 `Other(0)` 和 `1..=24` 之外的 `Function` 返回 `None`。 |

`from_vk`/`to_vk` 使用数值 Win32 虚拟键值，但不暴露任何 Win32 类型。

| 键 | VK | 键 | VK |
|---|---|---|---|
| Backspace | `0x08` | Left | `0x25` |
| Tab | `0x09` | Up | `0x26` |
| Enter | `0x0D` | Right | `0x27` |
| Escape | `0x1B` | Down | `0x28` |
| Space | `0x20` | Home | `0x24` |
| PageUp | `0x21` | PageDown | `0x22` |
| Insert | `0x2D` | Delete | `0x2E` |
| End | `0x23` | `A`..`Z` | `0x41`..`0x5A` |
| `0`..`9` | `0x30`..`0x39` | `F1`..`F24` | `0x70`..`0x87` |

## `KeyEvent`

```rust
pub struct KeyEvent {
    pub code: KeyCode,
    pub vk: u16,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub repeat: bool,   // 自动重复：该键此前已按下
}
```

`Event::Key(KeyEvent)` 到达 `Ui::key`；`Event::KeyUp` 到达 `Ui::key_up`
（观察）。`Event::Char(char)` 是独立的，且从不携带控制
字符：Enter、Tab 和 Backspace 以 `Key` 形式到达。应每次按下只执行一次的
处理器会检查 `repeat`。

指针修饰键是一个独立的两字段结构体：

```rust
pub struct PointerModifiers { pub ctrl: bool, pub shift: bool }
```

提供了常量 `NONE`、`ctrl(bool)`、`shift(bool)` 和 `any()`；
`PointerDown` 和 `PointerUp` 各自报告其自身时刻的状态。

## `Hotkey`

```rust
pub struct HotkeyModifiers {
    pub ctrl: bool, pub shift: bool, pub alt: bool, pub win: bool,
}

pub struct Hotkey {
    pub modifiers: HotkeyModifiers,
    pub vk: u16,   // 与 KeyEvent::vk 相同的数值空间
}
```

| API | 签名 | 备注 |
|---|---|---|
| `Hotkey::new` | `new(modifiers, key: KeyCode) -> Option<Self>` | 当该键没有稳定的 VK 时返回 `None`。 |
| `Hotkey::from_vk` | `from_vk(modifiers, vk: u16) -> Self` | 从原始输入构造。 |
| `Hotkey::parse` | `parse(spec: &str) -> Option<Self>` | 见下面的语法。 |
| `Hotkey::label` | `label(&self) -> String` | 可持久化的文本；可通过 `parse` 往返。 |
| `Hotkey::key` | `key(&self) -> KeyCode` | 具名键，或 `Other(vk)`。 |
| `Hotkey::is_global_safe` | `is_global_safe(&self) -> bool` | 有修饰键，或是功能键。 |
| `Hotkey::from_key_event` | `from_key_event(&KeyEvent) -> Option<Self>` | 使用事件的 ctrl/shift/alt；从不设置 Win。 |

`HotkeyModifiers` 常量：`NONE`、`CTRL`、`SHIFT`、`ALT`、`WIN`；构建器
`with_ctrl`、`with_shift`、`with_alt`、`with_win`；`is_empty`。
`hotkey::is_modifier_vk(vk) -> bool` 报告某个码是否只贡献于
组合键（Shift/Ctrl/Alt、Win，或左/右修饰键变体）。

### 解析语法

`spec` 按 `+` 分割；每个 token 去除首尾空白并大小写不敏感地比较。

| Token | 含义 |
|---|---|
| `ctrl`、`control` | Ctrl |
| `shift` | Shift |
| `alt` | Alt |
| `win`、`windows`、`meta`、`super` | Win |
| 单个 ASCII 字母 | `Letter(0..=25)` |
| 单个 ASCII 数字 | `Digit(0..=9)` |
| `enter`、`return` | Enter |
| `space`、`tab` | Space、Tab |
| `escape`、`esc` | Escape |
| `backspace`、`bksp` | Backspace |
| `delete`、`del`、`insert`、`ins` | Delete、Insert |
| `home`、`end` | Home、End |
| `pageup`、`pgup`、`pagedown`、`pgdn` | PageUp、PageDown |
| `up`、`down`、`left`、`right`（+ `arrow*`） | 方向键 |
| `f1`..`f24` | 功能键 |
| `vk` + 十六进制 | 任意虚拟键码（`VK2C` → `Other(0x2C)`） |

规则：

- 修饰键可以任意顺序出现且可多次出现（`ctrl+control` 仍是
  Ctrl）。恰好允许一个非修饰键。
- 快捷键必须至少有一个修饰键，**除非**该键是功能
  键。`M` 和 `Enter` 被拒绝；`F9` 和 `Shift+A` 被接受。
- 空 token、重复键、未知名称和超出范围的功能键
  返回 `None`。

`label()` 按该顺序打印 Ctrl、Alt、Shift、Win，然后打印键。键标签
包括 `Esc`、`PageUp`、`PageDown`、`Backspace`、`Delete`、`Insert`、`Home`、
`End`、`Up`、`Down`、`Left`、`Right`、`Space`、`Tab`、`Enter`、`F1`..`F24`，以及
对 `Other` 使用 `VKxx`（大写十六进制）。

```rust
use bamboo_native_ui::Hotkey;

let chord = Hotkey::parse("ctrl+shift+m").expect("valid");
assert_eq!(chord.label(), "Ctrl+Shift+M");   // parse("Ctrl+Shift+M") == Some(chord)
assert!(Hotkey::parse("M").is_none());       // 裸可打印键
assert!(Hotkey::parse("F9").is_some());      // 功能键无需修饰键
```

## 注册

| API | 在哪里触发 | 备注 |
|---|---|---|
| `WindowHandle::register_hotkey(hotkey)` | `Host::hotkey` | 全系统范围（`RegisterHotKey`）；当另一个进程拥有它时返回 `false`。 |
| `WindowHandle::unregister_hotkey(hotkey)` | — | 当它未被注册时返回 `false`。 |
| `WindowHandle::register_window_hotkey(hotkey, command)` | `Host::menu_bar_command` | 窗口作用域；没有系统注册，因此没有跨进程冲突。 |
| `WindowHandle::clear_window_hotkeys()` | — | 移除每一个窗口快捷键。 |
| `WindowHandle::begin_hotkey_capture()` | `Host::hotkey_captured` | 捕获下一个组合键；`None` 表示 Esc/失去焦点/取消。 |

`register_window_hotkey` 拒绝命令 `0`、没有修饰键且不是功能键的
组合键，以及重复项。菜单栏的加速键标签会
自动注册；对同一组合键显式设置的窗口热键会
覆盖它们。捕获的组合键**不会**被自动注册；由
应用决定。

## 常用组合键

这些是应用看到的 `KeyEvent` 模式（不是注册的全局
热键）。文本字段实现自己的编辑命令；其他控件
接收原始按键。

| 组合键 | `code` + 修饰键 | 典型用途 |
|---|---|---|
| Ctrl+C / Ctrl+V / Ctrl+X | `Letter(2/21/23)` + `ctrl` | 剪贴板复制/粘贴/剪切。 |
| Ctrl+A | `Letter(0)` + `ctrl` | 全选。 |
| Ctrl+Z / Ctrl+Y | `Letter(25/24)` + `ctrl` | 撤销 / 重做。 |
| Ctrl+S / Ctrl+O / Ctrl+N | `Letter(18/14/13)` + `ctrl` | 保存 / 打开 / 新建。 |
| Ctrl+F | `Letter(5)` + `ctrl` | 查找。 |
| Shift+Tab | `Tab` + `shift` | 反向聚焦。 |
| Shift+F10 | `Function(10)` + `shift` | 上下文菜单（也是手柄 Y 发送的内容）。 |
| F1 | `Function(1)` | 帮助。 |
| Alt+F4 | `Function(4)` + `alt` | Shell 关闭请求（`Host::close_requested`）。 |
| Delete / Backspace | `Delete` / `Backspace` | 删除。 |
| Enter / Space | `Enter` / `Space` | 聚焦控件上的 `WidgetInput::Activate`。 |

对于**全局**快捷键，请改用 `Hotkey::parse`：`"Ctrl+Alt+P"`、
`"Alt+F10"`、`"F9"`、`"Shift+Esc"` 都是典型示例；`"Ctrl+S"` 作为全局热键
会从每个应用中抢走 Save。

## 访问键（助记符）

窗口装饰标签（菜单栏、菜单按钮、工具栏、拆分按钮）会解析 `&`：

- `"&File"` 绘制 `File` 并响应 `Alt+F`。
- `"&&"` 是字面和号。
- 孤立的尾部 `&` 和后面跟空白的 `&` 保持字面。
- 只有第一个有效的标记是访问键；后续标记会失去 `&`
  但保留字符。

容器路由 `Alt+key`；控件暴露 `Widget::mnemonic` 和
`Widget::mnemonic_key`。普通按钮不使用访问键：用 Tab 和 Space/Enter
到达它们。

## 手柄映射

`Gamepad` 轮询一个 XInput 槽并返回 `Vec<KeyEvent>`；将它们转发给
`Ui::pad_key`（或 `Ui::key`）。映射是纯的，无需硬件即可测试；
`read_state(slot)` 是唯一的平台调用。

| 手柄输入 | 按键事件 |
|---|---|
| 方向键上/下/左/右 | `Up` / `Down` / `Left` / `Right` |
| 左摇杆（主导轴，`abs(axis) >= 0.55`） | 方向键 |
| A、Start | `Enter` |
| B、Back | `Escape` |
| X | `Space` |
| Y | `Function(10)` + `shift`（上下文菜单） |
| RB | `Tab` |
| LB | `Tab` + `shift` |

面/肩按钮是边沿触发的。按住的方向会立即触发，
然后在 400 ms 之后重复，此后每 90 ms 一次。方向键优先于摇杆。一个
被禁用的手柄或后台窗口不发出任何内容并丢弃按住状态，因此
重新启用后绝不会恢复陈旧的重复。聚焦文本字段上的 **A** 还会
通过 `Ui::pad_key` 唤起 Windows 屏幕键盘。

## 另请参阅

- [事件与输入](../api/events-and-input.md)
- [热键 API](../api/hotkeys.md)
- [手柄](../api/gamepad.md)
- [Ui 容器](../api/ui-container.md)
- [辅助功能指南](../guides/accessibility.md)
- [Host 与窗口](../api/host-and-window.md)
