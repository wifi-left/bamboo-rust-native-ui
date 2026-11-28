// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Bamboo Native UI showcase — M2 widget library.
//!
//! Demonstrates the declarative `Ui<M>` container: every standard control is
//! added once, positioned on resize, and talks to the application through
//! messages. Hovering repaints two rectangles; scrolling the list only paints
//! visible rows; the focus ring is Tab/arrow driven.
//!
//! Run:
//!   cargo run -p bamboo-native-ui-showcase
//!   cargo run -p bamboo-native-ui-showcase -- --self-test
//!   cargo run -p bamboo-native-ui-showcase -- --a11y-dump
//!   cargo run -p bamboo-native-ui-showcase -- --bench 3
//!   cargo run -p bamboo-native-ui-showcase -- --light

use std::ffi::c_void;
use std::process::ExitCode;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

mod catalog;
#[cfg(feature = "m7")]
mod meter;
mod uia_probe;
use bamboo_native_ui::Palette;
#[cfg(feature = "m7")]
use bamboo_native_ui::layout_xml::LayoutDocument;

use catalog::{
    layout, register_pages, LayoutSink, PageSpec, Scene, PAGE_HINTS, PAGE_NAMES, PAGE_SCROLLS,
};

use bamboo_native_ui::widgets::{
    Accordion, Avatar, Badge, BadgeTone, Banner, BannerKind, Breadcrumb, Button, Card, Checkbox,
    ColorPicker, ColorSwatch, ComboBox, CopyableText, Cover, CoverWall, Date, DatePicker, Divider,
    EditColors, EmptyState, Gauge,
    GroupBox, Icon, Kbd, Label, LabelStyle, ListView, MarqueeText, MenuBar, MenuButton,
    MultiSelectList,
    NavRail, NumberInput, Pager, PasswordBox, PlaybackTransport, PlaylistView, Popover,
    PopoverPlacement, ProgressBar, ProgressRing, PropertyGrid, PropertyRow, PropertySection,
    RadioButton, RadioGroup, RatingStars, RepeatMode,
    Ribbon, RibbonGroup, RibbonItem, RibbonTab, RichText, Scrim, SearchBox, SectionTitle,
    Segmented, Slider, Sparkline, Spinner, SplitButton, Splitter, StatCard, StatusBar,
    StatusSegment, Table, TableColumn, TableRow, Tabs, TextArea, TextEdit, Time, TimePicker, Toast,
    Toggle, Toolbar, ToolbarItem, TooltipCard, TransportAction, TreeItem, TreeView, WaveformSeekBar,
    place_popover,
};
#[cfg(feature = "m7")]
use bamboo_native_ui::widgets::PropertyKind;
#[cfg(feature = "m7")]
use bamboo_native_ui::widgets::RunStyle;
use bamboo_native_ui::{
    A11yAction, A11yRole, Canvas, CloseReason, Color, ColorDialog, Corners, Cursor,
    CustomWidget, DialogId,
    DialogResult,
    DragData, DropEffect,
    Event,
    FileDialog, FontDialog, Gamepad, Host, Hotkey, HotkeyModifiers, ImageId, KeyCode, KeyEvent, Menu,
    MenuBarModel, MenuDecision, MenuEntry, MenuEvent, MenuIcon, MenuItem, MenuPath,
    MessageAnswer, MessageButtons,
    MessageDialog, MessageIcon, MouseButton, Narration, PaintCtx, Point,
    Notification, NotificationResult,
    PointerModifiers, Rect, Rgba,
    Row, ScrollBars,
    Size, TextAlign, TextStyleRole, ThemeSettings, TrayAction, TrayEvent,
    TrayIcon, WindowVisualState,
    Trigger, Ui, UiImage, Widget, WidgetId, WidgetInput, WidgetProps, WindowBuilder, WindowHandle,
    read_gamepad,
};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::UI::Input::KeyboardAndMouse::IsWindowEnabled;
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, FindWindowW, GetClassNameW, GetWindowThreadProcessId, IsWindowVisible,
    PostMessageW, SendMessageW, WM_CLOSE, WM_COMMAND, WM_KEYDOWN,
};
use windows::Win32::UI::Controls::TDM_CLICK_BUTTON;
use windows::core::{BOOL, PCWSTR};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let bench = bench_seconds(&args);
    let self_test = bench.is_none() && args.iter().any(|arg| arg == "--self-test");
    let a11y_dump = bench.is_none() && args.iter().any(|arg| arg == "--a11y-dump");
    let light = args.iter().any(|arg| arg == "--light");
    // `--renderer gdi|d2d-software|d2d-hardware` is the same switch as the
    // `BAMBOO_UI_RENDERER` environment variable, for convenience in scripts.
    if let Some(index) = args.iter().position(|arg| arg == "--renderer") {
        if let Some(name) = args.get(index + 1) {
            unsafe { std::env::set_var("BAMBOO_UI_RENDERER", name) };
        }
    }

    let painted = Arc::new(AtomicU64::new(0));
    let paint_micros = Arc::new(AtomicU64::new(0));
    let ticks = Arc::new(AtomicU64::new(0));
    let invalids = Arc::new(AtomicU64::new(0));
    let a11y_result: Arc<Mutex<Option<Result<String, String>>>> = Arc::new(Mutex::new(None));
    let renderer_seen: Arc<Mutex<Option<bamboo_native_ui::Renderer>>> = Arc::new(Mutex::new(None));
    let host = Showcase::new(
        light,
        self_test,
        a11y_dump,
        bench,
        painted.clone(),
        paint_micros.clone(),
        ticks.clone(),
        invalids.clone(),
        a11y_result.clone(),
        renderer_seen.clone(),
    );
    let theme = host.theme.clone();
    let dark = host.dark;

    let result = WindowBuilder::new(host)
        .title("Bamboo Native UI — Showcase")
        .size(1180.0, 840.0)
        .min_size(760.0, 560.0)
        .app_id("BambooNativeUi.Showcase")
        .menu_bar(app_menu_bar())
        .theme(theme, dark)
        .run();

    if a11y_dump {
        let probe = a11y_result.lock().ok().and_then(|mut slot| slot.take());
        return match probe {
            Some(Ok(dump)) => {
                println!("{dump}");
                ExitCode::SUCCESS
            }
            Some(Err(error)) => {
                eprintln!("a11y-dump: {error}");
                ExitCode::FAILURE
            }
            None => {
                eprintln!("a11y-dump: the UIA probe did not run");
                ExitCode::FAILURE
            }
        };
    }

    match result {
        Err(error) => {
            eprintln!("showcase: {error}");
            ExitCode::FAILURE
        }
        Ok(code) if bench.is_none() && !self_test => ExitCode::from(code.clamp(0, 255) as u8),
        Ok(0) if painted.load(Ordering::Relaxed) > 0 => {
            let frames = painted.load(Ordering::Relaxed);
            let avg = paint_micros.load(Ordering::Relaxed) / frames.max(1);
            let private = bamboo_native_ui::diagnostics::private_bytes()
                .map(|bytes| format!("{:.1}MB", bytes as f64 / (1024.0 * 1024.0)))
                .unwrap_or_else(|| "n/a".to_string());
            let renderer = renderer_seen
                .lock()
                .ok()
                .and_then(|slot| *slot)
                .unwrap_or_default();
            if let Some(seconds) = bench {
                println!(
                    "bench {seconds}s: frames={frames} ticks={} invalids={} avg_paint={avg}us private={private} renderer={}",
                    ticks.load(Ordering::Relaxed),
                    invalids.load(Ordering::Relaxed),
                    renderer.name(),
                );
            } else {
                println!(
                    "self-test: painted={frames} avg_paint={avg}us private={private} renderer={}",
                    renderer.name()
                );
            }
            ExitCode::SUCCESS
        }
        Ok(code) => {
            eprintln!("self-test failed: exit={code} painted=0");
            ExitCode::FAILURE
        }
    }
}

fn bench_seconds(args: &[String]) -> Option<f32> {
    if let Some(index) = args.iter().position(|arg| arg == "--bench") {
        return args
            .get(index + 1)
            .and_then(|value| value.parse().ok())
            .or(Some(3.0));
    }
    args.iter()
        .find_map(|arg| arg.strip_prefix("--bench=").and_then(|value| value.parse().ok()))
}

#[derive(Clone, Debug)]
enum Msg {
    Primary,
    Secondary,
    Danger,
    Toggle(bool),
    Segment(usize),
    Tab(usize),
    Volume(f32),
    PlayRow(usize),
    Page(i32),
    Theme,
    Search(String),
    PaintedName(String),
    Count(i64),
    /// A rounded-surface demo button was clicked (its corner radius in DIPs).
    RoundClick(f32),
    /// The rounded-surface demo's pill search box committed text.
    RoundSearch(String),
    /// The API page's text-drag source committed an inline edit.
    ApiTextCommitted(String),
    /// Text dropped onto the API page's second field (the container inserted
    /// it; the message reports what arrived).
    TextDropped(String),
    Swatch,
    SwatchContext,
    SwatchDouble,
    RemoveTag,
    OpenLink,
    CopyText,
    RichLink(u32),
    MarqueeStart,
    MarqueeStop,
    Random(bool),
    Shuffle(bool),
    Square(bool),
    RadioOnly,
    RadioGroupPick(usize),
    GallerySearch(String),
    GalleryPassword(String),
    GalleryCombo(usize),
    GalleryDate(Date),
    GalleryTime(Time),
    GalleryColorRgba(Rgba),
    GalleryColorMore,
    GalleryNotes(String),
    GalleryNotesRaw(String),
    GalleryMulti(Vec<usize>),
    GalleryAccordion(usize, bool),
    GalleryBreadcrumb(usize),
    GalleryNavRail(usize),
    GallerySplit(f32),
    GalleryRating(u8),
    GalleryTableSelect(usize),
    GalleryTableOpen(usize),
    GalleryTableSort(usize),
    GalleryTableCell(usize, usize, String),
    GalleryTreeSelect(Vec<usize>),
    GalleryTreeOpen(Vec<usize>),
    GalleryTreeToggle(Vec<usize>, bool),
    GalleryTreeRename(Vec<usize>, String),
    GalleryTreeMove(Vec<usize>, Vec<usize>),
    /// The cover wall's current cover changed.
    CoverSelect(usize),
    /// A cover was opened (double click or Enter).
    CoverOpen(usize),
    /// The playlist's cursor moved to another track (click or assistive tech).
    PlaylistSelect(usize),
    /// A playlist track was played (double click or Enter).
    PlaylistOpen(usize),
    /// The playlist asked the application to move a track (`from`, `to`).
    PlaylistReorder(usize, usize),
    /// Open the popover/flyout demo.
    FlyoutOpen,
    /// The popover asked to close (outside press or its own button).
    FlyoutDismiss,
    /// A ribbon command was run (900..).
    RibbonCommand(u32),
    /// The ribbon tab changed.
    RibbonTab(usize),
    /// The property grid's cursor moved to another row (its key).
    PropertySelect(String),
    /// The property grid committed an edit (key, new value).
    PropertyChange(String, String),
    /// A read-only row was activated (key, value).
    PropertyInspect(String, String),
    /// The "切换收藏" button flipped the property through typed access.
    PropertyToggle,
    /// The "恢复默认" button replaced the property sections.
    PropertyReset,
    /// The "锁定选中项" button toggled one row's lock.
    PropertyLock,
    /// The "只读模式" check box pushes `WidgetProps::read_only` at runtime.
    PropertyReadOnly(bool),
    /// The waveform seek bar was scrubbed (`0..=1`).
    Seek(f32),
    /// A transport button (shuffle/previous/play/next/repeat) was pressed.
    Transport(TransportAction),
    /// A row of the scroll-view demo was clicked (index 0..8).
    ScrollRow(usize),
    /// A checkbox inside the scroll-view demo toggled.
    ScrollCheck(usize, bool),
    DynAdd,
    DynRemove,
    DynRaise,
    DynLower,
    DynItem(usize),
    OpenModal,
    ModalCancel,
    ModalConfirm,
    ModalCheck(bool),
    BannerRetry,
    BannerClose,
    /// A command from the gallery menu bar, toolbar, menu button or split
    /// button. Ids are application-defined like context-menu commands.
    Chrome(u32),
    // ---- Functional-API page -----------------------------------------------
    ApiMessage,
    ApiParallel,
    ApiCustom,
    CustomDrawClicked,
    ApiFileOpen,
    ApiFileSave,
    ApiFolder,
    ApiFont,
    ApiColor,
    ApiMenu,
    ApiContext,
    ApiTrayAdd,
    ApiTrayBalloon,
    ApiSystemNotify,
    ApiTrayRemove,
    ApiHotkeyAdd,
    ApiHotkeyCapture,
    ApiHotkeyRemove,
    ApiMinimize,
    ApiMaximize,
    ApiRestore,
    ApiHide,
    ApiClose,
    ApiFlush,
    ApiTitle,
    ApiAnnounce,
    ApiCopy,
    ApiRefresh,
    ApiTheme,
    /// Load the checked-in `.buitheme` document (M7.8, D150).
    #[cfg(feature = "m7")]
    ApiThemeDoc,
    /// Drag payload: which source card is being dragged (0 = A, 1 = B).
    DragCard(u8),
    /// A card was dropped on a target (`source`, `target`).
    Dropped(u8, u8),
    /// The pad-as-keyboard switch in the API page changed.
    PadEnable(bool),
    /// A drop zone was clicked (the drop path uses `Dropped`).
    DropZone,
    // ---- Layout-skin page (`m7`) -------------------------------------------
    /// A button on the 布局 page was pressed.
    #[cfg(feature = "m7")]
    M7Action(&'static str),
    /// The 布局 page's edit field committed.
    #[cfg(feature = "m7")]
    M7Edit(String),
    /// The 布局 page's slider moved (`0..=1`).
    #[cfg(feature = "m7")]
    M7Value(f32),
    /// A skin-driven list on the 布局 page selected an item (the widget
    /// already applied the selection; the demo reports it).
    #[cfg(feature = "m7")]
    M7Select(usize),
}

/// The popover demo: the card plus its content widgets. They follow the card in
/// the tree so they paint above it and receive input first.
const FLYOUT: [&str; 4] = ["flyout", "flyout_title", "flyout_body", "flyout_close"];
/// Scroll-view demo rows. They are laid out in *content* coordinates (the
/// viewport origin is `(0, 0)`); the container positions them at
/// `content - offset`, clips them and drags the bars.
const SCROLL_ROWS: [&str; 8] = [
    "scroll_row_1",
    "scroll_row_2",
    "scroll_row_3",
    "scroll_row_4",
    "scroll_row_5",
    "scroll_row_6",
    "scroll_row_7",
    "scroll_row_8",
];
/// Accessibility / identity of the scroll-view demo viewport.
const SCROLL_PANEL: &str = "data_scroll";
/// A nested scroll view inside the page content: its viewport is a rectangle in
/// the outer panel's content coordinates, so the outer scroll moves and clips
/// it too (D92).
const NEST_PANEL: &str = "data_nested";
const NEST_ROWS: [&str; 4] = ["nested_row_1", "nested_row_2", "nested_row_3", "nested_row_4"];
/// The dynamic menu demo: a menu button whose model the application edits at
/// runtime (`insert`/`remove_command`/`set_checked`) and re-opens from code.
const DYN_MENU_ID: &str = "gallery_dynmenu";
/// The dynamic widget-tree demo's fixed pool. A slot is removed with
/// `Ui::remove` and re-created with `Ui::insert`, so its id is reusable and no
/// ids are allocated at runtime.
const DYN_SLOT_IDS: [&str; 2] = ["dyn_slot_1", "dyn_slot_2"];
/// Command of the self-test's programmatic menu (routed through `Msg::Chrome`).
const PROBE_MENU_COMMAND: u32 = 745;
/// Item the self-test inserts into the context menu *while it is open*, to
/// prove the popup is rebuilt from the live model.
const PROBE_LIVE_COMMAND: u32 = 746;
/// First command id of a runtime-added dynamic menu item.
const DYN_FIRST_COMMAND: u32 = 750;
/// Command of the self-test's native menu bar selection.
const PROBE_MENU_BAR_COMMAND: u32 = 830;

/// The native window menu bar (the system-drawn bar under the title bar).
/// Labels keep their `&` access key; the system underlines it while Alt is
/// held, and the shortcut labels (Ctrl+P, F5…) are live window accelerators:
/// the window procedure matches them and routes the command through
/// `Host::menu_bar_command`.
fn app_menu_bar() -> MenuBarModel {
    let bar = MenuBarModel::new()
        .item(
            "程序(&P)",
            Menu::new()
                .shortcut_item(800, "播放 / 暂停", "Ctrl+P")
                .shortcut_item(801, "打开文件…", "Ctrl+O")
                .separator()
                .shortcut_item(802, "退出", "Alt+F4"),
        )
        .item(
            "页面(&P)",
            Menu::new()
                .item(810, "搜索")
                .item(811, "播放页")
                .item(812, "下载任务"),
        )
        .separator()
        .item(
            "工具(&T)",
            Menu::new()
                .checked_item(820, "深色主题", true)
                .item(821, "捕获快捷键…")
                .separator()
                .item(822, "显示组件页")
                .item(823, "显示数据页")
                .shortcut_item(824, "刷新示例", "F5"),
        )
        .separator();
    // The `调试` menu exists only in the M7 showcase build: the debug API is
    // behind `debug-form`, and the default catalog has no debug code.
    #[cfg(feature = "m7")]
    let bar = bar
        .item(
            "调试(&D)",
            Menu::new()
                .item(840, "布局快照…")
                .item(841, "控件树…")
                .item(842, "帧统计…")
                .item(843, "诊断日志…")
                .separator()
                .item(844, "重载布局")
                .item(845, "保存布局")
                .item(846, "重置布局")
                .separator()
                .item(847, "完整控件树到控制台"),
        )
        .separator();
    bar.item("帮助(&H)", Menu::new().item(830, "关于").item(831, "帮助文档"))
}

/// The dynamic menu's model. The application owns the value; `Ui::set_menu`
/// installs it into the container registry and the button's own model at once.
fn dynamic_menu(extra: usize, checked: bool) -> Menu {
    let mut menu = Menu::new()
        .checked_item(706, "自动续播", checked)
        .separator()
        .item(730, "添加一项")
        .item(731, "移除一项")
        .separator();
    for index in 0..extra {
        menu.insert(
            menu.len(),
            MenuEntry::item(
                DYN_FIRST_COMMAND + index as u32,
                format!("动态项 {}", index + 1),
            ),
        );
    }
    let submenu = Menu::new()
        .item(760, "子项一")
        .item(761, "子项二");
    menu.insert(menu.len(), MenuEntry::submenu("更多", submenu));
    menu.insert(menu.len(), MenuEntry::item(733, "展开「更多」（代码）"));
    menu.insert(menu.len(), MenuEntry::item(732, "再次打开（代码）"));
    menu
}
/// The in-window modal shell: scrim first, then the panel widgets on top.
const MODAL: [&str; 5] = [
    "modal_scrim",
    "modal_card",
    "modal_check",
    "modal_cancel",
    "modal_ok",
];

/// `--self-test` drives one native edit session so the host path is exercised
/// by the automated run: open on `search`, expect the editor, Tab commits, and
/// the committed text must match what the widget started with. It then focused
/// the primary button with the ring on and waits for the tooltip host to show
/// the button's tip after its dwell. Finally it right-clicks the button, waits
/// for the native context menu to open and cancels it, then opens a modal and a
/// parallel message dialog in turn: the modal one must be visible while the
/// owner is disabled, the parallel one while the owner stays enabled.
struct EditProbe {
    step: u8,
    opened: bool,
    committed: Option<String>,
    tooltip: bool,
    menu: bool,
    /// The shell accepted the tray icon.
    tray: bool,
    /// The shell accepted a tray notification balloon.
    tray_balloon: bool,
    /// A second tray icon was accepted and removed through its own id.
    tray_multi: bool,
    /// Close requests seen, by source ("user"/"system"/"app").
    close_reasons: Vec<&'static str>,
    /// The first frame painted (`Host::loaded`).
    loaded_frame: bool,
    /// Show/hide and visual-state callbacks observed.
    visibility_hidden: bool,
    visibility_shown: bool,
    min_seen: bool,
    normal_seen: bool,
    /// The window lifecycle API round trip (loaded, hide/show, vetoed close,
    /// minimize/restore).
    window_api: bool,
    /// The next close request is vetoed (proves the veto path).
    veto_close: bool,
    modal_id: Option<DialogId>,
    parallel_id: Option<DialogId>,
    custom_dialog_id: Option<DialogId>,
    /// The custom-button task dialog reported Custom(501) after a click.
    custom_buttons: bool,
    /// Modal dialog seen while the owner was disabled.
    modal_blocks: bool,
    /// Parallel dialog seen while the owner stayed enabled.
    parallel_free: bool,
    modal_result: bool,
    parallel_result: bool,
    /// Pickers left to open (file, font, color); each is cancelled once it
    /// appears, and the result callback must fire for each.
    picker_left: u8,
    picker: Option<PickerKind>,
    picker_id: Option<DialogId>,
    picker_seen: usize,
    picker_results: usize,
    /// The system accepted the demo hotkey registration.
    hotkey_registered: bool,
    /// What the capture step produced.
    hotkey_captured: Option<Hotkey>,
    /// The `MenuEvent` hook saw a full lifecycle of the programmatic menu.
    menu_opening: bool,
    menu_opened: bool,
    menu_selected: bool,
    menu_closed: bool,
    /// `Opened` reached the hook while the popup was still up.
    menu_opened_while_open: bool,
    /// How many `Opened` events the hook saw (a live rebuild fires another).
    menu_opened_count: u8,
    /// A timer-side `edit_open_menu` added an item to the live popup.
    menu_live_edit: bool,
    /// The rebuilt popup's `Opened` carried the added item.
    menu_live_content: bool,
    /// The hook saw `Selected` with the probe command.
    menu_probe_selected: bool,
    menu_closed_command: Option<u32>,
    /// `active_menu_widget` while the programmatic popup was up.
    menu_controller_widget: Option<WidgetId>,
    /// The `menu_click` replay reached `Host::menu_command`.
    menu_command_command: Option<u32>,
    /// The window has a native menu bar installed.
    menu_bar: bool,
    /// Focusing a row below the fold scrolled the demo panel into view.
    scroll_reveal: bool,
    /// The table edited a cell through the inline draft and reported it.
    table_edit: bool,
    /// A cell value containing a newline edited with the painted multiline
    /// model (Enter kept the session open, Escape left the data untouched).
    table_multiline: bool,
    /// The tree renamed a row through F2 and moved one by drag.
    tree_edit: bool,
    /// The wheel over the demo panel returned it to the top.
    scroll_wheel: bool,
    /// A nested scroll view scrolled first and then handed the notch to the
    /// page it sits in.
    nested_scroll: bool,
    /// End/Home on the cover wall moved the cursor to the ends and scrolled.
    covers: bool,
    /// A click on the waveform seek bar moved it and the time label.
    waveform: bool,
    /// A click on the transport's play cell flipped playback and the state
    /// label.
    transport: bool,
    /// A pointer drag on the playlist reported a reorder and the application
    /// pushed the new order back.
    playlist_drag: bool,
    /// The popover opened below its trigger and an outside press dismissed it.
    flyout: bool,
    /// Chrome refinements: a vertical toolbar, separators/icon-only cells, the
    /// status progress setter and the Alt-tap underline mode.
    chrome: bool,
    /// The ribbon switched tabs, ran a command and moved the selection back.
    ribbon: bool,
    /// A click on the property grid's editable value changed it through the
    /// edit message and typed access.
    props: bool,
    /// The gallery's date picker opened, moved the cursor and chose a date.
    date_picker: bool,
    /// The gallery's time and color pickers opened and chose a value.
    time_color: bool,
    /// The API page's cross-widget drag went from a card to a drop target.
    api_drag: bool,
    /// A text drag carried a painted field's selection into the second field.
    text_drag: bool,
    /// A click on the application's `CustomWidget` reached its input closure
    /// and the app-owned state label followed.
    custom_widget: bool,
    /// The posted `WM_COMMAND` reached `Host::menu_bar_command`.
    menu_bar_command_command: Option<u32>,
    /// The posted `F5` was matched by the menu bar's accelerator label and
    /// reached `Host::menu_bar_command` as command 824.
    menu_hotkey: bool,
    /// The 布局 page installed its skin and the breakpoint switched (passes
    /// trivially without the `m7` feature).
    m7: bool,
    /// The checked-in `.buitheme` document loaded and its palette painted
    /// (passes trivially without the `m7` feature).
    theme_doc: bool,
}

impl EditProbe {
    fn new() -> Self {
        Self {
            step: 0,
            opened: false,
            committed: None,
            tooltip: false,
            menu: false,
            tray: false,
            tray_balloon: false,
            tray_multi: false,
            close_reasons: Vec::new(),
            loaded_frame: false,
            visibility_hidden: false,
            visibility_shown: false,
            min_seen: false,
            normal_seen: false,
            window_api: false,
            veto_close: false,
            modal_id: None,
            parallel_id: None,
            custom_dialog_id: None,
            custom_buttons: false,
            modal_blocks: false,
            parallel_free: false,
            modal_result: false,
            parallel_result: false,
            picker_left: 3,
            picker: None,
            picker_id: None,
            picker_seen: 0,
            picker_results: 0,
            hotkey_registered: false,
            hotkey_captured: None,
            menu_opening: false,
            menu_opened: false,
            menu_selected: false,
            menu_closed: false,
            menu_opened_while_open: false,
            menu_opened_count: 0,
            menu_live_edit: false,
            menu_live_content: false,
            menu_probe_selected: false,
            menu_closed_command: None,
            menu_controller_widget: None,
            menu_command_command: None,
            menu_bar: false,
            menu_bar_command_command: None,
            menu_hotkey: false,
            scroll_reveal: false,
            table_edit: false,
            table_multiline: false,
            tree_edit: false,
            scroll_wheel: false,
            nested_scroll: false,
            covers: false,
            waveform: false,
            transport: false,
            playlist_drag: false,
            flyout: false,
            chrome: false,
            ribbon: false,
            props: false,
            date_picker: false,
            time_color: false,
            api_drag: false,
            text_drag: false,
            custom_widget: false,
            m7: !cfg!(feature = "m7"),
            theme_doc: !cfg!(feature = "m7"),
        }
    }

    fn ok(&self) -> bool {
        self.failures().is_empty()
    }

    /// Names of the checks that have not passed yet, in the order the probe
    /// makes them. The timeout report prints this so a failed run names the
    /// missing effect instead of only the step number.
    fn failures(&self) -> Vec<&'static str> {
        let checks: [(&'static str, bool); 51] = [
            ("opened", self.opened),
            (
                "committed",
                self.committed.as_deref() == Some("Bamboo Native UI"),
            ),
            ("tooltip", self.tooltip),
            ("menu", self.menu),
            ("tray", self.tray),
            ("tray_balloon", self.tray_balloon),
            ("tray_multi", self.tray_multi),
            ("modal_blocks", self.modal_blocks),
            ("modal_result", self.modal_result),
            ("custom_buttons", self.custom_buttons),
            ("window_api", self.window_api),
            ("parallel_free", self.parallel_free),
            ("parallel_result", self.parallel_result),
            ("picker_seen", self.picker_seen == 3),
            ("picker_results", self.picker_results == 3),
            ("hotkey_registered", self.hotkey_registered),
            ("hotkey_captured", self.hotkey_captured == Some(SELF_TEST_HOTKEY)),
            ("menu_opening", self.menu_opening),
            ("menu_opened", self.menu_opened),
            ("menu_closed", self.menu_closed),
            ("menu_opened_while_open", self.menu_opened_while_open),
            ("menu_opened_count", self.menu_opened_count >= 2),
            ("menu_live_edit", self.menu_live_edit),
            ("menu_live_content", self.menu_live_content),
            ("menu_probe_selected", self.menu_probe_selected),
            (
                "menu_controller_widget",
                self.menu_controller_widget == Some(WidgetId::new(DYN_MENU_ID)),
            ),
            (
                "menu_command_command",
                self.menu_command_command == Some(PROBE_MENU_COMMAND),
            ),
            ("menu_bar", self.menu_bar),
            (
                "menu_bar_command_command",
                self.menu_bar_command_command == Some(PROBE_MENU_BAR_COMMAND),
            ),
            ("menu_hotkey", self.menu_hotkey),
            ("scroll_reveal", self.scroll_reveal),
            ("table_edit", self.table_edit),
            ("table_multiline", self.table_multiline),
            ("tree_edit", self.tree_edit),
            ("scroll_wheel", self.scroll_wheel),
            ("nested_scroll", self.nested_scroll),
            ("covers", self.covers),
            ("waveform", self.waveform),
            ("transport", self.transport),
            ("playlist_drag", self.playlist_drag),
            ("flyout", self.flyout),
            ("chrome", self.chrome),
            ("date_picker", self.date_picker),
            ("time_color", self.time_color),
            ("ribbon", self.ribbon),
            ("props", self.props),
            ("api_drag", self.api_drag),
            ("text_drag", self.text_drag),
            ("custom_widget", self.custom_widget),
            ("m7", self.m7),
            ("theme_doc", self.theme_doc),
        ];
        checks
            .iter()
            .filter(|(_, passed)| !passed)
            .map(|(name, _)| *name)
            .collect()
    }
}

/// Commands of the primary button's context menu. Ids are application
/// defined and nonzero; the handler in `Host::menu_command` matches on them.
/// Accelerator text and a couple of RGBA icons exercise the native columns.
fn primary_menu(shuffle: bool, gamepad: bool) -> Menu {
    Menu::new()
        .push(
            MenuItem::new(101, "执行主操作")
                .shortcut("Enter")
                .icon(demo_menu_icon(0x4C6EF5, 0x1C7ED6)),
        )
        .checked_item(102, "随机播放", shuffle)
        .checked_item(112, "手柄导航", gamepad)
        .item(106, "自定义按钮对话框…")
        .separator()
        .submenu(
            "更多",
            Menu::new()
                .shortcut_item(103, "复制当前次数", "Ctrl+C")
                .shortcut_item(104, "重置计数", "Ctrl+R")
                .item(109, "捕获快捷键…"),
        )
        .separator()
        .push(
            MenuItem::new(106, "模态提示框…")
                .shortcut("Ctrl+M")
                .icon(demo_menu_icon(0x12B886, 0x087F5B)),
        )
        .item(107, "并行提示框…")
        .shortcut_item(108, "打开文件…", "Ctrl+O")
        .item(111, "选择多个文件…")
        .separator()
        .disabled_item(105, "属性（示例禁用）")
}

/// A 16×16 RGBA icon for the native menu left column (a diagonal gradient).
fn demo_menu_icon(top: u32, bottom: u32) -> MenuIcon {
    const SIZE: usize = 16;
    let from = Color::from_hex(top);
    let to = Color::from_hex(bottom);
    let mut rgba = Vec::with_capacity(SIZE * SIZE * 4);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let t = (x + y) as f32 / (2.0 * (SIZE as f32 - 1.0));
            let mix = |a: u8, b: u8| (a as f32 * (1.0 - t) + b as f32 * t).round() as u8;
            rgba.extend_from_slice(&[
                mix(from.r, to.r),
                mix(from.g, to.g),
                mix(from.b, to.b),
                255,
            ]);
        }
    }
    MenuIcon::from_rgba(SIZE as u16, SIZE as u16, rgba)
}

/// Tray icon menu. Commands are app ids like widget menu ids.
fn tray_menu() -> Menu {
    Menu::new()
        .item(401, "显示主窗口")
        .item(402, "切换主题")
        .item(404, "显示通知")
        .separator()
        .item(403, "退出")
}

/// F9 without modifiers: what the self-test posts while capturing.
const SELF_TEST_HOTKEY: Hotkey = Hotkey {
    modifiers: HotkeyModifiers::NONE,
    vk: 0x78,
};

/// The API page's demo hotkey (Ctrl+Alt+F8), registered and unregistered from
/// the buttons.
fn demo_hotkey() -> Hotkey {
    Hotkey {
        modifiers: HotkeyModifiers::CTRL.with_alt(),
        vk: 0x77,
    }
}

/// Unique captions the self-test uses to find its own dialogs.
const CUSTOM_DIALOG_TITLE: &str = "自检：自定义按钮";
const MODAL_DIALOG_TITLE: &str = "Bamboo 自检 · 模态";
const PARALLEL_DIALOG_TITLE: &str = "Bamboo 自检 · 并行";
const FILE_DIALOG_TITLE: &str = "Bamboo 自检 · 文件";

/// One of the shell/comdlg32 pickers the self-test opens and cancels.
#[derive(Clone, Copy, Debug, PartialEq)]
enum PickerKind {
    File,
    Font,
    Color,
}

struct Showcase {
    theme: ThemeSettings,
    dark: bool,
    /// The palette a `.buitheme` document installed; `None` derives it from
    /// `theme`/`dark` (M7.8).
    #[cfg(feature = "m7")]
    theme_palette: Option<Palette>,
    self_test: bool,
    a11y_dump: bool,
    a11y_result: Arc<Mutex<Option<Result<String, String>>>>,
    /// The renderer the window actually paints with (a Direct2D failure falls
    /// back to GDI); reported in the bench/self-test line.
    renderer_seen: Arc<Mutex<Option<bamboo_native_ui::Renderer>>>,
    bench: Option<f32>,
    start: Instant,
    ui: Ui<Msg>,
    scene: Scene,
    /// The eight catalog pages (scroll panel, children, captions). Rebuilt on
    /// every resize; the container keeps each panel's live scroll offset.
    specs: Vec<PageSpec>,
    counter: u64,
    toggle_on: bool,
    volume: f32,
    shuffle: bool,
    playing_row: Option<usize>,
    page: usize,
    pages: usize,
    /// The active catalog tab (index into `PAGE_NAMES`).
    tab: usize,
    rows: Vec<Row>,
    toast_until: Option<Instant>,
    /// Last application action, shown in the bottom strip ("最近：…").
    last_action: String,
    status: String,
    last_status: (u64, u64, usize),
    pulse: Rect,
    /// The marquee scrolls only while hovered, so an idle window keeps its
    /// "no periodic repaint" contract.
    marquee_playing: bool,
    /// Optional XInput pad; its edges become navigation keys.
    gamepad: Option<Gamepad>,
    /// The application switch mirrored in the context menu (手柄导航).
    gamepad_enabled: bool,
    last_tick: Instant,
    /// Application-owned playback state the transport strip displays.
    playing: bool,
    transport_shuffle: bool,
    transport_repeat: RepeatMode,
    /// The in-window modal shell is up (scrim + panel + focus scope).
    modal_open: bool,
    /// The color dialog the gallery picker's "more colors…" row opened.
    gallery_color_dialog: Option<DialogId>,
    /// The dynamic menu demo's model, edited at runtime by its own commands.
    dyn_menu: Menu,
    /// How many runtime-added items the dynamic menu currently carries.
    dyn_extra: usize,
    /// The table demo's data (sorted copies are pushed into the widget).
    table_rows: Vec<TableRow>,
    table_sort: Option<(usize, bool)>,
    /// The playlist demo's tracks; the app applies reorder drops here.
    playlist: Vec<Row>,
    /// The popover demo is open (the card and its content are visible).
    flyout_open: bool,
    /// Pool indices currently inserted by the dynamic widget-tree demo, in
    /// tree (insertion) order.
    dyn_slots: Vec<usize>,
    /// The custom-widget demo's app-owned click count (shared with its paint
    /// closure).
    custom_clicks: std::rc::Rc<std::cell::Cell<u32>>,
    probe: Option<EditProbe>,
    painted: Arc<AtomicU64>,
    paint_micros: Arc<AtomicU64>,
    ticks: Arc<AtomicU64>,
    invalids: Arc<AtomicU64>,
    /// M7 layout skin (`布局` page): installed document, mtime and status.
    #[cfg(feature = "m7")]
    m7: M7State,
}

/// M7 layout-skin state: the installed document, its modification time for the
/// hot reload and the last report/failure shown in the page's status line.
#[cfg(feature = "m7")]
#[derive(Default)]
struct M7State {
    document: Option<bamboo_native_ui::layout_xml::LayoutDocument>,
    modified: Option<std::time::SystemTime>,
    /// The last `LayoutReport` (or apply failure) text.
    status: String,
    /// The last reload's parse failure; the previous document keeps running.
    reload_failure: Option<String>,
}

#[cfg(feature = "m7")]
impl M7State {
    fn path() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("layouts/layout-demo.buixml")
    }

    fn embedded() -> &'static str {
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/layouts/layout-demo.buixml"
        ))
    }
}

const ROWS: usize = 600;

impl Showcase {
    #[allow(clippy::too_many_arguments)]
    fn new(
        light: bool,
        self_test: bool,
        a11y_dump: bool,
        bench: Option<f32>,
        painted: Arc<AtomicU64>,
        paint_micros: Arc<AtomicU64>,
        ticks: Arc<AtomicU64>,
        invalids: Arc<AtomicU64>,
        a11y_result: Arc<Mutex<Option<Result<String, String>>>>,
        renderer_seen: Arc<Mutex<Option<bamboo_native_ui::Renderer>>>,
    ) -> Self {
        let theme = ThemeSettings::default();
        let rows = make_rows(None);
        let dyn_menu = dynamic_menu(0, true);
        // App-owned state for the custom-widget demo: the paint closure reads
        // it, the message handler increments it.
        let custom_clicks = std::rc::Rc::new(std::cell::Cell::new(0u32));
        let mut ui: Ui<Msg> = Ui::new();
        // A registered image stands in for a decoded cover: the avatar demo
        // and the later cover components all reference it by id.
        let accent = Color::rgb(theme.accent);
        ui.set_image(UiImage::gradient(
            ImageId(1),
            96,
            96,
            accent.darken(0.45),
            accent.lighten(0.1),
        ));
        // Stand-in album artwork for the cover wall (the demo ships no image
        // assets): one vertical gradient per tint pair.
        for (index, (top, bottom)) in COVER_ART.iter().enumerate() {
            ui.set_image(UiImage::gradient(
                ImageId(COVER_IMAGE_BASE + index as u64),
                160,
                160,
                Color::from_hex(*top),
                Color::from_hex(*bottom),
            ));
        }
        // The standard controls set their per-instance details through
        // `WidgetProps`: descriptions reach Narrator's HelpText, tooltips are
        // stored for the M4 host and automation ids stay stable for clients.
        ui.add_with(
            Button::new("primary", "主按钮", Msg::Primary).primary(),
            WidgetProps::new()
                .describe("主操作：点击后累加计数并弹出提示")
                .tooltip("主按钮")
                .context_menu(primary_menu(false, true)),
        );
        ui.add_with(
            Button::new("secondary", "次按钮", Msg::Secondary),
            WidgetProps::new()
                .automation_id("secondary-action")
                .describe("次操作：只弹出提示")
                .tooltip("次按钮"),
        );
        ui.add_with(
            Button::new("icon", "", Msg::Primary).glyph('\u{E768}'),
            WidgetProps::new()
                .automation_id("icon-action")
                .describe("图标按钮：与主按钮相同")
                .tooltip("图标按钮"),
        );
        // A destructive variant is independent of the accent color.
        ui.add_with(
            Button::new("danger", "危险操作", Msg::Danger).danger(),
            WidgetProps::new()
                .automation_id("danger-action")
                .describe("危险操作：点击后弹出红色提示")
                .tooltip("危险操作"),
        );
        ui.add_with(
            Toggle::new("toggle", "开关", true, Msg::Toggle)
                .on_label("开")
                .off_label("关"),
            WidgetProps::new().describe("静音开关").tooltip("静音"),
        );
        // Composite controls can name their items: hovering a segment/tab or
        // scrolling to a row shows that item's tip instead of the container's.
        ui.add_with(
            Segmented::new(
                "segmented",
                &["全部", "最近", "收藏"],
                0,
                Msg::Segment,
            ),
            WidgetProps::new()
                .tooltip("筛选队列")
                .tooltip_at(0, "显示全部曲目")
                .tooltip_at(1, "最近播放的曲目")
                .tooltip_at(2, "收藏的曲目"),
        );
        ui.add_with(
            Tabs::new("tabs", &PAGE_NAMES, 0, Msg::Tab)
                .badge(2, "新"),
            WidgetProps::new()
                .tooltip_at(0, "基础控件与列表")
                .tooltip_at(1, "文本与内容")
                .tooltip_at(2, "输入、导航与反馈组件")
                .tooltip_at(3, "表格、树与动态控件")
                .tooltip_at(4, "封面墙与播放器")
                .tooltip_at(5, "功能区")
                .tooltip_at(6, "属性表格")
                .tooltip_at(7, "功能 API：对话框、托盘、热键、窗口、拖放"),
        );
        // A per-widget font size: the closed paint and the native editor both
        // use it (the theme face stays).
        ui.add_with(
            TextEdit::new("search", "Bamboo Native UI", Msg::Search)
                .cue("搜索曲目")
                .max_length(64),
            WidgetProps::new().font_size(15.0),
        );
        // Steppers exercise the composite number field: the stepper zones are
        // click targets with their own hover/press feedback and never open the
        // native editor.
        ui.add(
            NumberInput::new("count", 8, 1..=500, Msg::Count).steppers(),
        );
        // A framework-painted editor: no native child, so single-line text can
        // be vertically centered, the palette is fully configurable and Chinese
        // IME composition is drawn inline. Click it (or press Enter) and type.
        ui.add_with(
            TextEdit::new("painted_name", "自绘编辑框：点此编辑", Msg::PaintedName)
                .painted()
                .font_size(14.0)
                .max_length(40)
                .cue("输入名称（支持中文输入法）")
                .colors(
                    EditColors::new()
                        .background(Color::new(255, 250, 235))
                        .text(Color::new(58, 46, 24))
                        .selection(Color::new(255, 255, 255), Color::new(64, 120, 220))
                        .caret(Color::new(210, 60, 60))
                        .border(Color::new(226, 168, 58))
                        .border_idle(Color::new(208, 198, 170))
                        .cue(Color::new(150, 136, 105)),
                ),
            WidgetProps::new()
                .automation_id("painted-name")
                .describe("自绘编辑框：点击或回车开始编辑，Esc 取消，Tab 提交")
                .tooltip("自绘编辑框"),
        );
        ui.add_with(
            Slider::new("volume", 0.6, 0.0..=1.0, Msg::Volume)
                .discrete(0.05)
                .ticks_at(&[0.0, 0.25, 0.5, 0.75, 1.0])
                .value_text(|value| format!("{:.0}%", value * 100.0)),
            WidgetProps::new()
                .describe("音量 0 到 100，5% 步进，刻度每 25%")
                .tooltip("音量"),
        );
        ui.add_with(
            ProgressBar::new("progress", 0.6).label("进度"),
            WidgetProps::new().expose(true),
        );
        // A heading is decoration until the instance opts in; then it joins
        // Tab and the accessibility tree with the heading role.
        ui.add_with(
            SectionTitle::new("list_title", "播放队列 · 600 行虚拟列表"),
            WidgetProps::new().focusable(true).narrate(
                Narration::new().description("虚拟列表：只发布当前视口内的行"),
            ),
        );
        let mut list_props = WidgetProps::new()
            .tooltip("方向键移动，回车播放")
            .narrate(
                Narration::new()
                    .description("方向键移动，回车播放")
                    .row(|index, child| format!("第 {} 首，{}", index + 1, child.name)),
            );
        for index in 0..3 {
            list_props = list_props.tooltip_at(index, format!("第 {} 首 · 回车播放", index + 1));
        }
        ui.add_with(
            ListView::new("list")
                .row_height(44.0)
                .alternating(true)
                .empty_text("暂无曲目")
                .on_activate(Msg::PlayRow),
            list_props,
        );
        ui.add_with(
            Pager::new("pager", 0, 5, Msg::Page),
            WidgetProps::new()
                .describe("翻页：上一页与下一页")
                .tooltip("翻页"),
        );
        // A clickable display widget: mouse opt-in, button role, narration.
        ui.add_with(
            ColorSwatch::new("swatch", Color::rgb([127, 211, 168]), "竹绿"),
            WidgetProps::new()
                .focusable(true)
                .mouse(true)
                .cursor(Cursor::Hand)
                .role(A11yRole::Button)
                .automation_id("color-swatch")
                .tooltip("竹绿 #7FD3A8")
                .narrate(Narration::new().description("#7FD3A8，点击查看颜色")),
        );
        ui.on_click(WidgetId::new("swatch"), |_event, _ctx| Some(Msg::Swatch));
        ui.on_activate(WidgetId::new("swatch"), |_event, _ctx| Some(Msg::Swatch));
        // The extended triggers are bindings only; widgets have no default
        // double-click or right-click behavior yet (context menus are M4).
        ui.bind(WidgetId::new("swatch"), Trigger::DoubleClick, |_event, _ctx| {
            Some(Msg::SwatchDouble)
        });
        ui.bind(WidgetId::new("swatch"), Trigger::RightClick, |_event, _ctx| {
            Some(Msg::SwatchContext)
        });
        ui.add(StatCard::new("stat1", "600", "示例行").emphasis(true));
        ui.add_with(
            StatCard::new("stat2", "0", "点击次数"),
            WidgetProps::new().narrate(
                Narration::new().name_with(|info| format!("点击次数 {}", info.value.as_deref().unwrap_or("0"))),
            ),
        );
        // The rounded-surface demo: one `SurfaceStyle` per widget. The corner
        // radius rounds the fill *and* the focus ring, so the same button
        // paints square, soft or pill-shaped; `transparent_background` keeps
        // only the border (an outline button).
        for (id, radius, label) in [
            ("round_sharp", 0.0, "直角 0"),
            ("round_soft", 8.0, "圆角 8"),
            ("round_pill", 18.0, "胶囊 18"),
        ] {
            ui.add(Button::new(id, label, Msg::RoundClick(radius)).corner_radius(radius));
        }
        ui.add(
            Button::new("round_outline", "描边 10", Msg::RoundClick(10.0))
                .transparent_background(true)
                .corner_radius(10.0),
        );
        // A label paints a chip background when one is set; the radius follows
        // the same style (999 clamps to half the height, i.e. a pill).
        ui.add(
            Label::new("round_chip0", "芯片 0")
                .background(Color::rgb([47, 111, 235]))
                .color(Color::rgb([245, 248, 255]))
                .corner_radius(0.0),
        );
        ui.add(
            Label::new("round_chip8", "圆角 8")
                .background(Color::rgb([31, 157, 117]))
                .color(Color::rgb([245, 255, 250]))
                .corner_radius(8.0),
        );
        ui.add(
            Label::new("round_chip_pill", "胶囊 999")
                .background(Color::rgb([107, 79, 187]))
                .color(Color::rgb([248, 245, 255]))
                .corner_radius(999.0),
        );
        ui.add(
            SearchBox::new("round_search", "", Msg::RoundSearch)
                .cue("胶囊搜索框")
                .corner_radius(17.0),
        );
        ui.add(
            ProgressBar::new("round_progress", 0.55)
                .label("圆角进度")
                .corner_radius(12.0),
        );
        ui.add(
            Card::new("round_card_round", "统一圆角", "12 DIP：填充、焦点环与强调条同一形状。")
                .corner_radius(12.0),
        );
        // Per-corner radii: each corner of the same card can differ.
        ui.add(
            Card::new("round_card_cut", "切角卡片", "Corners::new(16, 16, 0, 0)：顶部圆、底部直角。")
                .corners(Corners::new(16.0, 16.0, 0.0, 0.0)),
        );
        // Text and content widgets. Each is decoration by default and joins
        // the accessibility tree only because this demo exposes it.
        ui.add_with(
            Divider::new("content_divider").label("文本与内容"),
            WidgetProps::new()
                .expose(true)
                .automation_id("content-divider"),
        );
        ui.add_with(
            Label::new(
                "wrap_label",
                "自动换行：中文逐字断行，英文按空格断行，超出高度的行不绘制。拖动窗口边缘改变宽度，文本会重新排版。",
            )
            .wrap(true)
            .style(LabelStyle::Small),
            WidgetProps::new()
                .expose(true)
                .automation_id("wrap-label"),
        );
        ui.add_with(
            Badge::new("badge_fav", "已收藏").tone(BadgeTone::Accent),
            WidgetProps::new()
                .expose(true)
                .automation_id("badge-favorite"),
        );
        ui.add_with(
            Badge::new("badge_lossless", "无损").tone(BadgeTone::Success),
            WidgetProps::new()
                .expose(true)
                .automation_id("badge-lossless"),
        );
        // A removable tag owns a close target, so it takes pointer input and
        // Delete/Backspace when the instance is focusable.
        ui.add_with(
            Badge::new("badge_flac", "FLAC")
                .tone(BadgeTone::Neutral)
                .removable(|| Msg::RemoveTag),
            WidgetProps::new()
                .focusable(true)
                .expose(true)
                .automation_id("badge-flac")
                .describe("可移除标签：点击 × 或按 Delete")
                .tooltip("移除标签"),
        );
        ui.add_with(
            Label::new("link_label", "打开歌词页 ›").link(true),
            WidgetProps::new()
                .focusable(true)
                .expose(true)
                .automation_id("link-label")
                .describe("链接样式标签：点击或回车触发")
                .narrate(Narration::new().name("打开歌词页")),
        );
        ui.on_click(WidgetId::new("link_label"), |_event, _ctx| Some(Msg::OpenLink));
        ui.on_activate(WidgetId::new("link_label"), |_event, _ctx| Some(Msg::OpenLink));
        ui.add_with(
            Kbd::new("kbd_search", "Ctrl+K"),
            WidgetProps::new()
                .expose(true)
                .automation_id("kbd-search"),
        );
        ui.add(Icon::new("icon_search", '\u{E721}'));
        ui.add_with(
            Avatar::new("avatar", "竹韵").ring(true),
            WidgetProps::new().expose(true).automation_id("avatar"),
        );
        ui.add_with(
            Label::new("avatar_name", "竹韵 · 正在播放").style(LabelStyle::Small),
            WidgetProps::new().expose(true),
        );
        // An image avatar shares the registered cover with the initials one;
        // the widget falls back to initials when the id is not registered.
        ui.add_with(
            Avatar::new("avatar_image", "夜曲").image(ImageId(1)).ring(true),
            WidgetProps::new()
                .expose(true)
                .automation_id("avatar-image"),
        );
        ui.add_with(
            CopyableText::new("copy_text", "Bamboo Native UI")
                .copyable(|| Msg::CopyText),
            WidgetProps::new()
                .focusable(true)
                .expose(true)
                .automation_id("copy-text")
                .describe("点击或按 Ctrl+C 复制文本")
                .tooltip("复制"),
        );
        ui.add_with(
            MarqueeText::new("marquee", "正在播放：竹韵 · 夜曲 · 无损音质")
                .speed(36.0)
                .gap(40.0)
                .pause(1.2),
            WidgetProps::new()
                .expose(true)
                .automation_id("marquee")
                .tooltip("悬停滚动"),
        );
        // Hover starts the marquee; leaving it stops when nothing else needs
        // the animation timer.
        ui.bind(WidgetId::new("marquee"), Trigger::HoverEnter, |_event, ctx| {
            ctx.default();
            Some(Msg::MarqueeStart)
        });
        ui.bind(WidgetId::new("marquee"), Trigger::HoverLeave, |_event, ctx| {
            ctx.default();
            Some(Msg::MarqueeStop)
        });
        ui.add_with(
            RichText::new("rich")
                .run("内联样式：")
                .strong("重点")
                .run("，")
                .muted("次要")
                .run("，")
                .link(1, "详情")
                .on_link(Msg::RichLink)
                .style(LabelStyle::Small),
            WidgetProps::new()
                .expose(true)
                .automation_id("rich-text")
                .tooltip("链接可点击"),
        );
        // Component gallery (third tab). The gallery replaces the list area so
        // the page stays one retained tree; every gallery widget starts
        // hidden through its instance props.
        ui.add_with(
            Checkbox::new("cb_random", "随机播放", false, Msg::Random),
            WidgetProps::new()
                .visible(false)
                .automation_id("gallery-checkbox-random"),
        );
        ui.add_with(
            Checkbox::new("cb_shuffle", "单曲循环", true, Msg::Shuffle),
            WidgetProps::new()
                .visible(false)
                .automation_id("gallery-checkbox-shuffle"),
        );
        // The same widget with `.square()`, next to the rounded pair above.
        ui.add_with(
            Checkbox::new("cb_square", "方框", false, Msg::Square).square(),
            WidgetProps::new()
                .visible(false)
                .automation_id("gallery-checkbox-square"),
        );
        ui.add_with(
            RadioButton::new("radio_only", "仅本机", false, || Msg::RadioOnly),
            WidgetProps::new()
                .visible(false)
                .automation_id("gallery-radio"),
        );
        ui.add_with(
            RadioGroup::new("radio_group", &["按顺序", "随机", "按热度"], 0, Msg::RadioGroupPick)
                .disable(2),
            WidgetProps::new()
                .visible(false)
                .automation_id("gallery-radio-group")
                .narrate(Narration::new().name("循环方式")),
        );
        ui.add_with(
            SearchBox::new("gallery_search", "", Msg::GallerySearch)
                .cue("搜索曲目")
                .debounce(Duration::from_millis(250)),
            WidgetProps::new()
                .visible(false)
                .automation_id("gallery-search"),
        );
        ui.add_with(
            PasswordBox::new("gallery_password", "", Msg::GalleryPassword)
                .password()
                .cue("密码")
                .max_length(32),
            WidgetProps::new()
                .visible(false)
                .automation_id("gallery-password"),
        );
        ui.add_with(
            ComboBox::new(
                "gallery_combo",
                &["列表循环", "随机播放", "单曲循环", "排序播放"],
                0,
                Msg::GalleryCombo,
            )
            .label("播放模式"),
            WidgetProps::new()
                .visible(false)
                .automation_id("gallery-combo"),
        );
        ui.add_with(
            DatePicker::new("gallery_date", Date::today(), Msg::GalleryDate),
            WidgetProps::new()
                .visible(false)
                .automation_id("gallery-date")
                .narrate(Narration::new().name("发布日期")),
        );
        ui.add_with(
            TimePicker::new("gallery_time", Time::new(9, 30), Msg::GalleryTime)
                .value_range(Some(Time::new(8, 0)), Some(Time::new(18, 30))),
            WidgetProps::new()
                .visible(false)
                .automation_id("gallery-time")
                .narrate(Narration::new().name("提醒时间")),
        );
        ui.add_with(
            ColorPicker::new_rgba(
                "gallery_color",
                Rgba::opaque(Color::new(0x19, 0x71, 0xC2)),
                Msg::GalleryColorRgba,
            )
            // Transparency is selectable here; the checkerboard shows it.
            .allow_alpha(true)
            // The popup's last row hands off to the shell's full color
            // dialog, which can mix any RGBA value.
            .on_more(|| Msg::GalleryColorMore),
            WidgetProps::new()
                .visible(false)
                .automation_id("gallery-color")
                .narrate(Narration::new().name("标签颜色")),
        );
        ui.add_with(
            TextArea::new(
                "gallery_notes",
                "第一行\n第二行\n第三行：多行文本在关闭时也会分行显示。\n第四行：滚轮在关闭状态下也能滚动。\n第五行。\n第六行，超出部分由竖向滚动条承担。",
                Msg::GalleryNotes,
            )
            .multiline()
            .wrap(true)
            .scrollbars(ScrollBars::Vertical)
            .cue("备注")
            .painted(),
            WidgetProps::new()
                .visible(false)
                .automation_id("gallery-notes")
                .font_size(13.0),
        );
        // The same field with wrapping off and both scrollbars: hard newlines
        // only, horizontal overflow on a long line.
        ui.add_with(
            TextArea::new(
                "gallery_notes_raw",
                "不换行的一行很长很长的文本，用于演示横向滚动条。\n第二行保持原样。",
                Msg::GalleryNotesRaw,
            )
            .multiline()
            .wrap(false)
            .scrollbars(ScrollBars::Both)
            .cue("原始文本")
            .painted(),
            WidgetProps::new()
                .visible(false)
                .automation_id("gallery-notes-raw")
                .font_size(13.0),
        );
        ui.add_with(
            MultiSelectList::new(
                "gallery_multi",
                &[
                    "无损音质",
                    "本地曲库",
                    "自动下载封面",
                    "按专辑分组",
                    "按文件夹分组",
                    "显示文件扩展名",
                    "记住播放位置",
                    "自动匹配歌词",
                ],
                Msg::GalleryMulti,
            )
            .checked(&[0, 2]),
            WidgetProps::new()
                .visible(false)
                .automation_id("gallery-multi")
                .narrate(Narration::new().name("多选列表")),
        );
        // Layout/navigation gallery: breadcrumb, nav rail and a splitter demo.
        ui.add_with(
            Breadcrumb::new(
                "gallery_breadcrumb",
                &["首页", "曲库", "夜曲"],
                Msg::GalleryBreadcrumb,
            )
            .label("位置"),
            WidgetProps::new()
                .visible(false)
                .automation_id("gallery-breadcrumb"),
        );
        ui.add_with(
            NavRail::new(
                "gallery_navrail",
                &["全部曲目", "最近播放", "我的收藏", "本地文件"],
                1,
                Msg::GalleryNavRail,
            )
            .icon(0, '♪')
            .icon(1, '↺')
            .icon(2, '★')
            .icon(3, '▤')
            .row_height(26.0)
            .label("导航"),
            WidgetProps::new()
                .visible(false)
                .automation_id("gallery-navrail"),
        );
        ui.add_with(
            Card::new("gallery_pane_a", "左面板", "拖动右侧分隔条调整宽度。"),
            WidgetProps::new().visible(false),
        );
        ui.add_with(
            Card::new("gallery_pane_b", "右面板", "两块面板共享 300 DIP 的演示宽度。"),
            WidgetProps::new().visible(false),
        );
        ui.add_with(
            Splitter::vertical("gallery_splitter", Msg::GallerySplit).label("拖动调整面板"),
            WidgetProps::new()
                .visible(false)
                .automation_id("gallery-splitter"),
        );
        // Application chrome: a painted menu bar (File/Edit/View), a toolbar
        // that overflows into its own menu, a status strip, a menu button and
        // a split button. Commands are app ids (500+, 600+, 700+) and arrive
        // through one `Msg::Chrome` handler.
        ui.add_with(
            MenuBar::new("gallery_menubar", Msg::Chrome)
                .item(
                    "文件(&F)",
                    Menu::new()
                        .item(500, "新建播放列表")
                        .item(501, "打开曲库…")
                        .separator()
                        .item(502, "导入文件…")
                        .disabled_item(503, "导出（示例禁用）")
                        .separator()
                        .submenu(
                            "最近打开",
                            Menu::new().item(504, "夜曲").item(505, "竹韵"),
                        ),
                )
                .item(
                    "编辑(&E)",
                    Menu::new()
                        .item(520, "撤销")
                        .item(521, "重做")
                        .separator()
                        .checked_item(522, "随机播放", false)
                        .submenu("更多", Menu::new().item(523, "全选").item(524, "查找…")),
                )
                .item(
                    "视图(&V)",
                    Menu::new()
                        .item(540, "显示歌词")
                        .item(541, "全屏")
                        .separator()
                        .item(542, "切换主题"),
                ),
            WidgetProps::new()
                .visible(false)
                .automation_id("gallery-menubar")
                .narrate(Narration::new().name("菜单栏")),
        );
        ui.add_with(
            Toolbar::new(
                "gallery_toolbar",
                vec![
                    ToolbarItem::new(600, "播放音乐").glyph('▶'),
                    ToolbarItem::new(601, "暂停播放").glyph('❚'),
                    ToolbarItem::new(602, "下一首歌曲").glyph('▶'),
                    ToolbarItem::separator(),
                    ToolbarItem::new(603, "收藏歌曲").checked(true),
                    ToolbarItem::new(614, "")
                        .glyph('\u{E721}')
                        .icon_only()
                        .name("搜索曲库"),
                    ToolbarItem::separator(),
                    ToolbarItem::new(604, "随机播放"),
                    ToolbarItem::new(605, "循环模式"),
                    ToolbarItem::new(606, "音量调节"),
                    ToolbarItem::new(607, "音量均衡"),
                    ToolbarItem::new(608, "显示歌词"),
                    ToolbarItem::new(609, "播放队列"),
                    ToolbarItem::new(610, "下载曲目"),
                    ToolbarItem::new(611, "播放设置"),
                    ToolbarItem::new(612, "备份曲库"),
                    ToolbarItem::new(613, "帮助中心"),
                ],
                Msg::Chrome,
            ),
            WidgetProps::new()
                .visible(false)
                .automation_id("gallery-toolbar")
                .narrate(Narration::new().name("工具栏")),
        );
        ui.add_with(
            Toolbar::new(
                "gallery_vtoolbar",
                vec![
                    ToolbarItem::new(620, "")
                        .glyph('\u{E768}')
                        .icon_only()
                        .name("播放"),
                    ToolbarItem::new(621, "")
                        .glyph('\u{E721}')
                        .icon_only()
                        .name("搜索"),
                    ToolbarItem::separator(),
                    ToolbarItem::new(622, "")
                        .glyph('\u{E713}')
                        .icon_only()
                        .name("设置"),
                    ToolbarItem::new(623, "")
                        .glyph('\u{E897}')
                        .icon_only()
                        .name("帮助")
                        .checked(true),
                ],
                Msg::Chrome,
            )
            .vertical(),
            WidgetProps::new()
                .visible(false)
                .automation_id("gallery-vtoolbar")
                .narrate(Narration::new().name("竖排工具栏")),
        );
        ui.add_with(
            StatusBar::new("gallery_status")
                .segment(StatusSegment::new("就绪").icon('♪'))
                .segment(
                    StatusSegment::new("600 首 · 无损")
                        .align(TextAlign::Right)
                        .width(160.0),
                )
                .segment(
                    StatusSegment::new("44.1 kHz")
                        .align(TextAlign::Right)
                        .width(96.0),
                )
                .size_grip(true)
                .progress(0.62)
                .label("状态栏"),
            WidgetProps::new()
                .visible(false)
                .expose(true)
                .automation_id("gallery-status"),
        );
        ui.add_with(
            MenuButton::new(
                "gallery_menubtn",
                "操作(&A)",
                Menu::new()
                    .shortcut_item(700, "开始播放", "Enter")
                    .checked_item(701, "顺序播放", true)
                    .separator()
                    .submenu("添加到…", Menu::new().item(702, "播放队列").item(703, "收藏夹"))
                    .push(
                        MenuItem::new(705, "查看曲目信息")
                            .shortcut("F2")
                            .icon(demo_menu_icon(0x9C36B5, 0x5F3DC4)),
                    )
                    .disabled_item(704, "不可用项"),
                Msg::Chrome,
            )
            .glyph('≡'),
            WidgetProps::new()
                .visible(false)
                .automation_id("gallery-menubtn")
                .narrate(Narration::new().name("操作菜单")),
        );
        ui.add_with(
            SplitButton::new(
                "gallery_splitbtn",
                "导出(&E)",
                Menu::new()
                    .item(720, "导出为 MP3")
                    .item(721, "导出为 FLAC")
                    .separator()
                    .item(722, "导出设置…"),
                || Msg::Chrome(799),
                Msg::Chrome,
            ),
            WidgetProps::new()
                .visible(false)
                .automation_id("gallery-splitbtn")
                .narrate(Narration::new().name("导出")),
        );
        // A menu whose model changes while the window runs: its own commands
        // add/remove items, and a command re-opens it from application code.
        ui.add_with(
            MenuButton::new(DYN_MENU_ID, "动态(&D)", dynamic_menu(0, true), Msg::Chrome)
                .glyph('\u{E8FD}'),
            WidgetProps::new()
                .visible(false)
                .automation_id("gallery-dynmenu")
                .narrate(Narration::new().name("动态菜单")),
        );
        // Mirror the button's model into the per-widget menu registry, so the
        // secondary click and `Ui::open_menu`/`expand_menu` use the same value.
        ui.set_menu(WidgetId::new(DYN_MENU_ID), Some(dyn_menu.clone()));
        // Data page: a heading, the table and the dynamic widget panel.
        ui.add_with(
            Label::new("data_title", "表格与动态控件").style(LabelStyle::Title),
            WidgetProps::new().visible(false).expose(true),
        );
        ui.add_with(
            Label::new(
                "data_hint",
                "点击表头排序；拖动表头分隔线调整列宽、拖动表头移动列（超出时出现横向滚动条）；双击单元格或按 Enter/F2 就地编辑（Enter 提交、Esc 取消）；动态面板的按钮会插入、移除和重排固定槽位。",
            )
            .style(LabelStyle::Small)
            .wrap(true),
            WidgetProps::new().visible(false).expose(true),
        );
        // A virtualized table with a sortable header and thirty rows; the
        // viewport fits ten, so the bar and the look-ahead row are live.
        // Header clicks emit the column; the app sorts and paints the
        // indicator through `set_sort`.
        let table_rows = gallery_table_rows();
        ui.add_with(
            Table::new("gallery_table")
                .columns(gallery_table_columns())
                .rows(table_rows.clone())
                .header_height(28.0)
                .row_height(28.0)
                .striped(true)
                .empty_text("没有曲目")
                .on_select(Msg::GalleryTableSelect)
                .on_activate(Msg::GalleryTableOpen)
                .on_header_click(Msg::GalleryTableSort)
                .editable(true)
                .on_cell_edit(Msg::GalleryTableCell),
            WidgetProps::new()
                .visible(false)
                .automation_id("gallery-table")
                .narrate(Narration::new().name("曲目表")),
        );
        // A tree under the table: expand/collapse with the chevron or arrows,
        // path-carrying messages, and twelve visible rows in an eight-row
        // viewport (so it scrolls and virtualizes like the table).
        ui.add_with(
            TreeView::new("gallery_tree")
                .items(gallery_tree_items())
                .row_height(24.0)
                .on_select(|path: &[usize]| Msg::GalleryTreeSelect(path.to_vec()))
                .on_activate(|path: &[usize]| Msg::GalleryTreeOpen(path.to_vec()))
                .on_toggle(|path: &[usize], expanded| {
                    Msg::GalleryTreeToggle(path.to_vec(), expanded)
                })
                .renameable(true)
                .on_rename(|path: &[usize], label| {
                    Msg::GalleryTreeRename(path.to_vec(), label.to_string())
                })
                .on_move(|from: &[usize], to: &[usize]| {
                    Msg::GalleryTreeMove(from.to_vec(), to.to_vec())
                }),
            WidgetProps::new()
                .visible(false)
                .automation_id("gallery-tree")
                .narrate(Narration::new().name("音乐库")),
        );
        // Scroll view: eight widgets laid out in content coordinates inside a
        // viewport that is smaller on both axes. The container positions them
        // at `content - offset`, clips them, paints the bars and skips the ones
        // scrolled out (see `layout` for the registration).
        for (index, id) in SCROLL_ROWS.iter().enumerate() {
            if index % 2 == 0 {
                ui.add_with(
                    Button::new(*id, format!("滚动按钮 {}", index / 2 + 1), Msg::ScrollRow(index)),
                    WidgetProps::new()
                        .visible(false)
                        .automation_id(format!("scroll-row-{}", index + 1)),
                );
            } else {
                ui.add_with(
                    Checkbox::new(*id, format!("滚动选项 {}", index / 2 + 1), false, move |checked| {
                        Msg::ScrollCheck(index, checked)
                    }),
                    WidgetProps::new()
                        .visible(false)
                        .automation_id(format!("scroll-row-{}", index + 1)),
                );
            }
        }
        // Nested scroll demo rows: children of the inner viewport (registered
        // in `layout` as a panel inside the page content).
        for (index, id) in NEST_ROWS.iter().enumerate() {
            ui.add_with(
                Button::new(*id, format!("嵌套 {}", index + 1), Msg::ScrollRow(index)),
                WidgetProps::new()
                    .visible(false)
                    .automation_id(format!("nested-row-{}", index + 1)),
            );
        }
        // The dynamic widget-tree demo: a fixed two-slot pool that the
        // application removes and re-inserts at runtime (`Ui::remove` /
        // `Ui::insert`), plus z-order changes (`Ui::raise`/`Ui::lower`).
        ui.add_with(
            GroupBox::new("dyn_group", "动态面板"),
            WidgetProps::new().visible(false),
        );
        ui.add_with(
            Button::new("dyn_add", "添加项", Msg::DynAdd),
            WidgetProps::new()
                .visible(false)
                .automation_id("dyn-add"),
        );
        ui.add_with(
            Button::new("dyn_remove", "移除项", Msg::DynRemove),
            WidgetProps::new()
                .visible(false)
                .automation_id("dyn-remove"),
        );
        ui.add_with(
            Button::new("dyn_raise", "置顶", Msg::DynRaise),
            WidgetProps::new()
                .visible(false)
                .automation_id("dyn-raise"),
        );
        ui.add_with(
            Button::new("dyn_lower", "置底", Msg::DynLower),
            WidgetProps::new()
                .visible(false)
                .automation_id("dyn-lower"),
        );
        ui.add_with(
            Label::new("dyn_info", "顺序：1 → 2").style(LabelStyle::Small),
            WidgetProps::new()
                .visible(false)
                .expose(true)
                .automation_id("dyn-info"),
        );
        for (index, id) in DYN_SLOT_IDS.iter().enumerate() {
            ui.add_with(
                Button::new(id, format!("动态项 {}", index + 1), Msg::DynItem(index)),
                WidgetProps::new()
                    .visible(false)
                    .automation_id(format!("dyn-slot-{}", index + 1)),
            );
        }
        ui.add_with(
            Label::new("dyn_hint", "两个槽位重叠，置顶/置底改变绘制顺序。")
                .style(LabelStyle::Small)
                .wrap(true),
            WidgetProps::new().visible(false).expose(true),
        );
        // Cover wall and playlist (fifth tab): the player browse surface and
        // the queue beside it. A click selects, a double click or Enter opens;
        // dragging a playlist row (or Alt+Up/Down) moves the track, and the
        // application applies the move to its own data.
        ui.add_with(
            Label::new("covers_title", "封面墙").style(LabelStyle::Title),
            WidgetProps::new().visible(false).expose(true),
        );
        ui.add_with(
            Label::new(
                "covers_hint",
                "点击选中封面，双击或回车打开。右侧列表可拖动排序；下方播放控制可点击，波形可拖动定位。",
            )
            .style(LabelStyle::Small)
            .wrap(true),
            WidgetProps::new().visible(false).expose(true),
        );
        ui.add_with(
            CoverWall::new("covers_wall")
                .covers(gallery_covers())
                .tile(148.0)
                .gap(14.0)
                .corner_radius(6.0)
                .empty_text("封面库为空")
                .on_select(Msg::CoverSelect)
                .on_open(Msg::CoverOpen),
            WidgetProps::new()
                .visible(false)
                .automation_id("covers-wall")
                .narrate(Narration::new().name("封面墙")),
        );
        let playlist = gallery_playlist();
        ui.add_with(
            PlaylistView::new("covers_playlist")
                .rows(playlist.clone())
                .row_height(40.0)
                .alternating(true)
                .empty_text("播放列表为空")
                .on_select(Msg::PlaylistSelect)
                .on_activate(Msg::PlaylistOpen)
                .on_reorder(Msg::PlaylistReorder),
            WidgetProps::new()
                .visible(false)
                .automation_id("covers-playlist")
                .narrate(Narration::new().name("播放列表")),
        );
        // Player strip under the wall: the transport (shuffle, previous,
        // play/pause, next, repeat), the state text it updates, and a waveform
        // seek bar over a deterministic synthetic waveform.
        ui.add_with(
            PlaybackTransport::new("covers_transport", Msg::Transport)
                .playing(false)
                .shuffle(false)
                .repeat(RepeatMode::Off),
            WidgetProps::new()
                .visible(false)
                .automation_id("covers-transport")
                .narrate(Narration::new().name("播放控制")),
        );
        ui.add_with(
            Label::new("covers_state", transport_state_text(false, false, RepeatMode::Off))
                .style(LabelStyle::Small),
            WidgetProps::new().visible(false).expose(true),
        );
        ui.add_with(
            WaveformSeekBar::new("covers_waveform", 0.32, Msg::Seek)
                .samples(gallery_waveform())
                .duration(COVER_TRACK_SECONDS)
                .bar_width(2.0)
                .gap(1.0),
            WidgetProps::new()
                .visible(false)
                .automation_id("covers-waveform")
                .narrate(Narration::new().name("播放进度")),
        );
        ui.add_with(
            Label::new("covers_time", cover_time_text(0.32))
                .style(LabelStyle::Small)
                .align(TextAlign::Right),
            WidgetProps::new().visible(false).expose(true),
        );
        // Data-display widgets: a bar sparkline, a gauge and a star rating.
        ui.add_with(
            Sparkline::new(
                "gallery_sparkline",
                &[3.0, 6.0, 4.0, 9.0, 7.0, 12.0, 8.0, 12.0, 5.0, 9.0, 11.0, 7.0],
            ),
            WidgetProps::new()
                .visible(false)
                .automation_id("gallery-sparkline")
                .narrate(Narration::new().name("音量趋势")),
        );
        ui.add_with(
            Gauge::new("gallery_gauge", 0.65).label("负载"),
            WidgetProps::new()
                .visible(false)
                .expose(true)
                .automation_id("gallery-gauge"),
        );
        ui.add_with(
            RatingStars::new("gallery_stars", 5, 3, Msg::GalleryRating).clearable(true),
            WidgetProps::new()
                .visible(false)
                .automation_id("gallery-stars")
                .narrate(Narration::new().name("评分")),
        );
        // The group frame is added before the accordion so it paints behind it.
        ui.add_with(
            GroupBox::new("gallery_group", "设置"),
            WidgetProps::new()
                .visible(false)
                .automation_id("gallery-group")
                .expose(true),
        );
        ui.add_with(
            Accordion::new(
                "gallery_accordion",
                &["播放选项", "音效选项"],
                Msg::GalleryAccordion,
            )
            .single_open(true)
            .open(0, true)
            .header_height(32.0)
            .body_height(56.0)
            .body(0, "播放选项会保存在本地，可随时恢复默认。")
            .label("播放设置"),
            WidgetProps::new()
                .visible(false)
                .automation_id("gallery-accordion"),
        );
        ui.add_with(
            Banner::new("gallery_banner", BannerKind::Info, "有可用更新")
                .body("新版本 0.2 已准备好，重启后生效。")
                .action("立即更新", || Msg::BannerRetry)
                .closable(|| Msg::BannerClose),
            WidgetProps::new()
                .visible(false)
                .automation_id("gallery-banner"),
        );
        ui.add_with(
            Spinner::new("gallery_spinner").speed(360.0),
            WidgetProps::new()
                .visible(false)
                .expose(true)
                .automation_id("gallery-spinner"),
        );
        ui.add_with(
            ProgressRing::new("gallery_ring", 0.72).show_value(true),
            WidgetProps::new()
                .visible(false)
                .automation_id("gallery-ring"),
        );
        ui.add_with(
            TooltipCard::new("gallery_tooltip", "提示卡片").body("页面内的提示与原生浮层使用同一套主题。"),
            WidgetProps::new()
                .visible(false)
                .expose(true)
                .automation_id("gallery-tooltip"),
        );
        ui.add_with(
            Button::new("modal_open", "打开弹窗", Msg::OpenModal),
            WidgetProps::new()
                .visible(false)
                .automation_id("open-modal"),
        );
        ui.add_with(
            Button::new("theme", "切换主题", Msg::Theme),
            WidgetProps::new()
                .automation_id("theme-toggle")
                .describe("切换明暗主题")
                .tooltip("主题")
                // Tab reaches the theme switch after the content controls.
                .focus_order(9),
        );
        // Popover demo: the trigger sits in the top button row; the card is
        // placed from that anchor in `layout`. The content widgets follow the
        // card, so they paint above it and receive input before its
        // light-dismiss hit area swallows the press.
        ui.add_with(
            Button::new("flyout_open", "弹出层", Msg::FlyoutOpen),
            WidgetProps::new()
                .automation_id("flyout-open")
                .describe("打开一个锚点定位的浮层")
                .tooltip("弹出层"),
        );
        ui.add_with(
            Popover::new("flyout")
                .name("弹出层提示")
                .description("锚点定位的浮层，点击外部或「关闭」收起")
                .pad(14.0)
                .on_dismiss(|| Msg::FlyoutDismiss),
            WidgetProps::new().visible(false).automation_id("flyout"),
        );
        ui.add_with(
            Label::new("flyout_title", "弹出层"),
            WidgetProps::new().visible(false),
        );
        ui.add_with(
            Label::new(
                "flyout_body",
                "锚点定位在按钮下方；点击卡片外部或「关闭」收起。",
            )
            .style(LabelStyle::Small)
            .wrap(true),
            WidgetProps::new().visible(false),
        );
        ui.add_with(
            Button::new("flyout_close", "关闭", Msg::FlyoutDismiss),
            WidgetProps::new()
                .visible(false)
                .automation_id("flyout-close"),
        );
        // Ribbon demo (sixth tab): tabs over grouped large/small commands.
        ui.add_with(
            Label::new("ribbon_title", "功能区").style(LabelStyle::Title),
            WidgetProps::new().visible(false).expose(true),
        );
        ui.add_with(
            Label::new(
                "ribbon_hint",
                "切换选项卡浏览命令组；点击大型或小型命令执行，Alt+字母可直达选项卡。",
            )
            .style(LabelStyle::Small)
            .wrap(true),
            WidgetProps::new().visible(false).expose(true),
        );
        ui.add_with(
            Ribbon::new("gallery_ribbon", gallery_ribbon_tabs(), Msg::RibbonCommand)
                .label("功能区")
                .on_tab(Msg::RibbonTab),
            WidgetProps::new()
                .visible(false)
                .automation_id("gallery-ribbon")
                .narrate(Narration::new().name("功能区")),
        );
        ui.add_with(
            Label::new("ribbon_state", "最近命令：无").style(LabelStyle::Small),
            WidgetProps::new().visible(false).expose(true),
        );
        // Property-grid demo (seventh tab): sectioned key/value rows. Two
        // values are editable; clicking one emits the edit message and the
        // application cycles the value through typed access. The buttons push
        // values from outside the grid, and the status line reports both paths.
        ui.add_with(
            Label::new("props_title", "属性").style(LabelStyle::Title),
            WidgetProps::new().visible(false).expose(true),
        );
        ui.add_with(
            Label::new(
                "props_hint",
                "分区显示键值；点击可编辑的值（或双击任意行）发出编辑消息，应用通过类型化访问写回。",
            )
            .style(LabelStyle::Small)
            .wrap(true),
            WidgetProps::new().visible(false).expose(true),
        );
        ui.add_with(
            PropertyGrid::new("gallery_props")
                .sections(gallery_property_sections())
                .header_height(28.0)
                .row_height(26.0)
                .key_width(120.0)
                .empty_text("没有属性")
                .on_select(|key: &str| Msg::PropertySelect(key.to_string()))
                .on_activate(|key: &str, value: &str| {
                    Msg::PropertyInspect(key.to_string(), value.to_string())
                })
                .on_change(|key: &str, value: &str| {
                    Msg::PropertyChange(key.to_string(), value.to_string())
                }),
            WidgetProps::new()
                .visible(false)
                .automation_id("gallery-props")
                .narrate(Narration::new().name("曲目属性")),
        );
        ui.add_with(
            Button::new("props_toggle", "切换收藏", Msg::PropertyToggle),
            WidgetProps::new()
                .visible(false)
                .automation_id("props-toggle")
                .narrate(Narration::new().name("切换收藏")),
        );
        ui.add_with(
            Button::new("props_reset", "恢复默认", Msg::PropertyReset),
            WidgetProps::new()
                .visible(false)
                .automation_id("props-reset")
                .narrate(Narration::new().name("恢复默认")),
        );
        ui.add_with(
            Button::new("props_lock", "锁定选中项", Msg::PropertyLock),
            WidgetProps::new()
                .visible(false)
                .automation_id("props-lock")
                .narrate(Narration::new().name("锁定选中项")),
        );
        ui.add_with(
            Checkbox::new("props_readonly", "只读模式", false, Msg::PropertyReadOnly),
            WidgetProps::new()
                .visible(false)
                .automation_id("props-readonly")
                .narrate(Narration::new().name("只读模式")),
        );
        ui.add_with(
            Label::new("props_state", "选中：名称").style(LabelStyle::Small),
            WidgetProps::new().visible(false).expose(true),
        );
        // A first-class empty state (the feedback/layout demo).
        ui.add_with(
            EmptyState::new(
                "gallery_empty",
                "暂无歌曲",
                "拖入音频文件，或点击上方按钮导入曲库。",
            ),
            WidgetProps::new()
                .visible(false)
                .expose(true)
                .automation_id("gallery-empty"),
        );
        // ---- Functional-API page --------------------------------------------
        // Every button here calls a public API directly; the bottom strip
        // reports what happened. The dialogs are shell dialogs (worker
        // threaded), so the window stays owned or live as marked.
        ui.add_with(
            Button::new("api_msg", "消息对话框", Msg::ApiMessage),
            WidgetProps::new().visible(false).automation_id("api-message"),
        );
        ui.add_with(
            Button::new("api_parallel", "并行消息框", Msg::ApiParallel),
            WidgetProps::new().visible(false).automation_id("api-parallel"),
        );
        ui.add_with(
            Button::new("api_custom", "自定义按钮", Msg::ApiCustom),
            WidgetProps::new().visible(false).automation_id("api-custom"),
        );
        ui.add_with(
            Button::new("api_file_open", "打开文件…", Msg::ApiFileOpen),
            WidgetProps::new().visible(false).automation_id("api-file-open"),
        );
        ui.add_with(
            Button::new("api_file_save", "保存文件…", Msg::ApiFileSave),
            WidgetProps::new().visible(false).automation_id("api-file-save"),
        );
        ui.add_with(
            Button::new("api_folder", "选择文件夹…", Msg::ApiFolder),
            WidgetProps::new().visible(false).automation_id("api-folder"),
        );
        ui.add_with(
            Button::new("api_font", "字体…", Msg::ApiFont),
            WidgetProps::new().visible(false).automation_id("api-font"),
        );
        ui.add_with(
            Button::new("api_color", "颜色…", Msg::ApiColor),
            WidgetProps::new().visible(false).automation_id("api-color"),
        );
        // A component written outside the library: a paint closure plus an
        // input handler, registered exactly like a built-in. Surface builders,
        // the focus ring, a11y, WidgetProps and the layout skin all apply.
        // The bars follow the application's click count, which the closure
        // reads through a shared Cell.
        let clicks = std::rc::Rc::clone(&custom_clicks);
        ui.add(
            CustomWidget::new("api_custom_draw", move |ctx, rect, state| {
                let palette = ctx.palette;
                let painter = ctx.painter;
                let count = clicks.get() as f32;
                let inner = rect.inset(10.0, 6.0);
                let bars = 6usize;
                let gap = 3.0;
                let width = ((inner.width() - gap * (bars - 1) as f32) / bars as f32).max(2.0);
                for index in 0..bars {
                    let level = 0.25 + 0.75 * (((count + index as f32) * 0.37).fract());
                    let height = (inner.height() * level).max(2.0);
                    let left = inner.left + index as f32 * (width + gap);
                    let bar = Rect::new(left, inner.bottom - height, left + width, inner.bottom);
                    painter.round_rect(
                        &mut *ctx.canvas,
                        bar,
                        2.0,
                        palette.accent.mix(palette.muted, index as f32 * 0.12),
                    );
                }
                ctx.text(
                    Rect::new(inner.left, rect.top, inner.right, rect.top + 16.0),
                    &format!("自定义控件 · 点击 {count}"),
                    TextStyleRole::Small,
                    TextAlign::Right,
                    if state.enabled { palette.text } else { palette.text_disabled },
                );
            })
            .focusable(true)
            .interactive(true)
            .cursor(Cursor::Hand)
            .corner_radius(10.0)
            .a11y_name(A11yRole::Button, "自定义控件")
            .input(|_rect, input| match input {
                WidgetInput::ClickAt(_) | WidgetInput::Activate => Some(Msg::CustomDrawClicked),
                _ => None,
            }),
        );
        ui.add_with(
            Label::new("api_custom_state", "自定义控件：等待点击").style(LabelStyle::Small),
            WidgetProps::new().visible(false),
        );
        ui.add_with(
            Button::new("api_menu", "从代码打开菜单", Msg::ApiMenu),
            WidgetProps::new().visible(false).automation_id("api-menu"),
        );
        ui.add_with(
            Button::new("api_context", "右键上下文菜单", Msg::ApiContext),
            WidgetProps::new()
                .visible(false)
                .automation_id("api-context")
                .context_menu(primary_menu(true, true)),
        );
        ui.add_with(
            Button::new("api_tray_add", "安装托盘图标", Msg::ApiTrayAdd),
            WidgetProps::new().visible(false).automation_id("api-tray-add"),
        );
        ui.add_with(
            Button::new("api_tray_balloon", "托盘气泡通知", Msg::ApiTrayBalloon),
            WidgetProps::new().visible(false).automation_id("api-tray-balloon"),
        );
        ui.add_with(
            Button::new("api_notify", "系统通知", Msg::ApiSystemNotify),
            WidgetProps::new().visible(false).automation_id("api-notify"),
        );
        ui.add_with(
            Button::new("api_tray_remove", "移除托盘图标", Msg::ApiTrayRemove),
            WidgetProps::new().visible(false).automation_id("api-tray-remove"),
        );
        ui.add_with(
            Button::new("api_hotkey_add", "注册 F8 热键", Msg::ApiHotkeyAdd),
            WidgetProps::new().visible(false).automation_id("api-hotkey-add"),
        );
        ui.add_with(
            Button::new("api_hotkey_capture", "捕获热键", Msg::ApiHotkeyCapture),
            WidgetProps::new().visible(false).automation_id("api-hotkey-capture"),
        );
        ui.add_with(
            Button::new("api_hotkey_remove", "注销热键", Msg::ApiHotkeyRemove),
            WidgetProps::new().visible(false).automation_id("api-hotkey-remove"),
        );
        ui.add_with(
            Button::new("api_min", "最小化", Msg::ApiMinimize),
            WidgetProps::new().visible(false).automation_id("api-minimize"),
        );
        ui.add_with(
            Button::new("api_max", "最大化", Msg::ApiMaximize),
            WidgetProps::new().visible(false).automation_id("api-maximize"),
        );
        ui.add_with(
            Button::new("api_restore", "还原", Msg::ApiRestore),
            WidgetProps::new().visible(false).automation_id("api-restore"),
        );
        ui.add_with(
            Button::new("api_hide", "隐藏（托盘恢复）", Msg::ApiHide),
            WidgetProps::new().visible(false).automation_id("api-hide"),
        );
        ui.add_with(
            Button::new("api_close", "请求关闭", Msg::ApiClose),
            WidgetProps::new().visible(false).automation_id("api-close"),
        );
        ui.add_with(
            Button::new("api_flush", "强制重绘 + flush", Msg::ApiFlush),
            WidgetProps::new().visible(false).automation_id("api-flush"),
        );
        ui.add_with(
            Button::new("api_title", "改标题", Msg::ApiTitle),
            WidgetProps::new().visible(false).automation_id("api-title"),
        );
        ui.add_with(
            Button::new("api_announce", "无障碍通知", Msg::ApiAnnounce),
            WidgetProps::new().visible(false).automation_id("api-announce"),
        );
        ui.add_with(
            Button::new("api_copy", "复制到剪贴板", Msg::ApiCopy),
            WidgetProps::new().visible(false).automation_id("api-copy"),
        );
        ui.add_with(
            Button::new("api_refresh", "重建无障碍树", Msg::ApiRefresh),
            WidgetProps::new().visible(false).automation_id("api-refresh"),
        );
        ui.add_with(
            Button::new("api_theme", "切换主题（API）", Msg::ApiTheme),
            WidgetProps::new().visible(false).automation_id("api-theme"),
        );
        #[cfg(feature = "m7")]
        {
            ui.add_with(
                Button::new("api_theme_doc", "加载 .buitheme", Msg::ApiThemeDoc),
                WidgetProps::new()
                    .visible(false)
                    .automation_id("api-theme-doc"),
            );
            ui.add_with(
                Label::new("api_theme_state", "主题：未加载（预设）"),
                WidgetProps::new()
                    .visible(false)
                    .automation_id("api-theme-state"),
            );
        }
        // Cross-widget drag and drop: two sources, two targets. The container
        // owns the gesture (threshold, target tracking, hover, drop).
        // A drag source must be an interactive widget (a card is display-only
        // and the pointer passes through it), so the cards are buttons.
        ui.add_with(
            Button::new("drag_a", "拖动卡片 A", Msg::DragCard(0)),
            WidgetProps::new()
                .visible(false)
                .automation_id("drag-a")
                .describe("按住拖到放置区：移动超过 4 DIP 即开始拖动")
                .tooltip("拖我"),
        );
        ui.add_with(
            Button::new("drag_b", "拖动卡片 B", Msg::DragCard(1)),
            WidgetProps::new()
                .visible(false)
                .automation_id("drag-b")
                .describe("载荷是一个 Msg；放置区把它变成 Dropped 消息")
                .tooltip("拖我"),
        );
        // A drop target must be hittable too, so the zones are ordinary
        // buttons; their click message is intentionally a no-op.
        ui.add_with(
            Button::new("drop_a", "放置区 A · 拖到这里", Msg::DropZone),
            WidgetProps::new()
                .visible(false)
                .automation_id("drop-a")
                .describe("把卡片拖到这里"),
        );
        ui.add_with(
            Button::new("drop_b", "放置区 B · 拖到这里", Msg::DropZone),
            WidgetProps::new()
                .visible(false)
                .automation_id("drop-b")
                .describe("把卡片拖到这里"),
        );
        ui.add_with(
            Label::new("api_drag_state", "拖放：等待拖动")
                .style(LabelStyle::Small)
                .wrap(true),
            WidgetProps::new().visible(false).expose(true).automation_id("api-drag-state"),
        );
        // Text drag and drop (D122): select text in the first field, press
        // inside the selection and drag it to the second. The container
        // carries the text; the target field inserts it at the release point.
        ui.add_with(
            TextEdit::new(
                "api_text_from",
                "选中这段文字，按住拖动到右侧输入框",
                Msg::ApiTextCommitted,
            )
            .painted(),
            WidgetProps::new()
                .visible(false)
                .automation_id("api-text-from")
                .describe("选中文字后按住拖动：拖到右侧输入框即可复制")
                .tooltip("文本拖放源"),
        );
        ui.add_with(
            TextEdit::new("api_text_to", "", Msg::ApiTextCommitted).painted(),
            WidgetProps::new()
                .visible(false)
                .automation_id("api-text-to")
                .describe("把左侧选中的文字拖到这里")
                .tooltip("文本拖放目标"),
        );
        ui.add_with(
            Label::new("api_text_state", "文本拖放：等待拖动")
                .style(LabelStyle::Small)
                .wrap(true),
            WidgetProps::new()
                .visible(false)
                .expose(true)
                .automation_id("api-text-state"),
        );
        ui.add_with(
            Checkbox::new("api_pad", "手柄当作键盘", true, Msg::PadEnable),
            WidgetProps::new().visible(false).automation_id("api-pad"),
        );
        ui.add_with(
            Label::new("api_pad_state", "手柄：未连接（XInput）")
                .style(LabelStyle::Small),
            WidgetProps::new().visible(false).expose(true).automation_id("api-pad-state"),
        );
        ui.add_with(
            Label::new("api_renderer", "渲染器：GDI").style(LabelStyle::Small),
            WidgetProps::new().visible(false).expose(true).automation_id("api-renderer"),
        );
        // In-window modal shell, added after every page widget: the backdrop
        // split is structural, so the scrim must be the first widget of the
        // overlay layer or page widgets added after it would paint (and hit
        // test) above the panel. The panel widgets follow it so they stay
        // clickable, and the toast comes last so it can still appear over the
        // modal.
        ui.add_with(
            Scrim::new("modal_scrim"),
            WidgetProps::new().visible(false),
        );
        ui.add_with(
            Card::new("modal_card", "删除播放列表？", "此操作会移除列表，且无法撤销。")
                .padding(18.0),
            WidgetProps::new()
                .visible(false)
                .expose(true)
                .automation_id("modal-card"),
        );
        ui.add_with(
            Checkbox::new("modal_check", "不再询问", false, Msg::ModalCheck),
            WidgetProps::new()
                .visible(false)
                .automation_id("modal-check"),
        );
        ui.add_with(
            Button::new("modal_cancel", "取消", Msg::ModalCancel),
            WidgetProps::new()
                .visible(false)
                .automation_id("modal-cancel"),
        );
        ui.add_with(
            Button::new("modal_ok", "确定", Msg::ModalConfirm).primary(),
            WidgetProps::new()
                .visible(false)
                .automation_id("modal-ok"),
        );
        ui.add(
            Toast::new("toast")
                .action("撤销")
                .duration_secs(2.2),
        );
        // Cross-widget drag and drop goes through the system (OLE): a source
        // offers text, so the same drag can be dropped into another window or
        // application, and the target receives whatever text arrived.
        ui.set_drag_source(WidgetId::new("drag_a"), || Some(DragData::text("卡片 A")));
        ui.set_drag_source(WidgetId::new("drag_b"), || Some(DragData::text("卡片 B")));
        for (zone, target) in [("drop_a", 0u8), ("drop_b", 1u8)] {
            ui.set_drop_target(WidgetId::new(zone), move |data, _pos| match data {
                DragData::Text(text) if text.starts_with("卡片 ") => {
                    let source = u8::from(text.ends_with('B'));
                    Some(Msg::Dropped(source, target))
                }
                _ => None,
            });
            // Hover feedback: the app paints an accent frame around the target
            // while the drag is over it, so `true` asks for that repaint.
            ui.set_drop_hover(WidgetId::new(zone), |_data, _pos, _over| true);
        }
        // Text drag and drop: the field supplies its selection as the payload;
        // the second field takes the text and the application reports the drop.
        ui.set_text_drag_source(WidgetId::new("api_text_from"));
        ui.set_text_drop_target(WidgetId::new("api_text_to"), |text, _pos| {
            Some(Msg::TextDropped(text))
        });
        ui.set_text_drop_hover(WidgetId::new("api_text_to"), |_text, _pos, _over| true);
        ui.set_rows(WidgetId::new("list"), rows.clone());
        // Access-key underlines start hidden; an Alt tap shows them until the
        // next key/click (D75).
        ui.set_mnemonic_underlines(false);

        // M7 `布局` page. The widgets are plain Rust; the `.buixml` file only
        // moves, styles and seeds them. `m7_status` is Rust-owned and shows the
        // last report. `m7_created` is the exception: the file creates it from
        // the standard widget registry (M7.7).
        #[cfg(feature = "m7")]
        {
            ui.set_widget_registry(bamboo_native_ui::creation::WidgetRegistry::standard());
            ui.add(Label::new("m7_title", "XML 布局演示"));
            ui.add(Button::new(
                "m7_action",
                "主操作",
                Msg::M7Action("主操作"),
            ));
            ui.add(Button::new("m7_outline", "描边", Msg::M7Action("描边")));
            ui.add(ProgressBar::new("m7_progress", 0.6));
            ui.add(Card::new(
                "m7_card",
                "覆盖在卡片上的字段",
                "位置、圆角与初始文本都来自 .buixml。",
            ));
            ui.add(
                TextEdit::new("m7_edit", "重载不会打断正在输入的会话。", Msg::M7Edit)
                    .painted(),
            );
            ui.add(Slider::new("m7_slider", 0.4, 0.0..=1.0, Msg::M7Value));
            ui.add(Label::new("m7_value", "40%"));
            ui.add(ListView::new("m7_list"));
            ui.set_rows(
                WidgetId::new("m7_list"),
                vec![
                    Row::new("夜曲").subtitle("周杰伦").trailing("3:58"),
                    Row::new("晴天").subtitle("周杰伦").trailing("4:29"),
                    Row::new("稻香").subtitle("周杰伦").trailing("3:43"),
                    Row::new("七里香").subtitle("周杰伦").trailing("4:59"),
                ],
            );
            // A third-party widget: it declares `x-level`/`x-units` and the
            // `x-item` child list itself (see `meter.rs`). The library knows
            // none of these names. It paints decoration only, so it asks the
            // a11y tree for exposure explicitly.
            ui.add_with(
                meter::MeterWidget::new("m7_meter"),
                WidgetProps::new().expose(true),
            );
            // Structural child-list demos (M7.4 step 2): the widgets are plain
            // Rust, the file supplies every column/row/cell, node, section/row
            // and page label.
            ui.add(Table::new("m7_table").row_height(26.0));
            ui.add(TreeView::new("m7_tree").row_height(26.0));
            ui.add(PropertyGrid::new("m7_props").row_height(26.0));
            ui.add(Pager::new("m7_pager", 0, 4, Msg::Page));
            // Label-owning lists (M7.4 step 3): the option/step/run/point
            // elements supply every label.
            ui.add(Tabs::new("m7_tabs", &["概览"], 0, Msg::M7Select));
            ui.add(Segmented::new("m7_seg", &["低"], 0, Msg::M7Select));
            ui.add(Breadcrumb::new("m7_crumb", &["根"], Msg::M7Select));
            ui.add(NavRail::new("m7_rail", &["项"], 0, Msg::M7Select).row_height(30.0));
            ui.add(RichText::new("m7_rich"));
            ui.add(Sparkline::new("m7_spark", &[0.0]));
            ui.add(Label::new("m7_status", "布局：尚未安装"));
        }

        let (scene, specs) = layout(Size::new(1180.0, 840.0));
        register_pages(&mut ui, &specs, &mut LayoutSink);
        // The container owns page visibility: everything outside page 0 starts
        // hidden, and a tab switch flips the same flags.
        for (index, spec) in specs.iter().enumerate() {
            for child in &spec.children {
                ui.set_visible(child.id, index == 0, &mut LayoutSink);
            }
            for view in &spec.nested {
                for child in view.children_ref() {
                    ui.set_visible(child.id, index == 0, &mut LayoutSink);
                }
            }
        }
        let pulse = Rect::from_xywh(
            scene.main.center().x - 20.0,
            scene.main.center().y - 20.0,
            40.0,
            40.0,
        );

        Self {
            theme,
            dark: !light,
            #[cfg(feature = "m7")]
            theme_palette: None,
            self_test,
            a11y_dump,
            a11y_result,
            renderer_seen,
            bench,
            start: Instant::now(),
            ui,
            scene,
            specs,
            counter: 0,
            toggle_on: true,
            volume: 0.6,
            shuffle: false,
            playing_row: None,
            page: 0,
            pages: 5,
            tab: 0,
            rows,
            toast_until: None,
            last_action: "就绪".to_string(),
            status: String::new(),
            last_status: (0, 0, 0),
            pulse,
            marquee_playing: false,
            gamepad: None,
            gamepad_enabled: true,
            last_tick: Instant::now(),
            playing: false,
            transport_shuffle: false,
            transport_repeat: RepeatMode::Off,
            modal_open: false,
            gallery_color_dialog: None,
            dyn_menu,
            dyn_extra: 0,
            table_rows,
            table_sort: None,
            playlist,
            flyout_open: false,
            dyn_slots: vec![0, 1],
            custom_clicks,
            probe: self_test.then(EditProbe::new),
            painted,
            paint_micros,
            ticks,
            invalids,
            #[cfg(feature = "m7")]
            m7: M7State::default(),
        }
    }

    /// The dynamic menu's live checkmark (the model is the source of truth).
    fn dyn_checked(&self) -> bool {
        self.dyn_menu
            .command(706)
            .map(|item| item.is_checked())
            .unwrap_or(true)
    }

    /// Label of a tree node path (the message only carries the path).
    fn tree_label(&self, path: &[usize]) -> String {
        self.ui
            .with_widget::<TreeView<Msg>, _>(WidgetId::new("gallery_tree"), |tree| {
                tree.item(path).map(|item| item.label.clone())
            })
            .flatten()
            .unwrap_or_default()
    }    /// "title" or "title — artist" for a wall index, read back through the
    /// typed-access seam (the application does not keep a second cover list).
    fn cover_label(&self, index: usize, with_artist: bool) -> String {
        self.ui
            .with_widget::<CoverWall<Msg>, _>(WidgetId::new("covers_wall"), |wall| {
                wall.covers_ref().get(index).map(|cover| {
                    if with_artist && !cover.subtitle.is_empty() {
                        format!("{} — {}", cover.title, cover.subtitle)
                    } else {
                        cover.title.clone()
                    }
                })
            })
            .flatten()
            .unwrap_or_default()
    }

    /// "title — artist" for a playlist index from the application's own data.
    fn playlist_label(&self, index: usize) -> String {
        self.playlist
            .get(index)
            .map(|row| {
                if row.subtitle.is_empty() {
                    row.title.clone()
                } else {
                    format!("{} — {}", row.title, row.subtitle)
                }
            })
            .unwrap_or_default()
    }

    /// Push the application's playlist order into the widget. The widget keeps
    /// its cursor; the reorder path already moved it to the destination.
    fn push_playlist(&mut self, window: &mut WindowHandle<'_>) {
        self.ui
            .set_rows(WidgetId::new("covers_playlist"), self.playlist.clone());
        window.a11y_refresh();
    }

    /// Hide the popover card and its content widgets (outside press, its own
    /// close button, or a tab switch).
    fn close_flyout(&mut self, window: &mut WindowHandle<'_>) {
        if !self.flyout_open {
            return;
        }
        self.flyout_open = false;
        for id in FLYOUT {
            self.ui.set_visible(WidgetId::new(id), false, window);
        }
        window.a11y_refresh();
    }

    /// Reflect the dynamic panel's current insertion/z-order in its label.
    /// Raise/lower changes the tree order without moving rectangles, so the
    /// label is the observable result of those two buttons.
    fn update_dyn_info(&mut self) {
        let order: Vec<String> = self
            .ui
            .ids()
            .filter_map(|id| {
                DYN_SLOT_IDS
                    .iter()
                    .position(|slot| *slot == id.0)
                    .map(|index| (index + 1).to_string())
            })
            .collect();
        let text = if order.is_empty() {
            "顺序：（空）".to_string()
        } else {
            format!("顺序：{}", order.join(" → "))
        };
        // The setter records the widget's rectangle; the host's end-of-event
        // flush repaints it.
        self.ui.set_label(WidgetId::new("dyn_info"), text);
    }

    /// Tear down the in-window modal shell: drop the focus scope first (so
    /// hiding the focused button relocates within the page, not the panel),
    /// hide every modal widget, then return focus to the opener.
    fn close_modal(&mut self, window: &mut WindowHandle<'_>, toast: &str) {
        if !self.modal_open {
            return;
        }
        self.modal_open = false;
        self.ui.set_focus_scope(None, window);
        for id in MODAL {
            self.ui.set_visible(WidgetId::new(id), false, window);
        }
        if self.tab == 2 {
            self.ui.set_focus(WidgetId::new("modal_open"), window);
        }
        window.a11y_focus(self.ui.focus_id());
        window.a11y_refresh();
        self.show_toast(toast.to_string(), window);
    }

    fn drain(&mut self, window: &mut WindowHandle<'_>) {
        for message in self.ui.drain_messages() {
            self.apply(message, window);
        }
    }

    /// Pick the tick interval the current state needs. **The pad is polled from
    /// this timer**, so it must never be stopped while the window is live — a
    /// toast expiring or a page switch used to stop it and silently kill
    /// controller input (it came back only after a click started another
    /// toast). No animation means a slow pad-watch tick that never paints.
    fn sync_timer(&mut self, window: &mut WindowHandle<'_>) {
        let interval = if self.bench.is_some() {
            Duration::from_millis(16)
        } else if self.self_test || self.a11y_dump {
            Duration::from_millis(50)
        } else if self.gamepad.is_some() {
            Duration::from_millis(33)
        } else if self.marquee_playing
            || self.toast_until.is_some()
            || (self.tab == 2 && !self.modal_open)
        {
            Duration::from_millis(40)
        } else if cfg!(feature = "m7") && self.tab == 8 {
            // The hot reload watches the file's mtime; 250 ms is frequent
            // enough to feel live without a second timer.
            Duration::from_millis(250)
        } else {
            Duration::from_millis(500)
        };
        if std::env::var_os("BAMBOO_UI_GAMEPAD_LOG").is_some() {
            eprintln!(
                "showcase timer: {:?} pad={} toast={} marquee={} gallery={}",
                interval,
                self.gamepad.is_some(),
                self.toast_until.is_some(),
                self.marquee_playing,
                self.tab == 2
            );
        }
        window.set_animation_timer(Some(interval));
    }

    /// Attach the pad layer if XInput reports a controller. The footer is
    /// repainted so the status line shows whether the pad is live.
    fn attach_gamepad(&mut self, window: &mut WindowHandle<'_>) {
        if self.gamepad.is_some() || read_gamepad(0).is_none() {
            return;
        }
        let mut pad = Gamepad::new(0);
        pad.set_enabled(self.gamepad_enabled);
        self.gamepad = Some(pad);
        self.ui.set_label(
            WidgetId::new("api_pad_state"),
            "手柄：已连接（XInput）",
        );
        window.invalidate(self.scene.footer);
        if std::env::var_os("BAMBOO_UI_GAMEPAD_LOG").is_some() {
            eprintln!("showcase: pad attached");
        }
        self.sync_timer(window);
    }

    /// Poll a present pad and notice a disconnect (including a sleeping pad):
    /// the layer is dropped and the slow watch tick takes over, so a pad that
    /// goes away for a moment comes back without restarting the showcase.
    fn poll_gamepad(&mut self, window: &mut WindowHandle<'_>) {
        if self.gamepad.is_none() {
            self.attach_gamepad(window);
            return;
        }
        let keys = self
            .gamepad
            .as_mut()
            .map(|pad| pad.poll(Instant::now(), window.is_foreground()))
            .unwrap_or_default();
        if read_gamepad(0).is_none() {
            self.gamepad = None;
            self.ui.set_label(
                WidgetId::new("api_pad_state"),
                "手柄：未连接（XInput）",
            );
            window.invalidate(self.scene.footer);
            if std::env::var_os("BAMBOO_UI_GAMEPAD_LOG").is_some() {
                eprintln!("showcase: pad detached");
            }
            self.sync_timer(window);
            return;
        }
        if !keys.is_empty() {
            if std::env::var_os("BAMBOO_UI_GAMEPAD_LOG").is_some() {
                for key in &keys {
                    eprintln!("showcase: pad key {:?} shift={}", key.code, key.shift);
                }
            }
            for key in keys {
                self.ui.pad_key(key, window);
            }
            self.drain(window);
        }
    }

    /// Place the widgets that live outside a page: the tab strip, the sidebar
    /// theme switch, the toast, the popover card and the modal shell. Page
    /// demos are positioned by their `ScrollView` instead.
    fn layout_chrome(&mut self, window: &mut WindowHandle<'_>) {
        let scene = &self.scene;
        let (tabs, theme, toast) = (scene.tabs, scene.theme, scene.toast);
        let size = Size::new(
            scene.sidebar.width() + scene.main.width(),
            scene.sidebar.height(),
        );
        for (id, rect) in [
            (WidgetId::new("tabs"), tabs),
            (WidgetId::new("theme"), theme),
            (WidgetId::new("toast"), toast),
        ] {
            self.ui.set_rect(id, rect);
        }
        // The badge is a Tabs feature, but a narrow window has no room for it;
        // the runtime setter keeps eight tabs readable at the minimum width.
        let badge = (tabs.width() >= 640.0).then(|| "新".to_string());
        self.ui.edit_widget::<Tabs<Msg>>(WidgetId::new("tabs"), |tabs| {
            tabs.set_badge(2, badge)
        });
        register_pages(&mut self.ui, &self.specs, window);
        // The popover anchors to its trigger's current client rectangle, so it
        // is placed after the page is positioned.
        let anchor = self
            .ui
            .rect(WidgetId::new("flyout_open"))
            .unwrap_or(tabs);
        let card = place_popover(
            anchor,
            Size::new(300.0, 170.0),
            PopoverPlacement::Auto,
            Rect::from_size(size),
            10.0,
        );
        self.ui.set_rect(WidgetId::new("flyout"), card);
        self.ui
            .edit_widget::<Popover<Msg>>(WidgetId::new("flyout"), |popover| {
                popover.set_anchor(anchor);
                popover.set_viewport(Rect::from_size(size));
                false
            });
        let content = card.inset(14.0, 14.0);
        self.ui.set_rect(
            WidgetId::new("flyout_title"),
            Rect::new(
                content.left,
                content.top,
                content.right,
                content.top + 22.0,
            ),
        );
        self.ui.set_rect(
            WidgetId::new("flyout_body"),
            Rect::new(
                content.left,
                content.top + 26.0,
                content.right,
                content.bottom - 38.0,
            ),
        );
        self.ui.set_rect(
            WidgetId::new("flyout_close"),
            Rect::from_xywh(content.right - 76.0, content.bottom - 30.0, 76.0, 30.0),
        );
        // Modal shell: scrim over the whole window, a centered card.
        let card = Rect::from_xywh(
            ((size.width - 420.0) / 2.0).max(16.0),
            ((size.height - 220.0) / 2.0).max(16.0),
            420.0,
            220.0,
        );
        self.ui
            .set_rect(WidgetId::new("modal_scrim"), Rect::from_size(size));
        self.ui.set_rect(WidgetId::new("modal_card"), card);
        self.ui.set_rect(
            WidgetId::new("modal_check"),
            Rect::from_xywh(card.left + 22.0, card.top + 92.0, 200.0, 28.0),
        );
        self.ui.set_rect(
            WidgetId::new("modal_cancel"),
            Rect::from_xywh(card.right - 18.0 - 96.0, card.bottom - 18.0 - 36.0, 96.0, 36.0),
        );
        self.ui.set_rect(
            WidgetId::new("modal_ok"),
            Rect::from_xywh(
                card.right - 18.0 - 96.0 - 12.0 - 96.0,
                card.bottom - 18.0 - 36.0,
                96.0,
                36.0,
            ),
        );
        // The runtime content setters (tab badge, popover anchor) recorded
        // their own damage; one request covers the whole chrome pass.
        self.ui.flush_damage(window);
    }

    /// Show the widgets of one page and hide every other page's. The toast,
    /// the sidebar theme switch, the popover and the modal shell keep their own
    /// visibility.
    fn apply_page_visibility(&mut self, window: &mut WindowHandle<'_>) {
        for (index, spec) in self.specs.iter().enumerate() {
            let visible = index == self.tab;
            for child in &spec.children {
                self.ui.set_visible(child.id, visible, window);
            }
            // A nested panel owns its rows, so they are not page children;
            // they follow the page too.
            for view in &spec.nested {
                for child in view.children_ref() {
                    self.ui.set_visible(child.id, visible, window);
                }
            }
        }
    }

    // -- M7 layout skin (`布局` page) ---------------------------------------

    /// Read and parse the skin file (falling back to the embedded copy), and
    /// keep the document when it parses. Returns whether the generation
    /// changed. A parse failure keeps the previous document running and stores
    /// the first diagnostic for the status line.
    #[cfg(feature = "m7")]
    fn m7_load(&mut self) -> bool {
        let path = M7State::path();
        let modified = std::fs::metadata(&path)
            .and_then(|meta| meta.modified())
            .ok();
        let text =
            std::fs::read_to_string(&path).unwrap_or_else(|_| M7State::embedded().to_string());
        self.m7.modified = modified;
        match LayoutDocument::parse(&text) {
            Ok(document) => {
                let changed = self
                    .m7
                    .document
                    .as_ref()
                    .map(|old| old.generation())
                    != Some(document.generation());
                self.m7.document = Some(document);
                self.m7.reload_failure = None;
                changed
            }
            Err(diagnostics) => {
                let first = diagnostics
                    .first()
                    .map(|item| item.to_string())
                    .unwrap_or_else(|| "unknown parse error".to_string());
                self.m7.reload_failure = Some(first.clone());
                self.m7.status = format!("重载错误：{first}");
                self.ui.debug_event("error", format!("parse: {first}"));
                false
            }
        }
    }

    /// The palette the showcase paints and resolves layout skin tokens with.
    /// A loaded `.buitheme` document wins; otherwise it follows the theme
    /// presets and the light/dark decision.
    fn palette(&self) -> Palette {
        #[cfg(feature = "m7")]
        if let Some(palette) = self.theme_palette {
            return palette;
        }
        Palette::new(&self.theme, self.dark)
    }

    /// Apply the installed document at `size`. A failed apply leaves the live
    /// tree untouched; the status line reports it.
    #[cfg(feature = "m7")]
    fn m7_apply(&mut self, size: Size, window: &mut WindowHandle<'_>) {
        if self.m7.document.is_none() {
            self.m7_load();
        }
        let Some(document) = self.m7.document.clone() else {
            if let Some(failure) = self.m7.reload_failure.clone() {
                self.ui
                    .set_label(WidgetId::new("m7_status"), format!("布局错误：{failure}"));
            }
            return;
        };
        let palette = self.palette();
        let mut status = match self.ui.apply_layout(&document, &palette, size, window) {
            Ok(report) => format!("布局：{report}"),
            Err(failure) => format!("布局失败：{failure}"),
        };
        if let Some(failure) = self.m7.reload_failure.as_ref() {
            status.push_str(" · 重载错误：");
            status.push_str(failure);
        }
        self.m7.status = status.clone();
        self.ui.set_label(WidgetId::new("m7_status"), status);
    }

    /// Load the checked-in `.buitheme` document and install its palette on the
    /// window and the layout token resolver (M7.8). Returns whether it parsed.
    #[cfg(feature = "m7")]
    fn load_night_theme(&mut self, window: &mut WindowHandle<'_>) -> bool {
        use bamboo_native_ui::theme_xml::{ThemeDocument, ThemeDocumentExt};
        const NIGHT: &str =
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/themes/night.buitheme"));
        let document = match ThemeDocument::parse(NIGHT) {
            Ok(document) => document,
            Err(diagnostics) => {
                let first = diagnostics
                    .first()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "theme parse error".to_string());
                self.ui.debug_event("error", format!("theme: {first}"));
                self.show_toast(format!("主题解析失败：{first}"), window);
                return false;
            }
        };
        let palette = document.palette(&self.theme);
        self.dark = palette.dark;
        self.theme.accent = palette.accent.to_rgb();
        self.theme_palette = Some(palette);
        window.set_theme(self.theme.clone(), self.dark);
        window.set_palette(palette);
        if self.m7.document.is_some() {
            self.m7_apply(window.size(), window);
        }
        self.ui.set_label(
            WidgetId::new("api_theme_state"),
            format!(
                "主题：{} · base {} · background #16181d",
                document.name().unwrap_or("未命名"),
                document.base().name()
            ),
        );
        true
    }

    /// Drop the document palette and return to the preset theme (used by the
    /// light/dark toggle and after the self-test check).
    #[cfg(feature = "m7")]
    fn clear_theme_document(&mut self, window: &mut WindowHandle<'_>) {
        self.theme_palette = None;
        let palette = self.palette();
        window.set_theme(self.theme.clone(), self.dark);
        window.set_palette(palette);
        if self.m7.document.is_some() {
            self.m7_apply(window.size(), window);
        }
    }

    /// The 250 ms hot reload: re-read the file when its mtime changed.
    #[cfg(feature = "m7")]
    fn m7_poll(&mut self, window: &mut WindowHandle<'_>) {
        let path = M7State::path();
        let modified = std::fs::metadata(&path)
            .and_then(|meta| meta.modified())
            .ok();
        if modified == self.m7.modified {
            return;
        }
        let size = window.size();
        let previous = self
            .m7
            .document
            .as_ref()
            .map(|document| document.generation());
        if self.m7_load() {
            let current = self
                .m7
                .document
                .as_ref()
                .map(|document| document.generation());
            if current != previous {
                self.m7_apply(size, window);
            }
        } else if let Some(failure) = self.m7.reload_failure.clone() {
            self.ui
                .set_label(WidgetId::new("m7_status"), format!("布局错误：{failure}"));
        }
    }

    // -- Debug API (`debug-form`, driven by the 调试 menu) -------------------

    /// The library paints no inspector: the menu formats a snapshot and shows
    /// it in an ordinary information dialog.
    #[cfg(feature = "m7")]
    fn debug_show(&mut self, title: &str, text: String, window: &mut WindowHandle<'_>) {
        window.show_dialog(MessageDialog::new(text).title(format!("调试 · {title}")));
    }

    #[cfg(feature = "m7")]
    fn debug_layout_text(&self) -> String {
        let snapshot = self.ui.debug_snapshot();
        let mut text = if snapshot.widget_types.is_empty() {
            "控件注册表：未安装（文档不能创建控件）\n".to_string()
        } else {
            format!(
                "控件注册表 {} 项 · 命令表 {}\n  {}\n",
                snapshot.widget_types.len(),
                if snapshot.has_commands { "有" } else { "无" },
                snapshot.widget_types.join(", ")
            )
        };
        let Some(layout) = snapshot.layout else {
            text.push_str("布局：尚未安装（页面使用 Rust 的初始矩形）");
            return text;
        };
        text.push_str(&format!(
            "generation {}\nvariant {} · client {:.0}×{:.0}\n\
             entries {} · content writes {} · skin writes {}\n\
             claims {} · warnings {}\n",
            layout.generation,
            layout.variant,
            layout.client.width,
            layout.client.height,
            layout.entries.len(),
            layout.content_writes,
            layout.skin_writes,
            layout.claims.len(),
            layout.warnings.len(),
        ));
        if !layout.scrolls.is_empty() {
            text.push_str("\n滚动面板\n");
            for scroll in &layout.scrolls {
                text.push_str(&format!(
                    "  {} viewport ({:.0},{:.0} {:.0}×{:.0}) offset ({:.0},{:.0}) content ({:.0}×{:.0})\n",
                    scroll.id.0,
                    scroll.viewport.left,
                    scroll.viewport.top,
                    scroll.viewport.width(),
                    scroll.viewport.height(),
                    scroll.offset.x,
                    scroll.offset.y,
                    scroll.content.width,
                    scroll.content.height,
                ));
            }
        }
        text.push_str("\n条目\n");
        for entry in layout.entries.iter().take(40) {
            text.push_str(&format!(
                "  {} ({:.0},{:.0} {:.0}×{:.0}) {:?}\n",
                entry.id.0,
                entry.rect.left,
                entry.rect.top,
                entry.rect.width(),
                entry.rect.height(),
                entry.space,
            ));
        }
        if layout.entries.len() > 40 {
            text.push_str("  …\n");
        }
        if !layout.warnings.is_empty() {
            text.push_str("\n警告\n");
            for warning in &layout.warnings {
                text.push_str(&format!("  ! {warning}\n"));
            }
        }
        text
    }

    #[cfg(feature = "m7")]
    fn debug_widget_text(&self, limit: usize) -> String {
        let snapshot = self.ui.debug_snapshot();
        let name = |id: Option<WidgetId>| id.map_or_else(|| "-".to_string(), |id| id.0.to_string());
        let mut text = format!(
            "{} 个控件 · focus {} · hover {} · press {}\n",
            snapshot.widgets.len(),
            name(snapshot.focus),
            name(snapshot.hover),
            name(snapshot.press),
        );
        for widget in snapshot.widgets.iter().take(limit) {
            let mut flags = Vec::new();
            if widget.focused {
                flags.push("焦点");
            }
            if widget.hovered {
                flags.push("悬停");
            }
            if widget.pressed {
                flags.push("按下");
            }
            let flags = if flags.is_empty() {
                String::new()
            } else {
                format!(" [{}]", flags.join("+"))
            };
            let scroll = widget
                .scroll_parent
                .map(|panel| format!(" 滚动({})", panel.0))
                .unwrap_or_default();
            text.push_str(&format!(
                "\n{}{}\n  rect ({:.0},{:.0} {:.0}×{:.0}){}{}{}{}{}\n",
                widget.id.0,
                flags,
                widget.rect.left,
                widget.rect.top,
                widget.rect.width(),
                widget.rect.height(),
                if widget.visible { " 可见" } else { " 隐藏" },
                if widget.enabled { " 可用" } else { " 禁用" },
                if widget.focusable { " 可聚焦" } else { "" },
                if widget.read_only { " 只读" } else { "" },
                scroll,
            ));
            if let Some(a11y) = &widget.a11y {
                text.push_str(&format!(
                    "  a11y {:?} \"{}\"{}\n",
                    a11y.role,
                    a11y.name,
                    a11y.value
                        .as_ref()
                        .map(|value| format!(" value=\"{value}\""))
                        .unwrap_or_default(),
                ));
            }
            if !widget.skin_attrs.is_empty() || !widget.skin_children.is_empty() {
                let attrs = widget
                    .skin_attrs
                    .iter()
                    .map(|(name, kind)| format!("{name}:{kind:?}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                text.push_str(&format!(
                    "  skin [{attrs}] 子元素 [{}]\n",
                    widget.skin_children.join(", "),
                ));
            }
        }
        if snapshot.widgets.len() > limit {
            text.push_str("\n…（完整列表见「完整控件树到控制台」）\n");
        }
        text
    }

    #[cfg(feature = "m7")]
    fn debug_frame_text(&self) -> String {
        let snapshot = self.ui.debug_snapshot();
        let paint = &snapshot.paint;
        let mut text = format!(
            "帧 {} · 最近一帧 {} 个控件 / {} µs · 累计 {} 次控件绘制\n\
             待处理脏矩形 {} · 最近一次提交 {} 块\n",
            paint.frames,
            paint.last_frame_widgets,
            paint.last_frame_micros,
            paint.widget_paints,
            snapshot.pending_damage.len(),
            snapshot.last_damage.len(),
        );
        if !snapshot.last_damage.is_empty() {
            text.push_str("\n最近提交的脏矩形\n");
            for rect in snapshot.last_damage.iter().take(16) {
                text.push_str(&format!(
                    "  ({:.0},{:.0} {:.0}×{:.0})\n",
                    rect.left,
                    rect.top,
                    rect.width(),
                    rect.height(),
                ));
            }
        }
        text.push_str("\n绘制耗时（累计）\n");
        for stat in paint.per_widget.iter().take(12) {
            let average = if stat.paints == 0 {
                0
            } else {
                stat.micros / stat.paints
            };
            text.push_str(&format!(
                "  {} {} 次 {} µs · 平均 {} µs\n",
                stat.id.0, stat.paints, stat.micros, average,
            ));
        }
        text
    }

    #[cfg(feature = "m7")]
    fn debug_log_text(&self, limit: usize) -> String {
        let snapshot = self.ui.debug_snapshot();
        let start = snapshot.log.len().saturating_sub(limit);
        let mut text = format!(
            "{} 条 · 显示最近 {}\n",
            snapshot.log.len(),
            snapshot.log.len() - start
        );
        for event in &snapshot.log[start..] {
            text.push_str(&format!(
                "\n#{} [{}] {}",
                event.sequence, event.kind, event.text
            ));
        }
        text.push('\n');
        text
    }

    #[cfg(feature = "m7")]
    fn debug_reload(&mut self, window: &mut WindowHandle<'_>) {
        let changed = self.m7_load();
        self.ui
            .debug_event("note", format!("reload: changed={changed}"));
        let size = window.size();
        self.m7_apply(size, window);
        self.show_toast(
            format!("布局已重载（{}）", if changed { "新代" } else { "同代" }),
            window,
        );
    }

    #[cfg(feature = "m7")]
    fn debug_save(&mut self, window: &mut WindowHandle<'_>) {
        let Some(document) = self.m7.document.clone() else {
            self.show_toast("没有可保存的布局文档".to_string(), window);
            return;
        };
        let path = M7State::path();
        let backup = path.with_extension("buixml.bak");
        let text = document.to_xml();
        let result = (|| -> std::io::Result<()> {
            if path.exists() {
                std::fs::copy(&path, &backup)?;
            }
            std::fs::write(&path, text.as_bytes())
        })();
        match result {
            Ok(()) => {
                self.m7.modified = std::fs::metadata(&path)
                    .and_then(|meta| meta.modified())
                    .ok();
                self.ui
                    .debug_event("note", format!("saved {}", path.display()));
                self.show_toast("布局已保存（已写 .buixml.bak 备份）".to_string(), window);
            }
            Err(error) => self.show_toast(format!("保存失败：{error}"), window),
        }
    }

    #[cfg(feature = "m7")]
    fn debug_reset(&mut self, window: &mut WindowHandle<'_>) {
        self.ui.clear_layout(window);
        self.m7.document = None;
        self.m7.status = "布局：已重置（文件再次变化会重新加载）".to_string();
        self.ui
            .set_label(WidgetId::new("m7_status"), self.m7.status.clone());
        self.ui.debug_event("note", "reset: layout cleared");
        self.show_toast("布局已重置".to_string(), window);
    }

    fn apply(&mut self, message: Msg, window: &mut WindowHandle<'_>) {
        match message {
            Msg::Primary => {
                self.counter += 1;
                self.ui
                    .set_label(WidgetId::new("primary"), format!("主按钮 {}", self.counter));
                self.ui
                    .set_label(WidgetId::new("stat2"), self.counter.to_string());
                self.show_toast(format!("主按钮被点击 {} 次", self.counter), window);
            }
            Msg::Secondary => self.show_toast("次按钮被点击".to_string(), window),
            Msg::Danger => self.show_toast("危险操作（红色按钮）".to_string(), window),
            Msg::Toggle(on) => {
                self.toggle_on = on;
                self.ui.set_toggle(WidgetId::new("toggle"), on);
                self.show_toast(
                    if on { "开关已打开" } else { "开关已关闭" }.to_string(),
                    window,
                );
            }
            Msg::Segment(index) => {
                self.ui.set_selected(WidgetId::new("segmented"), index);
            }
            Msg::Tab(index) => {
                let index = index.min(PAGE_NAMES.len() - 1);
                self.ui.set_selected(WidgetId::new("tabs"), index);
                // Park keyboard focus on the tab strip before the page's
                // widgets are hidden: otherwise hiding the focused widget
                // relocates focus into the new page and scrolls it to that
                // widget's rectangle.
                self.ui.set_focus(WidgetId::new("tabs"), window);
                // A page switch never leaves the flyout over the new page.
                self.close_flyout(window);
                if self.modal_open && index != 2 {
                    self.close_modal(window, "已关闭");
                }
                if index != self.tab {
                    self.tab = index;
                    // A cross-widget drag belongs to the page it started on.
                    self.ui.cancel_drag(window);
                    self.apply_page_visibility(window);
                    if index == 2 {
                        self.last_tick = Instant::now();
                    }
                    self.sync_timer(window);
                    window.a11y_refresh();
                    window.invalidate_all();
                    // Coming back to the skin page re-applies the document so
                    // it wins over any application-side change.
                    #[cfg(feature = "m7")]
                    if index == 8 {
                        self.m7_apply(window.size(), window);
                    }
                }
                self.set_action(format!("切换到「{}」", PAGE_NAMES[index]), window);
            }
            Msg::Volume(value) => {
                self.volume = value;
                // Both setters record paint damage; `apply` flushes it once.
                self.ui.set_value(WidgetId::new("volume"), value);
                self.ui.set_progress(WidgetId::new("progress"), value);
            }
            Msg::PlayRow(index) => {
                if index >= self.rows.len() {
                    return;
                }
                if let Some(old) = self.playing_row {
                    self.rows[old].current = false;
                }
                self.rows[index].current = true;
                self.playing_row = Some(index);
                self.ui.set_rows(WidgetId::new("list"), self.rows.clone());
                self.ui.set_selected(WidgetId::new("list"), index);
                // Updating the tray tooltip is a NIM_MODIFY of the same icon —
                // only once the API page's button installed one.
                if window.tray_icon_visible() {
                    window.set_tray_icon(Some(
                        TrayIcon::new(format!("正在播放：曲目 {:03}", index + 1)).menu(tray_menu()),
                    ));
                }
                self.show_toast(format!("播放第 {} 首", index + 1), window);
            }
            Msg::Page(delta) => {
                let next = (self.page as i32 + delta).clamp(0, self.pages as i32 - 1) as usize;
                if next != self.page {
                    self.page = next;
                    self.ui.set_selected(WidgetId::new("pager"), next);
                }
            }
            Msg::Theme => {
                #[cfg(feature = "m7")]
                {
                    self.theme_palette = None;
                }
                self.dark = !self.dark;
                window.set_theme(self.theme.clone(), self.dark);
                window.set_palette(self.palette());
                // The native bar's checkmark follows the same flag.
                let dark = self.dark;
                window.edit_menu_bar(|bar| bar.set_checked(820, dark));
                // Background tokens resolve against the palette, so re-apply
                // the skin (same generation: content is not rewritten).
                #[cfg(feature = "m7")]
                if self.m7.document.is_some() {
                    self.m7_apply(window.size(), window);
                }
            }
            #[cfg(feature = "m7")]
            Msg::ApiThemeDoc => {
                if self.load_night_theme(window) {
                    self.set_action("已加载 themes/night.buitheme", window);
                }
            }
            Msg::Search(text) => {
                self.ui.set_text(WidgetId::new("search"), text.clone());
                self.show_toast(format!("搜索“{text}”"), window);
            }
            Msg::Count(count) => {
                self.ui.set_number(WidgetId::new("count"), count);
                self.show_toast(format!("数量 {count}"), window);
            }
            // The rounded-surface buttons report the radius they paint, so the
            // toast doubles as a label for the demo.
            Msg::RoundClick(radius) => self.show_toast(
                format!("圆角 {radius:.0} DIP：填充与焦点环一起圆化"),
                window,
            ),
            Msg::RoundSearch(text) => {
                self.show_toast(format!("胶囊搜索框：“{text}”"), window)
            }
            Msg::ApiTextCommitted(text) => {
                self.ui.set_label(
                    WidgetId::new("api_text_state"),
                    format!("文本拖放：已提交 {} 个字符", text.chars().count()),
                );
            }
            Msg::TextDropped(text) => {
                self.ui.set_label(
                    WidgetId::new("api_text_state"),
                    format!("文本拖放：已接收 {} 个字符", text.chars().count()),
                );
                self.show_toast(format!("文本已拖入：{text}"), window);
            }
            Msg::Swatch => self.show_toast("竹绿 #7FD3A8".to_string(), window),
            Msg::SwatchContext => {
                self.show_toast("色块右键（Trigger::RightClick）".to_string(), window)
            }
            Msg::SwatchDouble => self.show_toast("双击色块".to_string(), window),
            Msg::RemoveTag => {
                self.ui.set_visible(WidgetId::new("badge_flac"), false, window);
                window.a11y_refresh();
                self.show_toast("已移除标签".to_string(), window);
            }
            Msg::OpenLink => self.show_toast("链接标签被激活".to_string(), window),
            Msg::CopyText => {
                let copied = window.copy_text("Bamboo Native UI");
                self.show_toast(
                    if copied { "已复制到剪贴板" } else { "剪贴板被占用" }.to_string(),
                    window,
                );
            }
            Msg::RichLink(id) => self.show_toast(format!("富文本链接被点击：{id}"), window),
    Msg::MarqueeStart => {
        self.marquee_playing = true;
        self.last_tick = Instant::now();
        self.sync_timer(window);
    }
    Msg::MarqueeStop => {
        self.marquee_playing = false;
        self.sync_timer(window);
    }
            Msg::Random(on) => {
                self.ui.set_toggle(WidgetId::new("cb_random"), on);
                self.show_toast(format!("随机播放：{}", if on { "开" } else { "关" }), window);
            }
            Msg::Shuffle(on) => {
                self.ui.set_toggle(WidgetId::new("cb_shuffle"), on);
                self.shuffle = on;
                self.show_toast(format!("单曲循环：{}", if on { "开" } else { "关" }), window);
            }
            Msg::Square(on) => {
                self.ui.set_toggle(WidgetId::new("cb_square"), on);
                self.show_toast(
                    format!("方框勾选：{}", if on { "选中" } else { "取消" }),
                    window,
                );
            }
            Msg::RadioOnly => self.show_toast("单选项被选中".to_string(), window),
            Msg::RadioGroupPick(index) => {
                self.ui.set_selected(WidgetId::new("radio_group"), index);
                self.show_toast(format!("循环方式切换为第 {} 项", index + 1), window);
            }
            Msg::GallerySearch(text) => {
                self.ui
                    .set_text(WidgetId::new("gallery_search"), text.clone());
                self.show_toast(format!("搜索“{text}”"), window);
            }
            Msg::PaintedName(text) => {
                self.ui
                    .set_text(WidgetId::new("painted_name"), text.clone());
                self.show_toast(format!("自绘编辑框已提交：{text}"), window);
            }
            Msg::GalleryPassword(text) => self.show_toast(
                format!("密码已更新（{} 个字符）", text.chars().count()),
                window,
            ),
            Msg::GalleryCombo(index) => {
                self.ui.set_selected(WidgetId::new("gallery_combo"), index);
                self.show_toast(format!("播放模式切换为第 {} 项", index + 1), window);
            }
            Msg::GalleryDate(date) => {
                self.show_toast(format!("日期选择：{}", date.display()), window);
            }
            Msg::GalleryTime(time) => {
                self.show_toast(format!("时间选择：{}", time.display()), window);
            }
            Msg::GalleryColorRgba(rgba) => {
                self.show_toast(format!("颜色选择：{}", rgba.display()), window);
            }
            Msg::GalleryColorMore => {
                // The picker's "more colors…" row opens the library's own
                // themed color window (not the system chooser), seeded with the
                // picker's color. Transparency is offered only when the picker
                // property allows it.
                let (current, allow_alpha) = self
                    .ui
                    .with_widget::<ColorPicker<Msg>, _>(WidgetId::new("gallery_color"), |picker| {
                        (picker.rgba(), picker.allows_alpha())
                    })
                    .unwrap_or((Rgba::opaque(Color::new(0x19, 0x71, 0xC2)), true));
                let id = window.show_color_window(current, allow_alpha);
                self.gallery_color_dialog = Some(id);
                self.set_action("已打开颜色窗口（更多颜色）", window);
            }
            Msg::GalleryNotes(text) => {
                self.ui.set_text(WidgetId::new("gallery_notes"), text.clone());
                self.show_toast(
                    format!("备注已提交（{} 个字符）", text.chars().count()),
                    window,
                );
            }
            Msg::GalleryNotesRaw(text) => {
                self.ui
                    .set_text(WidgetId::new("gallery_notes_raw"), text.clone());
                self.show_toast(
                    format!("原始文本已提交（{} 个字符）", text.chars().count()),
                    window,
                );
            }
            Msg::GalleryMulti(rows) => {
                self.show_toast(format!("多选：{} 项", rows.len()), window);
            }
            Msg::GalleryAccordion(index, open) => {
                self.show_toast(
                    format!("第 {} 节已{}", index + 1, if open { "展开" } else { "收起" }),
                    window,
                );
            }
            Msg::GalleryRating(value) => {
                self.show_toast(
                    if value == 0 {
                        "评分已清除".to_string()
                    } else {
                        format!("评分：{value} 星")
                    },
                    window,
                );
            }
            Msg::GalleryBreadcrumb(index) => {
                self.show_toast(format!("面包屑导航：第 {} 级", index + 1), window);
            }
            Msg::GalleryNavRail(index) => {
                self.show_toast(format!("导航：第 {} 项", index + 1), window);
            }
            Msg::Chrome(command) => {
                // The dynamic menu edits its own model. A change applies to
                // the next open (the open popup keeps its snapshot, D61).
                let mut dynamic_note = None;
                match command {
                    706 => {
                        let checked = self.dyn_checked();
                        self.dyn_menu.set_checked(706, !checked);
                        dynamic_note = Some(format!(
                            "自动续播：{}",
                            if checked { "关" } else { "开" }
                        ));
                    }
                    730 => {
                        let extra = self.dyn_extra + 1;
                        self.dyn_extra = extra;
                        let index = self.dyn_menu.len().saturating_sub(1);
                        self.dyn_menu.insert(
                            index,
                            MenuEntry::item(
                                DYN_FIRST_COMMAND + extra as u32 - 1,
                                format!("动态项 {extra}"),
                            ),
                        );
                        dynamic_note = Some(format!("已添加「动态项 {extra}」"));
                    }
                    731 => {
                        if self.dyn_extra == 0 {
                            dynamic_note = Some("没有可移除的动态项".to_string());
                        } else {
                            let command = DYN_FIRST_COMMAND + self.dyn_extra as u32 - 1;
                            if let Some(item) = self.dyn_menu.remove_command(command) {
                                self.dyn_extra -= 1;
                                dynamic_note = Some(format!("已移除「{}」", item.label()));
                            }
                        }
                    }
                    732 => {
                        // Imperative open: the same path a shortcut uses.
                        if let Some(rect) = self.ui.rect(WidgetId::new(DYN_MENU_ID)) {
                            let opened = self.ui.open_menu(
                                WidgetId::new(DYN_MENU_ID),
                                Point::new(rect.left, rect.bottom),
                                window,
                            );
                            dynamic_note =
                                Some(if opened { "已从代码打开菜单" } else { "菜单为空" }.to_string());
                        }
                    }
                    733 => {
                        // Reopen the submenu as its own root popup.
                        let path = MenuPath::root().label("更多");
                        let opened = self.ui.expand_menu(
                            WidgetId::new(DYN_MENU_ID),
                            &path,
                            window,
                        );
                        dynamic_note = Some(
                            if opened {
                                "已把「更多」作为根菜单打开"
                            } else {
                                "子菜单不存在"
                            }
                            .to_string(),
                        );
                    }
                    _ => {}
                }
                if let Some(note) = dynamic_note {
                    let id = WidgetId::new(DYN_MENU_ID);
                    self.ui.set_menu(id, Some(self.dyn_menu.clone()));
                    // `set_menu` is silent (a menu has no on-screen part), so a
                    // screen reader needs the snapshot rebuilt to see the edit.
                    window.a11y_refresh();
                    self.show_toast(note, window);
                    return;
                }
                let status = if command == 799 {
                    "主操作：导出".to_string()
                } else {
                    format!("命令 {command}")
                };
                self.ui.edit_widget::<StatusBar>(
                    WidgetId::new("gallery_status"),
                    |bar| bar.set_segment(0, status.clone()),
                );
                window.a11y_refresh();
                self.show_toast(status, window);
            }
            Msg::GallerySplit(position) => {
                // The splitter reports the dragged centre in client
                // coordinates (the pointer space). Page children live in the
                // scroll panel's content coordinates:
                //   content = client - content_origin + scroll_offset
                let (area, offset) = self
                    .ui
                    .scroll_view(WidgetId::new(PAGE_SCROLLS[2]))
                    .map(|view| (view.content_rect(), view.offset_of()))
                    .unwrap_or((Rect::default(), Point::new(0.0, 0.0)));
                let content_x = content_x_from_client(area, offset, position);
                let Some(spec) = self.specs.get_mut(2) else {
                    return;
                };
                let (Some(a), Some(b)) = (
                    spec.child_rect("gallery_pane_a"),
                    spec.child_rect("gallery_pane_b"),
                ) else {
                    return;
                };
                let top = a.top;
                let height = a.height();
                // Two 86 DIP minimum panes around a 12 DIP divider.
                let centre = content_x
                    .max(a.left + 86.0)
                    .min(b.right - 86.0);
                let new_a = Rect::new(a.left, top, centre - 6.0, top + height);
                let new_s = Rect::new(centre - 6.0, top, centre + 6.0, top + height);
                let new_b = Rect::new(centre + 6.0, top, b.right, top + height);
                spec.set_child_rect("gallery_pane_a", new_a);
                spec.set_child_rect("gallery_splitter", new_s);
                spec.set_child_rect("gallery_pane_b", new_b);
                register_pages(&mut self.ui, &self.specs, window);
                window.invalidate(self.scene.viewport);
            }
            Msg::BannerRetry => {
                self.ui
                    .set_visible(WidgetId::new("gallery_banner"), false, window);
                self.show_toast("开始更新".to_string(), window);
            }
            Msg::BannerClose => {
                self.ui
                    .set_visible(WidgetId::new("gallery_banner"), false, window);
                self.show_toast("已关闭通知".to_string(), window);
            }
            Msg::GalleryTableSelect(index) => {
                self.show_toast(format!("选中第 {} 行", index + 1), window);
            }
            Msg::GalleryTableOpen(index) => {
                let name = self
                    .table_rows
                    .get(index)
                    .map(|row| row.cell(1).to_string())
                    .unwrap_or_default();
                self.show_toast(format!("播放「{name}」"), window);
            }
            Msg::GalleryTableCell(row, column, text) => {
                // The widget already paints the committed text; the
                // application keeps its own copy as the source of truth.
                window.a11y_refresh();
                if let Some(row) = self.table_rows.get_mut(row) {
                    if row.cells.len() <= column {
                        row.cells.resize(column + 1, String::new());
                    }
                    row.cells[column] = text.clone();
                }
                let title = gallery_table_columns()
                    .get(column)
                    .map(|column| column.title().to_string())
                    .unwrap_or_default();
                self.show_toast(
                    format!("第 {} 行「{title}」改为「{text}」", row + 1),
                    window,
                );
            }
            Msg::GalleryTableSort(column) => {
                let ascending = match self.table_sort {
                    Some((sorted, ascending)) if sorted == column => !ascending,
                    _ => true,
                };
                self.table_sort = Some((column, ascending));
                let mut rows = self.table_rows.clone();
                rows.sort_by(|left, right| {
                    let order = left.cell(column).cmp(right.cell(column));
                    if ascending {
                        order
                    } else {
                        order.reverse()
                    }
                });
                self.ui.edit_widget::<Table<Msg>>(
                    WidgetId::new("gallery_table"),
                    |table| {
                        let rows_changed = table.set_rows(rows);
                        let sort_changed = table.set_sort(Some((column, ascending)));
                        rows_changed || sort_changed
                    },
                );
                window.a11y_refresh();
                let title = gallery_table_columns()
                    .get(column)
                    .map(|column| column.title().to_string())
                    .unwrap_or_default();
                self.show_toast(
                    format!(
                        "按「{title}」{}排序",
                        if ascending { "升序" } else { "降序" }
                    ),
                    window,
                );
            }
            Msg::GalleryTreeSelect(path) => {
                let label = self.tree_label(&path);
                self.show_toast(format!("选中：{label}"), window);
            }
            Msg::GalleryTreeOpen(path) => {
                let label = self.tree_label(&path);
                self.show_toast(format!("打开：{label}"), window);
            }
            Msg::CoverSelect(index) => {
                let label = self.cover_label(index, false);
                self.show_toast(format!("选中：{label}"), window);
            }
            Msg::CoverOpen(index) => {
                let label = self.cover_label(index, true);
                self.show_toast(format!("打开：{label}"), window);
            }
            Msg::PlaylistSelect(index) => {
                let label = self.playlist_label(index);
                self.show_toast(format!("选中曲目：{label}"), window);
            }
            Msg::PlaylistOpen(index) => {
                let label = self.playlist_label(index);
                // The application owns "which track is playing"; the widget
                // only paints the `current` flag it is given.
                for (position, row) in self.playlist.iter_mut().enumerate() {
                    row.current = position == index;
                }
                self.push_playlist(window);
                self.show_toast(format!("播放：{label}"), window);
            }
            Msg::PlaylistReorder(from, to) => {
                if from < self.playlist.len() {
                    let track = self.playlist.remove(from);
                    let to = to.min(self.playlist.len());
                    self.playlist.insert(to, track);
                    self.push_playlist(window);
                    let label = self.playlist_label(to);
                    self.show_toast(
                        format!("已把「{label}」移到第 {} 位", to + 1),
                        window,
                    );
                }
            }
            Msg::FlyoutOpen => {
                if !self.flyout_open {
                    // Place the card from the trigger's current rectangle (the
                    // page may have scrolled since the last resize).
                    self.layout_chrome(window);
                    self.flyout_open = true;
                    for id in FLYOUT {
                        self.ui.set_visible(WidgetId::new(id), true, window);
                    }
                    window.a11y_refresh();
                }
            }
            Msg::FlyoutDismiss => self.close_flyout(window),
            Msg::RibbonCommand(command) => {
                let label = ribbon_command_label(command);
                self.ui.set_label(
                    WidgetId::new("ribbon_state"),
                    format!("最近命令：{label}"),
                );
                self.show_toast(format!("功能区：{label}"), window);
            }
            Msg::RibbonTab(index) => {
                let title = self
                    .ui
                    .with_widget::<Ribbon<Msg>, _>(WidgetId::new("gallery_ribbon"), |ribbon| {
                        ribbon
                            .tabs()
                            .get(index)
                            .map(|tab| tab.title().to_string())
                    })
                    .flatten()
                    .unwrap_or_default();
                self.show_toast(format!("功能区：{title} 选项卡"), window);
            }
            Msg::PropertySelect(key) => {
                self.ui
                    .set_label(WidgetId::new("props_state"), format!("选中：{key}"));
            }
            Msg::PropertyChange(key, value) => {
                // The grid already applied the value; the application persists
                // it and mirrors the change in its own state line.
                self.ui.set_label(
                    WidgetId::new("props_state"),
                    format!("修改 {key}：{value}"),
                );
                window.a11y_refresh();
            }
            Msg::PropertyInspect(key, value) => {
                self.show_toast(format!("查看属性 {key}：{value}"), window);
            }
            Msg::PropertyToggle => {
                let grid = WidgetId::new("gallery_props");
                let current = self
                    .ui
                    .with_widget::<PropertyGrid<Msg>, _>(grid, |grid| grid.checked("收藏"))
                    .flatten();
                if let Some(current) = current {
                    let changed = self.ui.edit_widget::<PropertyGrid<Msg>>(grid, |grid| {
                        grid.set_checked("收藏", !current)
                    });
                    if changed {
                        let value = if current { "否" } else { "是" };
                        self.ui.set_label(
                            WidgetId::new("props_state"),
                            format!("收藏：{value}"),
                        );
                        window.a11y_refresh();
                    }
                }
            }
            Msg::PropertyReset => {
                let changed = self.ui.edit_widget::<PropertyGrid<Msg>>(
                    WidgetId::new("gallery_props"),
                    |grid| grid.set_sections(gallery_property_sections()),
                );
                if changed {
                    self.ui
                        .set_label(WidgetId::new("props_state"), "已恢复默认属性");
                    window.a11y_refresh();
                }
            }
            Msg::PropertyLock => {
                // Per-row lock: the cursor row keeps its value and paint but
                // no editor opens until it is unlocked again.
                let grid = WidgetId::new("gallery_props");
                let path = self
                    .ui
                    .with_widget::<PropertyGrid<Msg>, _>(grid, |grid| grid.selected_path())
                    .flatten();
                let Some((section, row)) = path else {
                    return;
                };
                let info = self
                    .ui
                    .with_widget::<PropertyGrid<Msg>, _>(grid, |grid| {
                        let row = grid.section(section)?.rows_ref().get(row)?;
                        Some((row.key().to_string(), row.is_read_only()))
                    })
                    .flatten();
                let Some((key, locked)) = info else {
                    return;
                };
                let next = !locked;
                let changed = self.ui.edit_widget::<PropertyGrid<Msg>>(grid, |grid| {
                    grid.set_row_read_only(section, row, next)
                });
                if changed {
                    self.ui.set_label(
                        WidgetId::new("props_state"),
                        format!("{}：{key}", if next { "已锁定" } else { "已解锁" }),
                    );
                    window.a11y_refresh();
                }
            }
            Msg::PropertyReadOnly(locked) => {
                // The whole inspector through the component property: the
                // container pushes `read_only` into the widget, which locks
                // every editor while navigation still works.
                self.ui.set_props(
                    WidgetId::new("gallery_props"),
                    WidgetProps::new()
                        .visible(self.tab == 6)
                        .read_only(locked)
                        .automation_id("gallery-props")
                        .narrate(Narration::new().name("曲目属性")),
                    window,
                );
                self.ui.set_label(
                    WidgetId::new("props_state"),
                    if locked { "只读模式：开" } else { "只读模式：关" },
                );
                window.a11y_refresh();
            }
            Msg::Seek(position) => {
                // The widget already tracks the scrub position; only the
                // application-owned time label follows.
                self.ui
                    .set_label(WidgetId::new("covers_time"), cover_time_text(position));
            }
            Msg::Transport(action) => self.transport_action(action, window),
            Msg::GalleryTreeToggle(path, expanded) => {
                // The widget already rebuilt its visible rows; a structural
                // change needs a fresh accessibility snapshot.
                window.a11y_refresh();
                let label = self.tree_label(&path);
                self.show_toast(
                    format!("{}：{label}", if expanded { "展开" } else { "折叠" }),
                    window,
                );
            }
            Msg::GalleryTreeRename(_path, label) => {
                window.a11y_refresh();
                self.show_toast(format!("重命名：{label}"), window);
            }
            Msg::GalleryTreeMove(from, to) => {
                window.a11y_refresh();
                let label = self.tree_label(&to);
                self.show_toast(
                    format!("移动「{label}」：{} → {}", path_text(&from), path_text(&to)),
                    window,
                );
            }
            Msg::ScrollRow(index) => {
                self.show_toast(format!("滚动面板：按钮 {}", index / 2 + 1), window);
            }
            Msg::ScrollCheck(index, checked) => {
                self.show_toast(
                    format!(
                        "滚动面板：选项 {} {}",
                        index / 2 + 1,
                        if checked { "已选中" } else { "未选中" }
                    ),
                    window,
                );
            }
            Msg::DynAdd => {
                let Some(index) = (0..DYN_SLOT_IDS.len()).find(|slot| !self.dyn_slots.contains(slot))
                else {
                    self.show_toast("固定池只有两个槽位".to_string(), window);
                    return;
                };
                let id = WidgetId::new(DYN_SLOT_IDS[index]);
                self.ui.insert(
                    self.ui.len(),
                    Button::new(
                        DYN_SLOT_IDS[index],
                        format!("动态项 {}", index + 1),
                        Msg::DynItem(index),
                    ),
                );
                // The container positions the re-inserted slot from the page's
                // content rectangles (`set_scroll_view`), so re-register the
                // pages instead of moving the widget by hand.
                register_pages(&mut self.ui, &self.specs, window);
                self.ui.set_props(
                    id,
                    WidgetProps::new()
                        .visible(self.tab == 3)
                        .automation_id(format!("dyn-slot-{}", index + 1)),
                    window,
                );
                self.dyn_slots.push(index);
                // A newly inserted widget has no pixels yet; repaint the page.
                window.invalidate(self.scene.viewport);
                window.a11y_refresh();
                self.update_dyn_info();
                self.show_toast(format!("已插入动态项 {}", index + 1), window);
            }
            Msg::DynRemove => {
                let Some(index) = self.dyn_slots.pop() else {
                    self.show_toast("没有可移除的动态项".to_string(), window);
                    return;
                };
                self.ui
                    .remove(WidgetId::new(DYN_SLOT_IDS[index]), window);
                window.a11y_refresh();
                self.update_dyn_info();
                self.show_toast(format!("已移除动态项 {}", index + 1), window);
            }
            Msg::DynRaise | Msg::DynLower => {
                let Some(index) = self.dyn_slots.first().copied() else {
                    self.show_toast("没有动态项".to_string(), window);
                    return;
                };
                let id = WidgetId::new(DYN_SLOT_IDS[index]);
                let changed = if matches!(message, Msg::DynRaise) {
                    self.ui.raise(id)
                } else {
                    self.ui.lower(id)
                };
                if changed {
                    // The slots overlap, so their union is what changed pixels.
                    window.invalidate(self.scene.viewport);
                    self.update_dyn_info();
                    self.show_toast(
                        format!(
                            "动态项 {} 已{}",
                            index + 1,
                            if matches!(message, Msg::DynRaise) {
                                "置顶"
                            } else {
                                "置底"
                            }
                        ),
                        window,
                    );
                }
            }
            Msg::DynItem(index) => {
                self.show_toast(format!("动态项 {} 被点击", index + 1), window);
            }
            Msg::OpenModal => {
                for id in MODAL {
                    self.ui.set_visible(WidgetId::new(id), true, window);
                }
                self.modal_open = true;
                self.ui.set_focus_scope(
                    Some(vec![
                        WidgetId::new("modal_check"),
                        WidgetId::new("modal_cancel"),
                        WidgetId::new("modal_ok"),
                    ]),
                    window,
                );
                self.ui.set_focus(WidgetId::new("modal_ok"), window);
                window.a11y_focus(self.ui.focus_id());
                window.a11y_refresh();
                // Paint one complete page+scrim frame before the compositor
                // switches to overlay-only repaints: a page that was scrolled
                // while the modal was up (or only partially painted) would
                // otherwise show stale pixels through the scrim.
                window.resync_overlay();
                window.invalidate_all_force();
            }
            Msg::ModalCancel => self.close_modal(window, "已取消"),
            Msg::ModalConfirm => self.close_modal(window, "已确认"),
            Msg::ModalCheck(on) => self.show_toast(
                format!("不再询问：{}", if on { "开" } else { "关" }),
                window,
            ),
            // ---- Functional-API page ---------------------------------------
            Msg::ApiMessage => {
                window.show_dialog(
                    MessageDialog::new("这是一个模态消息对话框：关闭之前主窗口不可用。")
                        .title("Bamboo · 模态消息")
                        .buttons(MessageButtons::OkCancel),
                );
                self.set_action("已打开模态消息框", window);
            }
            Msg::ApiParallel => {
                window.show_dialog(
                    MessageDialog::new("并行对话框：主窗口保持可用，可以继续操作。")
                        .title("Bamboo · 并行")
                        .parallel(),
                );
                self.set_action("已打开并行消息框", window);
            }
            Msg::CustomDrawClicked => {
                let count = self.custom_clicks.get().wrapping_add(1);
                self.custom_clicks.set(count);
                self.ui.set_label(
                    WidgetId::new("api_custom_state"),
                    format!("自定义控件：已点击 {count} 次"),
                );
                // The closure paints app-owned state, so this change has no
                // setter to record it; mark the widget explicitly.
                self.ui.mark_dirty(WidgetId::new("api_custom_draw"));
            }
            Msg::ApiCustom => {
                window.show_dialog(
                    MessageDialog::new("任务对话框可以带应用自己的按钮。")
                        .title("Bamboo · 自定义按钮")
                        .instruction("保存本次更改？")
                        .button("保存(&S)", 501)
                        .button("放弃(&D)", 502),
                );
                self.set_action("已打开自定义按钮对话框", window);
            }
            Msg::ApiFileOpen => {
                window.show_dialog(
                    FileDialog::open()
                        .title("Bamboo · 打开音频")
                        .filter("音频文件", "*.mp3;*.flac;*.wav")
                        .multi_select(true),
                );
                self.set_action("已打开文件选择框（多选）", window);
            }
            Msg::ApiFileSave => {
                window.show_dialog(
                    FileDialog::save()
                        .title("Bamboo · 保存")
                        .filter("MP3", "*.mp3")
                        .initial_name("playlist.mp3"),
                );
                self.set_action("已打开保存文件框", window);
            }
            Msg::ApiFolder => {
                window.show_dialog(FileDialog::folder().title("Bamboo · 选择文件夹"));
                self.set_action("已打开文件夹选择框", window);
            }
            Msg::ApiFont => {
                window.show_dialog(FontDialog::new());
                self.set_action("已打开字体对话框", window);
            }
            Msg::ApiColor => {
                window.show_dialog(ColorDialog::new().initial(Color::rgb([127, 211, 168])));
                self.set_action("已打开颜色对话框", window);
            }
            Msg::ApiMenu => {
                let anchor = self
                    .ui
                    .rect(WidgetId::new("api_menu"))
                    .map(|rect| Point::new(rect.left, rect.bottom))
                    .unwrap_or(Point::new(40.0, 40.0));
                let menu = Menu::new()
                    .shortcut_item(760, "从代码打开", "Enter")
                    .checked_item(761, "示例开关", true)
                    .separator()
                    .submenu("更多", Menu::new().item(762, "子项 A").item(763, "子项 B"));
                window.open_menu(WidgetId::new("api_menu"), menu, anchor, None);
                self.set_action("已从代码打开菜单", window);
            }
            Msg::ApiContext => {
                if self
                    .ui
                    .open_menu_for(WidgetId::new("api_context"), window)
                {
                    self.set_action("已打开上下文菜单", window);
                } else {
                    self.set_action("上下文菜单需要右键触发", window);
                }
            }
            Msg::ApiTrayAdd => {
                window.set_tray_icon(Some(
                    TrayIcon::new("Bamboo API 托盘图标").menu(tray_menu()),
                ));
                let installed = window.tray_icon_visible();
                self.set_action(
                    if installed {
                        "已安装托盘图标"
                    } else {
                        "托盘图标安装失败"
                    },
                    window,
                );
            }
            Msg::ApiTrayBalloon => {
                let shown = window.show_tray_balloon(
                    "Bamboo · API",
                    "这是一条托盘通知气泡。",
                    MessageIcon::Info,
                );
                self.set_action(
                    if shown {
                        "已发送托盘气泡通知"
                    } else {
                        "托盘图标未安装"
                    },
                    window,
                );
            }
            Msg::ApiSystemNotify => {
                // An unpackaged app needs a Start-menu shortcut carrying its
                // AppUserModelID before the shell shows anything; write/refresh
                // it on demand, then post the toast.
                let registered =
                    window.register_notification_shortcut("Bamboo Native UI — Showcase");
                let result = window.notify(
                    &Notification::new(
                        "Bamboo Native UI",
                        "系统通知：拖放、布局与控件都可以从这里开始。",
                    )
                    .tag("showcase")
                    .group("api"),
                );
                let text = match result {
                    NotificationResult::Toast if registered => {
                        "已发送 Windows 系统通知（Toast，已注册 AppUserModelID）"
                    }
                    NotificationResult::Toast => "已发送 Windows 系统通知（Toast）",
                    NotificationResult::Balloon => {
                        "Toast 不可用（未注册 AppUserModelID），已改用托盘气泡"
                    }
                    NotificationResult::Failed => {
                        "系统通知失败：请先安装托盘图标，或为应用注册 AppUserModelID"
                    }
                };
                self.set_action(text, window);
                self.show_toast(text.to_string(), window);
            }
            Msg::ApiTrayRemove => {
                let removed = window.tray_icon_visible();
                window.set_tray_icon(None);
                self.set_action(
                    if removed {
                        "已移除托盘图标"
                    } else {
                        "托盘图标不存在"
                    },
                    window,
                );
            }
            Msg::ApiHotkeyAdd => {
                let hotkey = demo_hotkey();
                let ok = window.register_hotkey(hotkey);
                self.set_action(
                    if ok {
                        "已注册全局热键 Ctrl+Alt+F8"
                    } else {
                        "热键注册失败（可能被占用）"
                    },
                    window,
                );
            }
            Msg::ApiHotkeyCapture => {
                window.begin_hotkey_capture();
                self.set_action("正在捕获热键：按任意组合键…", window);
            }
            Msg::ApiHotkeyRemove => {
                let ok = window.unregister_hotkey(demo_hotkey());
                self.set_action(
                    if ok {
                        "已注销热键 Ctrl+Alt+F8"
                    } else {
                        "热键没有注册"
                    },
                    window,
                );
            }
            Msg::ApiMinimize => {
                self.set_action("最小化窗口", window);
                window.minimize();
            }
            Msg::ApiMaximize => {
                self.set_action("最大化窗口", window);
                window.maximize();
            }
            Msg::ApiRestore => {
                self.set_action("还原窗口", window);
                window.restore();
            }
            Msg::ApiHide => {
                self.set_action("已隐藏窗口：点击托盘图标恢复", window);
                window.hide();
            }
            Msg::ApiClose => {
                self.set_action("请求关闭窗口", window);
                window.request_close();
            }
            Msg::ApiFlush => {
                self.set_action("已强制重绘并 flush", window);
                window.invalidate_all_force();
                window.flush();
            }
            Msg::ApiTitle => {
                self.counter += 1;
                let title = format!("Bamboo Native UI — Showcase ({})", self.counter);
                window.set_title(&title);
                self.set_action(format!("标题改为：{title}"), window);
            }
            Msg::ApiAnnounce => {
                let text = format!("无障碍通知 #{}", self.counter + 1);
                window.a11y_announce(text.clone());
                self.set_action(format!("已发布读屏通知：{text}"), window);
            }
            Msg::ApiCopy => {
                let copied = window.copy_text("Bamboo Native UI · 功能 API");
                self.set_action(
                    if copied {
                        "已把文本写入剪贴板"
                    } else {
                        "剪贴板被其他程序占用"
                    },
                    window,
                );
            }
            Msg::ApiRefresh => {
                window.a11y_refresh();
                self.set_action("已重建无障碍树", window);
            }
            Msg::ApiTheme => self.apply(Msg::Theme, window),
            // A press on a drag card only starts the gesture; releasing without
            // crossing the threshold is not a click action.
            Msg::DragCard(card) => {
                let _ = card;
            }
            Msg::DropZone => {}
            #[cfg(feature = "m7")]
            Msg::M7Action(text) => self.show_toast(format!("布局页：{text}"), window),
            #[cfg(feature = "m7")]
            Msg::M7Edit(text) => self.ui.set_label(
                WidgetId::new("m7_status"),
                format!("布局页提交：{} 个字符", text.chars().count()),
            ),
            #[cfg(feature = "m7")]
            Msg::M7Value(value) => {
                self.ui
                    .set_label(WidgetId::new("m7_value"), format!("{:.0}%", value * 100.0));
            }
            #[cfg(feature = "m7")]
            Msg::M7Select(index) => {
                self.show_toast(format!("布局列表选择：第 {} 项", index + 1), window);
            }
            Msg::Dropped(source, target) => {
                let text = format!(
                    "拖放：卡片 {} 放到了放置区 {}",
                    if source == 0 { "A" } else { "B" },
                    if target == 0 { "A" } else { "B" }
                );
                self.ui
                    .set_label(WidgetId::new("api_drag_state"), text.clone());
                self.show_toast(text, window);
            }
            Msg::PadEnable(on) => {
                self.gamepad_enabled = on;
                if let Some(pad) = self.gamepad.as_mut() {
                    pad.set_enabled(on);
                }
                self.set_action(
                    if on {
                        "手柄当作键盘：开"
                    } else {
                        "手柄当作键盘：关"
                    },
                    window,
                );
            }
        }
        // Code-driven content changes collect locally; one union rect repaints
        // them (the widgets' change callbacks never ran for these sets).
        self.ui.flush_damage(window);
    }

    /// Apply a transport button press: the application owns playback state,
    /// so it flips its own flags and pushes them back into the strip (the
    /// widget itself never guesses whether the player obeyed).
    fn transport_action(&mut self, action: TransportAction, window: &mut WindowHandle<'_>) {
        match action {
            TransportAction::Shuffle => self.transport_shuffle = !self.transport_shuffle,
            TransportAction::PlayPause => self.playing = !self.playing,
            TransportAction::Repeat => self.transport_repeat = self.transport_repeat.next(),
            TransportAction::Previous | TransportAction::Next => {}
        }
        let (playing, shuffle, repeat) = (self.playing, self.transport_shuffle, self.transport_repeat);
        self.ui.edit_widget::<PlaybackTransport<Msg>>(
            WidgetId::new("covers_transport"),
            |transport| {
                let playing = transport.set_playing(playing);
                let shuffle = transport.set_shuffle(shuffle);
                let repeat = transport.set_repeat(repeat);
                playing || shuffle || repeat
            },
        );
        self.ui.set_label(
            WidgetId::new("covers_state"),
            transport_state_text(playing, shuffle, repeat),
        );
        // The play/pause name and the toggle states changed.
        window.a11y_refresh();
        let text = match action {
            TransportAction::Shuffle => {
                format!("随机播放：{}", if shuffle { "开" } else { "关" })
            }
            TransportAction::PlayPause => {
                (if playing { "播放" } else { "暂停" }).to_string()
            }
            TransportAction::Repeat => format!("循环播放：{}", repeat.label()),
            TransportAction::Previous => "上一曲".to_string(),
            TransportAction::Next => "下一曲".to_string(),
        };
        self.show_toast(text, window);
    }

    fn show_toast(&mut self, text: String, window: &mut WindowHandle<'_>) {
        self.set_action(text.clone(), window);
        let announcement = text.clone();
        self.ui.set_message(WidgetId::new("toast"), text, false);
        // The toast owns its lifetime; the application just runs the timer.
        let duration = self
            .ui
            .with_widget::<Toast, _>(WidgetId::new("toast"), |toast| toast.duration_value())
            .flatten()
            .unwrap_or(Duration::from_millis(2200));
        self.toast_until = Some(Instant::now() + duration);
        // The message setter recorded the toast's rectangle; hand the batch to
        // the window now (a background callback such as a tray command may not
        // end in an event flush).
        self.ui.flush_damage(window);
        // A toast is transient status text: push it as a UIA notification so a
        // screen reader announces it without moving focus.
        window.a11y_announce(announcement);
        window.set_animation_timer(Some(Duration::from_millis(120)));
    }

    /// Record the last application action in the bottom strip.
    fn set_action(&mut self, text: impl Into<String>, window: &mut WindowHandle<'_>) {
        self.last_action = text.into();
        window.invalidate(self.scene.status);
    }

    /// Drive the native edit session in `--self-test`: focus the field, press
    /// Enter, verify the editor opened, then Tab to commit.
    fn probe_tick(&mut self, window: &mut WindowHandle<'_>) {
        let elapsed = self.start.elapsed();
        let step = self.probe.as_ref().map(|probe| probe.step).unwrap_or(0);
        if step == 0 && elapsed >= Duration::from_millis(200) {
            // The showcase installs no icon on startup, so the probe owns the
            // whole tray lifecycle: install, balloon, second id, remove.
            window.set_tray_icon(Some(
                TrayIcon::new("Bamboo 自检").menu(tray_menu()),
            ));
            let balloon = window.show_tray_balloon(
                "Bamboo 自检",
                "托盘通知已发送。",
                MessageIcon::Info,
            );
            let visible = window.tray_icon_visible();
            // A second icon with its own id: accepted, then removed.
            let second = window.set_tray_icon_at(2, Some(TrayIcon::new("Bamboo 自检 2")));
            let removed = window.set_tray_icon_at(2, None);
            // Leave the notification area as we found it.
            window.set_tray_icon(None);
            if let Some(probe) = self.probe.as_mut() {
                probe.step = 1;
                probe.tray = visible;
                probe.tray_balloon = balloon;
                probe.tray_multi = second && !removed;
                probe.menu_bar = window.menu_bar_installed();
            }
            self.ui.set_focus(WidgetId::new("search"), window);
            self.send_key(KeyCode::Enter, window);
        } else if step == 1 && elapsed >= Duration::from_millis(520) {
            let opened = window.edit_active();
            if let Some(probe) = self.probe.as_mut() {
                probe.opened = opened;
                probe.step = 2;
            }
            if opened {
                self.send_key(KeyCode::Tab, window);
            }
        } else if step == 2 && elapsed >= Duration::from_millis(800) {
            // Focus a widget with a tooltip and force the ring on: the host
            // arms the 450 ms keyboard dwell, just like Tab navigation.
            if let Some(probe) = self.probe.as_mut() {
                probe.step = 3;
            }
            self.ui
                .set_focus(WidgetId::new("primary"), window);
            self.ui.show_focus_ring(true, window);
        } else if step == 3 && window.tooltip_visible() {
            // Latch the observation: the popup can be dismissed by a focus
            // change on a shared desktop, so one visible tick is enough.
            if let Some(probe) = self.probe.as_mut() {
                probe.tooltip = true;
                probe.step = 4;
            }
        } else if step == 4 {
            // Right-click the focused button: the tooltip hides and `Ui` asks
            // the host for the button's menu, which opens from a posted
            // message (never inside `Host::event`).
            if let Some(rect) = self.ui.rect(WidgetId::new("primary")) {
                let pos = Point::new(rect.left + 12.0, rect.top + 12.0);
                let button = MouseButton::Right;
                let modifiers = bamboo_native_ui::PointerModifiers::NONE;
                self.event(
                    &Event::PointerDown {
                        pos,
                        button,
                        modifiers,
                    },
                    window,
                );
                self.event(
                    &Event::PointerUp {
                        pos,
                        button,
                        modifiers,
                    },
                    window,
                );
            }
            if let Some(probe) = self.probe.as_mut() {
                probe.step = 5;
            }
        } else if step == 5 {
            // The timer runs inside the popup's modal loop, so `menu_active`
            // is observable exactly while the native menu is up. The first
            // tick edits the live model: the host must close and rebuild the
            // popup, which fires another `Opened` carrying the new item.
            if window.menu_active() {
                if !self.probe.as_ref().is_some_and(|probe| probe.menu_live_edit) {
                    let changed = window.edit_open_menu(|menu| {
                        menu.insert(0, MenuItem::new(PROBE_LIVE_COMMAND, "自检：动态项"));
                    });
                    if let Some(probe) = self.probe.as_mut() {
                        probe.menu_live_edit = changed;
                    }
                } else if self
                    .probe
                    .as_ref()
                    .is_some_and(|probe| probe.menu_live_content)
                {
                    if let Some(probe) = self.probe.as_mut() {
                        probe.menu = true;
                        probe.step = 6;
                    }
                    window.dismiss_context_menu();
                }
            } else if elapsed >= Duration::from_millis(2200) {
                // The popup never came up: fail instead of hanging in a loop.
                self.probe_timeout(window, step);
            }
        } else if step == 6 && !window.menu_active() {
            if let Some(probe) = self.probe.as_mut() {
                probe.step = 7;
            }
        } else if step == 7 {
            let id = window.show_dialog(
                MessageDialog::new("自检：关闭之前主窗口不可用。")
                    .title(MODAL_DIALOG_TITLE)
                    .buttons(MessageButtons::OkCancel),
            );
            if let Some(probe) = self.probe.as_mut() {
                probe.modal_id = Some(id);
                probe.step = 8;
            }
        } else if step == 8 {
            if let Some(dialog) = find_dialog_window(MODAL_DIALOG_TITLE) {
                // A modal dialog must have disabled the owner.
                if !window_enabled(window.hwnd()) {
                    if let Some(probe) = self.probe.as_mut() {
                        probe.modal_blocks = true;
                        probe.step = 9;
                    }
                    close_window(dialog);
                }
            } else if elapsed >= Duration::from_millis(3200) {
                self.probe_timeout(window, step);
            }
        } else if step == 9 {
            if self.probe.as_ref().is_some_and(|probe| probe.modal_result) {
                let id = window.show_dialog(
                    MessageDialog::new("自检：主窗口仍然可用。")
                        .title(PARALLEL_DIALOG_TITLE)
                        .parallel(),
                );
                if let Some(probe) = self.probe.as_mut() {
                    probe.parallel_id = Some(id);
                    probe.step = 10;
                }
            } else if elapsed >= Duration::from_millis(4200) {
                self.probe_timeout(window, step);
            }
        } else if step == 10 {
            if let Some(dialog) = find_dialog_window(PARALLEL_DIALOG_TITLE) {
                // A parallel dialog must leave the owner enabled.
                if let Some(probe) = self.probe.as_mut() {
                    probe.parallel_free = window_enabled(window.hwnd());
                    probe.step = 11;
                }
                close_window(dialog);
            } else if elapsed >= Duration::from_millis(5200) {
                self.probe_timeout(window, step);
            }
        } else if step == 11
            && self
                .probe
                .as_ref()
                .is_some_and(|probe| probe.parallel_result)
            && let Some(probe) = self.probe.as_mut()
        {
            probe.step = 111;
        } else if step == 111 {
            // A task dialog with application buttons: clicking 保存 must come
            // back as `MessageAnswer::Custom(501)`.
            let id = window.show_dialog(
                MessageDialog::new("自检：自定义按钮走任务对话框。")
                    .title(CUSTOM_DIALOG_TITLE)
                    .instruction("保存本次更改？")
                    .button("保存(&S)", 501)
                    .button("放弃(&D)", 502),
            );
            if let Some(probe) = self.probe.as_mut() {
                probe.custom_dialog_id = Some(id);
                probe.step = 112;
            }
        } else if step == 112 {
            if let Some(dialog) = find_dialog_window(CUSTOM_DIALOG_TITLE) {
                // The documented way to press a task-dialog button by id.
                unsafe {
                    SendMessageW(dialog, TDM_CLICK_BUTTON.0 as u32, Some(WPARAM(501)), Some(LPARAM(0)));
                }
                if let Some(probe) = self.probe.as_mut() {
                    probe.step = 113;
                }
            } else if elapsed >= Duration::from_millis(3200) {
                self.probe_timeout(window, step);
            }
        } else if step == 113 {
            if self.probe.as_ref().is_some_and(|probe| probe.custom_buttons) {
                if let Some(probe) = self.probe.as_mut() {
                    probe.step = 12;
                }
            } else if elapsed >= Duration::from_millis(3200) {
                // Close a dialog that did not report, then continue (ok() stays
                // false and names the miss).
                if let Some(dialog) = find_dialog_window(CUSTOM_DIALOG_TITLE) {
                    close_window(dialog);
                }
                if let Some(probe) = self.probe.as_mut() {
                    probe.custom_dialog_id = None;
                    probe.step = 12;
                }
            }
        } else if step == 12 {
            // Open the next picker (file, font, color) in parallel mode, so the
            // window stays usable; cancel it as soon as it appears.
            let left = self.probe.as_ref().map(|probe| probe.picker_left).unwrap_or(0);
            if left == 0 {
                if let Some(probe) = self.probe.as_mut() {
                    probe.step = 15;
                }
            } else {
                let kind = match left {
                    3 => PickerKind::File,
                    2 => PickerKind::Font,
                    _ => PickerKind::Color,
                };
                let id = match kind {
                    PickerKind::File => window.show_dialog(
                        FileDialog::open()
                            .title(FILE_DIALOG_TITLE)
                            .filter("音频文件", "*.mp3;*.flac")
                            .parallel(),
                    ),
                    PickerKind::Font => window.show_dialog(FontDialog::new().parallel()),
                    PickerKind::Color => window.show_dialog(
                        ColorDialog::new()
                            .parallel()
                            .initial(Color::rgb([127, 211, 168])),
                    ),
                };
                if let Some(probe) = self.probe.as_mut() {
                    probe.picker_left -= 1;
                    probe.picker = Some(kind);
                    probe.picker_id = Some(id);
                    probe.step = 13;
                }
            }
        } else if step == 13 {
            let kind = self.probe.as_ref().and_then(|probe| probe.picker);
            let found = match kind {
                Some(PickerKind::File) => find_dialog_window(FILE_DIALOG_TITLE),
                // The font and color choosers have system captions, so match
                // the dialog class instead.
                Some(_) => find_any_dialog(),
                None => None,
            }
            .or_else(|| {
                // Any process dialog is still the picker (or its own error
                // box); closing it keeps the self-test moving.
                find_any_dialog()
            });
            if let Some(dialog) = found {
                if let Some(probe) = self.probe.as_mut() {
                    probe.picker_seen += 1;
                    probe.step = 14;
                }
                close_window(dialog);
            } else if elapsed >= Duration::from_millis(7000) {
                self.probe_timeout(window, step);
            }
        } else if step == 14 {
            if self.probe.as_ref().is_some_and(|probe| probe.picker.is_none()) {
                if let Some(probe) = self.probe.as_mut() {
                    probe.step = 12;
                }
            } else if elapsed >= Duration::from_millis(18000) {
                self.probe_timeout(window, step);
            }
        } else if step == 15 {
            let hotkey = Hotkey::parse("Ctrl+Alt+Shift+F12").expect("demo hotkey");
            let registered = window.register_hotkey(hotkey);
            window.begin_hotkey_capture();
            // Capture consumes keys in the window procedure (before `Host`), so
            // post a real key message instead of calling `event` directly.
            unsafe {
                let _ = PostMessageW(
                    Some(HWND(window.hwnd() as *mut c_void)),
                    WM_KEYDOWN,
                    WPARAM(SELF_TEST_HOTKEY.vk as usize),
                    LPARAM(0),
                );
            }
            if let Some(probe) = self.probe.as_mut() {
                probe.hotkey_registered = registered;
                probe.step = 16;
            }
        } else if step == 16 {
            if self
                .probe
                .as_ref()
                .is_some_and(|probe| probe.hotkey_captured.is_some())
            {
                if let Some(probe) = self.probe.as_mut() {
                    probe.step = 17;
                }
            } else if elapsed >= Duration::from_millis(19000) {
                self.probe_timeout(window, step);
            }
        } else if step == 17 {
            // Open a menu from code for the gallery's dynamic button. The
            // popup is shown from the message loop, so the next tick (which
            // runs inside its modal loop) observes it.
            let widget = WidgetId::new(DYN_MENU_ID);
            let menu = Menu::new()
                .item(PROBE_MENU_COMMAND, "自检命令")
                .checked_item(PROBE_MENU_COMMAND + 1, "选项", true);
            window.open_menu(widget, menu, Point::new(40.0, 40.0), None);
            if let Some(probe) = self.probe.as_mut() {
                probe.step = 18;
            }
        } else if step == 18 {
            if window.menu_active() {
                if let Some(probe) = self.probe.as_mut() {
                    probe.menu_controller_widget = window.active_menu_widget();
                    probe.step = 19;
                }
                window.dismiss_menu();
            } else if elapsed >= Duration::from_millis(24000) {
                self.probe_timeout(window, step);
            }
        } else if step == 19 {
            if !window.menu_active() {
                // Replay a selection without a popup: the hook sees `Selected`
                // and the command takes the normal `menu_command` path.
                window.menu_click(WidgetId::new(DYN_MENU_ID), PROBE_MENU_COMMAND);
                if let Some(probe) = self.probe.as_mut() {
                    probe.step = 20;
                }
            } else if elapsed >= Duration::from_millis(25000) {
                self.probe_timeout(window, step);
            }
        } else if step == 20 {
            if self
                .probe
                .as_ref()
                .is_some_and(|probe| probe.menu_command_command == Some(PROBE_MENU_COMMAND))
            {
                if let Some(probe) = self.probe.as_mut() {
                    probe.step = 21;
                }
            } else if elapsed >= Duration::from_millis(26000) {
                self.probe_timeout(window, step);
            }
        } else if step == 21 {
            // The accelerator below needs the window to be the foreground
            // window (TranslateAccelerator only runs for it), so restore and
            // focus first; this is also what a real menu selection implies.
            window.show();
            // The native window menu bar cannot be clicked from a test, so
            // post the selection message the bar sends when the user picks
            // "关于". The window procedure routes it through the menu hook and
            // `Host::menu_bar_command`.
            unsafe {
                let _ = PostMessageW(
                    Some(HWND(window.hwnd() as *mut c_void)),
                    WM_COMMAND,
                    WPARAM(PROBE_MENU_BAR_COMMAND as usize),
                    LPARAM(0),
                );
                // The accelerator column is live: F5 is one of the bar's
                // shortcuts and has no modifier, so a plain WM_KEYDOWN
                // reproduces it without faking modifier state.
                let _ = PostMessageW(
                    Some(HWND(window.hwnd() as *mut c_void)),
                    WM_KEYDOWN,
                    WPARAM(0x74),
                    LPARAM(0),
                );
            }
            if let Some(probe) = self.probe.as_mut() {
                probe.step = 22;
            }
        } else if step == 22 {
            if self.probe.as_ref().is_some_and(|probe| {
                probe.menu_bar_command_command == Some(PROBE_MENU_BAR_COMMAND)
                    && probe.menu_hotkey
            }) {
                if let Some(probe) = self.probe.as_mut() {
                    probe.step = 23;
                }
            } else if elapsed >= Duration::from_millis(28000) {
                self.probe_timeout(window, step);
            }
        } else if step == 23 {
            // The scroll view lives on the data page, which is hidden on the
            // other tabs; switch there through the same message the tab emits.
            self.apply(Msg::Tab(3), window);
            if let Some(probe) = self.probe.as_mut() {
                probe.step = 24;
            }
        } else if step == 24 {
            // The table edits a cell in place: Enter enters adjust mode, the
            // second Enter opens the cell draft, a character replaces the
            // selected text and the third Enter commits it to the application.
            let table = WidgetId::new("gallery_table");
            self.ui
                .edit_widget::<Table<Msg>>(table, |table| table.select_cell(0, 1));
            self.ui.set_focus(table, window);
            self.send_key(KeyCode::Enter, window);
            self.send_key(KeyCode::Enter, window);
            self.event(&Event::Char('甲'), window);
            self.send_key(KeyCode::Enter, window);
            let edited = self
                .ui
                .with_widget::<Table<Msg>, _>(table, |table| {
                    table
                        .cell_text(0, 1)
                        .map(str::to_string)
                        .unwrap_or_default()
                })
                .as_deref()
                == Some("甲")
                && self
                    .table_rows
                    .first()
                    .map(|row| row.cell(1))
                    == Some("甲");
            // A cell whose value contains a newline uses the painted multiline
            // model: a plain Enter inserts a line and keeps the session open,
            // Escape cancels, and the application's data is untouched.
            self.ui
                .edit_widget::<Table<Msg>>(table, |table| table.edit_cell(2, 1));
            let multiline = self
                .ui
                .with_widget::<Table<Msg>, _>(table, |table| table.multiline_edit())
                .unwrap_or(false);
            let multiline_before = self
                .table_rows
                .get(2)
                .map(|row| row.cell(1).to_string())
                .unwrap_or_default();
            self.send_key(KeyCode::Enter, window);
            self.send_key(KeyCode::Escape, window);
            let table_multiline = self
                .ui
                .with_widget::<Table<Msg>, _>(table, |table| {
                    (table.cell_text(2, 1).map(str::to_string), table.is_editing_cell())
                })
                .is_some_and(|(text, editing)| {
                    !editing && text.as_deref() == Some(multiline_before.as_str())
                })
                && multiline;
            // The tree renames the cursor row through F2 (the container's
            // painted session) and then moves a row by pointer drag.
            let tree = WidgetId::new("gallery_tree");
            self.ui
                .edit_widget::<TreeView<Msg>>(tree, |tree| tree.set_selection_path(&[0, 0, 0]));
            self.ui.set_focus(tree, window);
            self.send_key(KeyCode::Enter, window);
            self.send_key(KeyCode::Function(2), window);
            self.event(&Event::Char('新'), window);
            self.send_key(KeyCode::Enter, window);
            let renamed = self
                .ui
                .with_widget::<TreeView<Msg>, _>(tree, |tree| {
                    tree.item(&[0, 0, 0]).map(|item| item.label.clone())
                })
                .flatten()
                .as_deref()
                == Some("新");
            let points = self
                .ui
                .with_widget::<TreeView<Msg>, _>(tree, |tree| {
                    Some((tree.row_rect(5).center(), tree.row_rect(6).center()))
                })
                .flatten();
            if let Some((from, to)) = points {
                self.event(
                    &Event::PointerDown {
                        pos: from,
                        button: MouseButton::Left,
                        modifiers: PointerModifiers::NONE,
                    },
                    window,
                );
                self.event(
                    &Event::PointerMove {
                        pos: Point::new(from.x, from.y + 10.0),
                        buttons: 1,
                    },
                    window,
                );
                self.event(
                    &Event::PointerMove {
                        pos: to,
                        buttons: 1,
                    },
                    window,
                );
                self.event(
                    &Event::PointerUp {
                        pos: to,
                        button: MouseButton::Left,
                        modifiers: PointerModifiers::NONE,
                    },
                    window,
                );
            }
            let moved = self
                .ui
                .with_widget::<TreeView<Msg>, _>(tree, |tree| {
                    let moved = tree
                        .item(&[0, 1, 3])
                        .map(|item| item.label.as_str())
                        == Some("远山");
                    let shifted = tree
                        .item(&[0, 0, 2])
                        .map(|item| item.label.as_str())
                        == Some("晨雾");
                    moved && shifted
                })
                .unwrap_or(false);
            // Keyboard focus on a row below the fold must scroll it into view.
            let panel = WidgetId::new(SCROLL_PANEL);
            let row = WidgetId::new(SCROLL_ROWS[SCROLL_ROWS.len() - 1]);
            self.ui.set_focus(row, window);
            let offset = self.ui.scroll_offset(panel).unwrap_or(Point::new(0.0, 0.0));
            let inside = match (self.ui.rect(row), self.ui.scroll_view(panel)) {
                (Some(rect), Some(view)) => {
                    let viewport = view.viewport();
                    rect.top >= viewport.top && rect.bottom <= viewport.bottom
                }
                _ => false,
            };
            if let Some(probe) = self.probe.as_mut() {
                probe.table_edit = edited;
                probe.table_multiline = table_multiline;
                probe.tree_edit = renamed && moved;
                probe.scroll_reveal = offset.y > 0.0 && inside;
                probe.step = 25;
            }
        } else if step == 25 {
            // The wheel over a row button scrolls the panel (the button does
            // not own the wheel); enough notches return it to the top.
            if let Some(view) = self.ui.scroll_view(WidgetId::new(SCROLL_PANEL)) {
                let pos = view.viewport().center();
                self.event(
                    &Event::Wheel {
                        pos,
                        delta_x: 0.0,
                        delta_y: 8.0,
                    },
                    window,
                );
            }
            let offset = self
                .ui
                .scroll_offset(WidgetId::new(SCROLL_PANEL))
                .unwrap_or(Point::new(-1.0, -1.0));
            // The nested viewport scrolls first; at its end the next notch
            // moves the page it sits in.
            let nested = WidgetId::new(NEST_PANEL);
            let outer = WidgetId::new(SCROLL_PANEL);
            let mut nested_ok = false;
            // The reveal step left a horizontal offset; reset both axes so the
            // nested viewport sits inside the page viewport again.
            self.ui
                .set_scroll_offset(outer, Point::new(0.0, 0.0), window);
            if let Some(view) = self.ui.scroll_view(nested) {
                let pos = view.clip_rect().center();
                self.event(
                    &Event::Wheel {
                        pos,
                        delta_x: 0.0,
                        delta_y: -8.0,
                    },
                    window,
                );
                let inner_after_one = self.ui.scroll_offset(nested).unwrap_or(Point::new(0.0, 0.0)).y;
                let outer_before = self.ui.scroll_offset(outer).unwrap_or(Point::new(0.0, 0.0)).y;
                for _ in 0..20 {
                    self.event(
                        &Event::Wheel {
                            pos,
                            delta_x: 0.0,
                            delta_y: -8.0,
                        },
                        window,
                    );
                }
                let inner_end = self.ui.scroll_offset(nested).unwrap_or(Point::new(0.0, 0.0)).y;
                let inner_max = self.ui.scroll_view(nested).map(|view| view.max_offset().y).unwrap_or(0.0);
                self.event(
                    &Event::Wheel {
                        pos,
                        delta_x: 0.0,
                        delta_y: -8.0,
                    },
                    window,
                );
                let outer_after = self.ui.scroll_offset(outer).unwrap_or(Point::new(0.0, 0.0)).y;
                nested_ok = inner_after_one > 0.0
                    && (inner_end - inner_max).abs() < 0.5
                    && outer_after > outer_before;
            }
            if let Some(probe) = self.probe.as_mut() {
                // The reveal also moved the horizontal axis (a wide row), so
                // the wheel only has to reach the top of the vertical range.
                probe.scroll_wheel = offset.y == 0.0;
                probe.nested_scroll = nested_ok;
                probe.step = 26;
            }
        } else if step == 26 {
            // The cover wall lives on the fifth tab; switch there through the
            // same message the tab emits.
            self.apply(Msg::Tab(4), window);
            if let Some(probe) = self.probe.as_mut() {
                probe.step = 27;
            }
        } else if step == 27 {
            // Enter puts the wall in keyboard-adjust mode; End walks the cursor
            // to the last cover and must scroll it into view, Home returns to
            // the first cover and the top of the range.
            let wall = WidgetId::new("covers_wall");
            self.ui.set_focus(wall, window);
            self.send_key(KeyCode::Enter, window);
            self.send_key(KeyCode::End, window);
            let end = self.wall_state();
            self.send_key(KeyCode::Home, window);
            let home = self.wall_state();
            if let Some(probe) = self.probe.as_mut() {
                let ok = match (end, home) {
                    (Some((last, count, end_scroll)), Some((first, _, home_scroll))) => {
                        count > 0
                            && last == count - 1
                            && end_scroll > 0.0
                            && first == 0
                            && home_scroll == 0.0
                    }
                    _ => false,
                };
                probe.covers = ok;
                probe.step = 28;
            }
        } else if step == 28 {
            // Click the waveform seek bar at 60 %: the widget's position and
            // the application's time label must both follow.
            let waveform = WidgetId::new("covers_waveform");
            let rect = self.ui.rect(waveform).unwrap_or(Rect::from_xywh(0.0, 0.0, 1.0, 1.0));
            let pos = Point::new(rect.left + rect.width() * 0.6, rect.center().y);
            self.event(
                &Event::PointerDown {
                    pos,
                    button: MouseButton::Left,
                    modifiers: PointerModifiers::NONE,
                },
                window,
            );
            self.event(
                &Event::PointerUp {
                    pos,
                    button: MouseButton::Left,
                    modifiers: PointerModifiers::NONE,
                },
                window,
            );
            let progress = self
                .ui
                .with_widget::<WaveformSeekBar<Msg>, _>(waveform, |bar| bar.progress());
            let label = self
                .ui
                .with_widget::<Label, _>(WidgetId::new("covers_time"), |label| {
                    label.text().to_string()
                });
            if let Some(probe) = self.probe.as_mut() {
                probe.waveform = progress.is_some_and(|value| (value - 0.6).abs() < 0.02)
                    && label.as_deref() == Some(cover_time_text(0.6).as_str());
                probe.step = 29;
            }
        } else if step == 29 {
            // Click the transport's play cell: the application flips its own
            // playback flag and the strip and the state label must follow.
            let transport = WidgetId::new("covers_transport");
            let rect = self
                .ui
                .rect(transport)
                .unwrap_or(Rect::from_xywh(0.0, 0.0, 1.0, 1.0));
            let pos = rect.center();
            self.event(
                &Event::PointerDown {
                    pos,
                    button: MouseButton::Left,
                    modifiers: PointerModifiers::NONE,
                },
                window,
            );
            self.event(
                &Event::PointerUp {
                    pos,
                    button: MouseButton::Left,
                    modifiers: PointerModifiers::NONE,
                },
                window,
            );
            let playing = self
                .ui
                .with_widget::<PlaybackTransport<Msg>, _>(transport, |strip| {
                    strip.is_playing()
                });
            let state = self
                .ui
                .with_widget::<Label, _>(WidgetId::new("covers_state"), |label| {
                    label.text().to_string()
                });
            if let Some(probe) = self.probe.as_mut() {
                probe.transport = playing == Some(true)
                    && state.as_deref()
                        == Some(transport_state_text(true, false, RepeatMode::Off).as_str());
                probe.step = 30;
            }
        } else if step == 30 {
            // Drag the first playlist track below the third: the widget
            // reports the move, the application reorders its own data and
            // pushes the new order back into the list.
            let list = WidgetId::new("covers_playlist");
            let rect = self
                .ui
                .rect(list)
                .unwrap_or(Rect::from_xywh(0.0, 0.0, 1.0, 1.0));
            let before = self
                .playlist
                .first()
                .map(|row| row.title.clone())
                .unwrap_or_default();
            let x = rect.left + 48.0;
            let start = Point::new(x, rect.top + 20.0);
            let end = Point::new(x, rect.top + 3.0 * 40.0 + 10.0);
            self.event(
                &Event::PointerDown {
                    pos: start,
                    button: MouseButton::Left,
                    modifiers: PointerModifiers::NONE,
                },
                window,
            );
            self.event(
                &Event::PointerMove {
                    pos: Point::new(x, start.y + 10.0),
                    buttons: 1,
                },
                window,
            );
            self.event(
                &Event::PointerMove {
                    pos: end,
                    buttons: 1,
                },
                window,
            );
            self.event(
                &Event::PointerUp {
                    pos: end,
                    button: MouseButton::Left,
                    modifiers: PointerModifiers::NONE,
                },
                window,
            );
            let after = self.ui.with_widget::<PlaylistView<Msg>, _>(list, |view| {
                (
                    view.rows_ref()
                        .first()
                        .map(|row| row.title.clone())
                        .unwrap_or_default(),
                    view.drag_state().is_none(),
                )
            });
            if let Some(probe) = self.probe.as_mut() {
                probe.playlist_drag = after
                    .is_some_and(|(title, idle)| idle && !before.is_empty() && title != before);
                probe.step = 31;
            }
        } else if step == 31 {
            // Back to the main page: open the popover from its trigger, check
            // that it was placed below the anchor, then press outside to
            // light-dismiss it.
            self.apply(Msg::Tab(0), window);
            self.apply(Msg::FlyoutOpen, window);
            let anchor = self
                .ui
                .rect(WidgetId::new("flyout_open"))
                .unwrap_or(Rect::from_xywh(0.0, 0.0, 1.0, 1.0));
            let card = self.ui.rect(WidgetId::new("flyout"));
            let placed = card.is_some_and(|card| {
                card.top > anchor.top && card.width() > 0.0 && card.height() > 0.0
            });
            // A point in the page viewport that is outside the card (the
            // list's centre can lie below the fold on a short window).
            let outside = Point::new(
                anchor.left + 12.0,
                (self.scene.viewport.bottom - 24.0).max(anchor.bottom + 60.0),
            );
            self.event(
                &Event::PointerDown {
                    pos: outside,
                    button: MouseButton::Left,
                    modifiers: PointerModifiers::NONE,
                },
                window,
            );
            self.event(
                &Event::PointerUp {
                    pos: outside,
                    button: MouseButton::Left,
                    modifiers: PointerModifiers::NONE,
                },
                window,
            );
            if let Some(probe) = self.probe.as_mut() {
                probe.flyout = placed && !self.flyout_open;
                probe.step = 32;
            }
        } else if step == 32 {
            // Chrome refinements on the gallery page: the vertical bar's
            // geometry, separators/icon-only cells on the horizontal bar, the
            // status progress setter, and the Alt-tap underline mode through
            // the container.
            self.apply(Msg::Tab(2), window);
            let vertical = self.ui.with_widget::<Toolbar<Msg>, _>(
                WidgetId::new("gallery_vtoolbar"),
                |bar| {
                    (
                        bar.is_vertical(),
                        bar.visible_count(),
                        bar.items().len(),
                        bar.mnemonic_underlines(),
                    )
                },
            );
            let separators = self
                .ui
                .with_widget::<Toolbar<Msg>, _>(WidgetId::new("gallery_toolbar"), |bar| {
                    (
                        bar.items()
                            .iter()
                            .filter(|item| item.is_separator())
                            .count(),
                        bar.items()
                            .iter()
                            .filter(|item| item.is_icon_only())
                            .count(),
                    )
                });
            let status_changed = self.ui.edit_widget::<StatusBar>(
                WidgetId::new("gallery_status"),
                |bar| bar.set_progress(0.8),
            );
            let status = self
                .ui
                .with_widget::<StatusBar, _>(WidgetId::new("gallery_status"), |bar| {
                    (status_changed, bar.progress_value(), bar.has_size_grip())
                });
            self.event(&Event::Alt(false), window);
            let underlines_off = self
                .ui
                .with_widget::<Toolbar<Msg>, _>(WidgetId::new("gallery_vtoolbar"), |bar| {
                    bar.mnemonic_underlines()
                });
            self.event(&Event::Alt(true), window);
            let underlines_on = self
                .ui
                .with_widget::<Toolbar<Msg>, _>(WidgetId::new("gallery_vtoolbar"), |bar| {
                    bar.mnemonic_underlines()
                });
            // Date picker: Enter opens the calendar, Right moves the cursor,
            // Enter chooses the highlighted day.
            let date = WidgetId::new("gallery_date");
            self.ui.set_focus(date, window);
            let before = self
                .ui
                .with_widget::<DatePicker<Msg>, _>(date, |picker| picker.date())
                .unwrap_or_default();
            self.send_key(KeyCode::Enter, window);
            let opened = self
                .ui
                .with_widget::<DatePicker<Msg>, _>(date, |picker| picker.is_open())
                .unwrap_or(false);
            self.send_key(KeyCode::Right, window);
            self.send_key(KeyCode::Enter, window);
            let (still_open, chosen) = self
                .ui
                .with_widget::<DatePicker<Msg>, _>(date, |picker| {
                    (picker.is_open(), picker.date())
                })
                .unwrap_or((true, Date::default()));
            // Time picker: Enter opens, Down adds an hour (the column renders
            // increasing downwards), Enter commits.
            let time = WidgetId::new("gallery_time");
            self.ui.set_focus(time, window);
            let before_time = self
                .ui
                .with_widget::<TimePicker<Msg>, _>(time, |picker| picker.value())
                .unwrap_or_default();
            self.send_key(KeyCode::Enter, window);
            self.send_key(KeyCode::Down, window);
            self.send_key(KeyCode::Enter, window);
            let (time_open, time_value) = self
                .ui
                .with_widget::<TimePicker<Msg>, _>(time, |picker| {
                    (picker.is_open(), picker.value())
                })
                .unwrap_or((true, Time::default()));
            let time_ok = !time_open
                && time_value == Time::new(before_time.hour + 1, before_time.minute);
            // Color picker: Enter opens, Right moves, Enter selects.
            let color = WidgetId::new("gallery_color");
            self.ui.set_focus(color, window);
            let before_color = self
                .ui
                .with_widget::<ColorPicker<Msg>, _>(color, |picker| picker.value())
                .unwrap_or(Color::new(0, 0, 0));
            self.send_key(KeyCode::Enter, window);
            self.send_key(KeyCode::Right, window);
            self.send_key(KeyCode::Enter, window);
            let (color_open, color_value) = self
                .ui
                .with_widget::<ColorPicker<Msg>, _>(color, |picker| {
                    (picker.is_open(), picker.value())
                })
                .unwrap_or((true, before_color));
            if let Some(probe) = self.probe.as_mut() {
                probe.chrome = vertical.is_some_and(|(is_vertical, visible, total, _)| {
                    is_vertical && visible == 5 && total == 5
                }) && separators == Some((2, 1))
                    && status == Some((true, Some(0.8), true))
                    && underlines_off == Some(false)
                    && underlines_on == Some(true);
                probe.date_picker =
                    opened && !still_open && chosen == before.add_days(1);
                probe.time_color = time_ok && !color_open && color_value != before_color;
                probe.step = 33;
            }
        } else if step == 33 {
            // Ribbon page: keyboard switches to the 插入 tab, enters the body
            // and runs its first command; a pointer click then returns the
            // selection to the 开始 tab.
            self.apply(Msg::Tab(5), window);
            let ribbon = WidgetId::new("gallery_ribbon");
            self.ui.set_focus(ribbon, window);
            self.send_key(KeyCode::Enter, window);
            self.send_key(KeyCode::Right, window);
            let tab = self
                .ui
                .with_widget::<Ribbon<Msg>, _>(ribbon, |ribbon| ribbon.selected_tab());
            self.send_key(KeyCode::Down, window);
            self.send_key(KeyCode::Enter, window);
            let state = self
                .ui
                .with_widget::<Label, _>(WidgetId::new("ribbon_state"), |label| {
                    label.text().to_string()
                });
            let first_tab = self
                .ui
                .with_widget::<Ribbon<Msg>, _>(ribbon, |ribbon| ribbon.tab_rect(0))
                .flatten();
            if let Some(rect) = first_tab {
                let pos = rect.center();
                self.event(
                    &Event::PointerDown {
                        pos,
                        button: MouseButton::Left,
                        modifiers: PointerModifiers::NONE,
                    },
                    window,
                );
                self.event(
                    &Event::PointerUp {
                        pos,
                        button: MouseButton::Left,
                        modifiers: PointerModifiers::NONE,
                    },
                    window,
                );
            }
            let selected = self
                .ui
                .with_widget::<Ribbon<Msg>, _>(ribbon, |ribbon| ribbon.selected_tab());
            if let Some(probe) = self.probe.as_mut() {
                probe.ribbon = tab == Some(1)
                    && state.as_deref() == Some("最近命令：添加封面")
                    && selected == Some(0);
                probe.step = 34;
            }
        } else if step == 34 {
            // Property page: every editor kind through real pointer/keyboard
            // input — a check box toggle, a drop-down pick, a star click and a
            // typed number committed through the container's inline session.
            self.apply(Msg::Tab(6), window);
            let grid = WidgetId::new("gallery_props");
            let field = |state: &Self, title: &str, key: &str| {
                state
                    .ui
                    .with_widget::<PropertyGrid<Msg>, _>(grid, |grid| {
                        let sections = grid.sections_ref();
                        let section = sections.iter().position(|group| group.title() == title)?;
                        let row = sections[section]
                            .rows_ref()
                            .iter()
                            .position(|row| row.key() == key)?;
                        grid.value_rect(section, row)
                    })
                    .flatten()
            };
            let value = |state: &Self, key: &str| {
                state
                    .ui
                    .with_widget::<PropertyGrid<Msg>, _>(grid, |grid| {
                        grid.value(key).map(str::to_string)
                    })
                    .flatten()
            };

            let check_toggled = match field(&self, "标签", "收藏") {
                Some(rect) => {
                    self.probe_click(rect.center(), window);
                    value(&self, "收藏").as_deref() == Some("否")
                }
                None => false,
            };
            // The choice row sits below the fold at this window height; the
            // typed access scrolls it into view before the pointer lands.
            self.ui.edit_widget::<PropertyGrid<Msg>>(grid, |grid| {
                grid.select_key("音质")
            });
            let quality = match field(&self, "播放", "音质") {
                Some(rect) => {
                    self.probe_click(rect.center(), window);
                    // The open list owns the keys; the current value (无损) is
                    // highlighted, so Down twice and Enter pick 标准.
                    self.send_key(KeyCode::Down, window);
                    self.send_key(KeyCode::Down, window);
                    self.send_key(KeyCode::Enter, window);
                    value(&self, "音质").as_deref() == Some("标准")
                }
                None => false,
            };
            let rating = match field(&self, "标签", "评分") {
                Some(rect) => {
                    let pos = Point::new(rect.left + rect.width() * 0.9, rect.center().y);
                    self.probe_click(pos, window);
                    value(&self, "评分").as_deref() == Some("5")
                }
                None => false,
            };
            let number = {
                let selected = self.ui.edit_widget::<PropertyGrid<Msg>>(grid, |grid| {
                    grid.select_key("播放次数")
                });
                self.ui.set_focus(grid, window);
                // The first Enter enters adjust mode, the second opens the
                // row's text session; the character replaces the selected
                // draft and the third Enter commits it.
                self.send_key(KeyCode::Enter, window);
                self.send_key(KeyCode::Enter, window);
                self.event(&Event::Char('7'), window);
                self.send_key(KeyCode::Enter, window);
                selected && value(&self, "播放次数").as_deref() == Some("7")
            };
            let state = self
                .ui
                .with_widget::<Label, _>(WidgetId::new("props_state"), |label| {
                    label.text().to_string()
                });
            if let Some(probe) = self.probe.as_mut() {
                probe.props = check_toggled
                    && quality
                    && rating
                    && number
                    && state
                        .as_deref()
                        .is_some_and(|text| text.contains("播放次数"));
                probe.step = 35;
            }
        } else if step == 35 {
            // The functional-API page: drag a card onto a drop target. The
            // container owns the gesture; the application only sees the
            // payload and the drop message.
            self.apply(Msg::Tab(7), window);
            // The application's own CustomWidget: a click reaches its input
            // closure, and the app-owned count/label follow.
            self.ui
                .scroll_to_child(WidgetId::new("api_custom_draw"), window);
            let custom_pos = self
                .ui
                .rect(WidgetId::new("api_custom_draw"))
                .unwrap_or(Rect::from_xywh(0.0, 0.0, 1.0, 1.0))
                .center();
            self.probe_click(custom_pos, window);
            let custom_ok = self
                .ui
                .with_widget::<Label, _>(WidgetId::new("api_custom_state"), |label| {
                    label.text().to_string()
                })
                .as_deref()
                .is_some_and(|text| text.contains("已点击 1 次"));
            // The drag demo sits below the fold; reveal its bottom row so both
            // the source card and the target are inside the viewport.
            self.ui
                .scroll_to_child(WidgetId::new("drop_b"), window);
            let from = self
                .ui
                .rect(WidgetId::new("drag_a"))
                .unwrap_or(Rect::from_xywh(0.0, 0.0, 1.0, 1.0))
                .center();
            let to = self
                .ui
                .rect(WidgetId::new("drop_a"))
                .unwrap_or(Rect::from_xywh(0.0, 0.0, 1.0, 1.0))
                .center();
            self.event(
                &Event::PointerDown {
                    pos: from,
                    button: MouseButton::Left,
                    modifiers: PointerModifiers::NONE,
                },
                window,
            );
            self.event(
                &Event::PointerMove {
                    pos: Point::new(from.x + 12.0, from.y + 6.0),
                    buttons: 1,
                },
                window,
            );
            self.event(
                &Event::PointerMove { pos: to, buttons: 1 },
                window,
            );
            self.event(
                &Event::PointerUp {
                    pos: to,
                    button: MouseButton::Left,
                    modifiers: PointerModifiers::NONE,
                },
                window,
            );
            let state = self
                .ui
                .with_widget::<Label, _>(WidgetId::new("api_drag_state"), |label| {
                    label.text().to_string()
                });
            // Text drag and drop: select the source text, press inside the
            // selection and drag it onto the second field. The container
            // carries the text and the target field inserts it (D122).
            self.ui
                .scroll_to_child(WidgetId::new("api_text_to"), window);
            self.ui.edit_widget::<TextEdit<Msg>>(WidgetId::new("api_text_from"), |field| {
                field.select_all()
            });
            let text_from = self
                .ui
                .rect(WidgetId::new("api_text_from"))
                .unwrap_or(Rect::from_xywh(0.0, 0.0, 1.0, 1.0))
                .center();
            let text_to = self
                .ui
                .rect(WidgetId::new("api_text_to"))
                .unwrap_or(Rect::from_xywh(0.0, 0.0, 1.0, 1.0))
                .center();
            self.event(
                &Event::PointerDown {
                    pos: text_from,
                    button: MouseButton::Left,
                    modifiers: PointerModifiers::NONE,
                },
                window,
            );
            self.event(
                &Event::PointerMove {
                    pos: Point::new(text_from.x + 24.0, text_from.y + 12.0),
                    buttons: 1,
                },
                window,
            );
            self.event(
                &Event::PointerMove {
                    pos: text_to,
                    buttons: 1,
                },
                window,
            );
            self.event(
                &Event::PointerUp {
                    pos: text_to,
                    button: MouseButton::Left,
                    modifiers: PointerModifiers::NONE,
                },
                window,
            );
            let carried = self
                .ui
                .with_widget::<TextEdit<Msg>, _>(WidgetId::new("api_text_to"), |field| {
                    field.text().to_string()
                })
                .unwrap_or_default();
            // M7.8: the checked-in `.buitheme` document loads, and one token and
            // the painted page background follow it. The preset theme is
            // restored afterwards so the remaining steps are unaffected.
            let theme_ok = {
                #[cfg(feature = "m7")]
                {
                    let before_dark = self.dark;
                    let before_accent = self.theme.accent;
                    self.apply(Msg::ApiThemeDoc, window);
                    let palette = *window.palette();
                    self.dark = before_dark;
                    self.theme.accent = before_accent;
                    self.clear_theme_document(window);
                    palette.background == Color::new(0x16, 0x18, 0x1d)
                        && palette.accent == Color::new(0x7a, 0xa2, 0xf7)
                        && palette.text == Color::new(0xe8, 0xeb, 0xee)
                }
                #[cfg(not(feature = "m7"))]
                {
                    true
                }
            };
            if let Some(probe) = self.probe.as_mut() {
                probe.api_drag = state
                    .as_deref()
                    .is_some_and(|text| text.contains("卡片 A") && text.contains("放置区 A"));
                probe.text_drag = !carried.is_empty();
                probe.custom_widget = custom_ok;
                probe.theme_doc = theme_ok;
                probe.step = 36;
            }
        } else if step == 36 {
            // Window lifecycle: the first frame is loaded, then hide/show.
            window.hide();
            if let Some(probe) = self.probe.as_mut() {
                probe.step = 37;
            }
        } else if step == 37 {
            let hidden = self.probe.as_ref().is_some_and(|probe| probe.visibility_hidden);
            if hidden || elapsed >= Duration::from_millis(2000) {
                window.show();
                if let Some(probe) = self.probe.as_mut() {
                    probe.step = 38;
                }
            }
        } else if step == 38 {
            let shown = self.probe.as_ref().is_some_and(|probe| probe.visibility_shown);
            if shown || elapsed >= Duration::from_millis(2000) {
                if let Some(probe) = self.probe.as_mut() {
                    probe.veto_close = true;
                }
                window.request_close();
                if let Some(probe) = self.probe.as_mut() {
                    probe.step = 39;
                }
            }
        } else if step == 39 {
            let vetoed = self.probe.as_ref().is_some_and(|probe| {
                probe.close_reasons.contains(&"app") && window.is_visible()
            });
            if vetoed || elapsed >= Duration::from_millis(2000) {
                window.minimize();
                if let Some(probe) = self.probe.as_mut() {
                    probe.step = 40;
                }
            }
        } else if step == 40 {
            let minimized = self.probe.as_ref().is_some_and(|probe| probe.min_seen)
                || window.visual_state() == WindowVisualState::Minimized;
            if minimized || elapsed >= Duration::from_millis(2000) {
                window.restore();
                if let Some(probe) = self.probe.as_mut() {
                    probe.step = 41;
                }
            }
        } else if step == 41 {
            let restored = self.probe.as_ref().is_some_and(|probe| probe.normal_seen)
                || window.visual_state() == WindowVisualState::Normal;
            if restored {
                if let Some(probe) = self.probe.as_mut() {
                    probe.window_api = probe.loaded_frame
                        && probe.visibility_hidden
                        && probe.visibility_shown
                        && probe.min_seen
                        && probe.normal_seen
                        && probe.close_reasons.contains(&"app")
                        && window.is_visible();
                    probe.step = 42;
                }
            } else if elapsed >= Duration::from_millis(2000) {
                if let Some(probe) = self.probe.as_mut() {
                    probe.step = 42;
                }
            }
        } else if step == 42 {
            // The M7 skin page (a no-op without the `m7` feature; its check is
            // initialized to pass there).
            #[cfg(feature = "m7")]
            if self.m7_probe(window) {
                if let Some(probe) = self.probe.as_mut() {
                    probe.m7 = true;
                    probe.step = 43;
                }
            }
        }
    }

    /// The 布局 page must install its document, show the wide variant's
    /// geometry and content, and switch to the base variant when applied at the
    /// minimum client size. Restores the live size at the end.
    #[cfg(feature = "m7")]
    fn m7_probe(&mut self, window: &mut WindowHandle<'_>) -> bool {
        self.apply(Msg::Tab(8), window);
        let Some(document) = self.m7.document.clone() else {
            return false;
        };
        let Some(layout) = self.ui.installed_layout() else {
            return false;
        };
        if layout.variant_id() != "wide" {
            return false;
        }
        if layout.rect(WidgetId::new("m7_action"))
            != Some(Rect::from_xywh(18.0, 64.0, 120.0, 36.0))
        {
            return false;
        }
        let title = self
            .ui
            .with_widget::<Label, _>(WidgetId::new("m7_title"), |label| {
                label.text().to_string()
            })
            .unwrap_or_default();
        if title != "XML 布局演示" {
            return false;
        }
        // The third-party meter's own `x-level`/`x-units` scalars and `x-item`
        // child list came from the file. The local data is written once per
        // generation (children + two attributes = three writes), so the two
        // applies below under the same generation must not touch it again.
        let meter = self
            .ui
            .with_widget::<meter::MeterWidget, _>(WidgetId::new("m7_meter"), |meter| {
                (
                    meter.level(),
                    meter.units().to_string(),
                    meter.items().iter().map(|item| item.label.clone()).collect::<Vec<_>>(),
                    meter.writes(),
                )
            });
        let Some((level, units, items, writes)) = meter else {
            return false;
        };
        if level != 72.0 || units != "dB" || items != ["低频", "中频", "高频"] || writes != 3 {
            return false;
        }
        // Built-in local scalars (M7.3): the card's own `title`/`body`/
        // `emphasis` and the field's `cue` came from the file.
        let card = self
            .ui
            .with_widget::<Card, _>(WidgetId::new("m7_card"), |card| {
                (
                    card.title().to_string(),
                    card.body().to_string(),
                    card.is_emphasis(),
                )
            });
        if card != Some(("卡片标题来自文件".to_string(), "标题、正文与强调条都是控件自己声明的本地标量。".to_string(), true)) {
            return false;
        }
        let cue = self
            .ui
            .with_widget::<TextEdit<Msg>, _>(WidgetId::new("m7_edit"), |edit| {
                edit.cue_text().to_string()
            })
            .unwrap_or_default();
        if cue != "输入提示来自文件" {
            return false;
        }
        // M7.4: the list's `<item>` child elements replaced the Rust rows.
        let list = self.ui.with_widget::<ListView<Msg>, _>(WidgetId::new("m7_list"), |list| {
            (
                list.rows().len(),
                list.rows().first().map(|row| row.title.clone()),
                list.selected_row(),
                list.row_height_value(),
                list.is_alternating(),
            )
        });
        if list
            != Some((
                3,
                Some("文件驱动的列表".to_string()),
                1,
                36.0,
                true,
            ))
        {
            return false;
        }
        // M7.4 step 2: the structural lists came from the same file.
        let table = self
            .ui
            .with_widget::<Table<Msg>, _>(WidgetId::new("m7_table"), |table| {
                (
                    table.columns_ref().len(),
                    table.rows_ref().len(),
                    table.row(0).map(|row| row.cells.clone()),
                    table.selected_row(),
                    table.row(2).is_some_and(|row| row.disabled),
                    table.row_height_value(),
                    table.is_striped(),
                    table.is_editable(),
                )
            });
        if table
            != Some((
                3,
                3,
                Some(vec!["01".to_string(), "夜曲".to_string(), "3:58".to_string()]),
                1,
                true,
                24.0,
                true,
                true,
            ))
        {
            return false;
        }
        let tree = self
            .ui
            .with_widget::<TreeView<Msg>, _>(WidgetId::new("m7_tree"), |tree| {
                (
                    tree.items_ref().len(),
                    tree.items_ref().first().map(|node| node.label.clone()),
                    tree.visible_row_count(),
                    tree.selected_path(),
                    tree.row_height_value(),
                    tree.is_renameable(),
                    tree.is_draggable(),
                )
            });
        if tree != Some((1, Some("音乐库".to_string()), 5, Some(vec![0, 0]), 24.0, true, false)) {
            return false;
        }
        let props = self
            .ui
            .with_widget::<PropertyGrid<Msg>, _>(WidgetId::new("m7_props"), |grid| {
                (
                    grid.sections_ref().len(),
                    grid.row_count(),
                    grid.value("音量").map(str::to_string),
                    grid.row_by_key("音量")
                        .is_some_and(|row| matches!(row.kind(), PropertyKind::Number { .. })),
                    grid.key_width_value(),
                    grid.row_height_value(),
                )
            });
        if props != Some((2, 6, Some("72".to_string()), true, 110.0, 24.0)) {
            return false;
        }
        let pager = self
            .ui
            .with_widget::<Pager<Msg>, _>(WidgetId::new("m7_pager"), |pager| {
                (
                    pager.pages(),
                    pager.page(),
                    pager.page_labels().to_vec(),
                    pager.label_text().to_string(),
                )
            });
        if pager
            != Some((
                4,
                1,
                vec![
                    "封面".to_string(),
                    "曲目".to_string(),
                    "详情".to_string(),
                    "设置".to_string(),
                ],
                "曲目".to_string(),
            ))
        {
            return false;
        }
        // M7.5: the debug API behind the 调试 menu reports the installed
        // layout, the log ring and each widget's declared skin vocabulary.
        let snapshot = self.ui.debug_snapshot();
        let Some(debug_layout) = snapshot.layout else {
            return false;
        };
        if debug_layout.variant != "wide" || debug_layout.entries.is_empty() {
            return false;
        }
        if !snapshot
            .log
            .iter()
            .any(|event| event.kind == "layout" && event.text.contains("variant=wide"))
        {
            return false;
        }
        let Some(list_widget) = snapshot
            .widgets
            .iter()
            .find(|widget| widget.id == WidgetId::new("m7_list"))
        else {
            return false;
        };
        if list_widget.skin_children != ["item"] {
            return false;
        }
        let Some(meter_widget) = snapshot
            .widgets
            .iter()
            .find(|widget| widget.id == WidgetId::new("m7_meter"))
        else {
            return false;
        };
        if !meter_widget
            .skin_attrs
            .iter()
            .any(|(name, _)| *name == "x-level")
        {
            return false;
        }
        // Entries inside a scroll panel resolve in content coordinates, so the
        // document rect is the comparison (the live widget rect is in client
        // space and includes the viewport origin).
        if layout.rect(WidgetId::new("m7_meter")).map(|rect| rect.top) != Some(484.0) {
            return false;
        }
        if layout.rect(WidgetId::new("m7_pager")).map(|rect| rect.top) != Some(1188.0) {
            return false;
        }
        // M7.4 step 3: the label-owning lists came from the same file.
        let tabs = self
            .ui
            .with_widget::<Tabs<Msg>, _>(WidgetId::new("m7_tabs"), |tabs| {
                (
                    tabs.labels().to_vec(),
                    tabs.selected(),
                    tabs.label_alignment(),
                )
            });
        if tabs
            != Some((
                vec![
                    "概览".to_string(),
                    "详情".to_string(),
                    "设置".to_string(),
                ],
                1,
                TextAlign::Left,
            ))
        {
            return false;
        }
        let segment = self
            .ui
            .with_widget::<Segmented<Msg>, _>(WidgetId::new("m7_seg"), |seg| {
                (seg.options().to_vec(), seg.selected())
            });
        if segment
            != Some((vec!["低".to_string(), "中".to_string(), "高".to_string()], 2))
        {
            return false;
        }
        let crumb = self
            .ui
            .with_widget::<Breadcrumb<Msg>, _>(WidgetId::new("m7_crumb"), |crumb| {
                (
                    crumb.items().to_vec(),
                    crumb.current_index(),
                    crumb.separator_char(),
                )
            });
        if crumb
            != Some((
                vec!["音乐".to_string(), "专辑".to_string(), "夜曲".to_string()],
                Some(2),
                '›',
            ))
        {
            return false;
        }
        let rail = self
            .ui
            .with_widget::<NavRail<Msg>, _>(WidgetId::new("m7_rail"), |rail| {
                (
                    rail.items()
                        .iter()
                        .map(|item| (item.label.clone(), item.icon))
                        .collect::<Vec<_>>(),
                    rail.selected(),
                )
            });
        if rail
            != Some((
                vec![
                    ("首页".to_string(), Some('⌂')),
                    ("曲库".to_string(), None),
                    ("设置".to_string(), None),
                ],
                1,
            ))
        {
            return false;
        }
        let rich = self
            .ui
            .with_widget::<RichText<Msg>, _>(WidgetId::new("m7_rich"), |rich| {
                (
                    rich.runs_ref()
                        .iter()
                        .map(|run| (run.text.clone(), run.style, run.link))
                        .collect::<Vec<_>>(),
                    rich.plain_text(),
                    rich.alignment(),
                    rich.text_style(),
                )
            });
        if rich
            != Some((
                vec![
                    ("重点：".to_string(), RunStyle::Normal, 0),
                    ("夜曲".to_string(), RunStyle::Strong, 0),
                    ("查看".to_string(), RunStyle::Link, 7),
                ],
                "重点：夜曲查看".to_string(),
                TextAlign::Center,
                LabelStyle::Small,
            ))
        {
            return false;
        }
        let spark = self
            .ui
            .with_widget::<Sparkline, _>(WidgetId::new("m7_spark"), |spark| {
                (
                    spark.data().to_vec(),
                    spark.gap_value(),
                    spark.color_value().is_some(),
                )
            });
        if spark != Some((vec![1.0, 3.5, 4.0, 2.0], 3.0, true)) {
            return false;
        }
        if layout
            .rect(WidgetId::new("m7_spark"))
            .map(|rect| rect.top)
            != Some(1564.0)
        {
            return false;
        }
        // M7.7: the document created `m7_created` from the widget registry and
        // the file's label reached it.
        let created = self
            .ui
            .with_widget::<Label, _>(WidgetId::new("m7_created"), |label| {
                label.text().to_string()
            });
        if created.as_deref() != Some("来自 WidgetRegistry 的创建控件") {
            return false;
        }
        if layout
            .rect(WidgetId::new("m7_created"))
            .map(|rect| rect.top)
            != Some(1610.0)
        {
            return false;
        }
        if layout.created_ids().count() != 1 {
            return false;
        }
        // The breakpoint is a pure function of the client size: 760×560
        // selects the base variant and its geometry.
        let palette = self.palette();
        let Ok(base) = self.ui.apply_layout(
            &document,
            &palette,
            Size::new(760.0, 560.0),
            window,
        ) else {
            return false;
        };
        if base.variant_id != "base" {
            return false;
        }
        let base_rect = self
            .ui
            .installed_layout()
            .and_then(|layout| layout.rect(WidgetId::new("m7_action")));
        if base_rect.map(|rect| rect.top) != Some(64.0) {
            return false;
        }
        let meter_top = self
            .ui
            .installed_layout()
            .and_then(|layout| layout.rect(WidgetId::new("m7_meter")))
            .map(|rect| rect.top);
        if meter_top != Some(618.0) {
            return false;
        }
        let base_pager_top = self
            .ui
            .installed_layout()
            .and_then(|layout| layout.rect(WidgetId::new("m7_pager")))
            .map(|rect| rect.top);
        if base_pager_top != Some(1322.0) {
            return false;
        }
        let base_spark_top = self
            .ui
            .installed_layout()
            .and_then(|layout| layout.rect(WidgetId::new("m7_spark")))
            .map(|rect| rect.top);
        if base_spark_top != Some(1698.0) {
            return false;
        }
        // The created widget survives the breakpoint switch (same generation:
        // no re-creation) and follows the base geometry.
        let base_created_top = self
            .ui
            .installed_layout()
            .and_then(|layout| layout.rect(WidgetId::new("m7_created")))
            .map(|rect| rect.top);
        if base_created_top != Some(1744.0) {
            return false;
        }
        if !self.ui.contains(WidgetId::new("m7_created")) {
            return false;
        }
        // The same generation again must not rewrite the local skin data.
        let writes = self
            .ui
            .with_widget::<meter::MeterWidget, _>(WidgetId::new("m7_meter"), |meter| {
                meter.writes()
            })
            .unwrap_or(0);
        if writes != 3 {
            return false;
        }
        // Put the page back on the real client size.
        self.m7_apply(window.size(), window);
        true
    }

    /// Press/release pair for the self-test: a left-button click at `pos`.
    fn probe_click(&mut self, pos: Point, window: &mut WindowHandle<'_>) {
        self.event(
            &Event::PointerDown {
                pos,
                button: MouseButton::Left,
                modifiers: PointerModifiers::NONE,
            },
            window,
        );
        self.event(
            &Event::PointerUp {
                pos,
                button: MouseButton::Left,
                modifiers: PointerModifiers::NONE,
            },
            window,
        );
    }

    /// `(selected index, cover count, scroll offset)` of the cover wall, read
    /// through the typed-access seam.
    fn wall_state(&self) -> Option<(usize, usize, f32)> {
        self.ui.with_widget::<CoverWall<Msg>, _>(WidgetId::new("covers_wall"), |wall| {
            (
                wall.selected_index(),
                wall.covers_ref().len(),
                wall.scroll_offset(),
            )
        })
    }

    /// Report the step that ran out of time so a failed self-test says where
    /// it stopped instead of only an exit code.
    fn probe_timeout(&mut self, window: &mut WindowHandle<'_>, step: u8) {
        eprintln!("self-test: timeout at step {step}");
        window.quit(3);
    }

    fn send_key(&mut self, code: KeyCode, window: &mut WindowHandle<'_>) {
        let vk = match code {
            KeyCode::Enter => 0x0D,
            KeyCode::Tab => 0x09,
            _ => 0,
        };
        let key = KeyEvent {
            code,
            vk,
            ctrl: false,
            shift: false,
            alt: false,
            repeat: false,
        };
        self.event(&Event::Key(key), window);
    }
}

impl Host for Showcase {
    /// The in-window modal shell dims a cached page instead of repainting it.
    /// `BAMBOO_UI_NO_OVERLAY=1` forces the plain repaint for comparison (or for
    /// a page that must keep updating while the modal is up).
    fn overlay_paint(&self) -> bool {
        self.modal_open
            && self.ui.overlay_active()
            && std::env::var_os("BAMBOO_UI_NO_OVERLAY").is_none()
    }

    fn loaded(&mut self, window: &mut WindowHandle<'_>) {
        if let Some(probe) = self.probe.as_mut() {
            probe.loaded_frame = true;
        }
        if let Ok(mut slot) = self.renderer_seen.lock() {
            *slot = Some(window.renderer());
        }
    }

    // System drag and drop: the framework reports OLE events with client DIP
    // coordinates; the container routes them to the widget under the pointer
    // and the page paints the accent frames.
    fn drag_enter(
        &mut self,
        data: &DragData,
        pos: Point,
        window: &mut WindowHandle<'_>,
    ) -> DropEffect {
        let effect = self.ui.drag_enter(data, pos, window);
        self.ui.flush_damage(window);
        effect
    }

    fn drag_over(
        &mut self,
        data: &DragData,
        pos: Point,
        window: &mut WindowHandle<'_>,
    ) -> DropEffect {
        let effect = self.ui.drag_over(data, pos, window);
        self.ui.flush_damage(window);
        effect
    }

    fn drag_leave(&mut self, window: &mut WindowHandle<'_>) {
        self.ui.drag_leave(window);
        self.ui.flush_damage(window);
    }

    fn drag_drop(
        &mut self,
        data: DragData,
        pos: Point,
        window: &mut WindowHandle<'_>,
    ) -> DropEffect {
        let effect = self.ui.drag_drop(data, pos, window);
        self.drain(window);
        self.ui.flush_damage(window);
        effect
    }

    fn drag_finished(&mut self, _effect: DropEffect, window: &mut WindowHandle<'_>) {
        self.ui.drag_finished(window);
        self.ui.flush_damage(window);
    }

    fn visibility_changed(&mut self, visible: bool, _window: &mut WindowHandle<'_>) {
        if let Some(probe) = self.probe.as_mut() {
            if visible {
                probe.visibility_shown = true;
            } else {
                probe.visibility_hidden = true;
            }
        }
    }

    fn visual_state_changed(
        &mut self,
        state: WindowVisualState,
        _window: &mut WindowHandle<'_>,
    ) {
        if let Some(probe) = self.probe.as_mut() {
            match state {
                WindowVisualState::Minimized => probe.min_seen = true,
                WindowVisualState::Normal => probe.normal_seen = true,
                WindowVisualState::Maximized => {}
            }
        }
    }

    fn started(&mut self, window: &mut WindowHandle<'_>) {
        window.raise();
        // Automated runs drive synthetic pointer events; a real OLE drag loop
        // would wait for a physical button release that never comes. Turning
        // system drags off keeps the container's in-window fallback.
        if self.self_test || self.bench.is_some() || self.a11y_dump {
            window.set_system_drag_enabled(false);
        }
        self.ui.set_focus(WidgetId::new("primary"), window);
        // The API page reports the renderer that actually paints (a Direct2D
        // failure falls back to GDI for the window's lifetime).
        let renderer = window.renderer();
        self.ui
            .set_label(WidgetId::new("api_renderer"), format!("渲染器：{}", renderer.name()));
        self.ui.set_label(
            WidgetId::new("api_pad_state"),
            if read_gamepad(0).is_some() {
                "手柄：已连接（XInput）"
            } else {
                "手柄：未连接（XInput）"
            },
        );
        self.set_action("就绪：切换页面查看全部组件与 API", window);
        // No notification-area icon is installed on startup: the API page's
        // 安装托盘图标 / 移除托盘图标 buttons are the tray demo, so walking away
        // from the showcase leaves no stray icons behind.
        // Startup content (renderer/pad status, first action) was recorded as
        // damage; hand it over before the first frame.
        self.ui.flush_damage(window);
        // The launcher window may still cover us; force one full frame so the
        // self-test and benchmark actually exercise widget painting.
        if self.self_test || self.bench.is_some() || self.a11y_dump {
            window.invalidate_all_force();
        }
        if self.a11y_dump {
            window.set_animation_timer(Some(Duration::from_millis(50)));
            // The UIA client must not run on the UI thread that services the
            // provider, and the window must be up before it connects.
            let hwnd = window.hwnd();
            let slot = self.a11y_result.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(500));
                let result = uia_probe::run(hwnd);
                if let Ok(mut guard) = slot.lock() {
                    *guard = Some(result);
                }
            });
        } else if self.self_test {
            window.set_animation_timer(Some(Duration::from_millis(50)));
        } else if self.bench.is_some() {
            window.set_animation_timer(Some(Duration::from_millis(16)));
        } else {
            // A pad becomes the keyboard: the timer polls XInput and forwards
            // the translated keys. The slow watch tick keeps looking for one
            // (hot-plug or a sleeping pad) without painting anything.
            self.attach_gamepad(window);
            self.sync_timer(window);
        }
    }

    fn resize(&mut self, size: Size, _dpi: f32, window: &mut WindowHandle<'_>) {
        let (scene, specs) = layout(size);
        self.scene = scene;
        self.specs = specs;
        self.layout_chrome(window);
        // The skin is applied after `register_pages`, so the document's
        // rectangles win over the catalog fallback (and a resize keeps them).
        #[cfg(feature = "m7")]
        self.m7_apply(size, window);
        self.pulse = Rect::from_xywh(
            self.scene.main.center().x - 20.0,
            self.scene.main.center().y - 20.0,
            40.0,
            40.0,
        );
        // Keep an open native editor on the widget after the new layout.
        self.ui.sync_edit(window);
        // Bounds in the a11y snapshot changed.
        window.a11y_refresh();
        window.invalidate_all();
    }

    fn timer(&mut self, window: &mut WindowHandle<'_>) {
        self.ticks.fetch_add(1, Ordering::Relaxed);
        self.invalids.store(window.invalidations(), Ordering::Relaxed);
        // A connected pad is the keyboard: its edges go through the same key
        // path focus navigation consumes (never during the probes). `pad_key`
        // adds the one thing a controller needs: a text field raises the
        // Windows on-screen keyboard and leaving the field dismisses it.
        if !self.a11y_dump && !self.self_test && self.bench.is_none() {
            self.poll_gamepad(window);
            // Hot reload polls the skin file's mtime on the same timer; a
            // successful reload applies and damages its own widgets.
            #[cfg(feature = "m7")]
            if self.tab == 8 {
                self.m7_poll(window);
            }
            // The poll reports pad state through `set_label`, which records its
            // damage instead of invalidating.
            self.ui.flush_damage(window);
        }
        if self.a11y_dump {
            let finished = self
                .a11y_result
                .lock()
                .map(|slot| slot.is_some())
                .unwrap_or(true);
            if finished {
                window.quit(0);
            } else if self.start.elapsed() >= Duration::from_secs(15) {
                if let Ok(mut slot) = self.a11y_result.lock() {
                    *slot = Some(Err("timed out waiting for the UIA client".to_string()));
                }
                window.quit(4);
            }
            return;
        }
        if self.self_test {
            self.probe_tick(window);
            // The probe edits widgets through typed access; one flush per probe
            // step covers everything it touched.
            self.ui.flush_damage(window);
            // Overall cap; the probe's own step deadlines are tighter and
            // report where they stopped, so this only guards a hang.
            if self.start.elapsed() >= Duration::from_millis(35000) {
                let ok = self.probe.as_ref().is_some_and(EditProbe::ok);
                if !ok
                    && let Some(probe) = self.probe.as_ref()
                {
                    eprintln!(
                        "self-test: cap at step {}: failed={:?} opened={:?} committed={:?} tooltip={:?} menu={:?} tray={:?}/{:?} modal_blocks={:?} modal_result={:?} parallel_free={:?} parallel_result={:?} pickers={:?}/{:?} hotkey={:?}/{:?} menu_ctrl={:?} click={:?} bar={:?} bar_cmd={:?} scroll={:?}/{:?} nested={:?} table={:?} multi={:?} tree={:?} events={:?},{:?},{:?},{:?}",
                        probe.step,
                        probe.failures(),
                        probe.opened,
                        probe.committed,
                        probe.tooltip,
                        probe.menu,
                        probe.tray,
                        probe.tray_multi,
                        probe.modal_blocks,
                        probe.modal_result,
                        probe.parallel_free,
                        probe.parallel_result,
                        probe.picker_seen,
                        probe.picker_results,
                        probe.hotkey_registered,
                        probe.hotkey_captured,
                        probe.menu_controller_widget,
                        probe.menu_command_command,
                        probe.menu_bar,
                        probe.menu_bar_command_command,
                        probe.scroll_reveal,
                        probe.scroll_wheel,
                        probe.nested_scroll,
                        probe.table_edit,
                        probe.table_multiline,
                        probe.tree_edit,
                        probe.menu_opening,
                        probe.menu_opened,
                        probe.menu_selected,
                        probe.menu_closed,
                    );
                    if !probe.window_api {
                        eprintln!(
                            "self-test: lifecycle loaded={} hidden={} shown={} min={} normal={} close_app={} visible={}",
                            probe.loaded_frame,
                            probe.visibility_hidden,
                            probe.visibility_shown,
                            probe.min_seen,
                            probe.normal_seen,
                            probe.close_reasons.contains(&"app"),
                            window.is_visible(),
                        );
                    }
                }
                window.quit(if ok { 0 } else { 3 });
            }
            return;
        }
        if let Some(seconds) = self.bench {
            if self.start.elapsed().as_secs_f32() >= seconds {
                window.quit(0);
                return;
            }
            const PULSE: f32 = 40.0;
            let old = self.pulse;
            let floor = self.scene.viewport.left;
            let ceiling = (self.scene.viewport.right - PULSE).max(floor);
            self.pulse.left += 6.0;
            if self.pulse.left > ceiling {
                self.pulse.left = floor;
            }
            self.pulse.right = self.pulse.left + PULSE;
            self.pulse.bottom = self.pulse.top + PULSE;
            window.invalidate_force(old);
            window.invalidate_force(self.pulse);
            window.flush();
            return;
        }
        let animate_gallery = self.tab == 2 && !self.modal_open;
        if self.marquee_playing || animate_gallery {
            let now = Instant::now();
            let dt = now.saturating_duration_since(self.last_tick);
            self.last_tick = now;
            if self.marquee_playing {
                self.ui
                    .edit_widget::<MarqueeText>(WidgetId::new("marquee"), |marquee| {
                        marquee.advance(dt)
                    });
            }
            if animate_gallery {
                self.ui
                    .edit_widget::<Spinner>(WidgetId::new("gallery_spinner"), |spinner| {
                        spinner.advance(dt)
                    });
            }
            // One animation tick is one repaint request, however many widgets
            // advanced.
            self.ui.flush_damage(window);
        }
        if let Some(until) = self.toast_until
            && Instant::now() >= until
        {
            self.toast_until = None;
            self.ui.set_visible(WidgetId::new("toast"), false, window);
            // Hiding changes hit-testing and the exposed a11y tree; focus may
            // have relocated to another widget.
            window.a11y_focus(self.ui.focus_id());
            self.sync_timer(window);
        }
    }

    fn paint(&mut self, ctx: &mut PaintCtx<'_>) {
        // An overlay composite (the in-window modal) leaves the page in the
        // back buffer: only the scrim and the panel paint.
        if ctx.overlay {
            let started = Instant::now();
            self.ui.paint(ctx);
            if std::env::var_os("BAMBOO_UI_PAINT_LOG").is_some() {
                eprintln!(
                    "showcase overlay paint: first={} widgets={} {}us",
                    ctx.overlay_first,
                    self.ui.len(),
                    started.elapsed().as_micros()
                );
            }
            return;
        }
        let started = Instant::now();
        let dirty = ctx.dirty.len();
        {
            // The host keeps distant damage as separate rectangles: test every
            // piece, not just the first one.
            let dirty_rects: Vec<Rect> = ctx.dirty.to_vec();
            let hits = |rect: Rect| dirty_rects.iter().any(|dirty| dirty.intersects(rect));
            let painter = ctx.painter;
            let pal = *painter.palette();
            let scene = &self.scene;
            let page = self.tab.min(self.specs.len().saturating_sub(1));
            let offset = self
                .ui
                .scroll_offset(self.specs[page].panel)
                .unwrap_or(Point::new(0.0, 0.0));
            let viewport = self.specs[page].viewport;
            let captions = &self.specs[page].captions;
            let page_name = PAGE_NAMES.get(page).copied().unwrap_or("");
            let page_hint = PAGE_HINTS.get(page).copied().unwrap_or("");
            let heading_style = painter.ui_style(15.5);
            let canvas: &mut dyn Canvas = &mut *ctx.canvas;
            // The host blits each dirty piece; the app repaints the page
            // background under every piece, then the content that intersects
            // one. Anything drawn outside the pieces is simply not copied.
            for rect in ctx.dirty {
                canvas.fill_rect(*rect, pal.background);
            }
            if hits(scene.sidebar) {
                canvas.fill_rect(scene.sidebar, pal.sidebar);
                painter.fill(&mut *canvas, scene.divider, pal.divider);
                painter.text(
                    &mut *canvas,
                    scene.brand,
                    "Bamboo Native UI",
                    painter.brand(),
                    TextAlign::Left,
                    pal.accent_text,
                );
                painter.text(
                    &mut *canvas,
                    scene.page_title,
                    page_name,
                    painter.title(),
                    TextAlign::Left,
                    pal.text,
                );
                painter.text(
                    &mut *canvas,
                    scene.page_hint,
                    page_hint,
                    painter.small(),
                    TextAlign::Left,
                    pal.muted,
                );
                painter.text(
                    &mut *canvas,
                    scene.footer,
                    &self.status,
                    painter.small(),
                    TextAlign::Left,
                    pal.muted,
                );
            }
            // Section headings and API notes scroll with their page. They are
            // clipped to the viewport and skipped when the dirty box misses.
            if hits(viewport) {
                canvas.push_clip(viewport);
                for caption in captions {
                    let rect = Rect::from_xywh(
                        viewport.left + caption.rect.left - offset.x,
                        viewport.top + caption.rect.top - offset.y,
                        caption.rect.width(),
                        caption.rect.height(),
                    );
                    if rect.bottom < viewport.top || rect.top > viewport.bottom {
                        continue;
                    }
                    if !hits(rect) {
                        continue;
                    }
                    if caption.heading {
                        painter.text(
                            &mut *canvas,
                            rect,
                            caption.text,
                            &heading_style,
                            TextAlign::Left,
                            pal.text,
                        );
                    } else {
                        painter.text(
                            &mut *canvas,
                            rect,
                            caption.text,
                            painter.small(),
                            TextAlign::Left,
                            pal.muted,
                        );
                    }
                }
                canvas.pop_clip();
            }
            painter.text(
                &mut *canvas,
                scene.status,
                &format!("最近：{}", self.last_action),
                painter.small(),
                TextAlign::Left,
                pal.muted,
            );
        }

        self.ui.paint(ctx);

        // Cross-widget drag feedback: an accent frame around the target under
        // the pointer. The hover callback only asks for the repaint; the frame
        // is the application's visual answer.
        if self.ui.is_dragging() {
            let pal = *ctx.painter.palette();
            for zone in [WidgetId::new("drop_a"), WidgetId::new("drop_b")] {
                if self.ui.drag_target() != Some(zone) {
                    continue;
                }
                if let Some(rect) = self.ui.rect(zone) {
                    ctx.canvas.stroke_round_rect(rect, 4.0, 2.0, pal.accent);
                }
            }
            // The text target gets the same accent frame while text is over it.
            if self.ui.drag_text().is_some()
                && self.ui.drag_target() == Some(WidgetId::new("api_text_to"))
                && let Some(rect) = self.ui.rect(WidgetId::new("api_text_to"))
            {
                ctx.canvas.stroke_round_rect(rect, 4.0, 2.0, pal.accent);
            }
        }
        // The nested scroll demo sits over the outer panel's rows; a frame
        // marks where the inner viewport begins and ends.
        if self.tab == 3
            && let Some(view) = self.ui.scroll_view(WidgetId::new(NEST_PANEL))
        {
            let clip = view.clip_rect();
            if !clip.is_empty() {
                let pal = *ctx.painter.palette();
                ctx.canvas.stroke_round_rect(clip, 3.0, 1.0, pal.divider);
            }
        }

        if self.bench.is_some() {
            let pal = *ctx.painter.palette();
            ctx.canvas.ellipse(self.pulse, pal.accent);
        }

        let micros = started.elapsed().as_micros() as u64;
        if std::env::var_os("BAMBOO_UI_PAINT_LOG").is_some() {
            let union = ctx
                .dirty
                .iter()
                .copied()
                .reduce(|left, right| left.union(right));
            eprintln!(
                "showcase paint: t={}ms dirty={dirty} rect={union:?} widgets={} {micros}us",
                self.start.elapsed().as_millis(),
                self.ui.len()
            );
        }
        self.paint_micros.fetch_add(micros, Ordering::Relaxed);
        if dirty as u64 != self.last_status.0
            || micros != self.last_status.1
            || self.ui.len() != self.last_status.2
        {
            self.last_status = (dirty as u64, micros, self.ui.len());
            // The footer doubles as the pad indicator: a controller that went
            // to sleep or was unplugged is visible there (`poll_gamepad`
            // repaints it on a change).
            self.status = format!(
                "控件 {} · 脏区 {} · 绘制 {}µs{}",
                self.ui.len(),
                dirty,
                micros,
                if self.gamepad.is_some() {
                    " · 手柄导航"
                } else {
                    ""
                }
            );
        }
        self.painted.fetch_add(1, Ordering::Relaxed);
    }

    fn event(&mut self, event: &Event, window: &mut WindowHandle<'_>) -> bool {
        let handled = match *event {
            Event::PointerMove { pos, .. } => {
                self.ui.pointer_move(pos, window);
                window.set_cursor(self.ui.hover_cursor());
                self.drain(window);
                true
            }
            Event::PointerLeave => {
                self.ui.pointer_leave(window);
                window.set_cursor(Cursor::Arrow);
                true
            }
            Event::PointerDown {
                pos,
                button,
                modifiers,
            } => {
                self.ui.pointer_down_with(pos, button, modifiers, window);
                self.drain(window);
                true
            }
            Event::PointerUp {
                pos,
                button,
                modifiers,
            } => {
                self.ui.pointer_up_with(pos, button, modifiers, window);
                self.drain(window);
                true
            }
            Event::PointerDoubleClick { pos, .. } => {
                self.ui.pointer_double_click(pos, window);
                self.drain(window);
                true
            }
            Event::Wheel {
                pos,
                delta_x,
                delta_y,
            } => {
                let handled = self.ui.wheel_xy(pos, delta_x, delta_y, window);
                self.drain(window);
                handled
            }
            Event::Alt(show) => {
                // Alt tap: access-key underlines on; the next key/click hides
                // them. The host tracks the tap; the app owns the policy.
                self.ui.set_mnemonic_underlines(show);
                true
            }
            // Touch input: the host reports the device before the pointer
            // events and forwards a held press after its dwell. The editor
            // switches to its touch model (word selection, handles, shell
            // keyboard) for a touch/pen session (D121).
            Event::PointerKind(kind) => {
                self.ui.set_pointer_kind(kind);
                true
            }
            Event::LongPress { pos } => {
                let handled = self.ui.long_press(pos, window);
                self.drain(window);
                handled
            }
            Event::Key(key) => {
                if self.modal_open && key.code == KeyCode::Escape {
                    self.close_modal(window, "已取消");
                    return true;
                }
                if self.ui.key(key, window) {
                    self.drain(window);
                    true
                } else {
                    false
                }
            }
            Event::KeyUp(key) => {
                let handled = self.ui.key_up(key, window);
                self.drain(window);
                handled
            }
            Event::Char(ch) => {
                // Typed characters go to a focused framework-painted editor.
                // The native child receives its own WM_CHAR directly.
                let handled = self.ui.char(ch, window);
                self.drain(window);
                handled
            }
            _ => false,
        };
        self.ui.flush_damage(window);
        // Let a screen reader follow keyboard focus as it moves.
        window.a11y_focus(self.ui.focus_id());
        handled
    }

    fn a11y_nodes(&mut self) -> Vec<bamboo_native_ui::A11yNode> {
        self.ui.a11y_nodes()
    }

    fn a11y_action(&mut self, action: A11yAction, window: &mut WindowHandle<'_>) {
        if let Some(message) = self.ui.apply_a11y(&action, window) {
            self.apply(message, window);
        }
        self.drain(window);
        self.ui.flush_damage(window);
        window.a11y_focus(self.ui.focus_id());
    }

    fn close_requested(&mut self, reason: CloseReason, window: &mut WindowHandle<'_>) -> bool {
        if let Some(probe) = self.probe.as_mut() {
            probe.close_reasons.push(match reason {
                CloseReason::User => "user",
                CloseReason::System => "system",
                CloseReason::Programmatic => "app",
            });
            if probe.veto_close {
                probe.veto_close = false;
                self.show_toast("关闭已被应用拦截".to_string(), window);
                return false;
            }
        }
        true
    }

    fn edit_committed(&mut self, id: WidgetId, text: &str, window: &mut WindowHandle<'_>) {
        if id == WidgetId::new("search")
            && let Some(probe) = self.probe.as_mut()
        {
            probe.committed = Some(text.to_string());
        }
        if let Some(message) = self.ui.commit_edit(id, text.to_string(), window) {
            self.apply(message, window);
        }
    }

    fn edit_canceled(&mut self, id: WidgetId, window: &mut WindowHandle<'_>) {
        // Escape: the widget still holds the pre-edit text; repaint it.
        if let Some(rect) = self.ui.rect(id) {
            window.invalidate(rect);
        }
    }

    fn ime_compose(&mut self, text: &str, cursor: usize, window: &mut WindowHandle<'_>) {
        // The painted editor draws the composition inline; the host hid the
        // system composition window.
        self.ui.ime_compose(text, cursor, window);
        self.drain(window);
    }

    fn ime_commit(&mut self, text: &str, window: &mut WindowHandle<'_>) {
        self.ui.ime_commit(text, window);
        self.drain(window);
    }

    /// One hook for every native menu (context menus, chrome menus, the tray
    /// menu and programmatic opens). The demo logs each event to the status
    /// strip's third segment and the self-test observes the lifecycle; the
    /// default `Pass` keeps the normal command replay.
    fn menu_event(&mut self, event: &mut MenuEvent<'_>, window: &mut WindowHandle<'_>) -> MenuDecision {
        let (kind, source) = match event {
            MenuEvent::Opening { source, .. } => ("打开前", *source),
            MenuEvent::Opened { source, .. } => ("已打开", *source),
            MenuEvent::Selected { source, .. } => ("选中", *source),
            MenuEvent::Closed { source, .. } => ("关闭", *source),
        };
        if let Some(probe) = self.probe.as_mut() {
            match event {
                MenuEvent::Opening { .. } => probe.menu_opening = true,
                MenuEvent::Opened { menu, .. } => {
                    probe.menu_opened = true;
                    probe.menu_opened_count = probe.menu_opened_count.saturating_add(1);
                    probe.menu_opened_while_open |= window.menu_active();
                    if probe.menu_live_edit && menu.command(PROBE_LIVE_COMMAND).is_some() {
                        probe.menu_live_content = true;
                    }
                }
                MenuEvent::Selected { command, .. } => {
                    probe.menu_selected = true;
                    if **command == PROBE_MENU_COMMAND {
                        probe.menu_probe_selected = true;
                    }
                }
                MenuEvent::Closed { command, .. } => {
                    probe.menu_closed = true;
                    probe.menu_closed_command = *command;
                }
            }
        }
        let text = format!("菜单 · {} · {}", source.label(), kind);
        self.ui
            .edit_widget::<StatusBar>(WidgetId::new("gallery_status"), |bar| {
                bar.set_segment(2, text)
            });
        self.ui.flush_damage(window);
        MenuDecision::Pass
    }

    /// A context-menu selection. Command ids are app-defined, so one handler
    /// covers every widget's menu (the `widget` argument disambiguates).
    fn menu_command(&mut self, widget: WidgetId, command: u32, window: &mut WindowHandle<'_>) {
        if let Some(probe) = self.probe.as_mut()
            && command == PROBE_MENU_COMMAND
        {
            probe.menu_command_command = Some(command);
        }
        // Painted editors install a built-in Undo/Cut/Copy/Paste menu; its ids
        // are reserved and routed back to the widget by `Ui`.
        if self.ui.menu_command(widget, command, window) {
            self.drain(window);
            return;
        }
        // Application chrome (menu bar, toolbar, menu/split buttons) uses one
        // command range; the widget id says which one opened the menu.
        if (500..800).contains(&command) {
            self.apply(Msg::Chrome(command), window);
            return;
        }
        match command {
            // Replayed through the same code path as a click.
            101 => {
                // The label setters in `Msg::Primary` record their damage; the
                // host flushes it when this callback returns.
                self.apply(Msg::Primary, window);
            }
            102 => {
                self.shuffle = !self.shuffle;
                self.ui
                    .set_context_menu(widget, Some(primary_menu(self.shuffle, self.gamepad_enabled)));
                self.show_toast(
                    format!("随机播放：{}", if self.shuffle { "开" } else { "关" }),
                    window,
                );
            }
            112 => {
                self.gamepad_enabled = !self.gamepad_enabled;
                if let Some(gamepad) = self.gamepad.as_mut() {
                    gamepad.set_enabled(self.gamepad_enabled);
                }
                self.ui.set_context_menu(
                    widget,
                    Some(primary_menu(self.shuffle, self.gamepad_enabled)),
                );
                self.show_toast(
                    format!(
                        "手柄导航：{}",
                        if self.gamepad_enabled { "开" } else { "关" }
                    ),
                    window,
                );
            }
            103 => self.show_toast(format!("当前次数 {}", self.counter), window),
            104 => {
                self.counter = 0;
                self.ui.set_label(WidgetId::new("primary"), "主按钮");
                self.ui.set_label(WidgetId::new("stat2"), "0");
                self.show_toast("计数已重置".to_string(), window);
            }
            // Modal: the owner window is disabled until the dialog closes.
            106 => {
                window.show_dialog(
                    MessageDialog::new("这是一个模态对话框。\n关闭之前主窗口不可操作。")
                        .title("模态提示")
                        .icon(MessageIcon::Info)
                        .buttons(MessageButtons::OkCancel),
                );
            }
            // Parallel: no owner, so the application stays usable.
            107 => {
                window.show_dialog(
                    MessageDialog::new("这是一个并行对话框。\n主窗口仍然可以操作。")
                        .title("并行提示")
                        .icon(MessageIcon::Info)
                        .parallel(),
                );
            }
            108 => {
                window.show_dialog(
                    FileDialog::open()
                        .title("打开音频")
                        .filter("音频文件", "*.mp3;*.flac;*.wav;*.m4a")
                        .filter("所有文件", "*.*"),
                );
            }
            111 => {
                window.show_dialog(
                    FileDialog::open()
                        .title("选择多个文件")
                        .filter("音频文件", "*.mp3;*.flac;*.wav;*.m4a")
                        .multi_select(true),
                );
            }
            109 => {
                window.begin_hotkey_capture();
                self.show_toast("按下新的快捷键（Esc 取消）".to_string(), window);
            }
            _ => {}
        }
    }

    /// A command from the native window menu bar. One handler like the popup
    /// menus; `820` keeps the bar's own checkmark in sync and `802` quits.
    fn menu_bar_command(&mut self, command: u32, window: &mut WindowHandle<'_>) {
        if let Some(probe) = self.probe.as_mut()
            && command == PROBE_MENU_BAR_COMMAND
        {
            probe.menu_bar_command_command = Some(command);
        }
        match command {
            802 => window.quit(0),
            820 => self.apply(Msg::Theme, window),
            821 => {
                window.begin_hotkey_capture();
                self.show_toast("按下新的快捷键（Esc 取消）".to_string(), window);
            }
            106 => {
                window.show_dialog(
                    MessageDialog::new("这个对话框使用应用自己的按钮标签与命令 id。")
                        .title("自定义按钮")
                        .instruction("保存本次更改？")
                        .button("保存(&S)", 501)
                        .button("放弃(&D)", 502),
                );
            }
            822 => self.apply(Msg::Tab(2), window),
            823 => self.apply(Msg::Tab(3), window),
            824 => {
                // The bar's F5 accelerator label is a real window shortcut.
                if let Some(probe) = self.probe.as_mut() {
                    probe.menu_hotkey = true;
                }
                window.invalidate_all();
                self.show_toast("已刷新示例（F5）".to_string(), window);
            }
            #[cfg(feature = "m7")]
            840 => {
                let text = self.debug_layout_text();
                self.debug_show("布局快照", text, window);
            }
            #[cfg(feature = "m7")]
            841 => {
                let text = self.debug_widget_text(12);
                self.debug_show("控件树（前 12 个）", text, window);
            }
            #[cfg(feature = "m7")]
            842 => {
                let text = self.debug_frame_text();
                self.debug_show("帧统计", text, window);
            }
            #[cfg(feature = "m7")]
            843 => {
                let text = self.debug_log_text(25);
                self.debug_show("诊断日志（最近 25 条）", text, window);
            }
            #[cfg(feature = "m7")]
            844 => self.debug_reload(window),
            #[cfg(feature = "m7")]
            845 => self.debug_save(window),
            #[cfg(feature = "m7")]
            846 => self.debug_reset(window),
            #[cfg(feature = "m7")]
            847 => {
                let text = self.debug_widget_text(usize::MAX);
                for line in text.lines() {
                    println!("{line}");
                }
                self.show_toast("完整控件树已输出到控制台".to_string(), window);
            }
            _ => self.apply(Msg::Chrome(command), window),
        }
    }

    /// Results of every dialog, correlated by the id returned from
    /// `WindowHandle::show_dialog`.
    fn dialog_finished(
        &mut self,
        id: DialogId,
        result: DialogResult,
        window: &mut WindowHandle<'_>,
    ) {
        if let Some(probe) = self.probe.as_mut() {
            if Some(id) == probe.modal_id {
                probe.modal_result =
                    matches!(result, DialogResult::Message(Some(MessageAnswer::Cancel)));
                return;
            }
            if Some(id) == probe.parallel_id {
                probe.parallel_result = matches!(result, DialogResult::Message(_));
                return;
            }
            if Some(id) == probe.custom_dialog_id {
                probe.custom_dialog_id = None;
                probe.custom_buttons =
                    matches!(result, DialogResult::Message(Some(MessageAnswer::Custom(501))));
                return;
            }
            if Some(id) == probe.picker_id {
                probe.picker_id = None;
                probe.picker = None;
                probe.picker_results += 1;
                return;
            }
        }
        // The gallery picker's "more colors…" window: the picked color and
        // alpha land back in the widget.
        if Some(id) == self.gallery_color_dialog {
            self.gallery_color_dialog = None;
            if let DialogResult::ColorWindow(Some(rgba)) = result {
                let changed = self.ui.edit_widget::<ColorPicker<Msg>>(
                    WidgetId::new("gallery_color"),
                    |picker| picker.set_rgba(rgba),
                );
                if changed {
                    window.a11y_refresh();
                }
                self.show_toast(format!("更多颜色：{}", rgba.display()), window);
            }
            return;
        }
        let text = match result {
            DialogResult::Message(Some(answer)) => {
                format!("对话框：{}", message_answer_label(answer))
            }
            DialogResult::Message(None) => "对话框已取消".to_string(),
            DialogResult::File(Some(path)) => format!("选择：{path}"),
            DialogResult::File(None) => "未选择文件".to_string(),
            DialogResult::Files(Some(paths)) => format!("选择 {} 个文件", paths.len()),
            DialogResult::Files(None) => "未选择文件".to_string(),
            DialogResult::Font(Some(font)) => {
                format!("字体：{} {:.1}pt", font.family, font.size)
            }
            DialogResult::Font(None) => "未选择字体".to_string(),
            DialogResult::Color(Some(color)) => format!("颜色：#{:06X}", color.to_hex()),
            DialogResult::Color(None) => "未选择颜色".to_string(),
            DialogResult::ColorWindow(Some(rgba)) => format!("颜色：{}", rgba.display()),
            DialogResult::ColorWindow(None) => "未选择颜色".to_string(),
        };
        self.show_toast(text, window);
    }

    /// Tray icon: activate shows the window; the menu toggles the theme or
    /// quits. The icon keeps working while the main window is minimized.
    fn tray_event(&mut self, event: TrayEvent, window: &mut WindowHandle<'_>) {
        match event {
            TrayEvent::Action {
                action: TrayAction::Click | TrayAction::DoubleClick | TrayAction::KeySelect,
                ..
            } => window.show(),
            TrayEvent::Action {
                action: TrayAction::RightClick,
                ..
            } => window.show(),
            TrayEvent::Action {
                action: TrayAction::MiddleClick,
                ..
            } => {}
            TrayEvent::Command { command: 401, .. } => window.show(),
            TrayEvent::Command { command: 402, .. } => self.apply(Msg::Theme, window),
            TrayEvent::Command { command: 404, .. } => {
                window.show_tray_balloon("Bamboo Native UI", "这是一条托盘通知。", MessageIcon::Info);
            }
            TrayEvent::Command { command: 403, .. } => window.quit(0),
            TrayEvent::Command { .. } => {}
        }
    }

    fn hotkey(&mut self, hotkey: Hotkey, window: &mut WindowHandle<'_>) {
        window.show();
        self.show_toast(format!("全局快捷键 {}", hotkey.label()), window);
    }

    fn hotkey_captured(&mut self, hotkey: Option<Hotkey>, window: &mut WindowHandle<'_>) {
        if self.self_test {
            if let Some(probe) = self.probe.as_mut() {
                probe.hotkey_captured = hotkey;
            }
            return;
        }
        match hotkey {
            Some(hotkey) => {
                // The app decides whether the captured chord becomes global.
                window.register_hotkey(hotkey);
                self.show_toast(format!("已捕获快捷键 {}", hotkey.label()), window);
            }
            None => self.show_toast("已取消快捷键捕获".to_string(), window),
        }
    }
}

fn message_answer_label(answer: MessageAnswer) -> &'static str {
    match answer {
        MessageAnswer::Ok => "确定",
        MessageAnswer::Cancel => "取消",
        MessageAnswer::Yes => "是",
        MessageAnswer::No => "否",
        MessageAnswer::Retry => "重试",
        MessageAnswer::Abort => "中止",
        MessageAnswer::Ignore => "忽略",
        MessageAnswer::Custom(_) => "自定义按钮",
    }
}

fn find_dialog_window(title: &str) -> Option<HWND> {
    let title: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe { FindWindowW(PCWSTR::null(), PCWSTR(title.as_ptr())).ok() }
}

/// Any visible `#32770` dialog of this process. The font and color choosers
/// have fixed system captions, so their own class is the reliable identifier.
fn find_any_dialog() -> Option<HWND> {
    struct Search {
        process: u32,
        found: Option<HWND>,
    }

    unsafe extern "system" fn callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let search = unsafe { &mut *(lparam.0 as *mut Search) };
        let mut process = 0u32;
        unsafe {
            GetWindowThreadProcessId(hwnd, Some(&mut process));
        }
        if process != search.process || !unsafe { IsWindowVisible(hwnd) }.as_bool() {
            return BOOL(1);
        }
        let mut class = [0u16; 32];
        let len = unsafe { GetClassNameW(hwnd, &mut class) };
        let name = String::from_utf16_lossy(&class[..len.max(0) as usize]);
        if name == "#32770" {
            search.found = Some(hwnd);
            return BOOL(0);
        }
        BOOL(1)
    }

    let mut search = Search {
        process: unsafe { GetCurrentProcessId() },
        found: None,
    };
    let _ = unsafe { EnumWindows(Some(callback), LPARAM(&mut search as *mut Search as isize)) };
    search.found
}

fn window_enabled(hwnd: isize) -> bool {
    unsafe { IsWindowEnabled(HWND(hwnd as *mut c_void)).as_bool() }
}

fn close_window(hwnd: HWND) {
    unsafe {
        let _ = PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
    }
}

fn make_rows(playing: Option<usize>) -> Vec<Row> {
    (0..ROWS)
        .map(|index| {
            Row::new(format!("曲目 {:03}", index + 1))
                .subtitle(format!("艺人 {} · 专辑 {}", index % 17, index % 9))
                .trailing(format!("{}:{:02}", 3 + index % 2, (index * 7) % 60))
                .current(playing == Some(index))
        })
        .collect()
}

/// Columns of the gallery table demo (the name column absorbs the width).
fn gallery_table_columns() -> Vec<TableColumn> {
    vec![
        TableColumn::new("编号", 64.0),
        TableColumn::flex("曲名"),
        TableColumn::new("时长", 72.0).align(TextAlign::Right),
    ]
}

/// Thirty rows so the table's viewport both scrolls (its bar is visible) and
/// virtualizes: only the intersecting rows are painted and published.
fn gallery_table_rows() -> Vec<TableRow> {
    let names = [
        "晨雾", "竹韵", "夜曲", "远山", "溪流", "微风", "旧信", "灯影", "归途", "星屑", "薄暮",
        "青苔",
    ];
    (0..30)
        .map(|index| {
            // Row 3 carries a two-line title so the inline editor can be seen
            // using the multiline model (Enter inserts a line, Ctrl+Enter
            // commits); every other row stays single-line.
            let name = if index == 2 {
                format!("{}
（现场版）", names[index % names.len()])
            } else {
                names[index % names.len()].to_string()
            };
            TableRow::new([
                format!("{:02}", index + 1),
                name,
                format!("{}:{:02}", 2 + index % 3, (index * 11) % 60),
            ])
        })
        .collect()
}

/// Artwork ids the cover wall references. Registered once on page setup; a
/// later cover component (now-playing card, queue) can share them.
const COVER_IMAGE_BASE: u64 = 10;

/// Gradient tops/bottoms for the stand-in artwork. They are muted so the white
/// title initial stays readable.
const COVER_ART: [(u32, u32); 8] = [
    (0x5B6BA8, 0x222B4C),
    (0x3F7F6E, 0x18302B),
    (0xA86A4A, 0x3D2118),
    (0x7A4B82, 0x2B1833),
    (0x4A7FA6, 0x1A2E3D),
    (0x9A5B6E, 0x3A1D26),
    (0x5C8A5E, 0x20301F),
    (0x6B5C9A, 0x251F3D),
];

/// Track length of the demo waveform in seconds.
const COVER_TRACK_SECONDS: f32 = 298.0;

/// `"1:35 / 4:58"` for the waveform's position label.
fn cover_time_text(progress: f32) -> String {
    let position = (progress.clamp(0.0, 1.0) * COVER_TRACK_SECONDS).floor() as u32;
    let total = COVER_TRACK_SECONDS as u32;
    format!(
        "{}:{:02} / {}:{:02}",
        position / 60,
        position % 60,
        total / 60,
        total % 60
    )
}

/// The transport strip's status line: playback, shuffle and repeat state.
fn transport_state_text(playing: bool, shuffle: bool, repeat: RepeatMode) -> String {
    let repeat = if repeat.active() {
        format!("循环：{}", repeat.label())
    } else {
        "顺序播放".to_string()
    };
    format!(
        "{} · {}{}",
        if playing { "播放中" } else { "已暂停" },
        if shuffle { "随机 · " } else { "" },
        repeat
    )
}

/// A deterministic synthetic waveform: a slow envelope under three fast
/// components, so the seek bar has a recognizable shape without an audio file.
fn gallery_waveform() -> Vec<f32> {
    (0..240)
        .map(|index| {
            let i = index as f32;
            let envelope = 0.45 + 0.35 * (i * 0.045).sin();
            let body = 0.5
                + 0.28 * (i * 0.53).sin()
                + 0.18 * (i * 1.31).sin()
                + 0.12 * (i * 2.77).cos();
            (envelope * body).abs().clamp(0.04, 1.0)
        })
        .collect()
}

/// Covers for the wall: albums with gradient stand-in artwork, plus a few
/// without an image so the placeholder path is visible too.
fn gallery_covers() -> Vec<Cover> {
    let albums = [
        ("夜曲", "周杰伦"),
        ("晨雾", "林向晚"),
        ("竹韵", "尺八社"),
        ("远山", "旅行团"),
        ("溪流", "白沙滩"),
        ("微风", "海边诗人"),
        ("旧信", "陈年"),
        ("灯影", "夜航船"),
        ("归途", "北纬四十"),
        ("星屑", "天文台"),
        ("薄暮", "日落以后"),
        ("青苔", "雨林计划"),
        ("水平线", "远洋"),
        ("候鸟", "季节线"),
        ("木纹", "年轮"),
        ("雪落", "北方以北"),
        ("回声", "空谷"),
        ("晚风", "巷口"),
        ("灰蓝", "梅雨天"),
        ("雨季", "八月"),
        ("拾光", "胶片盒"),
        ("浮岛", "潮汐"),
        ("纸飞机", "天台"),
        ("余温", "炉火"),
    ];
    albums
        .iter()
        .enumerate()
        .map(|(index, (title, artist))| {
            let mut cover = Cover::new(*title).subtitle(*artist);
            // The last four paint the gradient placeholder (no artwork yet).
            if index < albums.len() - 4 {
                cover = cover.image(ImageId(COVER_IMAGE_BASE + (index as u64 % COVER_ART.len() as u64)));
            }
            cover
        })
        .collect()
}

/// The playlist demo: an album queue. The first track is the playing one; a
/// drag (or Alt+Up/Down) moves a track and the application applies the move to
/// this vector, then pushes the new order back into the widget.
fn gallery_playlist() -> Vec<Row> {
    let tracks = [
        ("夜曲", "周杰伦", "4:58"),
        ("晨雾", "林向晚", "3:42"),
        ("竹韵", "尺八社", "5:06"),
        ("远山", "旅行团", "4:12"),
        ("溪流", "白沙滩", "3:57"),
        ("微风", "海边诗人", "2:48"),
        ("旧信", "陈年", "4:33"),
        ("灯影", "夜航船", "3:21"),
        ("归途", "北纬四十", "4:05"),
        ("星屑", "天文台", "5:24"),
        ("薄暮", "日落以后", "3:36"),
        ("青苔", "雨林计划", "4:47"),
    ];
    tracks
        .iter()
        .enumerate()
        .map(|(index, (title, artist, duration))| {
            Row::new(*title)
                .subtitle(*artist)
                .trailing(*duration)
                .current(index == 0)
        })
        .collect()
}

/// The ribbon demo: four tabs of grouped commands. Commands are app ids.
fn gallery_ribbon_tabs() -> Vec<RibbonTab> {
    vec![
        RibbonTab::new("开始(&K)")
            .group(
                RibbonGroup::new("剪贴板")
                    .item(RibbonItem::large(900, "粘贴").glyph('\u{E77F}'))
                    .item(RibbonItem::small(901, "剪切"))
                    .item(RibbonItem::small(902, "复制")),
            )
            .group(
                RibbonGroup::new("播放")
                    .item(RibbonItem::large(903, "播放").glyph('\u{E768}'))
                    .item(RibbonItem::small(904, "暂停"))
                    .item(RibbonItem::small(905, "下一曲"))
                    .item(RibbonItem::small(906, "上一曲")),
            )
            .group(
                RibbonGroup::new("库")
                    .item(RibbonItem::small(907, "收藏").checked(true))
                    .item(RibbonItem::small(908, "设置")),
            ),
        RibbonTab::new("插入(&I)")
            .group(
                RibbonGroup::new("封面")
                    .item(RibbonItem::large(910, "添加封面").glyph('\u{E91B}'))
                    .item(RibbonItem::small(911, "移除封面")),
            )
            .group(RibbonGroup::new("标签").item(RibbonItem::large(912, "编辑标签"))),
        RibbonTab::new("播放(&P)")
            .group(
                RibbonGroup::new("模式")
                    .item(RibbonItem::small(920, "随机播放").checked(true))
                    .item(RibbonItem::small(921, "单曲循环")),
            )
            .group(
                RibbonGroup::new("音量").item(RibbonItem::large(922, "静音").glyph('\u{E74F}')),
            ),
        RibbonTab::new("视图(&V)").group(
            RibbonGroup::new("布局")
                .item(RibbonItem::small(930, "紧凑布局"))
                .item(RibbonItem::small(931, "宽松布局"))
                .item(RibbonItem::large(932, "全屏").glyph('\u{E740}')),
        ),
    ]
}

/// Display label of a ribbon command id.
fn ribbon_command_label(command: u32) -> &'static str {
    match command {
        900 => "粘贴",
        901 => "剪切",
        902 => "复制",
        903 => "播放",
        904 => "暂停",
        905 => "下一曲",
        906 => "上一曲",
        907 => "收藏",
        908 => "设置",
        910 => "添加封面",
        911 => "移除封面",
        912 => "编辑标签",
        920 => "随机播放",
        921 => "单曲循环",
        922 => "静音",
        930 => "紧凑布局",
        931 => "宽松布局",
        932 => "全屏",
        _ => "命令",
    }
}

/// Human-readable tree path (`1.2.3`) for the status toasts.
fn path_text(path: &[usize]) -> String {
    path.iter()
        .map(|index| (index + 1).to_string())
        .collect::<Vec<_>>()
        .join(".")
}

/// The tree demo: a music library with two expanded branches, a disabled
/// entry and a dozen visible rows (more than the viewport holds).
fn gallery_tree_items() -> Vec<TreeItem> {
    vec![
        TreeItem::new("音乐库")
            .glyph('\u{E8B7}')
            .secondary("12 项")
            .expanded(true)
            .child(
                TreeItem::new("专辑")
                    .glyph('\u{E8B7}')
                    .secondary("4 首")
                    .expanded(true)
                    .child(TreeItem::new("夜曲").glyph('\u{E8D6}').secondary("3:32"))
                    .child(TreeItem::new("竹韵").glyph('\u{E8D6}').secondary("4:05"))
                    .child(TreeItem::new("晨雾").glyph('\u{E8D6}').secondary("2:00"))
                    .child(TreeItem::new("远山").glyph('\u{E8D6}').secondary("3:48")),
            )
            .child(
                TreeItem::new("歌手")
                    .glyph('\u{E77B}')
                    .secondary("3 位")
                    .expanded(true)
                    .child(TreeItem::new("竹韵").glyph('\u{E8D6}'))
                    .child(TreeItem::new("夜曲").glyph('\u{E8D6}'))
                    .child(TreeItem::new("晨雾").glyph('\u{E8D6}')),
            )
            .child(TreeItem::new("播放列表").glyph('\u{E8FD}').disabled(true)),
        TreeItem::new("下载")
            .glyph('\u{E896}')
            .secondary("1 项")
            .child(TreeItem::new("正在下载").glyph('\u{E8D6}')),
    ]
}

/// The property-grid demo's data: five sections of file, audio, tag, playback
/// and provenance metadata. The rows cover every editor kind (text, number,
/// choice, check and rating) and keep a read-only row next to them.
fn gallery_property_sections() -> Vec<PropertySection> {
    vec![
        PropertySection::new("文件")
            .row(PropertyRow::new("名称", "Demo Track.flac").text())
            .row(PropertyRow::new("大小", "42.3 MB"))
            .row(PropertyRow::new("路径", "/音乐/Night Drive/Demo Track.flac"))
            .row(PropertyRow::new("修改时间", "2026-11-20 21:14")),
        PropertySection::new("音频")
            .row(PropertyRow::new("时长", "3:45"))
            .row(PropertyRow::new("码率", "1,411 kbps"))
            .row(PropertyRow::new("采样率", "44.1 kHz"))
            .row(PropertyRow::new("声道", "立体声"))
            .row(PropertyRow::new("编码", "FLAC")),
        PropertySection::new("标签")
            .row(PropertyRow::new("标题", "Demo Track"))
            .row(PropertyRow::new("艺术家", "WiFi Left"))
            .row(PropertyRow::new("专辑", "Night Drive"))
            .row(PropertyRow::new("年份", "2026"))
            .row(PropertyRow::new("收藏", "是").check("是", "否"))
            .row(PropertyRow::new("评分", "4").rating(5)),
        PropertySection::new("播放")
            .row(PropertyRow::new("播放次数", "128").number(0.0, 999_999.0))
            .row(PropertyRow::new("音量", "80").number_with(0.0, 100.0, 5.0, 0))
            .row(PropertyRow::new("最近播放", "昨天 22:03"))
            .row(PropertyRow::new("音质", "无损").choice(["无损", "高品质", "标准"])),
        PropertySection::new("来源")
            .row(PropertyRow::new("校验", "MD5 4f2c…9a81"))
            .row(PropertyRow::new("指纹", "1a2b3c4d"))
            .row(PropertyRow::new("许可", "CC BY 4.0")),
    ]
}

/// Map a client X reported by a scroll-child control (a `Splitter` drag) to the
/// panel's content X. A scroll child's client rect is
/// `content_origin + content_rect - scroll_offset`, so the inverse is
/// `client - content_origin + scroll_offset`.
fn content_x_from_client(area: Rect, offset: Point, client: f32) -> f32 {
    client - area.left + offset.x
}

#[cfg(test)]
mod id_tests {
    use super::*;

    #[test]
    fn showcase_widget_ids_are_unique() {
        let zero = || Arc::new(AtomicU64::new(0));
        let host = Showcase::new(
            false,
            false,
            false,
            None,
            zero(),
            zero(),
            zero(),
            zero(),
            Arc::new(Mutex::new(None)),
            Arc::new(Mutex::new(None)),
        );
        let ids: Vec<&str> = host.ui.ids().map(|id| id.0).collect();
        let mut unique = ids.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), ids.len(), "duplicate widget ids in the tree");
        assert_eq!(
            ids.iter().filter(|id| **id == "gallery_empty").count(),
            1,
            "one empty state"
        );
    }

    // Regression: the splitter reports a client X, but the pane rectangles are
    // content coordinates. Missing the panel origin made a drag jump right by
    // the sidebar width; the inverse mapping keeps the divider under the
    // pointer (with the press grab offset preserved by the widget).
    #[test]
    fn a_splitter_client_x_maps_back_to_content_space() {
        // The 组件 page panel starts at client x = 230 (208 sidebar + 22 pad).
        let area = Rect::from_xywh(230.0, 58.0, 900.0, 700.0);
        assert_eq!(
            content_x_from_client(area, Point::new(0.0, 0.0), 558.0),
            328.0
        );
        // A horizontal scroll adds to the content coordinate.
        assert_eq!(
            content_x_from_client(area, Point::new(5.0, 40.0), 558.0),
            333.0
        );
    }

    // The builder macros live in the library but expand in the caller's crate,
    // so an application widget gets the same `.corner_radius`/`.on_click`
    // surface as a built-in. This test is a cross-crate check of that seam.
    #[test]
    fn the_public_builder_macros_work_from_another_crate() {
        struct Chip<M> {
            surface: bamboo_native_ui::SurfaceStyle,
            observers: bamboo_native_ui::Observers<M>,
        }
        bamboo_native_ui::surface_builders!(Chip);
        bamboo_native_ui::observer_builders!(Chip);

        let chip: Chip<&'static str> = Chip {
            surface: bamboo_native_ui::SurfaceStyle::new(),
            observers: bamboo_native_ui::Observers::new(),
        }
        .corner_radius(8.0)
        .on_click(|_pos| Some("clicked"));
        assert_eq!(chip.surface().corners, Corners::uniform(8.0));
        assert_eq!(
            chip.observers.click(Point::new(1.0, 2.0)),
            Some("clicked")
        );
    }
}
