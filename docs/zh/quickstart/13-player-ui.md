# 13 — 播放器 UI

> 将封面墙、播放列表、播放控制条和波形拖动条组合成
> 一个媒体播放器页面。

## 你将构建

一个分为侧边栏和主区域的播放器页面。侧边栏持有
一个展示渐变封面的 `CoverWall`、一个 `PlaybackTransport`（随机、上一曲、
播放/暂停、下一曲、循环）和一个 `WaveformSeekBar`；主区域持有一个
`PlaylistView`。打开一个封面或双击一首曲目使其成为当前曲目；
播放控制条切换应用状态。没有解码任何内容：波形是
合成的，封面是注册的渐变。

控件报告意图；`current`、`playing`、`shuffle`、`repeat` 和 `progress`
存在于 `App` 中并通过 `set_*` 推回，因此 UI 不会漂移。

## 前置条件

- [03](03-first-window.md) 中的项目可以构建并运行；无需额外特性。

## 本节新术语

- **封面墙** —— 由方形专辑封面瓦片组成的网格。打开一个瓦片（双击
  或 Enter）会报告该瓦片的索引。
- **播放列表** —— 可滚动的曲目列表，每首一个 `Row`，当前曲目
  会高亮。
- **播放控制条** —— 那一条播放按钮：随机、上一曲、播放/暂停、
  下一曲、循环。
- **波形拖动条** —— 绘制采样振幅、并报告用户在哪里按下或拖动的条。
- **行** —— 列表数据的一行：标题、副标题、尾部文本和一个
  `current` 标志。
- **应用状态** —— 由应用而不是控件保存 `current`、
  `playing`、`shuffle`、`repeat` 和 `progress`。控件报告意图；应用
  决定新值并把它推回去，因此显示不会偏离
  真实状态。
- **推送 / setter** —— 一个 `set_*` 方法，把应用状态复制进控件，并
  记录需要重绘的脏矩形。
- **采样 / 振幅** —— 合成波形中一根竖条的高度，范围 `0..=1`。
- **进度** —— 播放位置，用从 `0.0` 到 `1.0` 的比例表示。

本页所有 Rust 代码都写入 crate 的 `src/main.rs`
（`bamboo-hello/bamboo-hello/src/main.rs`）。

## 步骤

### 1. 定义状态和曲目数据

```rust
use bamboo_native_ui::widgets::{
    Cover, CoverWall, PlaybackTransport, PlaylistView, RepeatMode, TransportAction,
    WaveformSeekBar,
};
use bamboo_native_ui::{
    Color, Host, ImageId, Rect, Row, Size, Ui, UiImage, WidgetId, WindowHandle,
};

#[derive(Clone, Copy, Debug, PartialEq)]
enum Msg {
    // 控件意图：打开/激活某个索引、一个播放控制按钮、一个拖动比例。
    CoverOpen(usize), TrackActivate(usize),
    Transport(TransportAction), Seek(f32),
}

struct App {
    ui: Ui<Msg>,
    tracks: Vec<Row>,
    current: usize, // 正在播放的曲目索引
    playing: bool,  // 所有播放状态都在这里，而不在控件里
    shuffle: bool,
    repeat: RepeatMode,
    progress: f32, // 0.0 ..= 1.0
}

fn build_tracks() -> Vec<Row> {
    let data = [
        ("Nocturne", "Jay Chou", "4:58"),
        ("Morning Mist", "Lin Xiangwan", "3:42"),
        ("Bamboo Song", "Shakuhachi Club", "5:06"),
        ("Breeze", "Seaside Poet", "2:48"),
    ];
    data.iter().enumerate()
        // 在任何点击之前把第一首标记为当前曲目。
        .map(|(i, (t, a, d))| Row::new(*t).subtitle(*a).trailing(*d).current(i == 0))
        .collect()
}
```

`Row` 携带 `title`、`subtitle`、`trailing`、`current` 和 `disabled`。

### 2. 注册封面并构建控件

封面按数字引用页面图像。每一对色调注册一个垂直渐变；
`ImageId(COVER_BASE + index)` 是每个 `Cover` 使用的数字。

```rust
const COVER_BASE: u64 = 100; // 第一张封面图像的 id；id 从这里向上计
const COVER_TINTS: [(u32, u32); 4] = [
    (0x5B6BA8, 0x222B4C), (0x3F7F6E, 0x18302B),
    (0xA86A4A, 0x3D2118), (0x7A4B82, 0x2B1833),
];

impl App {
    fn new() -> Self {
        let tracks = build_tracks();
        let covers: Vec<Cover> = tracks.iter().enumerate().map(|(i, row)| {
            let mut cover = Cover::new(row.title.clone()).subtitle(row.subtitle.clone());
            // 最后两张封面不设图像，以展示回退效果。
            if i + 2 < tracks.len() {
                cover = cover.image(ImageId(COVER_BASE + (i % 4) as u64));
            }
            cover
        }).collect();

        let mut ui = Ui::new();
        // 在控件使用之前，为每一对色调注册一个渐变。
        for (index, (top, bottom)) in COVER_TINTS.iter().enumerate() {
            ui.set_image(UiImage::gradient(ImageId(COVER_BASE + index as u64),
                160, 160, Color::from_hex(*top), Color::from_hex(*bottom)));
        }
        ui.add(CoverWall::new("wall").covers(covers).tile(148.0).gap(14.0)
            .corner_radius(6.0).on_open(Msg::CoverOpen));
        // 双击或 Enter 激活一行；单击只做选择。
        ui.add(PlaylistView::new("list").rows(tracks.clone()).row_height(40.0)
            .alternating(true).on_activate(Msg::TrackActivate));
        // 从 `App` 拥有的状态开始。
        ui.add(PlaybackTransport::new("transport", Msg::Transport)
            .playing(false).shuffle(false).repeat(RepeatMode::Off));
        // 合成波形：160 个 0..=1 的振幅，没有解码任何内容。
        ui.add(WaveformSeekBar::new("seek", 0.0, Msg::Seek).samples(
            (0..160).map(|i| {
                let i = i as f32;
                (0.5 + 0.28 * (i * 0.53).sin() + 0.18 * (i * 1.31).sin())
                    .abs().clamp(0.04, 1.0)
            }),
        ).duration(298.0));
        Self {
            ui, tracks, current: 0, playing: false,
            shuffle: false, repeat: RepeatMode::Off, progress: 0.0,
        }
    }
}
```

`on_open` 在双击或 Enter 时触发，因此单击只选择；
最后两个封面显示回退。`samples` 是 `0..=1` 范围内的振幅；
`duration` 只影响无障碍的 `m:ss` 文本；`on_seek` 接收按下
和拖拽位置。添加 `.on_drag_start`/`.on_drag`/`.on_drag_end` 以进行拖拽。

**刚才发生了什么** —— `new` 读取了曲目数据，在数字 id 下注册了四个渐变，
然后用每个封面各自色调对应的 id 构建了它们（最后两个则没有 id）。
控件从 `App` 状态的副本开始；控件树里没有任何东西是播放状态的
真实来源。

### 3. 布局侧边栏和主区域

```rust
impl Host for App {
    fn resize(&mut self, size: Size, _dpi: f32, window: &mut WindowHandle<'_>) {
        let (pad, gap) = (16.0, 12.0); // 以 DIP 为单位的间距
        let (transport_h, seek_h) = (44.0, 56.0);
        // 侧边栏随窗口增长，但保持在 260 到 380 DIP 之间。
        let side = (size.width * 0.38).clamp(260.0, 380.0);
        let column = (size.height - 2.0 * pad).max(200.0);
        let wall_h = (column - transport_h - seek_h - 2.0 * gap).max(120.0);
        let main_x = pad + side + pad;
        let main_w = (size.width - main_x - pad).max(160.0);
        let list_y = pad + 40.0;
        // 放置四个控件：封面墙、控制条、拖动条、播放列表。
        for (id, rect) in [
            ("wall", Rect::from_xywh(pad, pad, side, wall_h)),
            ("transport", Rect::from_xywh(pad, pad + wall_h + gap, side, transport_h)),
            ("seek", Rect::from_xywh(pad, pad + wall_h + gap + transport_h, side, seek_h)),
            ("list", Rect::from_xywh(main_x, list_y, main_w,
                (size.height - pad - list_y).max(120.0))),
        ] {
            self.ui.set_rect(WidgetId::new(id), rect);
        }
        window.invalidate_all();
    }
}
```

`paint`/`event` 是第 03 页的；`event` 取出到 `apply`，然后 `flush_damage`。

### 4. 用控件 setter 推回状态

```rust
impl App {
    fn push_rows(&mut self) {
        // 重建行并交回去；列表自己从不编辑它们。
        self.ui.set_rows(WidgetId::new("list"), self.tracks.clone());
    }

    fn push_transport(&mut self) {
        let (p, s, r) = (self.playing, self.shuffle, self.repeat);
        // `|`（而不是 `||`）会运行全部三个 setter；任何变化都标记脏矩形。
        self.ui.edit_widget::<PlaybackTransport<Msg>>(WidgetId::new("transport"),
            |strip| strip.set_playing(p) | strip.set_shuffle(s) | strip.set_repeat(r));
    }

    fn push_progress(&mut self) {
        let progress = self.progress;
        // `set_progress` 记录脏矩形，但从不发出消息。
        self.ui.edit_widget::<WaveformSeekBar<Msg>>(WidgetId::new("seek"),
            |bar| { bar.set_progress(progress); true });
    }

    fn select_track(&mut self, index: usize) {
        if self.tracks.is_empty() { return; }
        self.current = index.min(self.tracks.len() - 1);
        // 恰好有一行带有 `current = true`。
        for (i, row) in self.tracks.iter_mut().enumerate() { row.current = i == self.current; }
        self.progress = 0.0; // 新曲目从开头开始
        self.push_rows();
        self.push_transport();
        self.push_progress();
    }
}
```

每个 setter 报告更改，因此更新合并为一次重绘；`set_progress` 从不发出。

**刚才发生了什么** —— 每个辅助函数都把 `App` 状态中的一部分复制进对应的
控件，并让 setter 记录脏矩形。`select_track` 是移动 `current` 的唯一
地方，所以封面墙、列表和波形会在下一次重绘时一起
变化。

### 5. 处理消息

```rust
impl App {
    fn apply(&mut self, message: Msg, window: &mut WindowHandle<'_>) {
        match message {
            Msg::CoverOpen(index) => self.select_track(index),
            // 激活一行既开始播放，也选中它。
            Msg::TrackActivate(index) => { self.playing = true; self.select_track(index); }
            Msg::Transport(action) => {
                // 控件报告点击；由 `App` 决定新状态。
                match action {
                    TransportAction::Shuffle => self.shuffle = !self.shuffle,
                    TransportAction::PlayPause => self.playing = !self.playing,
                    TransportAction::Repeat => self.repeat = self.repeat.next(),
                    TransportAction::Next => self.step(1),
                    TransportAction::Previous => self.step(-1),
                }
                self.push_transport();
            }
            Msg::Seek(position) => {
                // 把条的位置钳制到它自己的 0..=1 范围内。
                self.progress = position.clamp(0.0, 1.0);
                self.push_progress();
            }
        }
        self.ui.flush_damage(window);
    }

    fn step(&mut self, delta: isize) {
        if self.tracks.is_empty() { return; }
        let len = self.tracks.len() as isize;
        self.playing = true;
        // `rem_euclid` 在播放列表的两端都会回绕。
        self.select_track((self.current as isize + delta).rem_euclid(len) as usize);
    }
}
```

`RepeatMode::next` 遍历 Off → All → One → Off；控件只报告
动作，应用推回结果。从解码器驱动进度并调用
`push_progress`（四分之一秒的定时器对演示可行）。

### 6. 运行它

从 crate 文件夹运行（`bamboo-hello/bamboo-hello`，也就是存放
`src/main.rs` 的那个）。

```powershell
# 工作目录：bamboo-hello/bamboo-hello
cargo run
```

窗口打开，左侧是侧边栏（封面墙、控制条、波形），
右侧是播放列表。双击第一个封面或播放列表中的一行：
当前行移动，波形回到开头。构建错误会指出出错的行，
且不会打开窗口。

**刚才发生了什么** —— `run` 编译了这个页面并打开窗口；`resize`
计算了四个矩形，控件用 `App` 交给它们的数据完成绘制。从这时起，
每次点击都是一条意图消息：`apply` 更新 `App`，`push_*` 辅助函数把结果
送回去，`flush_damage` 只重绘发生变化的矩形。

## 预期结果

运行 `cargo run`。双击一个封面：播放列表的 `current` 标志移动
且波形重置。播放控制条切换播放状态。

- 左列含封面墙、控制条和波形；播放列表填充
  右侧。
- 前两个封面有渐变封面；后两个使用回退瓦片。
- 播放/暂停、随机和循环反映 `App` 状态，而不是最后一次点击。
- 拖动寻址时绘制预览，`set_progress` 从
  已提交位置继续。

## 故障排查

- **窗口空白，或所有控件都堆在左上角** —— `resize`
  没有设置矩形。实现 `Host::resize`，并像第 3 步那样为
  `wall`、`transport`、`seek` 和 `list` 调用 `set_rect`。
- **封面墙是空的** —— `covers` 向量里没有条目，或封面墙
  没有矩形。四个 `Cover` 值由 `tracks` 构建；检查列表
  是否为空。
- **单击封面不会切换曲目** —— `on_open` 在双击或 Enter 时触发；
  单击只选中该瓦片。
- **波形是一条平线** —— `samples` 迭代器为空，或返回的振幅
  接近零。它们是 `0..=1` 的比例；`duration` 只影响
  无障碍文本。
- **在波形上拖动没有反应** —— 寻址消息没有接上：
  `WaveformSeekBar::new("seek", 0.0, Msg::Seek)` 把寻址回调作为
  第三个参数。
- **`ui.set_progress` 不移动波形** —— 使用 `ui.set_value` 或
  `edit_widget` + `bar.set_progress`；该控件实现了 `set_value`。
- **当前行不保持高亮** —— 应用拥有
  `Row::current`。更改后重建行并调用 `ui.set_rows`。
- **封面全是回退** —— 传给 `Cover::image` 的 `ImageId` 没有
  用 `Ui::set_image` 注册。
- **控制条在点击后弹回** —— 将动作应用到 `App`，然后推回它。
- **拖拽时波形跳动** —— 在 `is_dragging()` 时跳过播放滴答。

## 下一步

[12 — 文本编辑与输入法](12-text-editing-and-ime.md) 涵盖播放器
搜索和设置所需的字段；API 参考始于
[宿主与窗口](../api/host-and-window.md)。
