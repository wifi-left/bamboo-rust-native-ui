# 性能

> 让空闲绘制保持为零、只重绘变化的部分，并理解
> 后备缓冲区和后端缓存。

## 性能契约

Bamboo Native UI 是一个保留模式框架。一次状态更改不会
重新布局，也不会重绘窗口；它记录损坏（damage）。宿主
将损坏合并为一小组矩形，只绘制与它们相交的控件
到持久化后备缓冲区中，并只将那些片段 blit 到屏幕。

| 规则 | 结果 |
|---|---|
| 应用计算矩形 | 没有测量阶段；布局成本是你在调整尺寸时的算术。 |
| 只有脏矩形会绘制 | 两个相距很远的控件上的悬停只需两次小的重绘。 |
| 每个窗口一个后备缓冲区 | 控件可以绘制其整个矩形；未触及的像素保持正确。 |
| 绘制路径上无分配 | 绘制成本稳定，且与分配器无关。 |
| 空闲即零绘制 | 除非你安装计时器，否则没有周期性重绘。 |

完整的理由见[设计原则](../overview/design-principles.md)。

## 步骤

### 1. 让 setter 记录损坏，然后刷新一次

每个 `Ui` setter（`set_label`、`set_progress`、`set_value`、`set_visible`
等）都会将受影响的控件记录为待处理损坏。在
宿主使窗口失效之前不会重绘任何内容。在做出更改的回调
末尾调用一次 `flush_damage`：

```rust
use bamboo_native_ui::{Host, WindowHandle};

impl Host for App {
    fn event(
        &mut self,
        event: &bamboo_native_ui::Event,
        window: &mut WindowHandle<'_>,
    ) -> bool {
        let consumed = self.route(event, window);
        self.ui.flush_damage(window);
        consumed
    }
}
```

一个回调中的一串 setter 会变成一次重绘请求。相距较远的控件
保持为独立片段，因此一个更新标签和一个旋转指示器的计时器
不会重绘它们之间的页面。

损坏会合并重叠或相接（带一点容差）的矩形，
并最多保留 32 个独立片段。超过该上限后，列表会折叠为单个
包围盒，因此病态的页面会退化为整窗重绘，而不是
无限制的记账。

### 2. 保持矩形准确

`Ui` 使用控件的绘制边界来决定某个控件是否与
脏矩形相交。矩形过期的控件会留下残留像素或
重绘错误的区域。在 `Host::resize` 中重新计算每一个矩形（布局可以运行的唯
一地方），并调用 `Ui::set_rect`；矩形变化是
应用发出的“页面上移动了”的信号。

对于垂直流，`bamboo_native_ui::layout` 中的辅助函数（`VStack`、
`HStack`、`padded`、`split_h`、`split_v`、`grid`）是纯粹的矩形
算术。在调整尺寸时使用它们，绝不要在 `paint` 中使用。

### 3. 仅在动画时使用动画计时器

没有帧循环。动画通过安装一个计时器并只使
动画区域失效：

```rust
use std::time::Duration;
use bamboo_native_ui::{Host, Rect, WindowHandle};

impl Host for App {
    fn started(&mut self, window: &mut WindowHandle<'_>) {
        window.set_animation_timer(Some(Duration::from_millis(16)));
    }

    fn timer(&mut self, window: &mut WindowHandle<'_>) {
        self.phase = (self.phase + 0.08) % 1.0;
        self.ui.set_progress(bamboo_native_ui::WidgetId::new("meter"), self.phase);
        self.ui.flush_damage(window);
    }
}
```

动画结束时用 `set_animation_timer(None)` 停止计时器。没有计时器且没有输入的
窗口完全不绘制。旋转指示器是
典型的例外：它在可见期间保持一个计时器，并在
应用隐藏它时停止。

### 4. 显式标记应用拥有的重绘

大多数更改都经过 setter。当应用绘制容器
无法观察到的东西时（控件旁应用拥有的覆盖层、
合成效果），请自行记录控件或矩形：

```rust
use bamboo_native_ui::{Rect, Ui, WidgetId};

fn damage_hud(ui: &mut Ui<()>, window: &mut bamboo_native_ui::WindowHandle<'_>) {
    ui.mark_dirty(WidgetId::new("hud"));
    window.invalidate(Rect::from_xywh(16.0, 16.0, 180.0, 48.0));
    ui.flush_damage(window);
}
```

`Ui::mark_dirty` 记录控件当前的边界并返回它是否
记录了任何内容（隐藏和零尺寸控件不记录任何内容）。
`WindowHandle::invalidate` 接受一个矩形；`invalidate_all` 用于页面
切换、调整尺寸或主题更改。`invalidate_force` 适用于
覆盖窗口的某物刚刚消失而窗口仍被视为
被遮挡的情况。

### 5. 只在控件矩形内绘制

宿主从后备缓冲区 blit 每个脏片段；它不会为你
将每个控件裁剪到其矩形。绘制到 `rect` 之外的控件要么在脏
边界处被裁剪，要么留下陈旧像素。控件也不得
假定其矩形之外的像素是当前的：另一个控件可能已经
独立地重绘了自己的区域。

绘制昂贵整表面效果（半透明遮罩）的控件
必须将其限制在 `WidgetPaint::dirty` 内；为一次
小的失效而重绘整个表面很慢，并且对于 alpha 而言，当片段重叠时会明显错误。

### 6. 不要在 `paint` 中分配

控件绘制代码不得分配。文本是借用的，脏矩形是
借用的，临时值属于控件或容器。常见的
违规是每帧构建一个 `String`（`format!`、
`to_string()`）来绘制文本，以及在绘制期间将行收集到 `Vec` 中。

绘制器本身没有每帧状态。后端在窗口的整个生命周期内
进行缓存：

| 缓存 | 内容 |
|---|---|
| 画刷 | 每个实色一个 GDI 画刷。 |
| 字体 | 每个字面/字号/字重一个字体；在主题或 DPI 更改时丢弃。 |
| 图像 | 以 `ImageId` 为键的转换后 BGRA 像素，16 MiB 预算。 |
| 圆角遮罩 | 以像素尺寸、形状和颜色为键的抗锯齿圆角遮罩，8 MiB 预算。 |
| 覆盖图 | 每个像素一字节，由某几何的所有颜色共享，4 MiB 预算。 |

`Painter::set_theme` 返回文本样式是否改变；窗口用
它来丢弃缓存的字体。`Host::dpi_changed` 同样在字体
缓存被丢弃之后到达。非常大的圆角填充（超过约 100 万设备像素）
会回退到组合矩形和椭圆，而不是构建遮罩，因此
当抗锯齿边缘很重要时，避免巨大的圆角表面。

图像是页面作用域的，以 `ImageId` 为键。再次注册相同的 id
会替换像素；只要 id 存在，后端缓存就会复用转换后的图像。
不要每帧创建一个新的 `ImageId`。

### 7. 剔除滚动内容

`Ui::paint` 将滚动视图的子项裁剪到视口，并跳过边界不与其相交的
子项，因此一个 600 行的列表只绘制可见
行和一个预读行。列表控件也只在实际悬停
改变时重绘：在一行内移动或沿已悬停的滚动条移动
不会使列表失效。

对于比窗口更长的页面，将内容放入 `ScrollView` 而不是
手工偏移每个控件；面板负责裁剪、剔除、滚轮
路由和焦点显现。

### 8. 审慎选择渲染器

`Renderer::Gdi` 是默认值，也是基线。`d2d-software` 和
`d2d-hardware` 通过 `ID2D1DCRenderTarget` 绘制到同一个后备缓冲区；
硬件会为 D3D 设备付出代价（数十 MB 私有内存），仅在
大量文本/形状页面时才值得。无法启动的 Direct2D 模式会在
`BAMBOO_UI_DEBUG` 下记录日志并永久回退到 GDI。

用 `WindowBuilder::renderer`、`BAMBOO_UI_RENDERER` 或 showcase 的
`--renderer` 标志选择。

## 测量

```powershell
# 固定时长运行：帧数、tick 数、失效次数、平均绘制时间、
# 私有字节数以及实际活动的渲染器。
cargo run -p bamboo-native-ui-showcase -- --bench 3

# 绘制一次，然后退出 0（同时会执行一次原生编辑会话）。
cargo run -p bamboo-native-ui-showcase -- --self-test

# 逐次绘制诊断：启用 debug-form 时，任何慢于 1.5 ms 的控件
# 会连同其 id 和矩形一起打印；showcase 会额外输出一行帧信息，
# 包含脏/控件计数和微秒数。
$env:BAMBOO_UI_PAINT_LOG=1; cargo run -p bamboo-native-ui-showcase

# 来自被遮挡窗口的帧/失效突发是预期行为：遮挡会
# 抑制绘制，而显露重绘会增加失效。请在没有遮挡的桌面上
# 进行同类对比。
```

`bamboo_native_ui::diagnostics::private_bytes()` 返回进程的私有
提交字节数（非 Windows 上为 `None`）；showcase 在运行后打印它。

## 常见错误

- **每次计时器 tick 都调用 `invalidate_all`** —— 那会破坏脏
  矩形。请使动画矩形失效。
- **每个 setter 之后都调用 `flush_damage`** —— 每个回调一次刷新就
  足够了；中间刷新会产生额外的重绘请求。
- **忘记 `flush_damage`** —— 状态改变了，但控件看起来
  是陈旧的，直到别的什么使它失效。
- **在 `paint` 中重新计算布局** —— 布局属于 `resize`（或一次
  页面重建）；绘制必须是当前状态的纯函数。
- **在 `paint` 中用 `format!` 构建文本** —— 在值改变时
  将格式化后的字符串存储在控件上。
- **绘制到 `rect` 之外** —— 容器 blit 的是脏片段，而不是你
  想要的形状。
- **假定控件在弹出层下会绘制** —— 覆盖层在第二遍绘制，
  被覆盖的滚动子项会被完全跳过。
- **为慢速进度条使用 60 Hz 计时器** —— 每秒更新一次，并在
  没有动画时停止计时器。

## 另请参阅

- [设计原则](../overview/design-principles.md)
- [架构](../overview/architecture.md)
- [Ui 容器](../api/ui-container.md)
- [绘制与 Canvas](../api/painting-and-canvas.md)
- [滚动](../api/scrolling.md)
- [调试](debugging.md)
- [特性开关](feature-flags.md)
