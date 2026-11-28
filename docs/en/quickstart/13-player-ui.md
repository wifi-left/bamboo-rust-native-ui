# 13 — Player UI

> Compose a cover wall, a playlist, a transport strip and a waveform seek bar
> into one media-player page.

## What you will build

A player page split into a sidebar and a main area. The sidebar holds a
`CoverWall` of gradient artwork, a `PlaybackTransport` (shuffle, previous,
play/pause, next, repeat) and a `WaveformSeekBar`; the main area holds a
`PlaylistView`. Opening a cover or double-clicking a track makes it current; the
transport toggles application state. Nothing is decoded: the waveform is
synthetic and the covers are registered gradients.

Widgets report intent; `current`, `playing`, `shuffle`, `repeat` and `progress`
live in `App` and are pushed back with `set_*`, so the UI cannot drift.

## Prerequisites

- The project from [03](03-first-window.md) builds and runs; no extra feature is needed.

## New terms on this page

- **Cover wall** — a grid of square album-art tiles. Opening one (double click
  or Enter) reports the tile's index.
- **Playlist view** — a scrollable list of tracks, one `Row` each, with the
  current track highlighted.
- **Transport** — the strip of playback buttons: shuffle, previous, play/pause,
  next, repeat.
- **Waveform seek bar** — a bar that draws sample amplitudes and reports where
  the user pressed or dragged.
- **Row** — one line of list data: title, subtitle, trailing text and a
  `current` flag.
- **Application-owned state** — the app, not the widget, stores `current`,
  `playing`, `shuffle`, `repeat` and `progress`. Widgets report intent; the app
  decides the new value and pushes it back, so the display cannot drift from the
  real state.
- **Push / setter** — a `set_*` method that copies app state into a widget and
  records the damage to repaint.
- **Sample / amplitude** — one bar height in the synthetic waveform, in `0..=1`.
- **Progress** — the playback position as a fraction from `0.0` to `1.0`.

All Rust on this page goes into the crate's `src/main.rs`
(`bamboo-hello/bamboo-hello/src/main.rs`).

## Steps

### 1. Define the state and the track data

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
    // Widget intents: open/activate an index, a transport button, a seek fraction.
    CoverOpen(usize), TrackActivate(usize),
    Transport(TransportAction), Seek(f32),
}

struct App {
    ui: Ui<Msg>,
    tracks: Vec<Row>,
    current: usize, // index of the playing track
    playing: bool,  // all playback state lives here, not in the widgets
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
        // Mark the first track as current before any click.
        .map(|(i, (t, a, d))| Row::new(*t).subtitle(*a).trailing(*d).current(i == 0))
        .collect()
}
```

`Row` carries `title`, `subtitle`, `trailing`, `current` and `disabled`.

### 2. Register covers and build the widgets

Covers reference page images by number. Register one vertical gradient per tint
pair; `ImageId(COVER_BASE + index)` is the number every `Cover` uses.

```rust
const COVER_BASE: u64 = 100; // first cover image id; ids count up from here
const COVER_TINTS: [(u32, u32); 4] = [
    (0x5B6BA8, 0x222B4C), (0x3F7F6E, 0x18302B),
    (0xA86A4A, 0x3D2118), (0x7A4B82, 0x2B1833),
];

impl App {
    fn new() -> Self {
        let tracks = build_tracks();
        let covers: Vec<Cover> = tracks.iter().enumerate().map(|(i, row)| {
            let mut cover = Cover::new(row.title.clone()).subtitle(row.subtitle.clone());
            // Leave the last two covers without an image to show fallbacks.
            if i + 2 < tracks.len() {
                cover = cover.image(ImageId(COVER_BASE + (i % 4) as u64));
            }
            cover
        }).collect();

        let mut ui = Ui::new();
        // Register one gradient per tint pair before the widgets use them.
        for (index, (top, bottom)) in COVER_TINTS.iter().enumerate() {
            ui.set_image(UiImage::gradient(ImageId(COVER_BASE + index as u64),
                160, 160, Color::from_hex(*top), Color::from_hex(*bottom)));
        }
        ui.add(CoverWall::new("wall").covers(covers).tile(148.0).gap(14.0)
            .corner_radius(6.0).on_open(Msg::CoverOpen));
        // Double click or Enter activates a row; a single click only selects.
        ui.add(PlaylistView::new("list").rows(tracks.clone()).row_height(40.0)
            .alternating(true).on_activate(Msg::TrackActivate));
        // Start from the state `App` owns.
        ui.add(PlaybackTransport::new("transport", Msg::Transport)
            .playing(false).shuffle(false).repeat(RepeatMode::Off));
        // Synthetic waveform: 160 amplitudes in 0..=1, nothing is decoded.
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

`on_open` fires on a double click or Enter, so a single click only selects;
the last two covers show the fallback. `samples` are amplitudes in `0..=1`;
`duration` affects only the accessible `m:ss` text; `on_seek` receives press
and drag positions. Add `.on_drag_start`/`.on_drag`/`.on_drag_end` for scrubs.

**What just happened** — `new` read the track data, registered four gradients
under numeric ids, then built each cover with the id for its tint (and left the
last two without one). The widgets started from copies of `App`'s state; nothing
in the widget tree is the source of truth for playback.

### 3. Lay out a sidebar and a main area

```rust
impl Host for App {
    fn resize(&mut self, size: Size, _dpi: f32, window: &mut WindowHandle<'_>) {
        let (pad, gap) = (16.0, 12.0); // spacing in DIPs
        let (transport_h, seek_h) = (44.0, 56.0);
        // Sidebar grows with the window but stays between 260 and 380 DIPs.
        let side = (size.width * 0.38).clamp(260.0, 380.0);
        let column = (size.height - 2.0 * pad).max(200.0);
        let wall_h = (column - transport_h - seek_h - 2.0 * gap).max(120.0);
        let main_x = pad + side + pad;
        let main_w = (size.width - main_x - pad).max(160.0);
        let list_y = pad + 40.0;
        // Place the four widgets: wall, transport, seek bar, playlist.
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

`paint`/`event` are page 03's; `event` drains into `apply` then `flush_damage`.

### 4. Push state back with the widget setters

```rust
impl App {
    fn push_rows(&mut self) {
        // Rebuild and hand the rows back; the list never edits them itself.
        self.ui.set_rows(WidgetId::new("list"), self.tracks.clone());
    }

    fn push_transport(&mut self) {
        let (p, s, r) = (self.playing, self.shuffle, self.repeat);
        // `|` (not `||`) runs all three setters; any change marks damage.
        self.ui.edit_widget::<PlaybackTransport<Msg>>(WidgetId::new("transport"),
            |strip| strip.set_playing(p) | strip.set_shuffle(s) | strip.set_repeat(r));
    }

    fn push_progress(&mut self) {
        let progress = self.progress;
        // `set_progress` records damage but never emits a message.
        self.ui.edit_widget::<WaveformSeekBar<Msg>>(WidgetId::new("seek"),
            |bar| { bar.set_progress(progress); true });
    }

    fn select_track(&mut self, index: usize) {
        if self.tracks.is_empty() { return; }
        self.current = index.min(self.tracks.len() - 1);
        // Exactly one row carries `current = true`.
        for (i, row) in self.tracks.iter_mut().enumerate() { row.current = i == self.current; }
        self.progress = 0.0; // a new track starts at the beginning
        self.push_rows();
        self.push_transport();
        self.push_progress();
    }
}
```

Each setter reports changes, so updates coalesce into one repaint; `set_progress` never emits.

**What just happened** — each helper copies one slice of `App` state into its
widget and lets the setter record damage. `select_track` is the single place
that moves `current`, so the cover wall, the list and the waveform all change
together on the next repaint.

### 5. Handle the messages

```rust
impl App {
    fn apply(&mut self, message: Msg, window: &mut WindowHandle<'_>) {
        match message {
            Msg::CoverOpen(index) => self.select_track(index),
            // Activating a row starts playback as well as selecting it.
            Msg::TrackActivate(index) => { self.playing = true; self.select_track(index); }
            Msg::Transport(action) => {
                // The widget reports the click; `App` decides the new state.
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
                // Clamp the bar's position to its own 0..=1 range.
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
        // `rem_euclid` wraps around at both ends of the playlist.
        self.select_track((self.current as isize + delta).rem_euclid(len) as usize);
    }
}
```

`RepeatMode::next` walks Off → All → One → Off; the widget only reports the
action and the application pushes the result back. Drive progress from the
decoder and call `push_progress` (a quarter-second timer works for a demo).

### 6. Run it

Run from the crate folder (`bamboo-hello/bamboo-hello`, the one holding
`src/main.rs`).

```powershell
# Working directory: bamboo-hello/bamboo-hello
cargo run
```

The window opens with the sidebar on the left (cover wall, transport, waveform)
and the playlist on the right. Double-click the first cover or a playlist row:
the current row moves and the waveform returns to the start. A build error names
the failing line, and no window opens.

**What just happened** — `run` compiled the page and opened the window; `resize`
computed the four rectangles, and the widgets painted from the data `App` handed
them. From here every click is an intent message: `apply` updates `App`, the
`push_*` helpers send the result back, and `flush_damage` repaints only the
changed rectangles.

## Expected result

Run `cargo run`. Double-click a cover: the playlist's `current` flag moves and
the waveform resets. The transport toggles playback state.

- A left column with cover wall, transport and waveform; the playlist fills the
  right.
- Gradient artwork on the first two covers; the last two use fallback tiles.
- Play/pause, shuffle and repeat reflect `App` state, not the last click.
- Seeking paints a preview during the drag and `set_progress` continues from the
  committed position.

## Troubleshooting

- **The window is blank or every widget sits in the top-left corner** — `resize`
  did not set the rectangles. Implement `Host::resize` and call `set_rect` for
  `wall`, `transport`, `seek` and `list`, as step 3 does.
- **The cover wall is empty** — the `covers` vector has no entries, or the wall
  has no rectangle. Four `Cover` values are built from `tracks`; check that the
  list is not empty.
- **A single click on a cover does not change the track** — `on_open` fires on a
  double click or Enter; a single click only selects the tile.
- **The waveform is a flat line** — the `samples` iterator is empty or returns
  near-zero amplitudes. They are fractions in `0..=1`; `duration` only affects
  the accessible text.
- **Dragging across the waveform does nothing** — the seek message is not wired:
  `WaveformSeekBar::new("seek", 0.0, Msg::Seek)` takes the seek callback as its
  third argument.
- **`ui.set_progress` does not move the waveform** — use `ui.set_value` or
  `edit_widget` + `bar.set_progress`; the widget implements `set_value`.
- **The current row does not stay highlighted** — the application owns
  `Row::current`. Rebuild the rows and call `ui.set_rows` after changing it.
- **Covers are all fallbacks** — the `ImageId` passed to `Cover::image` was
  not registered with `Ui::set_image`.
- **The transport snaps back after a click** — apply the action to `App`, then push it back.
- **The waveform jumps while dragging** — skip playback ticks while
  `is_dragging()`.

## Next

[12 — Text editing and IME](12-text-editing-and-ime.md) covers the fields a
player needs for search and settings; the API reference starts at
[Host and window](../api/host-and-window.md).
