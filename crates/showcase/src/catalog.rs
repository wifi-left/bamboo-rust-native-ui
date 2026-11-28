// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! The showcase catalog: page definitions and the layout that places every
//! demo.
//!
//! The catalog is one page per tab. Each page is a `ScrollView` whose children
//! are laid out once in *content* coordinates; the container positions them at
//! `content - offset`, clips them and paints the bars. That is what keeps every
//! component reachable on a short window: a page that does not fit scrolls
//! instead of clipping.
//!
//! Section headings are app-painted caption lines, not widgets, so adding a
//! heading costs no widget and no accessibility node. The demos themselves are
//! ordinary retained widgets with their normal properties.

use bamboo_native_ui::{
    Invalidator, Rect, ScrollBars, ScrollChild, ScrollView, Size, Ui, UiHost, WidgetId,
};

use crate::{Msg, DYN_SLOT_IDS, NEST_PANEL, NEST_ROWS, SCROLL_PANEL, SCROLL_ROWS};

/// The catalog pages: one tab each, one scrollable page each.
#[cfg(feature = "m7")]
pub(crate) const PAGE_NAMES: [&str; 9] = [
    "概览", "详情", "组件", "数据", "封面", "功能区", "属性", "API", "布局",
];
#[cfg(not(feature = "m7"))]
pub(crate) const PAGE_NAMES: [&str; 8] = [
    "概览", "详情", "组件", "数据", "封面", "功能区", "属性", "API",
];
/// One short line for the sidebar under the page name.
#[cfg(feature = "m7")]
pub(crate) const PAGE_HINTS: [&str; 9] = [
    "按钮、输入、圆角与显示控件",
    "文本、徽章、头像与富文本",
    "输入、导航、反馈、外壳与数据展示",
    "表格、树、动态控件与滚动面板",
    "封面墙、播放列表与播放器",
    "选项卡式命令面",
    "分区属性表格与行内编辑",
    "对话框、菜单、托盘、热键、窗口与拖放",
    "XML 语法的皮肤文件与热重载",
];
#[cfg(not(feature = "m7"))]
pub(crate) const PAGE_HINTS: [&str; 8] = [
    "按钮、输入、圆角与显示控件",
    "文本、徽章、头像与富文本",
    "输入、导航、反馈、外壳与数据展示",
    "表格、树、动态控件与滚动面板",
    "封面墙、播放列表与播放器",
    "选项卡式命令面",
    "分区属性表格与行内编辑",
    "对话框、菜单、托盘、热键、窗口与拖放",
];
/// The scroll panel of each page.
#[cfg(feature = "m7")]
pub(crate) const PAGE_SCROLLS: [&str; 9] = [
    "page_scroll_0",
    "page_scroll_1",
    "page_scroll_2",
    "page_scroll_3",
    "page_scroll_4",
    "page_scroll_5",
    "page_scroll_6",
    "page_scroll_7",
    "page_scroll_8",
];
#[cfg(not(feature = "m7"))]
pub(crate) const PAGE_SCROLLS: [&str; 8] = [
    "page_scroll_0",
    "page_scroll_1",
    "page_scroll_2",
    "page_scroll_3",
    "page_scroll_4",
    "page_scroll_5",
    "page_scroll_6",
    "page_scroll_7",
];

/// Gap between demos in a row, and between rows.
const GAP: f32 = 14.0;
const ROW_GAP: f32 = 12.0;
/// Content margin around a page.
const PAD: f32 = 18.0;

/// The window chrome rectangles the application paints itself.
pub(crate) struct Scene {
    pub sidebar: Rect,
    pub divider: Rect,
    pub brand: Rect,
    pub page_title: Rect,
    pub page_hint: Rect,
    pub footer: Rect,
    pub tabs: Rect,
    pub theme: Rect,
    pub toast: Rect,
    pub main: Rect,
    /// The bottom strip: last action on the left, paint metrics on the right.
    pub status: Rect,
    /// The page viewport (also the repaint region for page-local effects).
    pub viewport: Rect,
}

/// One painted line inside a page's scroll content (section heading or the API
/// note under it). Captions are app-painted text, so the catalog can have
/// dozens of section labels without adding widgets or a11y nodes.
pub(crate) struct Caption {
    /// Content coordinates; painted at `rect - scroll offset`.
    pub rect: Rect,
    pub text: &'static str,
    pub heading: bool,
}

/// One page of the catalog: its scroll panel, viewport, demo widgets in
/// content coordinates, the painted captions and the content height.
pub(crate) struct PageSpec {
    pub panel: WidgetId,
    pub viewport: Rect,
    pub children: Vec<ScrollChild>,
    pub captions: Vec<Caption>,
    pub content: f32,
    /// Panels nested inside this page (registered after it).
    pub nested: Vec<ScrollView>,
}

impl PageSpec {
    pub fn child_rect(&self, id: &str) -> Option<Rect> {
        self.children
            .iter()
            .find(|child| child.id.0 == id)
            .map(|child| child.rect)
    }

    pub fn set_child_rect(&mut self, id: &str, rect: Rect) -> bool {
        let Some(child) = self.children.iter_mut().find(|child| child.id.0 == id) else {
            return false;
        };
        child.rect = rect;
        true
    }
}

/// A vertical flow with wrapping rows: demos are packed left to right and
/// wrap to the next row when the page is too narrow.
struct Flow {
    width: f32,
    y: f32,
    cx: f32,
    row_h: f32,
    children: Vec<ScrollChild>,
    captions: Vec<Caption>,
    nested: Vec<ScrollView>,
}

impl Flow {
    fn new(width: f32) -> Self {
        Self {
            width: width.max(160.0),
            y: PAD,
            cx: PAD,
            row_h: 0.0,
            children: Vec::new(),
            captions: Vec::new(),
            nested: Vec::new(),
        }
    }

    fn inner_width(&self) -> f32 {
        (self.width - PAD * 2.0).max(120.0)
    }

    fn right(&self) -> f32 {
        PAD + self.inner_width()
    }

    /// Start a new section: a larger heading and a small, wrapped API note.
    fn section(&mut self, heading: &'static str, note: &'static str) {
        self.end_row();
        self.y += 12.0;
        self.captions.push(Caption {
            rect: Rect::from_xywh(PAD, self.y, self.inner_width(), 22.0),
            text: heading,
            heading: true,
        });
        self.y += 26.0;
        for (start, end) in wrap_note(note, self.inner_width() - 4.0) {
            self.captions.push(Caption {
                rect: Rect::from_xywh(PAD, self.y, self.inner_width(), 15.0),
                text: &note[start..end],
                heading: false,
            });
            self.y += 15.0;
        }
        self.y += 9.0;
    }

    /// A heading + wrapped note at an explicit column position. Returns the
    /// block height, so a hand-placed two-column section can stack under it.
    fn caption_block(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        heading: &'static str,
        note: &'static str,
    ) -> f32 {
        self.captions.push(Caption {
            rect: Rect::from_xywh(x, y, width, 22.0),
            text: heading,
            heading: true,
        });
        let mut dy = 26.0;
        for (start, end) in wrap_note(note, width - 4.0) {
            self.captions.push(Caption {
                rect: Rect::from_xywh(x, y + dy, width, 15.0),
                text: &note[start..end],
                heading: false,
            });
            dy += 15.0;
        }
        dy + 9.0
    }

    fn end_row(&mut self) {
        if self.row_h > 0.0 {
            self.y += self.row_h + ROW_GAP;
            self.cx = PAD;
            self.row_h = 0.0;
        }
    }

    /// Pack one demo, wrapping when it does not fit on the current row.
    fn demo(&mut self, id: &'static str, w: f32, h: f32) -> Rect {
        let w = w.min(self.inner_width());
        if self.cx > PAD && self.cx + w > self.right() + 0.5 {
            self.end_row();
        }
        let rect = Rect::from_xywh(self.cx, self.y, w, h);
        self.children.push(ScrollChild::new(WidgetId::new(id), rect));
        self.cx += w + GAP;
        self.row_h = self.row_h.max(h);
        rect
    }

    /// One demo that spans the content width.
    fn full(&mut self, id: &'static str, h: f32) -> Rect {
        self.end_row();
        let rect = Rect::from_xywh(PAD, self.y, self.inner_width(), h);
        self.children.push(ScrollChild::new(WidgetId::new(id), rect));
        self.y += h + ROW_GAP;
        rect
    }

    /// Place a demo at an explicit content rectangle (custom two-column
    /// sections). Does not advance the flow.
    fn raw(&mut self, id: &'static str, rect: Rect) {
        self.children.push(ScrollChild::new(WidgetId::new(id), rect));
    }

    /// Move the flow cursor down past a raw block.
    fn advance(&mut self, height: f32) {
        self.end_row();
        self.y += height + ROW_GAP;
    }

    fn finish(self, index: usize, viewport: Rect) -> PageSpec {
        let content = (self.y + PAD).max(viewport.height());
        PageSpec {
            panel: WidgetId::new(PAGE_SCROLLS[index]),
            viewport,
            children: self.children,
            captions: self.captions,
            content,
            nested: self.nested,
        }
    }
}

/// Break a static note into lines that fit `width` at the small text size.
/// Widths are estimated per character (CJK full-width, Latin about half), which
/// is enough for a layout-time caption; the painted text itself is measured by
/// the backend and clipped to the page.
fn wrap_note(text: &'static str, width: f32) -> Vec<(usize, usize)> {
    const LATIN: f32 = 6.2;
    const CJK: f32 = 11.5;
    let mut lines = Vec::new();
    let mut start = 0usize;
    let mut used = 0.0f32;
    let mut pending = 0usize;
    for (index, ch) in text.char_indices() {
        let w = if ch.is_ascii() { LATIN } else { CJK };
        if used + w > width && index > start {
            let end = if pending > start && pending < index {
                pending
            } else {
                index
            };
            lines.push((start, end));
            start = end;
            used = 0.0;
            pending = start;
            for ch in text[start..index].chars() {
                used += if ch.is_ascii() { LATIN } else { CJK };
            }
        }
        used += w;
        if matches!(ch, ' ' | '，' | '、' | '；' | '。' | '：' | '（' | '）' | '/') {
            pending = index + ch.len_utf8();
        }
    }
    if start < text.len() {
        lines.push((start, text.len()));
    }
    if lines.is_empty() {
        lines.push((0, text.len()));
    }
    lines
}

pub(crate) struct LayoutSink;

impl Invalidator for LayoutSink {
    fn invalidate(&mut self, _rect: Rect) {}
}

impl UiHost for LayoutSink {}

/// Compute the window chrome and every page's content rectangles.
pub(crate) fn layout(size: Size) -> (Scene, Vec<PageSpec>) {
    let sidebar_w = 208.0_f32.min((size.width * 0.32).max(160.0));
    let main = Rect::from_xywh(
        sidebar_w,
        0.0,
        (size.width - sidebar_w).max(240.0),
        size.height,
    );
    let inner_x = main.left + 22.0;
    let inner_w = (main.width() - 44.0).max(180.0);
    let tabs = Rect::from_xywh(inner_x, 14.0, inner_w, 34.0);
    let viewport = Rect::from_xywh(
        inner_x,
        58.0,
        inner_w,
        (size.height - 58.0 - 30.0).max(120.0),
    );
    let h = size.height;
    let scene = Scene {
        sidebar: Rect::from_xywh(0.0, 0.0, sidebar_w, h),
        divider: Rect::from_xywh(sidebar_w, 0.0, 1.0, h),
        brand: Rect::from_xywh(18.0, 20.0, (sidebar_w - 36.0).max(80.0), 26.0),
        page_title: Rect::from_xywh(18.0, 60.0, (sidebar_w - 36.0).max(80.0), 24.0),
        page_hint: Rect::from_xywh(18.0, 90.0, (sidebar_w - 36.0).max(80.0), 18.0),
        theme: Rect::from_xywh(
            18.0,
            (h - 70.0).max(140.0),
            (sidebar_w - 36.0).max(80.0),
            34.0,
        ),
        footer: Rect::from_xywh(
            18.0,
            (h - 98.0).max(112.0),
            (sidebar_w - 36.0).max(80.0),
            20.0,
        ),
        tabs,
        toast: Rect::from_xywh(
            (size.width - 352.0).max(16.0),
            (h - 62.0).max(16.0),
            334.0,
            44.0,
        ),
        main,
        status: Rect::from_xywh(inner_x, (h - 24.0).max(60.0), inner_w, 16.0),
        viewport,
    };

    let w = viewport.width();
    let mut pages = Vec::with_capacity(PAGE_NAMES.len());
    for index in 0..PAGE_NAMES.len() {
        let flow = match index {
            0 => page_overview(w),
            1 => page_text(w),
            2 => page_gallery(w),
            3 => page_data(w),
            4 => page_covers(w),
            5 => page_ribbon(w),
            6 => page_props(w),
            7 => page_api(w),
            #[cfg(feature = "m7")]
            8 => page_layout(w),
            _ => page_api(w),
        };
        pages.push(flow.finish(index, viewport));
    }
    (scene, pages)
}

/// Register every page panel and its nested panels. Re-registering keeps each
/// panel's live scroll offset, so a resize never jumps a page back to the top.
pub(crate) fn register_pages(ui: &mut Ui<Msg>, pages: &[PageSpec], sink: &mut impl UiHost) {
    for (index, page) in pages.iter().enumerate() {
        ui.set_scroll_view(
            ScrollView::new(page.panel, page.viewport, ScrollBars::Vertical)
                .children(page.children.iter().copied())
                .content(Size::new(page.viewport.width(), page.content))
                .name(PAGE_NAMES.get(index).copied().unwrap_or("页面")),
            sink,
        );
        for nested in &page.nested {
            ui.set_scroll_view(nested.clone(), sink);
        }
    }
}

// ---------------------------------------------------------------------------
// Page 0 — overview
// ---------------------------------------------------------------------------

fn page_overview(w: f32) -> Flow {
    let mut f = Flow::new(w);
    f.section(
        "按钮 Button",
        "Button::new(id, 文本, Msg).primary() / .danger() / .glyph('…')；WidgetProps::describe / tooltip / context_menu 是每个控件通用的属性。",
    );
    f.demo("primary", 140.0, 36.0);
    f.demo("secondary", 140.0, 36.0);
    f.demo("icon", 44.0, 36.0);
    f.demo("danger", 110.0, 36.0);
    f.demo("flyout_open", 112.0, 36.0);
    f.section(
        "开关与分段 Toggle · Segmented",
        "Toggle::new(id, 文本, 初值, Msg).on_label / off_label；Segmented::new(id, 选项, 选中, Msg) 用 Ui::set_selected 同步。Tabs 就是上方的页面导航。",
    );
    f.demo("toggle", 190.0, 36.0);
    f.demo("segmented", 300.0, 34.0);
    f.section(
        "文本输入 TextEdit · NumberInput",
        "TextEdit::new(id, 文本, Msg).painted() 自绘会话（居中、IME、撤销、选择）；NumberInput::new(id, 值, 范围, Msg).steppers() 带步进区与范围钳制。",
    );
    f.demo("search", 320.0, 34.0);
    f.demo("count", 130.0, 34.0);
    f.demo("painted_name", 320.0, 34.0);
    f.section(
        "滑块与进度 Slider · ProgressBar",
        "Slider::new(id, 值, 范围, Msg).discrete() / ticks_at(...) / value_text(...)；ProgressBar::new(id, 值).label()，Ui::set_progress 更新。",
    );
    f.full("volume", 32.0);
    f.full("progress", 24.0);
    f.section(
        "列表与翻页 ListView · Pager",
        "ListView::new(id).row_height() / alternating(true) / empty_text() / on_activate()，600 行虚拟化，UIA 只发布视口内行；Pager::new(id, 页, 总页, Msg)。",
    );
    f.full("list_title", 24.0);
    f.full("list", 300.0);
    f.demo("pager", 300.0, 32.0);
    f.section(
        "显示控件 ColorSwatch · StatCard",
        "ColorSwatch::new(id, 颜色, 名) 可点击（WidgetProps::role / cursor / narrate）；StatCard::new(id, 值, 标题).emphasis()；Narration 定制读屏文本。",
    );
    f.demo("swatch", 200.0, 30.0);
    f.demo("stat1", 200.0, 76.0);
    f.demo("stat2", 200.0, 76.0);
    f.section(
        "圆角与表面 SurfaceStyle",
        "自绘控件共享 SurfaceStyle：.corner_radius(dips) 同时圆化填充与焦点环（超过半高即胶囊）；.corners(Corners) 逐角设置；.background(color) 覆盖主题填充；.transparent_background(true) 只留边框；Label 的 .background 画出芯片式标签。",
    );
    f.demo("round_sharp", 118.0, 36.0);
    f.demo("round_soft", 118.0, 36.0);
    f.demo("round_pill", 118.0, 36.0);
    f.demo("round_outline", 118.0, 36.0);
    f.demo("round_chip0", 112.0, 26.0);
    f.demo("round_chip8", 112.0, 26.0);
    f.demo("round_chip_pill", 132.0, 26.0);
    f.demo("round_search", 260.0, 34.0);
    f.demo("round_progress", 260.0, 24.0);
    f.demo("round_card_round", 300.0, 92.0);
    f.demo("round_card_cut", 300.0, 92.0);
    f.end_row();
    f
}

// ---------------------------------------------------------------------------
// Page 1 — text and content
// ---------------------------------------------------------------------------

fn page_text(w: f32) -> Flow {
    let mut f = Flow::new(w);
    f.section(
        "分隔与标签 Divider · Label",
        "Divider::new(id).label(文本)；Label::new(id, 文本).wrap(true) / .link(true) / .style(LabelStyle::Title|Small)。装饰控件默认不进无障碍树，WidgetProps::expose(true) 才发布。",
    );
    f.full("content_divider", 16.0);
    f.demo("wrap_label", 380.0, 48.0);
    f.demo("link_label", 160.0, 22.0);
    f.demo("avatar_name", 190.0, 24.0);
    f.section(
        "徽章、快捷键与图标 Badge · Kbd · Icon",
        "Badge::new(id, 文本).tone(BadgeTone::…)；可移除徽章 removable(|| Msg)；Kbd::new(id, \"Ctrl+K\")；Icon::new(id, '\\u{E721}') 是 MDL2 字形。",
    );
    f.demo("badge_fav", 76.0, 24.0);
    f.demo("badge_lossless", 60.0, 24.0);
    f.demo("badge_flac", 86.0, 24.0);
    f.demo("kbd_search", 64.0, 24.0);
    f.demo("icon_search", 24.0, 24.0);
    f.section(
        "头像 Avatar",
        "Avatar::new(id, 首字母).ring(true) / .image(ImageId) — 注册图像后画圆形封面，未注册时回落到首字母。",
    );
    f.demo("avatar", 48.0, 48.0);
    f.demo("avatar_image", 48.0, 48.0);
    f.section(
        "复制、跑马灯与富文本 CopyableText · MarqueeText · RichText",
        "CopyableText::new(id, 文本).copyable(|| Msg) 经 WindowHandle::copy_text 写剪贴板；MarqueeText 由 advance(dt) 驱动（悬停启动）；RichText 由 run/strong/muted/link 组成，链接回调带编号。",
    );
    f.demo("copy_text", 280.0, 24.0);
    f.demo("marquee", 340.0, 24.0);
    f.demo("rich", 420.0, 30.0);
    f.end_row();
    f
}

// ---------------------------------------------------------------------------
// Page 2 — inputs, navigation, feedback, chrome, data display
// ---------------------------------------------------------------------------

fn page_gallery(w: f32) -> Flow {
    let mut f = Flow::new(w);
    f.section(
        "复选框与单选框 Checkbox · RadioButton · RadioGroup",
        "Checkbox::new(id, 文本, 初值, Msg).square()；RadioButton::new(id, 文本, 选中, || Msg)；RadioGroup::new(id, 选项, 选中, Msg).disable(i)。",
    );
    f.demo("cb_random", 130.0, 30.0);
    f.demo("cb_shuffle", 130.0, 30.0);
    f.demo("cb_square", 110.0, 30.0);
    f.demo("radio_only", 120.0, 30.0);
    f.demo("radio_group", 220.0, 96.0);
    f.section(
        "搜索、密码、下拉与多选 SearchBox · PasswordBox · ComboBox · MultiSelectList",
        "SearchBox 防抖后回调；PasswordBox 掩码；ComboBox 浮层列表（自动翻转）；MultiSelectList 复选、Shift 范围、Ctrl 切换。",
    );
    f.demo("gallery_search", 260.0, 34.0);
    f.demo("gallery_password", 260.0, 34.0);
    f.demo("gallery_combo", 220.0, 34.0);
    f.demo("gallery_multi", 220.0, 112.0);
    f.section(
        "多行文本 TextArea",
        "TextArea::new(id, 文本, Msg).multiline().wrap(true|false).scrollbars(ScrollBars::Vertical|Both).painted() — 关闭状态下也分行、可滚动。",
    );
    f.demo("gallery_notes", 430.0, 92.0);
    f.demo("gallery_notes_raw", 270.0, 104.0);
    f.section(
        "日期、时间与颜色选择 DatePicker · TimePicker · ColorPicker",
        "三个窗口内选择器共用 value / set_value、open / close、is_open 与 on_change 事件，并支持观察者。DatePicker 与 TimePicker 还可 value_range / min_date / max_date 限定可选范围；Date::today()/new、Time::now()/new。日期用方向键与 PageUp/PageDown 换月；时间的列自上而下递增，Down/Up 步进（滚轮同样按悬停列步进）；颜色用方向键选色板，开启 allow_alpha 后多出透明度条，底部「更多颜色…」交给系统颜色对话框（new_rgba / on_change_rgba 报告 Rgba）。",
    );
    f.demo("gallery_date", 210.0, 34.0);
    f.demo("gallery_time", 130.0, 34.0);
    f.demo("gallery_color", 190.0, 34.0);
    f.section(
        "导航 Breadcrumb · NavRail",
        "Breadcrumb::new(id, 路径, Msg).label()；NavRail::new(id, 选项, 选中, Msg).icon(i, '字符').row_height()。",
    );
    f.demo("gallery_breadcrumb", 240.0, 26.0);
    f.demo("gallery_navrail", 220.0, 112.0);
    f.section(
        "折叠、分组与分隔条 Accordion · GroupBox · Splitter",
        "Accordion 单开模式、键盘可展开（UIA ExpandCollapse）；GroupBox 分组框；Splitter::vertical(id, Msg) 报告分隔条位置，应用移动两侧面板。",
    );
    let group = f.demo("gallery_group", 330.0, 178.0);
    f.raw(
        "gallery_accordion",
        Rect::from_xywh(
            group.left + 10.0,
            group.top + 40.0,
            group.width() - 20.0,
            120.0,
        ),
    );
    f.end_row();
    let split_top = f.y;
    f.raw(
        "gallery_pane_a",
        Rect::from_xywh(PAD, split_top, 150.0, 92.0),
    );
    f.raw(
        "gallery_splitter",
        Rect::from_xywh(PAD + 150.0, split_top, 12.0, 92.0),
    );
    f.raw(
        "gallery_pane_b",
        Rect::from_xywh(PAD + 162.0, split_top, 150.0, 92.0),
    );
    f.advance(92.0);
    f.section(
        "数据展示 Sparkline · Gauge · RatingStars",
        "Sparkline::new(id, &[..])；Gauge::new(id, 0..=1).label()；RatingStars::new(id, 总数, 当前, Msg).clearable(true)。",
    );
    f.demo("gallery_sparkline", 230.0, 60.0);
    f.demo("gallery_gauge", 96.0, 96.0);
    f.demo("gallery_stars", 160.0, 28.0);
    f.section(
        "反馈 Banner · Spinner · ProgressRing · TooltipCard · EmptyState",
        "Banner 带动作与关闭；Spinner 由 edit_widget(advance) 驱动；ProgressRing 显示百分比；TooltipCard 是页面内提示卡；EmptyState 是空数据占位。Toast/Snackbar 是同一个浮层控件。",
    );
    f.demo("gallery_banner", 430.0, 64.0);
    f.demo("gallery_spinner", 28.0, 28.0);
    f.demo("gallery_ring", 60.0, 60.0);
    f.demo("gallery_tooltip", 240.0, 66.0);
    f.demo("gallery_empty", 330.0, 120.0);
    f.demo("modal_open", 120.0, 36.0);
    f.section(
        "应用外壳 MenuBar · Toolbar · StatusBar · MenuButton · SplitButton",
        "MenuBar / Toolbar 的命令 id 走同一个 Msg::Chrome；Toolbar 支持分组分隔线、纯图标格与竖向排列；StatusBar 有分段、进度和尺寸握把；MenuButton 与 SplitButton 打开原生菜单（含快捷键列与图标）。",
    );
    f.full("gallery_menubar", 28.0);
    f.full("gallery_toolbar", 34.0);
    f.demo("gallery_vtoolbar", 40.0, 158.0);
    f.demo("gallery_menubtn", 120.0, 30.0);
    f.demo("gallery_splitbtn", 120.0, 30.0);
    f.demo("gallery_dynmenu", 220.0, 30.0);
    f.full("gallery_status", 28.0);
    f.end_row();
    f
}

// ---------------------------------------------------------------------------
// Page 3 — data page
// ---------------------------------------------------------------------------

fn page_data(w: f32) -> Flow {
    let mut f = Flow::new(w);
    f.section(
        "虚拟表格 Table",
        "Table::new(id).columns(...).rows(...).editable(true).on_cell_edit(row, 列, 文本)；表头可排序、拖动分隔线调宽、拖动表头换列；UIA 发布 Grid 和每个单元格的 SelectionItem/Value。",
    );
    let table_h = 280.0;
    let tree_h = 320.0;
    let panel;
    if f.inner_width() >= 800.0 {
        // Two columns: the table and the tree on the left, the dynamic panel
        // and the scroll panel on the right. Each column gets its own heading
        // directly above its demos.
        let top = f.y;
        f.raw("gallery_table", Rect::from_xywh(PAD, top, 500.0, table_h));
        let tree_top = top + table_h + 14.0;
        let used = f.caption_block(
            PAD,
            tree_top,
            500.0,
            "树 TreeView",
            "TreeView::new(id).items(...).renameable(true).on_rename / on_move / on_toggle；缓存扁平化、拖拽重排、F2 就地重命名；UIA Tree + 每个节点的 ExpandCollapse。",
        );
        f.raw(
            "gallery_tree",
            Rect::from_xywh(PAD, tree_top + used, 500.0, tree_h),
        );
        let left_bottom = tree_top + used + tree_h;

        let rx = PAD + 520.0;
        let rw = (f.right() - rx).max(240.0);
        let used = f.caption_block(
            rx,
            top,
            rw,
            "动态控件与滚动面板",
            "动态面板：两个重叠槽位演示插入、移除与置顶/置底（Ui::remove / insert / raise / lower）。滚动面板：ScrollView::new(id, 视口, ScrollBars).children(...).padding(12)；嵌套面板滚到末端后把滚轮移交给外层页。",
        );
        let group = Rect::from_xywh(rx, top + used, 280.0, 300.0);
        f.raw("dyn_group", group);
        dyn_children(&mut f, group);
        let panel_top = top + used + 316.0;
        panel = Some(Rect::from_xywh(rx, panel_top, 280.0, 246.0));
        f.y = left_bottom.max(panel_top + 246.0) + ROW_GAP;
        f.cx = PAD;
        f.row_h = 0.0;
    } else {
        f.full("gallery_table", table_h);
        f.section(
            "树 TreeView",
            "TreeView::new(id).items(...).renameable(true).on_rename / on_move / on_toggle；UIA Tree + 每个节点的 ExpandCollapse。",
        );
        f.full("gallery_tree", tree_h);
        f.section(
            "动态控件与滚动面板",
            "动态面板：两个重叠槽位演示插入、移除与置顶/置底。滚动面板：ScrollView::new(id, 视口, ScrollBars)，嵌套面板在末端把滚轮移交给外层页。",
        );
        let top = f.y;
        let group = Rect::from_xywh(PAD, top, 280.0, 300.0);
        f.raw("dyn_group", group);
        dyn_children(&mut f, group);
        panel = Some(Rect::from_xywh(PAD, top + 316.0, 280.0, 246.0));
        f.y = top + 316.0 + 246.0 + ROW_GAP;
        f.cx = PAD;
        f.row_h = 0.0;
    }
    if let Some(viewport) = panel {
        f.nested.push(
            ScrollView::new(WidgetId::new(SCROLL_PANEL), viewport, ScrollBars::Both)
                .nested_in(WidgetId::new(PAGE_SCROLLS[3]))
                .children(SCROLL_ROWS.iter().enumerate().map(|(index, id)| {
                    ScrollChild::new(
                        WidgetId::new(*id),
                        Rect::from_xywh(0.0, index as f32 * 34.0, 300.0, 30.0),
                    )
                }))
                .padding(12.0)
                .name("滚动面板"),
        );
        f.nested.push(
            ScrollView::new(
                WidgetId::new(NEST_PANEL),
                Rect::from_xywh(10.0, 30.0, 120.0, 84.0),
                ScrollBars::Vertical,
            )
            .nested_in(WidgetId::new(SCROLL_PANEL))
            .children(NEST_ROWS.iter().enumerate().map(|(index, id)| {
                ScrollChild::new(
                    WidgetId::new(*id),
                    Rect::from_xywh(0.0, index as f32 * 26.0, 110.0, 24.0),
                )
            }))
            .padding(4.0)
            .name("嵌套面板"),
        );
    }
    f.end_row();
    f
}

/// The dynamic widget-tree panel's buttons, labels and overlapping slots.
fn dyn_children(f: &mut Flow, group: Rect) {
    let x = group.left;
    let y = group.top;
    f.raw("dyn_add", Rect::from_xywh(x + 14.0, y + 44.0, 108.0, 30.0));
    f.raw(
        "dyn_remove",
        Rect::from_xywh(x + 150.0, y + 44.0, 108.0, 30.0),
    );
    f.raw(
        "dyn_raise",
        Rect::from_xywh(x + 14.0, y + 80.0, 108.0, 30.0),
    );
    f.raw(
        "dyn_lower",
        Rect::from_xywh(x + 150.0, y + 80.0, 108.0, 30.0),
    );
    f.raw("dyn_info", Rect::from_xywh(x + 14.0, y + 118.0, 250.0, 22.0));
    f.raw("dyn_hint", Rect::from_xywh(x + 14.0, y + 144.0, 248.0, 52.0));
    f.raw(
        DYN_SLOT_IDS[0],
        Rect::from_xywh(x + 14.0, y + 210.0, 252.0, 34.0),
    );
    f.raw(
        DYN_SLOT_IDS[1],
        Rect::from_xywh(x + 20.0, y + 232.0, 240.0, 34.0),
    );
}

// ---------------------------------------------------------------------------
// Page 4 — cover wall and player
// ---------------------------------------------------------------------------

fn page_covers(w: f32) -> Flow {
    let mut f = Flow::new(w);
    f.section(
        "播放控制 PlaybackTransport · WaveformSeekBar",
        "PlaybackTransport::new(id, Msg).playing() / set_playing / set_shuffle / set_repeat；WaveformSeekBar::new(id, 进度, Msg).samples(...) / .duration(秒)，点击与拖动定位。",
    );
    let top = f.y;
    f.raw("covers_transport", Rect::from_xywh(PAD, top, 260.0, 44.0));
    f.raw(
        "covers_state",
        Rect::from_xywh(
            PAD + 276.0,
            top + 12.0,
            (f.inner_width() - 276.0 - 140.0).max(120.0),
            20.0,
        ),
    );
    f.raw(
        "covers_time",
        Rect::from_xywh(PAD + f.inner_width() - 130.0, top + 12.0, 130.0, 20.0),
    );
    f.advance(44.0);
    f.full("covers_waveform", 56.0);
    f.section(
        "封面墙 CoverWall",
        "CoverWall::new(id).covers(...).tile(148).gap(14).on_select / on_open；虚拟化网格、滚轮与方向键、UIA List + Scroll。",
    );
    let wall_h = 340.0;
    if f.inner_width() >= 620.0 {
        let wall_w = (f.inner_width() * 0.62).max(320.0);
        let top = f.y;
        f.raw("covers_wall", Rect::from_xywh(PAD, top, wall_w, wall_h));
        let rx = PAD + wall_w + GAP;
        let pw = (f.inner_width() - wall_w - GAP).max(180.0);
        // The playlist's heading and API note sit above its list, beside the
        // wall; the list keeps the rest of the column.
        let used = f.caption_block(
            rx,
            top,
            pw,
            "播放列表 PlaylistView",
            "PlaylistView::new(id).rows(...).on_reorder(from, to) — 拖动行或 Alt+Up/Down 重排，应用自己保存顺序；UIA List + Selection + Scroll。",
        );
        f.raw(
            "covers_playlist",
            Rect::from_xywh(rx, top + used, pw, (wall_h - used).max(120.0)),
        );
        f.advance(wall_h);
    } else {
        f.full("covers_wall", wall_h);
        f.section(
            "播放列表 PlaylistView",
            "PlaylistView::new(id).rows(...).on_reorder(from, to) — 拖动行或 Alt+Up/Down 重排，应用自己保存顺序；UIA List + Selection + Scroll。",
        );
        f.full("covers_playlist", 260.0);
    }
    f.end_row();
    f
}

// ---------------------------------------------------------------------------
// Page 5 — ribbon
// ---------------------------------------------------------------------------

fn page_ribbon(w: f32) -> Flow {
    let mut f = Flow::new(w);
    f.section(
        "功能区 Ribbon",
        "Ribbon::new(id, RibbonTab 列表, Msg::RibbonCommand).on_tab(Msg)；大按钮 + 三行小按钮 + 分组；Alt 显示访问键，Right/Left 切选项卡，Down 进入命令区。",
    );
    f.full("gallery_ribbon", 152.0);
    f.demo("ribbon_state", 340.0, 22.0);
    f.end_row();
    f
}

// ---------------------------------------------------------------------------
// Page 6 — property grid
// ---------------------------------------------------------------------------

fn page_props(w: f32) -> Flow {
    let mut f = Flow::new(w);
    f.section(
        "属性表格 PropertyGrid",
        "PropertyGrid::new(id).sections(...).on_change(key, 值)；行类型：文本、数字、选项、复选、评分；类型化访问 set_value / set_checked / set_row_read_only；WidgetProps::read_only 锁整个控件。",
    );
    let grid_h = 560.0;
    if f.inner_width() >= 760.0 {
        let grid_w = (f.inner_width() * 0.64).max(360.0);
        let top = f.y;
        f.raw("gallery_props", Rect::from_xywh(PAD, top, grid_w, grid_h));
        let bx = PAD + grid_w + 18.0;
        let bw = (f.inner_width() - grid_w - 18.0).max(120.0);
        f.raw(
            "props_toggle",
            Rect::from_xywh(bx, top, bw.min(130.0), 32.0),
        );
        f.raw(
            "props_reset",
            Rect::from_xywh(bx, top + 44.0, bw.min(130.0), 32.0),
        );
        f.raw(
            "props_lock",
            Rect::from_xywh(bx, top + 88.0, bw.min(130.0), 32.0),
        );
        f.raw("props_readonly", Rect::from_xywh(bx, top + 132.0, bw, 28.0));
        f.raw("props_state", Rect::from_xywh(bx, top + 172.0, bw, 60.0));
        f.advance(grid_h);
    } else {
        f.full("gallery_props", grid_h);
        f.demo("props_toggle", 130.0, 32.0);
        f.demo("props_reset", 130.0, 32.0);
        f.demo("props_lock", 130.0, 32.0);
        f.demo("props_readonly", 150.0, 28.0);
        f.full("props_state", 60.0);
    }
    f.end_row();
    f
}

// ---------------------------------------------------------------------------
// Page 7 — functional API
// ---------------------------------------------------------------------------

fn page_api(w: f32) -> Flow {
    let mut f = Flow::new(w);
    f.section(
        "对话框 Dialog",
        "WindowHandle::show_dialog(MessageDialog / FileDialog / FontDialog / ColorDialog)；模态会禁用宿主，.parallel() 并行打开；结果经 Host::dialog_result 返回。",
    );
    for id in [
        "api_msg",
        "api_parallel",
        "api_custom",
        "api_file_open",
        "api_file_save",
        "api_folder",
        "api_font",
        "api_color",
    ] {
        f.demo(id, 150.0, 34.0);
    }
    f.section(
        "菜单与上下文 Menu",
        "Menu::new().item(id, 文本).shortcut_item(...).checked_item(...).submenu(...)；Ui::open_menu(id, 锚点, sink) 从代码打开；WidgetProps::context_menu 配右键、菜单键与 Shift+F10。",
    );
    f.demo("api_menu", 170.0, 34.0);
    f.demo("api_context", 170.0, 34.0);
    f.section(
        "托盘 Tray",
        "WindowHandle::set_tray_icon(TrayIcon::new(提示).menu(...))；set_tray_icon_at(id, ..) 支持多个图标；show_tray_balloon 发通知气泡；Host::tray_event 收回点击。",
    );
    f.demo("api_tray_add", 150.0, 34.0);
    f.demo("api_tray_balloon", 150.0, 34.0);
    f.demo("api_tray_remove", 150.0, 34.0);
    f.section(
        "系统通知 Notification",
        "WindowBuilder::app_id(\"...\") 设置 AppUserModelID；WindowHandle::notify(&Notification::new(标题, 正文)) 走 Windows Toast，被拒绝时回退到托盘气泡并返回 NotificationResult。",
    );
    f.demo("api_notify", 150.0, 34.0);
    f.section(
        "全局热键 Hotkey",
        "register_hotkey(Hotkey::parse(\"Ctrl+Alt+F8\"))；begin_hotkey_capture 让下一次按键成为热键；unregister_hotkey；Host::hotkey 接收触发。",
    );
    f.demo("api_hotkey_add", 150.0, 34.0);
    f.demo("api_hotkey_capture", 150.0, 34.0);
    f.demo("api_hotkey_remove", 150.0, 34.0);
    f.section(
        "窗口 Window",
        "minimize / maximize / restore / hide / show / request_close / is_visible / visual_state；invalidate_all_force + flush 立即重绘；set_title 改标题。",
    );
    for id in [
        "api_min",
        "api_max",
        "api_restore",
        "api_hide",
        "api_close",
        "api_flush",
        "api_title",
    ] {
        f.demo(id, 132.0, 34.0);
    }
    f.section(
        "无障碍与剪贴板 A11y",
        "a11y_announce(文本) 发布读屏通知；a11y_refresh() 在结构变化后重建快照；a11y_focus(id)；WindowHandle::copy_text 写剪贴板；Ui::apply_a11y 接收屏幕阅读器动作。",
    );
    f.demo("api_announce", 150.0, 34.0);
    f.demo("api_copy", 160.0, 34.0);
    f.demo("api_refresh", 170.0, 34.0);
    f.demo("api_theme", 170.0, 34.0);
    f.section(
        "主题文档 .buitheme",
        "ThemeDocument::parse(文本) → palette(&ThemeSettings) → WindowHandle::set_palette；整张调色板是数据：base 决定明暗，accent 参与派生，<color> 逐项覆盖，布局里的颜色 token 随之解析。",
    );
    f.demo("api_theme_doc", 150.0, 34.0);
    f.demo("api_theme_state", 360.0, 20.0);
    f.section(
        "手柄与渲染器 Gamepad · Renderer",
        "Gamepad::poll(now, 前台) 把 D-pad / 摇杆 / A / B / X / Y / RB / LB 翻译成按键，Ui::pad_key 还会为文本框拉起屏幕键盘；window.renderer() 报告实际生效的 GDI / Direct2D。",
    );
    f.demo("api_pad", 190.0, 28.0);
    f.demo("api_pad_state", 320.0, 20.0);
    f.demo("api_renderer", 260.0, 20.0);
    f.section(
        "自定义控件 CustomWidget",
        "CustomWidget::new(id, |ctx, rect, state| { … })：一个绘制闭包 + 一个输入闭包，注册方式和内置控件一样（ui.add / add_with、WidgetProps、焦点环、a11y、corner_radius/background 等 surface）。需要自有状态的复杂控件直接实现 Widget（crates/showcase/src/meter.rs）；XML 布局可另加 skin_* 方法（docs/CUSTOM-WIDGETS.md）。",
    );
    f.demo("api_custom_draw", 240.0, 56.0);
    f.demo("api_custom_state", 260.0, 20.0);
    f.end_row();
    f.section(
        "跨控件拖放 Drag and drop",
        "Ui::set_drag_source(id, || Some(Msg))；set_drop_target(id, |payload, pos| Some(Msg))；set_drop_hover(id, |载荷, 位置, over| 重绘)。按住拖动超过 4 DIP 开始，Esc / 移除 / 切页中止。",
    );
    f.demo("drag_a", 240.0, 96.0);
    f.demo("drag_b", 240.0, 96.0);
    f.end_row();
    let drop_top = f.y;
    f.raw("drop_a", Rect::from_xywh(PAD, drop_top, 240.0, 96.0));
    f.raw(
        "drop_b",
        Rect::from_xywh(PAD + 254.0, drop_top, 240.0, 96.0),
    );
    f.advance(96.0);
    f.full("api_drag_state", 40.0);
    f.section(
        "文本拖放 Text drag and drop",
        "Ui::set_text_drag_source(id)：在已选文字内按住并移动，选中的文本就是载荷（源保留文字）；set_text_drop_target(id, |文本, 位置| Some(Msg))：自绘输入框在落点插入，宿主收到消息。文本载荷只到文本目标，消息载荷只到消息目标。",
    );
    f.demo("api_text_from", 280.0, 34.0);
    f.demo("api_text_to", 280.0, 34.0);
    f.full("api_text_state", 24.0);
    f.end_row();
    f
}

/// Page 8 — the M7 layout skin (`m7` feature). The page is valid before any
/// document is read: this is the plain vertical fallback, matching the base
/// variant's geometry in `layouts/layout-demo.buixml`. `m7_status` is
/// Rust-owned (the file never mentions it) and shows the last
/// `LayoutReport`/failure.
#[cfg(feature = "m7")]
fn page_layout(w: f32) -> Flow {
    let mut f = Flow::new(w);
    let inner = (w - PAD * 2.0).max(120.0);
    // Status line first: the document starts its content below it.
    f.raw("m7_status", Rect::from_xywh(PAD, 0.0, inner, 20.0));
    f.raw("m7_title", Rect::from_xywh(PAD, 28.0, inner, 30.0));
    f.raw("m7_action", Rect::from_xywh(PAD, 64.0, inner, 36.0));
    f.raw("m7_outline", Rect::from_xywh(PAD, 108.0, inner, 36.0));
    f.raw("m7_progress", Rect::from_xywh(PAD, 152.0, inner, 30.0));
    f.raw("m7_card", Rect::from_xywh(PAD, 190.0, inner, 96.0));
    f.raw("m7_edit", Rect::from_xywh(PAD, 294.0, inner, 32.0));
    f.raw("m7_slider", Rect::from_xywh(PAD, 334.0, inner, 28.0));
    f.raw("m7_value", Rect::from_xywh(PAD, 370.0, 80.0, 28.0));
    f.raw("m7_list", Rect::from_xywh(PAD, 406.0, inner, 200.0));
    f.raw("m7_meter", Rect::from_xywh(PAD, 618.0, inner, 92.0));
    f.raw("m7_table", Rect::from_xywh(PAD, 728.0, inner, 180.0));
    f.raw("m7_tree", Rect::from_xywh(PAD, 926.0, inner, 180.0));
    f.raw("m7_props", Rect::from_xywh(PAD, 1124.0, inner, 180.0));
    f.raw("m7_pager", Rect::from_xywh(PAD, 1322.0, inner, 32.0));
    f.raw("m7_tabs", Rect::from_xywh(PAD, 1370.0, inner, 32.0));
    f.raw("m7_seg", Rect::from_xywh(PAD, 1420.0, inner, 32.0));
    f.raw("m7_crumb", Rect::from_xywh(PAD, 1470.0, inner, 24.0));
    f.raw("m7_rail", Rect::from_xywh(PAD, 1506.0, inner, 140.0));
    f.raw("m7_rich", Rect::from_xywh(PAD, 1664.0, inner, 24.0));
    f.raw("m7_spark", Rect::from_xywh(PAD, 1698.0, inner, 36.0));
    // Created by the document itself (M7.7); listed here so the page
    // visibility pass covers it.
    f.raw("m7_created", Rect::from_xywh(PAD, 1744.0, inner, 22.0));
    f.advance(1770.0);
    f
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_page_registers_each_child_once_and_no_id_crosses_pages() {
        let (_, pages) = layout(Size::new(1100.0, 820.0));
        for page in &pages {
            let ids: Vec<&str> = page.children.iter().map(|child| child.id.0).collect();
            let mut unique = ids.clone();
            unique.sort_unstable();
            unique.dedup();
            assert_eq!(
                unique.len(),
                ids.len(),
                "duplicate child in page {:?}",
                page.panel
            );
        }
        let mut all: Vec<&str> = pages
            .iter()
            .flat_map(|page| page.children.iter().map(|child| child.id.0))
            .collect();
        let total = all.len();
        all.sort_unstable();
        all.dedup();
        assert_eq!(all.len(), total, "an id is registered on more than one page");
        // The empty state that showed repeated is registered exactly once.
        let empty: Vec<&str> = pages
            .iter()
            .flat_map(|page| page.children.iter())
            .filter(|child| child.id.0 == "gallery_empty")
            .map(|child| child.id.0)
            .collect();
        assert_eq!(empty.len(), 1);
    }

    #[test]
    fn every_demo_and_caption_lies_inside_its_page_content() {
        // The scroll panel culls children outside the content extent, so a
        // demo placed under it would silently never paint. Check both a wide
        // and a narrow layout (rows wrap at the minimum width).
        for size in [Size::new(1100.0, 820.0), Size::new(760.0, 560.0)] {
            let (_, pages) = layout(size);
            for page in &pages {
                for child in &page.children {
                    assert!(
                        child.rect.bottom <= page.content,
                        "{} ends at {} beyond content {} at {size:?}",
                        child.id.0,
                        child.rect.bottom,
                        page.content
                    );
                }
                for caption in &page.captions {
                    assert!(
                        caption.rect.bottom <= page.content,
                        "caption {:?} ends at {} beyond content {} at {size:?}",
                        caption.text,
                        caption.rect.bottom,
                        page.content
                    );
                }
            }
        }
    }
}
