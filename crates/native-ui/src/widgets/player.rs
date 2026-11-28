// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Player transport composites: the waveform seek bar and the transport strip.

use crate::a11y::{A11yChild, A11yInfo, A11yRole};
use crate::backend::{TextAlign, TextStyle};
use crate::event::KeyCode;
use crate::geometry::{Point, Rect};
use crate::theme::radius;
use crate::skin::{SkinAttrDecl, SkinValue, SkinValueKind};
use crate::widget::{
    TextStyleRole, Widget, WidgetId, WidgetInput, WidgetPaint, WidgetState,
};

use crate::color::Color;

static TRANSPORT_SKIN_ATTRS: &[SkinAttrDecl] = &[
    SkinAttrDecl::new("playing", SkinValueKind::Bool),
    SkinAttrDecl::new("shuffle", SkinValueKind::Bool),
    SkinAttrDecl::new("repeat", SkinValueKind::Text),
];

/// A seek bar drawn as a waveform.
///
/// The samples are normalized amplitudes (`0..=1`), one bar per slot across
/// the widget; the played part is accent-colored and the rest muted, with a
/// playhead at the current position. A press seeks, a drag follows (each move
/// emits, so the player scrubs), and the keyboard steps in adjust mode.
/// Without samples it paints a flat center line, so the control still reads as
/// a seek bar before the decoder delivers a waveform.
pub struct WaveformSeekBar<M> {
    id: WidgetId,
    rect: Rect,
    samples: Vec<f32>,
    /// Current position `0..=1` (application-owned; the drag shows a preview).
    progress: f32,
    dragging: bool,
    preview: f32,
    hover: Option<Point>,
    /// Width of one bar and the space after it, in DIPs.
    bar_width: f32,
    gap: f32,
    /// `(played, unplayed)`; `None` resolves from the palette.
    colors: Option<(Color, Color)>,
    /// Track length in seconds, used for the accessible value text only.
    duration: Option<f32>,
    on_seek: Box<dyn Fn(f32) -> M>,
    /// Pointer-scrub lifecycle observers; see [`Slider`](super::controls::Slider).
    on_drag_start: Option<Box<dyn Fn(f32) -> M>>,
    on_drag: Option<Box<dyn Fn(f32) -> M>>,
    on_drag_end: Option<Box<dyn Fn(f32) -> M>>,
    /// A drag observer's message reported beside the primary `on_seek`.
    extra: Option<M>,
}

impl<M> WaveformSeekBar<M> {
    /// Default keyboard/wheel step: 2 % of the track.
    pub const STEP: f32 = 0.02;

    pub fn new(id: &'static str, progress: f32, on_seek: impl Fn(f32) -> M + 'static) -> Self {
        let progress = progress.clamp(0.0, 1.0);
        Self {
            id: WidgetId::new(id),
            rect: Rect::from_xywh(0.0, 0.0, 480.0, 56.0),
            samples: Vec::new(),
            progress,
            dragging: false,
            preview: progress,
            hover: None,
            bar_width: 2.0,
            gap: 1.0,
            colors: None,
            duration: None,
            on_seek: Box::new(on_seek),
            on_drag_start: None,
            on_drag: None,
            on_drag_end: None,
            extra: None,
        }
    }

    /// Normalized amplitudes, one per bar slot (extra samples are ignored,
    /// missing ones repeat from the start).
    pub fn samples(mut self, samples: impl IntoIterator<Item = f32>) -> Self {
        self.samples = samples.into_iter().map(|a| a.clamp(0.0, 1.0)).collect();
        self
    }

    pub fn bar_width(mut self, width: f32) -> Self {
        self.bar_width = width.clamp(1.0, 12.0);
        self
    }

    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = gap.clamp(0.0, 8.0);
        self
    }

    /// Explicit played/unplayed colors.
    pub fn colors(mut self, played: Color, unplayed: Color) -> Self {
        self.colors = Some((played, unplayed));
        self
    }

    /// Track length in seconds; the accessible value text becomes `m:ss`.
    pub fn duration(mut self, seconds: f32) -> Self {
        self.duration = (seconds > 0.0).then_some(seconds);
        self
    }

    pub fn set_samples(&mut self, samples: impl IntoIterator<Item = f32>) {
        self.samples = samples.into_iter().map(|a| a.clamp(0.0, 1.0)).collect();
    }

    pub fn samples_ref(&self) -> &[f32] {
        &self.samples
    }

    pub fn set_duration(&mut self, seconds: Option<f32>) {
        self.duration = seconds.filter(|seconds| *seconds > 0.0);
    }

    pub fn duration_value(&self) -> Option<f32> {
        self.duration
    }

    /// Position `0..=1` (typed access / application playback updates).
    pub fn progress(&self) -> f32 {
        self.progress
    }

    /// Update the position without emitting (playback progress).
    pub fn set_progress(&mut self, progress: f32) {
        self.progress = progress.clamp(0.0, 1.0);
        if !self.dragging {
            self.preview = self.progress;
        }
    }

    pub fn is_dragging(&self) -> bool {
        self.dragging
    }

    /// The position the bar paints while scrubbing (typed access); `None` when
    /// no drag is in progress.
    pub fn drag_preview(&self) -> Option<f32> {
        self.dragging.then_some(self.preview)
    }

    /// Called once when a scrub drag begins, with the position under the press.
    /// Additive: the built-in `on_seek` still fires.
    pub fn on_drag_start(mut self, handler: impl Fn(f32) -> M + 'static) -> Self {
        self.on_drag_start = Some(Box::new(handler));
        self
    }

    /// Called on every pointer move while scrubbing, with the position under
    /// the pointer. Additive: the built-in `on_seek` still fires.
    pub fn on_drag(mut self, handler: impl Fn(f32) -> M + 'static) -> Self {
        self.on_drag = Some(Box::new(handler));
        self
    }

    /// Called when the scrub drag ends, with the committed position.
    pub fn on_drag_end(mut self, handler: impl Fn(f32) -> M + 'static) -> Self {
        self.on_drag_end = Some(Box::new(handler));
        self
    }

    /// The position the bar paints (the drag preview while scrubbing).
    fn shown(&self) -> f32 {
        if self.dragging { self.preview } else { self.progress }
    }

    fn fraction_at(&self, x: f32) -> f32 {
        ((x - self.rect.left) / self.rect.width().max(1.0)).clamp(0.0, 1.0)
    }

    fn seek(&mut self, position: f32) -> Option<M> {
        let position = position.clamp(0.0, 1.0);
        self.preview = position;
        Some((self.on_seek)(position))
    }

    fn preview_at(&mut self, pos: Point) -> Option<M> {
        self.dragging = true;
        self.seek(self.fraction_at(pos.x))
    }

    /// Adjust by `step` (keyboard); no message when already at the edge. The
    /// position updates optimistically; the application's `set_progress` (or
    /// the next playback tick) confirms it.
    fn adjust(&mut self, delta: f32) -> Option<M> {
        let next = (self.preview + delta).clamp(0.0, 1.0);
        if (next - self.preview).abs() < 0.0005 {
            return None;
        }
        self.preview = next;
        self.progress = next;
        Some((self.on_seek)(next))
    }

    fn value_text(&self, position: f32) -> String {
        match self.duration {
            Some(duration) => {
                let total = (position * duration).max(0.0);
                let minutes = (total / 60.0).floor() as u32;
                let seconds = (total % 60.0).floor() as u32;
                format!("{minutes}:{seconds:02}")
            }
            None => format!("{:.0}%", position * 100.0),
        }
    }

    /// Colors for one bar: `(played, unplayed)`.
    fn bar_colors(&self, palette: &crate::theme::Palette) -> (Color, Color) {
        self.colors
            .unwrap_or((palette.accent, palette.muted.mix(palette.background, 0.35)))
    }
}

impl<M: 'static> Widget<M> for WaveformSeekBar<M> {
    fn content_caps(&self) -> crate::props::ContentCaps {
        crate::props::ContentCaps::VALUE
    }

    fn id(&self) -> WidgetId {
        self.id
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn rect(&self) -> Rect {
        self.rect
    }

    fn set_rect(&mut self, rect: Rect) {
        self.rect = rect;
    }

    fn preferred_height(&self, _width: f32) -> f32 {
        56.0
    }

    fn adjustable(&self) -> bool {
        true
    }

    fn repaint_on_move(&self) -> bool {
        true
    }

    fn set_value(&mut self, value: f32) {
        self.set_progress(value);
    }

    fn a11y(&self) -> Option<A11yInfo> {
        Some(
            A11yInfo::new(A11yRole::Slider, self.id.0)
                .range(0.0, 1.0, self.progress as f64)
                .value(self.value_text(self.progress)),
        )
    }

    fn a11y_set_value(&mut self, text: &str) -> Option<M> {
        let trimmed = text.trim();
        let raw = trimmed.trim_end_matches('%');
        let parsed: f32 = raw.parse().ok()?;
        // A percentage ("45%") or a value in the range ("0.45") both select
        // the same position; anything else is clamped into the range.
        let position = if trimmed.ends_with('%') {
            parsed / 100.0
        } else {
            parsed
        };
        let position = position.clamp(0.0, 1.0);
        self.progress = position;
        self.preview = position;
        Some((self.on_seek)(position))
    }

    fn paint(&self, ctx: &mut WidgetPaint<'_>, _state: &WidgetState) {
        let pal = ctx.palette;
        let (played, unplayed) = self.bar_colors(&pal);
        let center_y = self.rect.center().y;
        let amplitude = (self.rect.height() - 6.0).max(2.0) / 2.0;
        let step = self.bar_width + self.gap;
        let count = ((self.rect.width() + self.gap) / step).floor().max(1.0) as usize;
        // Center the strip when the width is not an exact multiple.
        let strip = count as f32 * step - self.gap;
        let left = self.rect.left + ((self.rect.width() - strip) / 2.0).max(0.0);
        let position = self.shown();
        let played_x = self.rect.left + self.rect.width() * position;

        if self.samples.is_empty() {
            // No waveform yet: one flat line so the seek target is visible.
            ctx.canvas.fill_rect(
                Rect::new(self.rect.left, center_y - 1.0, self.rect.right, center_y + 1.0),
                unplayed,
            );
        } else {
            for index in 0..count {
                let sample = self.samples[index % self.samples.len()];
                let height = (2.0 + sample * amplitude * 2.0).min(self.rect.height());
                let x = left + index as f32 * step;
                let rect = Rect::new(
                    x,
                    center_y - height / 2.0,
                    x + self.bar_width,
                    center_y + height / 2.0,
                );
                ctx.canvas.fill_rect(
                    rect,
                    if x < played_x { played } else { unplayed },
                );
            }
        }

        // Playhead: a full-height line with a round handle on the center line.
        let x = played_x.clamp(self.rect.left, self.rect.right);
        ctx.canvas.fill_rect(
            Rect::new(x - 1.0, self.rect.top, x + 1.0, self.rect.bottom),
            played,
        );
        let painter = ctx.painter;
        painter.round_rect(
            &mut *ctx.canvas,
            Rect::new(x - 3.0, center_y - 3.0, x + 3.0, center_y + 3.0),
            3.0,
            played,
        );

        // Hover marker: a faint line where a click would seek.
        if let Some(hover) = self.hover {
            let hx = hover.x.clamp(self.rect.left, self.rect.right);
            ctx.canvas
                .fill_translucent(Rect::new(hx - 1.0, self.rect.top, hx + 1.0, self.rect.bottom), pal.text, 0.25);
        }

        // The accessible value doubles as a status line while scrubbing.
        if self.dragging {
            let label = self.value_text(position);
            let width = 52.0;
            let rect = Rect::new(
                (x - width / 2.0).clamp(self.rect.left, self.rect.right - width),
                self.rect.top,
                (x - width / 2.0).clamp(self.rect.left, self.rect.right - width) + width,
                self.rect.top + 16.0,
            );
            ctx.canvas.fill_translucent(rect, pal.background, 0.85);
            ctx.text(
                rect,
                &label,
                TextStyleRole::Small,
                crate::backend::TextAlign::Center,
                pal.text,
            );
        }
    }

    fn input(&mut self, input: WidgetInput) -> Option<M> {
        match input {
            WidgetInput::HoverAt(pos) => {
                self.hover = Some(pos);
                None
            }
            WidgetInput::HoverLeave => {
                self.hover = None;
                None
            }
            WidgetInput::PressAt(pos) => {
                let message = self.preview_at(pos);
                if let Some(handler) = &self.on_drag_start {
                    self.extra = Some(handler(self.preview));
                }
                message
            }
            WidgetInput::DragTo(pos) => {
                if self.dragging {
                    let message = self.preview_at(pos);
                    if let Some(handler) = &self.on_drag {
                        self.extra = Some(handler(self.preview));
                    }
                    message
                } else {
                    None
                }
            }
            WidgetInput::ReleaseAt(_) => {
                if self.dragging {
                    self.dragging = false;
                    self.progress = self.preview;
                    return self.on_drag_end.as_ref().map(|handler| handler(self.progress));
                }
                None
            }
            WidgetInput::Key(key) => match key.code {
                KeyCode::Left => self.adjust(-Self::STEP),
                KeyCode::Right => self.adjust(Self::STEP),
                KeyCode::Home => self.adjust(-1.0),
                KeyCode::End => self.adjust(1.0),
                _ => None,
            },
            WidgetInput::Activate => self.adjust(0.0),
            _ => None,
        }
    }

    fn take_extra_message(&mut self) -> Option<M> {
        self.extra.take()
    }
}

/// What a [`PlaybackTransport`] button asks the application to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransportAction {
    Shuffle,
    Previous,
    PlayPause,
    Next,
    Repeat,
}

impl TransportAction {
    /// The cells in strip order, left to right.
    pub const ALL: [Self; 5] = [
        Self::Shuffle,
        Self::Previous,
        Self::PlayPause,
        Self::Next,
        Self::Repeat,
    ];

    /// The button's accessible name (the action it performs).
    pub const fn label(self) -> &'static str {
        match self {
            Self::Shuffle => "随机播放",
            Self::Previous => "上一曲",
            Self::PlayPause => "播放或暂停",
            Self::Next => "下一曲",
            Self::Repeat => "循环播放",
        }
    }

    /// MDL2 glyph. Play/pause and repeat depend on the widget state, so the
    /// widget resolves those separately.
    const fn glyph(self) -> char {
        match self {
            Self::Shuffle => '\u{E8B1}',
            Self::Previous => '\u{E100}',
            Self::PlayPause => '\u{E768}',
            Self::Next => '\u{E101}',
            Self::Repeat => '\u{E8EE}',
        }
    }
}

/// Repeat state the transport paints. The application owns the mode; the
/// widget only shows it, exactly like [`crate::Toggle`] owns its switch.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RepeatMode {
    #[default]
    Off,
    /// Repeat the playlist.
    All,
    /// Repeat the current track.
    One,
}

impl RepeatMode {
    /// The next mode in the Off → All → One → Off cycle.
    pub const fn next(self) -> Self {
        match self {
            Self::Off => Self::All,
            Self::All => Self::One,
            Self::One => Self::Off,
        }
    }

    /// Whether repeat is on (either kind).
    pub const fn active(self) -> bool {
        !matches!(self, Self::Off)
    }

    /// Short name for the accessible label and application status text.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Off => "关",
            Self::All => "列表",
            Self::One => "单曲",
        }
    }

    /// Parse the skin's `repeat` text (`off`, `all`, `one`).
    pub fn parse(text: &str) -> Option<Self> {
        match text.trim().to_ascii_lowercase().as_str() {
            "off" | "none" => Some(Self::Off),
            "all" | "list" => Some(Self::All),
            "one" | "single" => Some(Self::One),
            _ => None,
        }
    }
}

const TRANSPORT_CELLS: usize = TransportAction::ALL.len();

/// The player transport strip: shuffle, previous, play/pause, next and repeat.
///
/// One widget paints five icon buttons across its rectangle; a click (or
/// Enter/Space on the cursor cell) emits the matching [`TransportAction`]. The
/// widget never decides what should happen: `playing`, `shuffle` and `repeat`
/// are application state pushed back through the setters, so a failed playback
/// command leaves the strip showing the truth.
///
/// The keyboard model is a cursor, not a selection: focus the strip and
/// Left/Right/Home/End move the cursor, Enter/Space runs the cursor cell, and
/// Up/Down leave for the neighbouring widget. Assistive technology gets a
/// `ToolBar` of Button children, with the two toggles carrying Toggle state.
pub struct PlaybackTransport<M> {
    id: WidgetId,
    rect: Rect,
    playing: bool,
    shuffle: bool,
    repeat: RepeatMode,
    enabled: bool,
    /// Cell Enter/Space activates; also the keyboard focus ring.
    cursor: usize,
    hover: Option<usize>,
    pressed: Option<usize>,
    on_action: Box<dyn Fn(TransportAction) -> M>,
}

impl<M> PlaybackTransport<M> {
    pub fn new(id: &'static str, on_action: impl Fn(TransportAction) -> M + 'static) -> Self {
        Self {
            id: WidgetId::new(id),
            rect: Rect::from_xywh(0.0, 0.0, 300.0, 44.0),
            playing: false,
            shuffle: false,
            repeat: RepeatMode::Off,
            enabled: true,
            cursor: 2,
            hover: None,
            pressed: None,
            on_action: Box::new(on_action),
        }
    }

    /// Initial playing state (the play cell shows pause while `true`).
    pub fn playing(mut self, playing: bool) -> Self {
        self.playing = playing;
        self
    }

    pub fn shuffle(mut self, on: bool) -> Self {
        self.shuffle = on;
        self
    }

    pub fn repeat(mut self, mode: RepeatMode) -> Self {
        self.repeat = mode;
        self
    }

    /// Application playback update. Returns whether anything changed.
    pub fn set_playing(&mut self, playing: bool) -> bool {
        if self.playing == playing {
            return false;
        }
        self.playing = playing;
        true
    }

    pub fn set_shuffle(&mut self, on: bool) -> bool {
        if self.shuffle == on {
            return false;
        }
        self.shuffle = on;
        true
    }

    pub fn set_repeat(&mut self, mode: RepeatMode) -> bool {
        if self.repeat == mode {
            return false;
        }
        self.repeat = mode;
        true
    }

    pub fn is_playing(&self) -> bool {
        self.playing
    }

    pub fn shuffle_enabled(&self) -> bool {
        self.shuffle
    }

    pub fn repeat_mode(&self) -> RepeatMode {
        self.repeat
    }

    /// Keyboard cursor cell (typed access / diagnostics).
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Move the cursor without emitting. Returns whether it moved.
    pub fn set_cursor(&mut self, index: usize) -> bool {
        let index = index.min(TRANSPORT_CELLS - 1);
        if self.cursor == index {
            return false;
        }
        self.cursor = index;
        true
    }

    fn cell_width(&self) -> f32 {
        self.rect.width() / TRANSPORT_CELLS as f32
    }

    fn cell_rect(&self, index: usize) -> Rect {
        let left = self.rect.left + index as f32 * self.cell_width();
        Rect::new(left, self.rect.top, left + self.cell_width(), self.rect.bottom)
    }

    /// The painted button square inside a cell: centered, inset from the cell
    /// edges so neighbouring hover tints keep a gap.
    fn button_rect(&self, index: usize) -> Rect {
        let cell = self.cell_rect(index);
        let side = (cell.height() - 8.0).min(cell.width() - 8.0).max(16.0);
        let center = cell.center();
        Rect::new(
            center.x - side * 0.5,
            center.y - side * 0.5,
            center.x + side * 0.5,
            center.y + side * 0.5,
        )
    }

    fn cell_at(&self, pos: Point) -> Option<usize> {
        if !self.rect.contains_point(pos) {
            return None;
        }
        let index = ((pos.x - self.rect.left) / self.cell_width().max(1.0)).floor();
        Some((index.max(0.0) as usize).min(TRANSPORT_CELLS - 1))
    }

    fn action_at(&self, index: usize) -> Option<TransportAction> {
        TransportAction::ALL.get(index).copied()
    }

    /// Run one cell: move the cursor there and emit its action.
    fn activate(&mut self, index: usize) -> Option<M> {
        let action = self.action_at(index)?;
        self.cursor = index;
        Some((self.on_action)(action))
    }

    fn move_cursor(&mut self, delta: isize) -> Option<M> {
        let next = (self.cursor as isize + delta).clamp(0, TRANSPORT_CELLS as isize - 1) as usize;
        self.set_cursor(next);
        None
    }

    fn active(&self, action: TransportAction) -> bool {
        match action {
            TransportAction::Shuffle => self.shuffle,
            TransportAction::Repeat => self.repeat.active(),
            _ => false,
        }
    }

    fn glyph(&self, action: TransportAction) -> char {
        match action {
            TransportAction::PlayPause if self.playing => '\u{E769}',
            TransportAction::Repeat if self.repeat == RepeatMode::One => '\u{E8ED}',
            other => other.glyph(),
        }
    }

    fn action_name(&self, action: TransportAction) -> String {
        match action {
            TransportAction::PlayPause if self.playing => "暂停".to_string(),
            TransportAction::PlayPause => "播放".to_string(),
            TransportAction::Repeat => {
                format!("{}（{}）", action.label(), self.repeat.label())
            }
            other => other.label().to_string(),
        }
    }
}

impl<M: 'static> Widget<M> for PlaybackTransport<M> {
    fn skin_attrs(&self) -> &'static [SkinAttrDecl] {
        TRANSPORT_SKIN_ATTRS
    }

    fn validate_skin_attr(&self, name: &str, value: &SkinValue<'_>) -> bool {
        match (name, value) {
            ("repeat", SkinValue::Text(mode)) => RepeatMode::parse(mode).is_some(),
            _ => true,
        }
    }

    fn apply_skin_attr(&mut self, name: &str, value: &SkinValue<'_>) {
        match (name, value) {
            ("playing", SkinValue::Bool(playing)) => {
                self.set_playing(*playing);
            }
            ("shuffle", SkinValue::Bool(shuffle)) => {
                self.set_shuffle(*shuffle);
            }
            ("repeat", SkinValue::Text(mode)) => {
                if let Some(mode) = RepeatMode::parse(mode) {
                    self.set_repeat(mode);
                }
            }
            _ => {}
        }
    }

    fn id(&self) -> WidgetId {
        self.id
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn rect(&self) -> Rect {
        self.rect
    }

    fn set_rect(&mut self, rect: Rect) {
        self.rect = rect;
    }

    fn preferred_height(&self, _width: f32) -> f32 {
        44.0
    }

    fn enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.hover = None;
            self.pressed = None;
        }
    }

    /// The arrows and Enter belong to the strip while it is focused; Up/Down
    /// still move focus out, and Tab always moves focus.
    fn owns_navigation_key(&self, key: KeyCode) -> bool {
        matches!(
            key,
            KeyCode::Left | KeyCode::Right | KeyCode::Home | KeyCode::End
                | KeyCode::Enter | KeyCode::Space
        )
    }

    fn repaint_on_move_at(&self, pos: Point) -> bool {
        self.cell_at(pos) != self.hover
    }

    fn focus_ring_rect(&self) -> Rect {
        self.button_rect(self.cursor).inflate(2.0, 2.0)
    }

    fn tooltip_rect(&self) -> Rect {
        self.button_rect(self.cursor)
    }

    fn child_at(&self, pos: Point) -> Option<usize> {
        self.cell_at(pos)
    }

    fn focus_item(&self) -> Option<usize> {
        Some(self.cursor)
    }

    fn a11y(&self) -> Option<A11yInfo> {
        let children = TransportAction::ALL
            .iter()
            .enumerate()
            .map(|(index, action)| {
                let role = match action {
                    TransportAction::Shuffle | TransportAction::Repeat => A11yRole::Toggle,
                    _ => A11yRole::Button,
                };
                let checked = match action {
                    TransportAction::Shuffle => Some(self.shuffle),
                    TransportAction::Repeat => Some(self.repeat.active()),
                    _ => None,
                };
                let child = A11yChild::new(index, self.action_name(*action), self.button_rect(index))
                    .role(role)
                    .enabled(self.enabled);
                match checked {
                    Some(checked) => child.checked(checked),
                    None => child,
                }
            })
            .collect();
        Some(A11yInfo::new(A11yRole::ToolBar, self.id.0).children(children))
    }

    fn a11y_select_child(&mut self, index: usize) -> Option<M> {
        if !self.enabled {
            return None;
        }
        self.activate(index)
    }

    fn paint(&self, ctx: &mut WidgetPaint<'_>, _state: &WidgetState) {
        let pal = ctx.palette;
        let icon = TextStyle::glyph((self.rect.height() * 0.42).clamp(12.0, 22.0));
        for index in 0..TRANSPORT_CELLS {
            let action = TransportAction::ALL[index];
            let cell = self.cell_rect(index);
            let button = self.button_rect(index);
            let hovered = self.enabled && self.hover == Some(index);
            let pressed = self.enabled && self.pressed == Some(index);
            let color = if action == TransportAction::PlayPause {
                // The primary action: a filled circle, accent while playing so
                // the strip itself shows playback state.
                let fill = if !self.enabled {
                    pal.button_disabled
                } else if pressed {
                    pal.button_active_hover
                } else if self.playing {
                    pal.accent
                        .mix(pal.play_hover, if hovered { 0.25 } else { 0.0 })
                } else if hovered {
                    pal.play_hover
                } else {
                    pal.play_button
                };
                ctx.painter
                    .round_rect(&mut *ctx.canvas, button, button.height() * 0.5, fill);
                if !self.enabled {
                    pal.text_disabled
                } else if self.playing && !pressed {
                    pal.background
                } else {
                    pal.accent_text
                }
            } else {
                if hovered || pressed {
                    let fill = if pressed { pal.row_pressed } else { pal.row_hover };
                    ctx.painter.round_rect(
                        &mut *ctx.canvas,
                        button.inflate(2.0, 2.0),
                        radius::MEDIUM,
                        fill,
                    );
                }
                if !self.enabled {
                    pal.text_disabled
                } else if self.active(action) {
                    pal.accent
                } else if hovered {
                    pal.text
                } else {
                    pal.muted
                }
            };
            let glyph = self.glyph(action);
            let mut buf = [0u8; 4];
            let text = glyph.encode_utf8(&mut buf);
            ctx.canvas.text(cell, text, &icon, TextAlign::Center, color);
        }
    }

    fn input(&mut self, input: WidgetInput) -> Option<M> {
        if !self.enabled {
            return None;
        }
        match input {
            WidgetInput::HoverAt(pos) => {
                self.hover = self.cell_at(pos);
                None
            }
            WidgetInput::HoverLeave => {
                self.hover = None;
                None
            }
            WidgetInput::PressAt(pos) => {
                self.pressed = self.cell_at(pos);
                if let Some(index) = self.pressed {
                    self.cursor = index;
                }
                None
            }
            WidgetInput::ReleaseAt(pos) => {
                let pressed = self.pressed.take();
                match (pressed, self.cell_at(pos)) {
                    (Some(pressed), Some(released)) if pressed == released => {
                        self.activate(released)
                    }
                    _ => None,
                }
            }
            // The release already ran the cell; the click must not run it
            // twice.
            WidgetInput::ClickAt(_) => None,
            WidgetInput::Key(key) => match key.code {
                KeyCode::Left => self.move_cursor(-1),
                KeyCode::Right => self.move_cursor(1),
                KeyCode::Home => {
                    self.set_cursor(0);
                    None
                }
                KeyCode::End => {
                    self.set_cursor(TRANSPORT_CELLS - 1);
                    None
                }
                KeyCode::Enter | KeyCode::Space => self.activate(self.cursor),
                _ => None,
            },
            WidgetInput::Activate => self.activate(self.cursor),
            WidgetInput::Step(delta) => self.move_cursor(delta.signum() as isize),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bar() -> WaveformSeekBar<f32> {
        let mut bar = WaveformSeekBar::new("wave", 0.25, |position| position);
        bar.set_rect(Rect::from_xywh(0.0, 0.0, 400.0, 56.0));
        bar.set_samples([0.2, 0.8, 0.4, 0.9]);
        bar
    }

    fn key(code: KeyCode) -> WidgetInput {
        WidgetInput::Key(crate::event::KeyEvent {
            code,
            vk: 0,
            ctrl: false,
            shift: false,
            alt: false,
            repeat: false,
        })
    }

    #[test]
    fn a_press_seeks_and_a_drag_scrubs() {
        let mut bar = bar();
        let message = bar.input(WidgetInput::PressAt(Point::new(100.0, 20.0)));
        assert_eq!(message, Some(0.25));
        assert!(bar.is_dragging());
        // Dragging keeps emitting so the player scrubs continuously.
        assert_eq!(
            bar.input(WidgetInput::DragTo(Point::new(300.0, 20.0))),
            Some(0.75)
        );
        assert_eq!(bar.input(WidgetInput::ReleaseAt(Point::new(300.0, 20.0))), None);
        assert!(!bar.is_dragging());
        assert_eq!(bar.progress(), 0.75);
        // A press outside the ends clamps to the range.
        bar.input(WidgetInput::PressAt(Point::new(-40.0, 20.0)));
        assert_eq!(bar.input(WidgetInput::ReleaseAt(Point::new(-40.0, 20.0))), None);
        assert_eq!(bar.progress(), 0.0);
    }

    #[test]
    fn the_scrub_lifecycle_reports_start_move_and_end() {
        let mut bar: WaveformSeekBar<&'static str> =
            WaveformSeekBar::new("wave", 0.0, |_| "seek")
                .on_drag_start(|_| "start")
                .on_drag(|_| "drag")
                .on_drag_end(|_| "end");
        bar.set_rect(Rect::from_xywh(0.0, 0.0, 400.0, 56.0));
        // Press: on_seek stays primary, on_drag_start rides beside it.
        assert_eq!(
            bar.input(WidgetInput::PressAt(Point::new(100.0, 20.0))),
            Some("seek")
        );
        assert_eq!(bar.take_extra_message(), Some("start"));
        assert!((bar.drag_preview().unwrap() - 0.25).abs() < 0.001);
        // Move: on_seek again, on_drag beside it.
        assert_eq!(
            bar.input(WidgetInput::DragTo(Point::new(300.0, 20.0))),
            Some("seek")
        );
        assert_eq!(bar.take_extra_message(), Some("drag"));
        // Release: the drag end is the only message, with the committed value.
        assert_eq!(
            bar.input(WidgetInput::ReleaseAt(Point::new(300.0, 20.0))),
            Some("end")
        );
        assert!(!bar.is_dragging());
        assert_eq!(bar.drag_preview(), None);
        assert!((bar.progress() - 0.75).abs() < 0.001);
    }

    #[test]
    fn keyboard_steps_and_clamps() {
        let mut bar = bar();
        assert_eq!(bar.input(key(KeyCode::Right)), Some(0.27));
        assert_eq!(bar.input(key(KeyCode::Home)), Some(0.0));
        // At the left edge Home is a no-op and emits nothing.
        assert_eq!(bar.input(key(KeyCode::Home)), None);
        assert_eq!(bar.input(key(KeyCode::End)), Some(1.0));
        assert_eq!(bar.input(key(KeyCode::Right)), None);
    }

    #[test]
    fn a11y_exposes_a_range_and_set_value_seeks() {
        let mut bar = bar();
        let info = bar.a11y().expect("seek bar is exposed");
        assert_eq!(info.role, A11yRole::Slider);
        assert_eq!(info.range, Some((0.0, 1.0, 0.25)));
        assert_eq!(info.value.as_deref(), Some("25%"));
        assert_eq!(bar.a11y_set_value("50%"), Some(0.5));
        assert_eq!(bar.progress(), 0.5);
        assert_eq!(bar.a11y_set_value("0.9"), Some(0.9));
        assert_eq!(bar.a11y_set_value("2"), Some(1.0));
        assert_eq!(bar.a11y_set_value("nope"), None);
    }

    #[test]
    fn duration_formats_the_position_and_playback_updates_do_not_emit() {
        let mut bar = bar().duration(305.0);
        assert_eq!(bar.a11y().unwrap().value.as_deref(), Some("1:16"));
        // Application playback progress moves the bar without a message.
        bar.set_progress(0.5);
        assert_eq!(bar.progress(), 0.5);
        assert_eq!(bar.a11y().unwrap().value.as_deref(), Some("2:32"));
        bar.set_progress(2.0);
        assert_eq!(bar.progress(), 1.0);
        assert_eq!(bar.a11y_set_value("nope"), None);
    }

    fn transport() -> PlaybackTransport<TransportAction> {
        let mut transport = PlaybackTransport::new("transport", |action| action);
        transport.set_rect(Rect::from_xywh(0.0, 0.0, 300.0, 44.0));
        transport
    }

    fn click_at(widget: &mut PlaybackTransport<TransportAction>, index: usize) -> Option<TransportAction> {
        let rect = widget.cell_rect(index);
        let pos = rect.center();
        assert_eq!(widget.input(WidgetInput::PressAt(pos)), None);
        let message = widget.input(WidgetInput::ReleaseAt(pos));
        // The container sends a click after the release; it must not repeat it.
        assert_eq!(widget.input(WidgetInput::ClickAt(pos)), None);
        message
    }

    #[test]
    fn each_cell_emits_its_action_and_a_dragged_release_does_not() {
        let mut transport = transport();
        assert_eq!(click_at(&mut transport, 0), Some(TransportAction::Shuffle));
        assert_eq!(click_at(&mut transport, 1), Some(TransportAction::Previous));
        assert_eq!(click_at(&mut transport, 2), Some(TransportAction::PlayPause));
        assert_eq!(click_at(&mut transport, 3), Some(TransportAction::Next));
        assert_eq!(click_at(&mut transport, 4), Some(TransportAction::Repeat));
        // Pressing one cell and releasing over another is a drag, not a click.
        let play = transport.cell_rect(2).center();
        let next = transport.cell_rect(3).center();
        transport.input(WidgetInput::PressAt(play));
        assert_eq!(transport.input(WidgetInput::ReleaseAt(next)), None);
        // The click moved the keyboard cursor to the pressed cell.
        assert_eq!(transport.cursor(), 2);
    }

    #[test]
    fn the_keyboard_cursor_moves_clamps_and_activates() {
        let mut transport = transport();
        assert_eq!(transport.cursor(), 2);
        assert_eq!(transport.input(key(KeyCode::Left)), None);
        assert_eq!(transport.cursor(), 1);
        assert_eq!(transport.input(key(KeyCode::End)), None);
        assert_eq!(transport.cursor(), 4);
        // At the end Right does not wrap.
        assert_eq!(transport.input(key(KeyCode::Right)), None);
        assert_eq!(transport.cursor(), 4);
        assert_eq!(
            transport.input(key(KeyCode::Enter)),
            Some(TransportAction::Repeat)
        );
        assert_eq!(transport.input(key(KeyCode::Home)), None);
        assert_eq!(transport.cursor(), 0);
        assert_eq!(
            transport.input(key(KeyCode::Space)),
            Some(TransportAction::Shuffle)
        );
        assert_eq!(
            transport.input(WidgetInput::Activate),
            Some(TransportAction::Shuffle)
        );
    }

    #[test]
    fn state_setters_report_change_and_drive_the_painted_glyphs() {
        let mut transport = transport();
        assert!(transport.set_playing(true));
        assert!(transport.is_playing());
        assert!(!transport.set_playing(true));
        assert_eq!(transport.glyph(TransportAction::PlayPause), '\u{E769}');
        transport.set_playing(false);
        assert_eq!(transport.glyph(TransportAction::PlayPause), '\u{E768}');
        assert!(transport.set_shuffle(true));
        assert!(transport.shuffle_enabled());
        assert!(!transport.set_shuffle(true));
        assert!(transport.set_repeat(RepeatMode::One));
        assert_eq!(transport.repeat_mode().label(), "单曲");
        assert_eq!(transport.glyph(TransportAction::Repeat), '\u{E8ED}');
        assert!(transport.repeat_mode().active());
        transport.set_repeat(RepeatMode::Off);
        assert_eq!(transport.glyph(TransportAction::Repeat), '\u{E8EE}');
        // Off → All → One → Off.
        assert_eq!(RepeatMode::Off.next(), RepeatMode::All);
        assert_eq!(RepeatMode::All.next(), RepeatMode::One);
        assert_eq!(RepeatMode::One.next(), RepeatMode::Off);
        assert!(!RepeatMode::Off.active());
    }

    #[test]
    fn disabled_transport_ignores_every_input() {
        let mut transport = transport();
        transport.set_enabled(false);
        let pos = transport.cell_rect(2).center();
        assert_eq!(transport.input(WidgetInput::PressAt(pos)), None);
        assert_eq!(transport.input(WidgetInput::ReleaseAt(pos)), None);
        assert_eq!(transport.input(key(KeyCode::Enter)), None);
    }

    #[test]
    fn a11y_publishes_a_toolbar_with_toggle_state() {
        let mut transport = transport().playing(true).shuffle(true);
        transport.set_repeat(RepeatMode::All);
        let info = transport.a11y().expect("the transport is exposed");
        assert_eq!(info.role, A11yRole::ToolBar);
        assert_eq!(info.children.len(), 5);
        assert_eq!(info.children[0].role, Some(A11yRole::Toggle));
        assert_eq!(info.children[0].checked, Some(true));
        assert_eq!(info.children[1].role, Some(A11yRole::Button));
        assert_eq!(info.children[2].name, "暂停");
        assert_eq!(info.children[4].role, Some(A11yRole::Toggle));
        assert_eq!(info.children[4].checked, Some(true));
        assert_eq!(info.children[4].name, "循环播放（列表）");
        // Invoking a child runs the same activation a click does.
        assert_eq!(
            transport.a11y_select_child(2),
            Some(TransportAction::PlayPause)
        );
        assert_eq!(transport.cursor(), 2);
        assert_eq!(transport.a11y_select_child(5), None);
        transport.set_enabled(false);
        assert_eq!(transport.a11y_select_child(0), None);
    }
}
