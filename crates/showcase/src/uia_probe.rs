// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Client-side UI Automation verification for `--a11y-dump`.
//!
//! This is a real UIA client: it asks the window for its provider, walks the
//! control view and asserts names, roles, patterns, values, text ranges and
//! selection. It runs on a worker thread — a client must not call into its own
//! provider from the UI thread that is servicing it.

use std::fmt::Write as _;
use std::sync::{Arc, Mutex};

use windows::core::{BSTR, Interface, implement};
use windows::Win32::System::Variant::{VARIANT, VT_BOOL, VT_I4};
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize,
};
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationExpandCollapsePattern,
    IUIAutomationInvokePattern,
    IUIAutomationEventHandler, IUIAutomationEventHandler_Impl,
    IUIAutomationPropertyChangedEventHandler, IUIAutomationPropertyChangedEventHandler_Impl,
    IUIAutomationRangeValuePattern,
    IUIAutomationScrollPattern, IUIAutomationSelectionItemPattern, IUIAutomationSelectionPattern,
    IUIAutomationTextPattern,
    IUIAutomationTogglePattern, IUIAutomationTreeWalker, IUIAutomationValuePattern,
    IUIAutomationGridPattern, IUIAutomationGridItemPattern,
    ScrollAmount_LargeIncrement, ScrollAmount_NoAmount,
    TreeScope_Subtree,
    ToggleState_Off, ToggleState_On, UIA_EVENT_ID, UIA_PROPERTY_ID,
    UIA_SelectionItem_ElementSelectedEventId, UIA_ToggleToggleStatePropertyId,
    UIA_ButtonControlTypeId, UIA_CheckBoxControlTypeId, UIA_ComboBoxControlTypeId,
    UIA_DataGridControlTypeId, UIA_DataItemControlTypeId,
    UIA_EditControlTypeId,
    UIA_GridItemPatternId, UIA_GridPatternId,
    UIA_HeaderItemControlTypeId,
    UIA_GroupControlTypeId, UIA_InvokePatternId, UIA_ListItemControlTypeId, UIA_ListControlTypeId,
    UIA_NotificationEventId, UIA_PaneControlTypeId, UIA_ProgressBarControlTypeId,
    UIA_RadioButtonControlTypeId,
    UIA_RangeValuePatternId, UIA_ScrollPatternId, UIA_SelectionItemPatternId,
    UIA_SelectionPatternId,
    ExpandCollapseState_Expanded, ExpandCollapseState_LeafNode, UIA_ExpandCollapsePatternId,
    UIA_MenuBarControlTypeId,
    UIA_MenuControlTypeId,
    UIA_MenuItemControlTypeId, UIA_SeparatorControlTypeId, UIA_SliderControlTypeId,
    UIA_StatusBarControlTypeId, UIA_TabControlTypeId, UIA_TabItemControlTypeId,
    UIA_TextControlTypeId, UIA_TextPatternId, UIA_ToolBarControlTypeId,
    UIA_TreeControlTypeId, UIA_TreeItemControlTypeId,
    UIA_TogglePatternId, UIA_ValuePatternId, UIA_WindowControlTypeId, SupportedTextSelection_Single,
};

const TITLE: &str = "Bamboo Native UI — Showcase";
const SEARCH_TEXT: &str = "Bamboo Native UI";

/// Run the client probe and return a human-readable dump of the tree.
pub fn run(hwnd: isize) -> Result<String, String> {
    unsafe {
        let init = CoInitializeEx(None, COINIT_MULTITHREADED);
        init.ok()
            .map_err(|error| format!("CoInitializeEx: {error}"))?;
        let result = probe(hwnd);
        CoUninitialize();
        result
    }
}

/// Collects automation events (selection changes) delivered to a client.
#[implement(IUIAutomationEventHandler)]
struct EventSpy {
    events: Arc<Mutex<Vec<i32>>>,
}

impl IUIAutomationEventHandler_Impl for EventSpy_Impl {
    fn HandleAutomationEvent(
        &self,
        _sender: windows::core::Ref<IUIAutomationElement>,
        eventid: UIA_EVENT_ID,
    ) -> windows::core::Result<()> {
        if let Ok(mut events) = self.events.lock() {
            events.push(eventid.0);
        }
        Ok(())
    }
}

/// Collects property-change events so the probe can prove that a state change
/// (not just the value returned by a pattern) reached a UIA client.
#[implement(IUIAutomationPropertyChangedEventHandler)]
struct PropertySpy {
    property: i32,
    values: Arc<Mutex<Vec<i32>>>,
}

impl IUIAutomationPropertyChangedEventHandler_Impl for PropertySpy_Impl {
    fn HandlePropertyChangedEvent(
        &self,
        _sender: windows::core::Ref<IUIAutomationElement>,
        propertyid: UIA_PROPERTY_ID,
        newvalue: &VARIANT,
    ) -> windows::core::Result<()> {
        if propertyid.0 == self.property {
            let value = unsafe {
                let inner = &newvalue.Anonymous.Anonymous;
                match inner.vt {
                    VT_I4 => inner.Anonymous.lVal,
                    VT_BOOL => inner.Anonymous.boolVal.0 as i32,
                    _ => i32::MIN,
                }
            };
            if let Ok(mut values) = self.values.lock() {
                values.push(value);
            }
        }
        Ok(())
    }
}

#[derive(Default)]
struct Seen {
    element: Option<IUIAutomationElement>,
    id: String,
    name: String,
    description: Option<String>,
    control: i32,
    value: Option<String>,
    text: Option<String>,
    toggled: Option<bool>,
    range: Option<f64>,
    invoke: bool,
    selection: Option<i32>,
    /// Container SelectionPattern.CanSelectMultiple.
    multi: Option<bool>,
    selected_item: bool,
    focused: bool,
    offscreen: bool,
    supported_selection: Option<i32>,
    selection_ranges: Option<i32>,
    selection_text: Option<String>,
    /// ExpandCollapseState is Expanded (a menu button, a menu bar).
    expanded: bool,
    /// The element exposes the ExpandCollapse pattern at all (a tree row).
    expandable: bool,
    /// `ExpandCollapseState` as the provider reported it (a leaf row reports
    /// `LeafNode`; an accordion section reports Expanded/Collapsed).
    expand_state: Option<i32>,
    /// Scroll pattern values; `None` when the element does not scroll.
    scroll: Option<ScrollState>,
    /// IGridProvider extent on a grid container, IGridItemProvider
    /// coordinates on a cell.
    grid_rows: Option<i32>,
    grid_columns: Option<i32>,
    grid_row: Option<i32>,
    grid_column: Option<i32>,
}

/// Where a scrollable element sits, as a UIA client sees it.
#[derive(Clone, Copy, Debug, Default)]
struct ScrollState {
    horizontal_scrollable: bool,
    vertical_scrollable: bool,
    horizontal_percent: f64,
    vertical_percent: f64,
    vertical_view: f64,
}

fn control_name(control: i32) -> &'static str {
    if control == UIA_WindowControlTypeId.0 {
        "window"
    } else if control == UIA_ButtonControlTypeId.0 {
        "button"
    } else if control == UIA_CheckBoxControlTypeId.0 {
        "checkbox"
    } else if control == UIA_EditControlTypeId.0 {
        "edit"
    } else if control == UIA_GroupControlTypeId.0 {
        "group"
    } else if control == UIA_PaneControlTypeId.0 {
        "pane"
    } else if control == UIA_ListControlTypeId.0 {
        "list"
    } else if control == UIA_ListItemControlTypeId.0 {
        "list item"
    } else if control == UIA_TreeControlTypeId.0 {
        "tree"
    } else if control == UIA_TreeItemControlTypeId.0 {
        "tree item"
    } else if control == UIA_DataGridControlTypeId.0 {
        "data grid"
    } else if control == UIA_DataItemControlTypeId.0 {
        "data item"
    } else if control == UIA_HeaderItemControlTypeId.0 {
        "header item"
    } else if control == UIA_RadioButtonControlTypeId.0 {
        "radio"
    } else if control == UIA_SeparatorControlTypeId.0 {
        "separator"
    } else if control == UIA_SliderControlTypeId.0 {
        "slider"
    } else if control == UIA_TabControlTypeId.0 {
        "tab"
    } else if control == UIA_TabItemControlTypeId.0 {
        "tab item"
    } else if control == UIA_TextControlTypeId.0 {
        "text"
    } else if control == UIA_ProgressBarControlTypeId.0 {
        "progress bar"
    } else if control == UIA_MenuBarControlTypeId.0 {
        "menu bar"
    } else if control == UIA_MenuControlTypeId.0 {
        "menu"
    } else if control == UIA_MenuItemControlTypeId.0 {
        "menu item"
    } else if control == UIA_ToolBarControlTypeId.0 {
        "toolbar"
    } else if control == UIA_StatusBarControlTypeId.0 {
        "status bar"
    } else {
        "other"
    }
}

unsafe fn probe(hwnd: isize) -> Result<String, String> {
    let automation: IUIAutomation = unsafe {
        CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)
    }
    .map_err(|error| format!("CoCreateInstance(CUIAutomation): {error}"))?;
    let root = unsafe { automation.ElementFromHandle(HWND(hwnd as *mut core::ffi::c_void)) }
        .map_err(|error| format!("ElementFromHandle: {error}"))?;
    let walker = unsafe { automation.ControlViewWalker() }
        .map_err(|error| format!("ControlViewWalker: {error}"))?;

    let mut seen = Vec::new();
    let mut dump = String::new();
    unsafe { collect(&walker, &root, 0, &mut seen, &mut dump)? };

    // Focus is informational: the snapshot can be built before the first
    // focus event reaches the client.
    let focus = unsafe { automation.GetFocusedElement() }
        .ok()
        .and_then(|element| unsafe { element.CurrentAutomationId() }.ok())
        .map(|id| id.to_string())
        .unwrap_or_else(|| "none".to_string());
    let _ = writeln!(dump, "focus={focus}");

    if let Err(error) = check(&seen) {
        return Err(format!("{error}
--- UIA tree ---
{dump}"));
    }
    if let Err(error) = invoke_round_trip(&automation, &root, &seen) {
        return Err(format!("{error}
--- UIA tree ---
{dump}"));
    }
    let _ = writeln!(dump, "actions=invoke+notification-ok");
    if let Err(error) = toggle_round_trip(&automation, &seen) {
        return Err(format!("{error}
--- UIA tree ---
{dump}"));
    }
    let _ = writeln!(dump, "toggle-property-event=ok");
    if let Err(error) = focus_round_trip(&automation, hwnd, &seen) {
        return Err(format!("{error}
--- UIA tree ---
{dump}"));
    }
    let _ = writeln!(dump, "focus-routing=ok");
    if let Err(error) = flyout_round(&automation, &root, &seen) {
        return Err(format!("{error}
--- UIA tree ---
{dump}"));
    }
    let _ = writeln!(dump, "flyout=ok");
    if let Err(error) = gallery_round(&automation, &root) {
        return Err(format!("{error}
--- UIA tree ---
{dump}"));
    }
    let _ = writeln!(dump, "gallery=ok");
    if let Err(error) = data_round(&automation, &root) {
        return Err(format!("{error}
--- UIA tree ---
{dump}"));
    }
    let _ = writeln!(dump, "data=ok");
    if let Err(error) = covers_round(&automation, &root) {
        return Err(format!("{error}
--- UIA tree ---
{dump}"));
    }
    let _ = writeln!(dump, "covers=ok");
    if let Err(error) = ribbon_round(&automation, &root) {
        return Err(format!("{error}
--- UIA tree ---
{dump}"));
    }
    let _ = writeln!(dump, "ribbon=ok");
    if let Err(error) = props_round(&automation, &root) {
        return Err(format!("{error}
--- UIA tree ---
{dump}"));
    }
    let _ = writeln!(dump, "props=ok");
    if let Err(error) = text_round(&automation, &root) {
        return Err(format!("{error}
--- UIA tree ---
{dump}"));
    }
    let _ = writeln!(dump, "text=ok");
    #[cfg(feature = "m7")]
    if let Err(error) = layout_round(&automation, &root) {
        return Err(format!("{error}
--- UIA tree ---
{dump}"));
    }
    #[cfg(feature = "m7")]
    let _ = writeln!(dump, "layout=ok");
    let _ = writeln!(dump, "modal=ok");
    Ok(dump)
}

/// Switch to the "封面" tab and check the cover wall: a `List` over visible
/// `ListItem` covers with exactly one selected, a working SelectionItem, and a
/// Scroll pattern that reaches the end of the wall.
fn covers_round(
    automation: &IUIAutomation,
    root: &IUIAutomationElement,
) -> Result<(), String> {
    let walker = unsafe { automation.ControlViewWalker() }
        .map_err(|error| format!("covers ControlViewWalker: {error}"))?;
    let collect_page = |walker: &IUIAutomationTreeWalker| -> Result<Vec<Seen>, String> {
        let mut page = Vec::new();
        let mut dump = String::new();
        unsafe { collect(walker, root, 0, &mut page, &mut dump)? };
        Ok(page)
    };
    let (page, _) = select_tab(automation, root, "封面")?;

    let wall = find(&page, "covers-wall").ok_or("missing cover wall")?;
    require(
        wall.control == UIA_ListControlTypeId.0
            && wall.name == "封面墙"
            && wall.selection == Some(1),
        format!(
            "cover wall {:?} name {:?} selection {:?}",
            wall.control, wall.name, wall.selection
        ),
    )?;
    require(
        wall.scroll
            .is_some_and(|state| state.vertical_scrollable && state.vertical_percent < 1.0),
        "the cover wall must start at the top of its Scroll range",
    )?;
    let covers: Vec<&Seen> = page
        .iter()
        .filter(|entry| {
            entry.id == "covers-wall" && entry.control == UIA_ListItemControlTypeId.0
        })
        .collect();
    require(covers.len() >= 6, format!("{} cover items", covers.len()))?;
    require(
        covers
            .iter()
            .any(|entry| entry.name == "夜曲 — 周杰伦"),
        format!(
            "cover names: {:?}",
            covers.iter().map(|entry| &entry.name).collect::<Vec<_>>()
        ),
    )?;
    require(
        covers.iter().filter(|entry| entry.selected_item).count() == 1,
        "exactly one cover must be selected",
    )?;

    // Select the second visible cover through its SelectionItem pattern and
    // require the selected flag to move there.
    let target = covers
        .get(1)
        .ok_or("the wall published fewer than two covers")?;
    let name = target.name.clone();
    let element = target.element.as_ref().ok_or("cover element gone")?;
    let sink: IUIAutomationSelectionItemPattern =
        unsafe { element.GetCurrentPattern(UIA_SelectionItemPatternId) }
            .map_err(|error| format!("cover SelectionItemPattern: {error}"))?
            .cast()
            .map_err(|error| format!("cover cast: {error}"))?;
    unsafe { sink.Select() }.map_err(|error| format!("cover Select: {error}"))?;
    std::thread::sleep(std::time::Duration::from_millis(300));
    let after = collect_page(&walker)?;
    let selected: Vec<&Seen> = after
        .iter()
        .filter(|entry| {
            entry.id == "covers-wall"
                && entry.control == UIA_ListItemControlTypeId.0
                && entry.selected_item
        })
        .collect();
    require(
        selected.len() == 1 && selected[0].name == name,
        format!(
            "selected covers after Select: {:?}",
            selected.iter().map(|entry| &entry.name).collect::<Vec<_>>()
        ),
    )?;

    // Drive the wall's Scroll pattern to the end and require the offset to
    // follow (the same axis state the bar paints from).
    let wall = find(&after, "covers-wall").ok_or("missing cover wall after select")?;
    let pattern = scroll_pattern(wall)?;
    // -1 is UIA's "no scroll on this axis".
    unsafe { pattern.SetScrollPercent(-1.0, 100.0) }
        .map_err(|error| format!("cover wall SetScrollPercent: {error}"))?;
    std::thread::sleep(std::time::Duration::from_millis(300));
    let end = collect_page(&walker)?;
    let wall = find(&end, "covers-wall").ok_or("missing cover wall at the end")?;
    let state = wall.scroll.ok_or("the cover wall lost its Scroll pattern")?;
    require(
        state.vertical_scrollable && state.vertical_percent > 99.0,
        format!("cover wall position after scrolling: {state:?}"),
    )?;

    // The playlist beside the wall is a second List: visible track rows, one
    // selected, a working SelectionItem and its own Scroll range.
    let playlist = find(&end, "covers-playlist").ok_or("missing playlist")?;
    require(
        playlist.control == UIA_ListControlTypeId.0
            && playlist.name == "播放列表"
            && playlist.selection == Some(1),
        format!(
            "playlist {:?} name {:?} selection {:?}",
            playlist.control, playlist.name, playlist.selection
        ),
    )?;
    require(
        playlist
            .scroll
            .is_some_and(|state| state.vertical_scrollable && state.vertical_percent < 1.0),
        "the playlist must start at the top of its Scroll range",
    )?;
    let tracks: Vec<&Seen> = end
        .iter()
        .filter(|entry| {
            entry.id == "covers-playlist" && entry.control == UIA_ListItemControlTypeId.0
        })
        .collect();
    require(tracks.len() >= 6, format!("{} playlist rows", tracks.len()))?;
    require(
        tracks.iter().any(|entry| entry.name == "夜曲 — 周杰伦"),
        format!(
            "playlist names: {:?}",
            tracks.iter().map(|entry| &entry.name).collect::<Vec<_>>()
        ),
    )?;
    require(
        tracks.iter().filter(|entry| entry.selected_item).count() == 1,
        "exactly one playlist row must be selected",
    )?;
    let target = tracks
        .get(1)
        .ok_or("the playlist published fewer than two rows")?;
    let name = target.name.clone();
    let element = target.element.as_ref().ok_or("playlist row element gone")?;
    let sink: IUIAutomationSelectionItemPattern =
        unsafe { element.GetCurrentPattern(UIA_SelectionItemPatternId) }
            .map_err(|error| format!("playlist SelectionItemPattern: {error}"))?
            .cast()
            .map_err(|error| format!("playlist cast: {error}"))?;
    unsafe { sink.Select() }.map_err(|error| format!("playlist Select: {error}"))?;
    std::thread::sleep(std::time::Duration::from_millis(300));
    let picked = collect_page(&walker)?;
    let selected: Vec<&Seen> = picked
        .iter()
        .filter(|entry| {
            entry.id == "covers-playlist"
                && entry.control == UIA_ListItemControlTypeId.0
                && entry.selected_item
        })
        .collect();
    require(
        selected.len() == 1 && selected[0].name == name,
        format!(
            "selected playlist rows after Select: {:?}",
            selected.iter().map(|entry| &entry.name).collect::<Vec<_>>()
        ),
    )?;
    let playlist = find(&picked, "covers-playlist").ok_or("missing playlist after select")?;
    let pattern = scroll_pattern(playlist)?;
    unsafe { pattern.SetScrollPercent(-1.0, 100.0) }
        .map_err(|error| format!("playlist SetScrollPercent: {error}"))?;
    std::thread::sleep(std::time::Duration::from_millis(300));
    let end = collect_page(&walker)?;
    let playlist = find(&end, "covers-playlist").ok_or("missing playlist at the end")?;
    let state = playlist.scroll.ok_or("the playlist lost its Scroll pattern")?;
    require(
        state.vertical_scrollable && state.vertical_percent > 99.0,
        format!("playlist position after scrolling: {state:?}"),
    )?;

    // The waveform seek bar is a Slider with a 0..=1 RangeValue; SetValue must
    // move it (and the app's time label with it).
    let seek = find(&end, "covers-waveform").ok_or("missing waveform seek bar")?;
    require(
        seek.control == UIA_SliderControlTypeId.0,
        format!("seek bar control {:?}", seek.control),
    )?;
    let value = seek.range.ok_or("the seek bar has no RangeValue")?;
    require(
        (value - 0.32).abs() < 0.01,
        format!("seek bar initial value {value}"),
    )?;
    let element = seek.element.as_ref().ok_or("seek bar element gone")?;
    let pattern: IUIAutomationRangeValuePattern =
        unsafe { element.GetCurrentPattern(UIA_RangeValuePatternId) }
            .map_err(|error| format!("seek bar RangeValuePattern: {error}"))?
            .cast()
            .map_err(|error| format!("seek bar cast: {error}"))?;
    unsafe { pattern.SetValue(0.5) }.map_err(|error| format!("seek bar SetValue: {error}"))?;
    std::thread::sleep(std::time::Duration::from_millis(300));
    let seeked = collect_page(&walker)?;
    let seek = find(&seeked, "covers-waveform").ok_or("missing seek bar after SetValue")?;
    let value = seek.range.ok_or("the seek bar lost its RangeValue")?;
    require(
        (value - 0.5).abs() < 0.01,
        format!("seek bar value after SetValue: {value}"),
    )?;
    let time = find(&seeked, "covers_time").ok_or("missing the time label")?;
    require(
        time.name.starts_with("2:29 / 4:58"),
        format!("time label after seeking: {:?}", time.name),
    )?;

    // The transport strip is a ToolBar of five buttons; shuffle and repeat are
    // toggles. Invoking play must flip the application's playback state (the
    // button renames to 暂停) and the status label must follow.
    let transport = find(&seeked, "covers-transport").ok_or("missing the transport strip")?;
    require(
        transport.control == UIA_ToolBarControlTypeId.0,
        format!("transport control {:?}", transport.control),
    )?;
    let cells: Vec<&Seen> = seeked
        .iter()
        .filter(|entry| entry.id == "covers-transport" && entry.control != UIA_ToolBarControlTypeId.0)
        .collect();
    require(cells.len() == 5, format!("{} transport cells", cells.len()))?;
    let names: Vec<&str> = cells.iter().map(|entry| entry.name.as_str()).collect();
    require(
        names.as_slice() == ["随机播放", "上一曲", "播放", "下一曲", "循环播放（关）"],
        format!("transport cell names: {names:?}"),
    )?;
    require(
        cells[0].toggled == Some(false) && cells[4].toggled == Some(false),
        "shuffle and repeat must start off",
    )?;
    let play = cells[cells.iter().position(|entry| entry.name == "播放").ok_or("missing the play cell")?]
        .element
        .as_ref()
        .ok_or("play element gone")?;
    let invoke: IUIAutomationInvokePattern =
        unsafe { play.GetCurrentPattern(UIA_InvokePatternId) }
            .map_err(|error| format!("play InvokePattern: {error}"))?
            .cast()
            .map_err(|error| format!("play cast: {error}"))?;
    unsafe { invoke.Invoke() }.map_err(|error| format!("play Invoke: {error}"))?;
    std::thread::sleep(std::time::Duration::from_millis(300));
    let played = collect_page(&walker)?;
    require(
        played
            .iter()
            .any(|entry| entry.id == "covers-transport" && entry.name == "暂停"),
        "the play cell must rename to 暂停",
    )?;
    let state = find(&played, "covers_state").ok_or("missing the transport state label")?;
    require(
        state.name.starts_with("播放中"),
        format!("transport state after play: {:?}", state.name),
    )?;

    // Toggle shuffle through its Toggle pattern: the state must move and the
    // status label must mention randomization.
    let shuffle = played
        .iter()
        .find(|entry| entry.id == "covers-transport" && entry.name == "随机播放")
        .ok_or("missing the shuffle cell")?;
    let element = shuffle.element.as_ref().ok_or("shuffle element gone")?;
    let toggle: IUIAutomationTogglePattern =
        unsafe { element.GetCurrentPattern(UIA_TogglePatternId) }
            .map_err(|error| format!("shuffle TogglePattern: {error}"))?
            .cast()
            .map_err(|error| format!("shuffle cast: {error}"))?;
    unsafe { toggle.Toggle() }.map_err(|error| format!("shuffle Toggle: {error}"))?;
    std::thread::sleep(std::time::Duration::from_millis(300));
    let shuffled = collect_page(&walker)?;
    let shuffle = shuffled
        .iter()
        .find(|entry| entry.id == "covers-transport" && entry.name == "随机播放")
        .ok_or("missing the shuffle cell after toggling")?;
    require(
        shuffle.toggled == Some(true),
        format!("shuffle toggle state: {:?}", shuffle.toggled),
    )?;
    let state = find(&shuffled, "covers_state").ok_or("missing the state label after shuffle")?;
    require(
        state.name.contains("随机"),
        format!("transport state after shuffle: {:?}", state.name),
    )?;
    Ok(())
}

/// Switch to the "组件" tab and check the input gallery. The gallery widgets are
/// hidden on the first page, so this is the only way to prove their roles
/// reach a client without making the main page unrepresentative.
fn gallery_round(
    automation: &IUIAutomation,
    root: &IUIAutomationElement,
) -> Result<(), String> {
    let walker = unsafe { automation.ControlViewWalker() }
        .map_err(|error| format!("gallery ControlViewWalker: {error}"))?;
    let (gallery, dump) = select_tab(automation, root, "组件")?;
    check_gallery(&gallery).map_err(|error| format!("{error}
--- gallery tree ---
{dump}"))?;
    accordion_round(automation, root, &gallery).map_err(|error| format!("{error}
--- gallery tree ---
{dump}"))?;
    chrome_round(automation, root, &gallery).map_err(|error| format!("{error}
--- gallery tree ---
{dump}"))?;
    // The chrome round runs commands, and the runtime menu edit rebuilds the
    // accessibility snapshot; re-walk so the modal round holds current
    // elements instead of the pre-edit ones.
    let mut refreshed = Vec::new();
    let mut refreshed_dump = String::new();
    unsafe { collect(&walker, root, 0, &mut refreshed, &mut refreshed_dump)? };
    modal_round(automation, root, &refreshed).map_err(|error| format!("{error}
--- refreshed tree ---
{refreshed_dump}"))?;
    Ok(())
}

/// Switch to the "数据" tab and check the virtualized table and the dynamic
/// widget-tree panel. The round then removes the second pool slot through its
/// UIA Invoke button, requires the node to disappear from the refreshed tree,
/// adds it back and requires it to return — the dynamic insertion path end to
/// end through a real client.
fn data_round(
    automation: &IUIAutomation,
    root: &IUIAutomationElement,
) -> Result<(), String> {
    let walker = unsafe { automation.ControlViewWalker() }
        .map_err(|error| format!("data ControlViewWalker: {error}"))?;
    let (page, mut dump) = select_tab(automation, root, "数据")?;
    check_data(&page).map_err(|error| format!("{error}
--- data tree ---
{dump}"))?;

    // A grid cell exposes Value; SetValue edits it like a committed draft.
    let cell = page
        .iter()
        .find(|entry| {
            entry.id == "gallery-table"
                && entry.control == UIA_DataItemControlTypeId.0
                && entry.grid_row == Some(0)
                && entry.grid_column == Some(1)
        })
        .ok_or("missing cell (0, 1)")?;
    let element = cell.element.as_ref().ok_or("cell element gone")?;
    let value_pattern: IUIAutomationValuePattern =
        unsafe { element.GetCurrentPattern(UIA_ValuePatternId) }
            .map_err(|error| format!("cell ValuePattern: {error}"))?
            .cast()
            .map_err(|error| format!("cell ValuePattern cast: {error}"))?;
    let before = unsafe { value_pattern.CurrentValue() }
        .map_err(|error| format!("cell CurrentValue: {error}"))?
        .to_string();
    require(
        before == "晨雾",
        format!("cell (0, 1) value {before:?}"),
    )?;
    require(
        !unsafe { value_pattern.CurrentIsReadOnly() }
            .map_err(|error| format!("cell IsReadOnly: {error}"))?
            .as_bool(),
        "an editable cell must not report read-only",
    )?;
    unsafe { value_pattern.SetValue(&BSTR::from("自检曲")) }
        .map_err(|error| format!("cell SetValue: {error}"))?;
    std::thread::sleep(std::time::Duration::from_millis(350));
    let mut edited = Vec::new();
    unsafe { collect(&walker, root, 0, &mut edited, &mut dump)? };
    let cell = edited
        .iter()
        .find(|entry| {
            entry.id == "gallery-table"
                && entry.control == UIA_DataItemControlTypeId.0
                && entry.grid_row == Some(0)
                && entry.grid_column == Some(1)
        })
        .ok_or("cell (0, 1) vanished after SetValue")?;
    require(
        cell.name == "自检曲",
        format!("cell (0, 1) after SetValue: {:?}", cell.name),
    )?;

    // Remove the later pool slot through its button (the app calls
    // `Ui::remove`), then require the element and its row to be gone.
    let remove = find(&page, "dyn-remove").ok_or("missing dynamic remove button")?;
    require(remove.invoke, "the remove button must support Invoke")?;
    invoke_seen(remove)?;
    std::thread::sleep(std::time::Duration::from_millis(400));
    let mut after = Vec::new();
    unsafe { collect(&walker, root, 0, &mut after, &mut dump)? };
    require(
        find(&after, "dyn-slot-2").is_none(),
        "the removed slot is still in the accessibility tree",
    )?;
    require(
        find(&after, "dyn-slot-1").is_some(),
        "removing one slot dropped the other",
    )?;
    let info = find(&after, "dyn-info").ok_or("missing dyn info after removal")?;
    require(
        info.name == "顺序：1",
        format!("dyn info after removal: {:?}", info.name),
    )?;

    // Add it back: the tree must carry both slots again.
    let add = find(&after, "dyn-add").ok_or("missing dynamic add button")?;
    require(add.invoke, "the add button must support Invoke")?;
    invoke_seen(add)?;
    std::thread::sleep(std::time::Duration::from_millis(400));
    let mut back = Vec::new();
    unsafe { collect(&walker, root, 0, &mut back, &mut dump)? };
    require(
        find(&back, "dyn-slot-2").is_some(),
        "the re-added slot is missing from the tree",
    )?;
    let info = find(&back, "dyn-info").ok_or("missing dyn info after re-add")?;
    require(
        info.name == "顺序：1 → 2",
        format!("dyn info after re-add: {:?}", info.name),
    )?;

    // Tree rows are TreeItems: collapse the root through its pattern and
    // require the child rows to leave the tree, then expand it back. This is
    // the per-node ExpandCollapse a screen reader drives.
    let root_row = find_row(&back, "gallery-tree", "音乐库").ok_or("missing tree root row")?;
    require(
        root_row.expandable && root_row.expanded,
        "the tree root must start expanded",
    )?;
    let pattern = expand_pattern(root_row)?;
    unsafe { pattern.Collapse() }.map_err(|error| format!("tree Collapse: {error}"))?;
    std::thread::sleep(std::time::Duration::from_millis(300));
    let mut collapsed = Vec::new();
    unsafe { collect(&walker, root, 0, &mut collapsed, &mut dump)? };
    let rows = |seen: &[Seen]| -> Vec<String> {
        seen.iter()
            .filter(|entry| {
                entry.id == "gallery-tree" && entry.control == UIA_TreeItemControlTypeId.0
            })
            .map(|entry| entry.name.clone())
            .collect()
    };
    let names = rows(&collapsed);
    require(
        names == ["音乐库", "下载"],
        format!("tree rows after collapsing the root: {names:?}"),
    )?;
    let root_row = find_row(&collapsed, "gallery-tree", "音乐库").ok_or("tree root disappeared")?;
    require(!root_row.expanded, "the collapsed root must report Collapsed")?;
    let pattern = expand_pattern(root_row)?;
    unsafe { pattern.Expand() }.map_err(|error| format!("tree Expand: {error}"))?;
    std::thread::sleep(std::time::Duration::from_millis(300));
    let mut expanded = Vec::new();
    unsafe { collect(&walker, root, 0, &mut expanded, &mut dump)? };
    let names = rows(&expanded);
    require(
        names.len() >= 6 && names.get(1).is_some_and(|name| name == "专辑"),
        format!("tree rows after expanding the root: {names:?}"),
    )?;

    scroll_round(automation, root, &back)?;
    Ok(())
}

/// Drive the Scroll pattern the way a screen reader does: send the demo panel
/// to the end and require a row that was offscreen to come into view, bring it
/// back, then step the table (a widget-owned viewport) by one page.
fn scroll_round(
    automation: &IUIAutomation,
    root: &IUIAutomationElement,
    seen: &[Seen],
) -> Result<(), String> {
    let walker = unsafe { automation.ControlViewWalker() }
        .map_err(|error| format!("scroll ControlViewWalker: {error}"))?;
    let collect_page = |walker: &IUIAutomationTreeWalker| -> Result<Vec<Seen>, String> {
        let mut page = Vec::new();
        let mut dump = String::new();
        unsafe { collect(walker, root, 0, &mut page, &mut dump)? };
        Ok(page)
    };
    let row_visible = |tree: &[Seen], id: &str| {
        tree.iter()
            .any(|entry| entry.id == id && !entry.offscreen)
    };

    require(
        !row_visible(seen, "scroll-row-8"),
        "the last scroll row must start out of view",
    )?;
    let pane = find(seen, "data_scroll").ok_or("missing scroll panel")?;
    let pattern = scroll_pattern(pane)?;
    unsafe { pattern.SetScrollPercent(100.0, 100.0) }
        .map_err(|error| format!("scroll panel SetScrollPercent: {error}"))?;
    std::thread::sleep(std::time::Duration::from_millis(300));
    let end = collect_page(&walker)?;
    let pane = find(&end, "data_scroll").ok_or("missing scroll panel after scrolling")?;
    let state = pane.scroll.ok_or("the panel lost its Scroll pattern")?;
    require(
        state.vertical_percent > 99.0 && state.horizontal_percent > 99.0,
        format!("panel position after scrolling to the end: {state:?}"),
    )?;
    require(
        row_visible(&end, "scroll-row-8"),
        "the last scroll row must be visible at the end",
    )?;

    // Back to the origin: the first row is visible again and a later one is
    // offscreen.
    let pattern = scroll_pattern(pane)?;
    unsafe { pattern.SetScrollPercent(0.0, 0.0) }
        .map_err(|error| format!("scroll panel SetScrollPercent(home): {error}"))?;
    std::thread::sleep(std::time::Duration::from_millis(300));
    let home = collect_page(&walker)?;
    let pane = find(&home, "data_scroll").ok_or("missing scroll panel at home")?;
    let state = pane.scroll.ok_or("the panel lost its Scroll pattern")?;
    require(
        state.vertical_percent < 1.0,
        format!("panel position at home: {state:?}"),
    )?;
    require(
        row_visible(&home, "scroll-row-1"),
        "the first scroll row must be visible at home",
    )?;

    // The table owns its own offset; one large step must move it and the
    // published rows follow.
    let table = find(&home, "gallery-table").ok_or("missing gallery table")?;
    let before = table.scroll.ok_or("the table lost its Scroll pattern")?;
    require(
        before.vertical_percent < 1.0,
        format!("table must start at the top: {before:?}"),
    )?;
    let pattern = scroll_pattern(table)?;
    unsafe { pattern.Scroll(ScrollAmount_NoAmount, ScrollAmount_LargeIncrement) }
        .map_err(|error| format!("table Scroll: {error}"))?;
    std::thread::sleep(std::time::Duration::from_millis(300));
    let after = collect_page(&walker)?;
    let table = find(&after, "gallery-table").ok_or("missing gallery table after scrolling")?;
    let state = table.scroll.ok_or("the table lost its Scroll pattern")?;
    require(
        state.vertical_percent > before.vertical_percent + 1.0,
        format!("table position after one page: {state:?}"),
    )?;
    Ok(())
}

/// The Scroll pattern of one element, as the client would get it.
fn scroll_pattern(entry: &Seen) -> Result<IUIAutomationScrollPattern, String> {    let element = entry.element.as_ref().ok_or("element gone")?;
    unsafe { element.GetCurrentPattern(UIA_ScrollPatternId) }
        .map_err(|error| format!("ScrollPattern: {error}"))?
        .cast()
        .map_err(|error| format!("ScrollPattern cast: {error}"))
}

/// The Grid pattern of one element (its row/column extent and cell lookup).
fn grid_pattern(entry: &Seen) -> Result<IUIAutomationGridPattern, String> {
    let element = entry.element.as_ref().ok_or("element gone")?;
    unsafe { element.GetCurrentPattern(UIA_GridPatternId) }
        .map_err(|error| format!("GridPattern: {error}"))?
        .cast()
        .map_err(|error| format!("GridPattern cast: {error}"))
}

/// One named row of a tree, as the client sees it.
fn find_row<'a>(seen: &'a [Seen], id: &str, name: &str) -> Option<&'a Seen> {
    seen.iter().find(|entry| {
        entry.id == id && entry.control == UIA_TreeItemControlTypeId.0 && entry.name == name
    })
}

/// The ExpandCollapse pattern of one element, as the client would get it.
fn expand_pattern(entry: &Seen) -> Result<IUIAutomationExpandCollapsePattern, String> {
    let element = entry.element.as_ref().ok_or("element gone")?;
    unsafe { element.GetCurrentPattern(UIA_ExpandCollapsePatternId) }
        .map_err(|error| format!("ExpandCollapsePattern: {error}"))?
        .cast()
        .map_err(|error| format!("ExpandCollapsePattern cast: {error}"))
}

/// Table and dynamic-panel roles, names, selection and published rows.
fn check_data(seen: &[Seen]) -> Result<(), String> {
    let table = find(seen, "gallery-table").ok_or("missing gallery table")?;
    require(
        table.control == UIA_DataGridControlTypeId.0
            && table.name == "曲目表"
            && table.selection == Some(1),
        format!(
            "table {:?} name {:?} selection {:?}",
            table.control, table.name, table.selection
        ),
    )?;
    require(
        table.grid_rows == Some(30) && table.grid_columns == Some(3),
        format!(
            "grid extent {:?}x{:?}",
            table.grid_rows, table.grid_columns
        ),
    )?;
    // The header row publishes one HeaderItem per column.
    let headers: Vec<&Seen> = seen
        .iter()
        .filter(|entry| {
            entry.id == "gallery-table" && entry.control == UIA_HeaderItemControlTypeId.0
        })
        .collect();
    require(
        headers.len() == 3,
        format!("{} table headers", headers.len()),
    )?;
    require(
        headers.iter().any(|entry| entry.name == "编号")
            && headers.iter().any(|entry| entry.name == "曲名")
            && headers.iter().any(|entry| entry.name == "时长"),
        format!(
            "table header names: {:?}",
            headers.iter().map(|entry| &entry.name).collect::<Vec<_>>()
        ),
    )?;
    let cells: Vec<&Seen> = seen
        .iter()
        .filter(|entry| {
            entry.id == "gallery-table" && entry.control == UIA_DataItemControlTypeId.0
        })
        .collect();
    // Ten realized rows (nine visible plus the look-ahead) times three columns.
    require(cells.len() == 30, format!("{} table cells", cells.len()))?;
    require(
        cells.iter().filter(|entry| entry.offscreen).count() == 3,
        "the table must publish exactly one offscreen look-ahead row",
    )?;
    require(
        cells.iter().any(|entry| entry.name == "晨雾"),
        format!(
            "table cell names: {:?}",
            cells.iter().map(|entry| &entry.name).collect::<Vec<_>>()
        ),
    )?;
    require(
        cells.iter().filter(|entry| entry.selected_item).count() == 1,
        "exactly one table cell must be selected",
    )?;
    // Cells carry their grid coordinates (IGridItemProvider).
    require(
        cells
            .iter()
            .all(|entry| entry.grid_row.is_some() && entry.grid_column.is_some()),
        "every cell must report its row and column",
    )?;
    let origin = cells
        .iter()
        .find(|entry| entry.grid_row == Some(0) && entry.grid_column == Some(0))
        .ok_or("missing the cell at (0, 0)")?;
    require(
        origin.name == "01",
        format!("cell (0, 0) is {:?}", origin.name),
    )?;
    // IGridProvider.GetItem resolves a cell the grid has realized.
    let grid = grid_pattern(table)?;
    let element = unsafe { grid.GetItem(0, 1) }
        .map_err(|error| format!("grid GetItem(0, 1): {error}"))?;
    let name = unsafe { element.CurrentName() }
        .map(|name| name.to_string())
        .unwrap_or_default();
    require(
        name == "晨雾",
        format!("grid GetItem(0, 1) is {name:?}"),
    )?;

    let tree = find(seen, "gallery-tree").ok_or("missing gallery tree")?;
    require(
        tree.control == UIA_TreeControlTypeId.0
            && tree.name == "音乐库"
            && tree.selection == Some(1),
        format!(
            "tree {:?} name {:?} selection {:?}",
            tree.control, tree.name, tree.selection
        ),
    )?;
    let nodes: Vec<&Seen> = seen
        .iter()
        .filter(|entry| {
            entry.id == "gallery-tree" && entry.control == UIA_TreeItemControlTypeId.0
        })
        .collect();
    // The tree height follows the window (it anchors to the chrome), so only
    // the lower bound and the invariants are asserted here.
    require(nodes.len() >= 6, format!("{} tree rows", nodes.len()))?;
    require(
        nodes.first().is_some_and(|entry| entry.name == "音乐库"),
        format!(
            "tree row names: {:?}",
            nodes.iter().map(|entry| &entry.name).collect::<Vec<_>>()
        ),
    )?;
    require(
        nodes.iter().filter(|entry| entry.offscreen).count() <= 1,
        "the tree may publish at most one offscreen look-ahead row",
    )?;
    require(
        nodes.iter().filter(|entry| entry.selected_item).count() == 1,
        "exactly one tree row must be selected",
    )?;
    require(
        nodes.first().is_some_and(|entry| entry.expandable && entry.expanded),
        "the tree root must be an open TreeItem with ExpandCollapse",
    )?;
    require(
        nodes
            .iter()
            .filter(|entry| entry.name == "播放列表")
            .all(|entry| entry.expand_state == Some(ExpandCollapseState_LeafNode.0)),
        "a leaf tree row must report ExpandCollapseState LeafNode",
    )?;

    let add = find(seen, "dyn-add").ok_or("missing dynamic add button")?;
    require(
        add.control == UIA_ButtonControlTypeId.0 && add.name == "添加项",
        format!("dyn add {:?} name {:?}", add.control, add.name),
    )?;
    let remove = find(seen, "dyn-remove").ok_or("missing dynamic remove button")?;
    require(
        remove.control == UIA_ButtonControlTypeId.0 && remove.name == "移除项",
        format!("dyn remove {:?} name {:?}", remove.control, remove.name),
    )?;
    let slot = find(seen, "dyn-slot-1").ok_or("missing dynamic slot")?;
    require(
        slot.control == UIA_ButtonControlTypeId.0 && slot.name == "动态项 1",
        format!("dyn slot {:?} name {:?}", slot.control, slot.name),
    )?;
    let info = find(seen, "dyn-info").ok_or("missing dyn info label")?;
    require(
        info.control == UIA_TextControlTypeId.0 && info.name == "顺序：1 → 2",
        format!("dyn info {:?} name {:?}", info.control, info.name),
    )?;

    // Scroll view: a named Group over the viewport with its child widgets
    // nested under it. Rows scrolled out of the viewport keep their geometric
    // bounds but report IsOffscreen, the way a virtualized row does.
    let pane = find(seen, "data_scroll").ok_or("missing scroll panel group")?;
    require(
        pane.control == UIA_GroupControlTypeId.0 && pane.name == "滚动面板",
        format!("scroll pane {:?} name {:?}", pane.control, pane.name),
    )?;
    let rows: Vec<&Seen> = seen
        .iter()
        .filter(|entry| entry.id.starts_with("scroll-row-"))
        .collect();
    require(rows.len() == 8, format!("{} scroll rows", rows.len()))?;
    require(
        rows.iter()
            .any(|entry| entry.control == UIA_ButtonControlTypeId.0 && !entry.offscreen),
        "a visible scroll row must publish as an on-screen button",
    )?;
    require(
        rows.iter().any(|entry| entry.control == UIA_CheckBoxControlTypeId.0),
        "the scroll panel must publish its checkbox rows",
    )?;
    require(
        rows.iter().any(|entry| entry.offscreen),
        "a row scrolled out of the viewport must report IsOffscreen",
    )?;
    // Both viewport kinds publish the Scroll pattern: the panel is a container
    // region, the table a widget that owns its own offset.
    let scroll = pane
        .scroll
        .ok_or("the scroll panel must support the Scroll pattern")?;
    require(
        scroll.vertical_scrollable && scroll.vertical_view < 100.0,
        format!("scroll panel state {scroll:?}"),
    )?;
    let table_scroll = table
        .scroll
        .ok_or("the table must support the Scroll pattern")?;
    require(
        table_scroll.vertical_scrollable && !table_scroll.horizontal_scrollable,
        format!("table scroll state {table_scroll:?}"),
    )?;
    Ok(())
}

/// Exercise the application chrome through UIA the way a client would: check
/// the roles and the published commands, then run a menu command and a toolbar
/// overflow command and check both reached the application (the status strip).
///
/// Menus are native popups, so the probe never opens one: a native menu runs a
/// modal loop on the UI thread, and the command path is the same either way.
fn chrome_round(
    automation: &IUIAutomation,
    root: &IUIAutomationElement,
    seen: &[Seen],
) -> Result<(), String> {
    let bar = find(seen, "gallery-menubar").ok_or("missing gallery menu bar")?;
    require(
        bar.control == UIA_MenuBarControlTypeId.0 && bar.name == "菜单栏",
        format!("menu bar {:?} name {:?}", bar.control, bar.name),
    )?;
    let top: Vec<&Seen> = seen
        .iter()
        .filter(|entry| {
            entry.id == "gallery-menubar" && entry.control == UIA_MenuItemControlTypeId.0
        })
        .collect();
    // Three top-level items plus the cursor item's File menu entries.
    require(
        top.len() > 3,
        format!("{} menu bar children", top.len()),
    )?;
    require(
        top.iter().any(|entry| entry.name == "文件(F)"),
        "the menu bar must publish its top-level items",
    )?;
    let command = top
        .iter()
        .find(|entry| entry.name == "新建播放列表")
        .ok_or("the menu bar did not publish the cursor menu's commands")?;
    require(command.invoke, "a menu command must support Invoke")?;

    let toolbar = find(seen, "gallery-toolbar").ok_or("missing gallery toolbar")?;
    require(
        toolbar.control == UIA_ToolBarControlTypeId.0 && toolbar.name == "工具栏",
        format!("toolbar {:?} name {:?}", toolbar.control, toolbar.name),
    )?;
    let tools: Vec<&Seen> = seen
        .iter()
        .filter(|entry| entry.id == "gallery-toolbar" && entry.control == UIA_ButtonControlTypeId.0)
        .collect();
    require(tools.len() >= 5, format!("{} toolbar buttons", tools.len()))?;
    require(
        tools.iter().any(|entry| entry.name == "播放音乐"),
        "the toolbar must publish its first command",
    )?;
    let hidden: Vec<&Seen> = seen
        .iter()
        .filter(|entry| {
            entry.id == "gallery-toolbar" && entry.control == UIA_MenuItemControlTypeId.0
        })
        .collect();
    require(
        !hidden.is_empty(),
        "the toolbar must publish the commands that overflowed",
    )?;

    // The vertical toolbar is icon-only: square cells with explicit names,
    // separators skipped from the tree.
    let vertical = find(seen, "gallery-vtoolbar").ok_or("missing vertical toolbar")?;
    require(
        vertical.control == UIA_ToolBarControlTypeId.0 && vertical.name == "竖排工具栏",
        format!(
            "vertical toolbar {:?} name {:?}",
            vertical.control, vertical.name
        ),
    )?;
    let vertical_tools: Vec<&Seen> = seen
        .iter()
        .filter(|entry| {
            entry.id == "gallery-vtoolbar" && entry.control == UIA_ButtonControlTypeId.0
        })
        .collect();
    let names: Vec<&str> = vertical_tools
        .iter()
        .map(|entry| entry.name.as_str())
        .collect();
    require(
        names.as_slice() == ["播放", "搜索", "设置", "帮助"],
        format!("vertical toolbar names: {names:?}"),
    )?;
    require(
        vertical_tools.iter().all(|entry| entry.invoke),
        "every vertical toolbar cell must support Invoke",
    )?;

    let button = find(seen, "gallery-menubtn").ok_or("missing menu button")?;
    require(
        button.control == UIA_MenuControlTypeId.0 && button.name == "操作菜单",
        format!("menu button {:?} name {:?}", button.control, button.name),
    )?;
    let split = find(seen, "gallery-splitbtn").ok_or("missing split button")?;
    require(
        split.control == UIA_MenuControlTypeId.0
            && split.name == "导出"
            && split.description.as_deref() == Some("主操作与菜单"),
        format!(
            "split button {:?} name {:?} desc {:?}",
            split.control, split.name, split.description
        ),
    )?;
    let status = find(seen, "gallery-status").ok_or("missing status bar")?;
    require(
        status.control == UIA_StatusBarControlTypeId.0 && status.name == "状态栏",
        format!("status bar {:?} name {:?}", status.control, status.name),
    )?;
    let texts: Vec<&Seen> = seen
        .iter()
        .filter(|entry| {
            entry.id == "gallery-status" && entry.control == UIA_TextControlTypeId.0
        })
        .collect();
    require(
        texts.len() == 3 && texts.iter().any(|entry| entry.name == "就绪"),
        format!("{} status segments", texts.len()),
    )?;
    require(
        seen.iter().any(|entry| {
            entry.id == "gallery-status"
                && entry.control == UIA_ProgressBarControlTypeId.0
                && entry.name == "进度 62%"
        }),
        "the status bar must publish its progress segment",
    )?;

    // The dynamic menu publishes its model; the runtime edit invoked below
    // must show up in the tree after the snapshot refresh.
    let dynmenu = find(seen, "gallery-dynmenu").ok_or("missing dynamic menu button")?;
    require(
        dynmenu.control == UIA_MenuControlTypeId.0 && dynmenu.name == "动态菜单",
        format!(
            "dynamic menu {:?} name {:?}",
            dynmenu.control, dynmenu.name
        ),
    )?;
    let dyn_names = dynamic_menu_children(seen);
    require(
        dyn_names.iter().any(|name| name == "添加一项"),
        format!("dynamic menu commands: {dyn_names:?}"),
    )?;
    require(
        !dyn_names.iter().any(|name| name == "动态项 1"),
        "the dynamic item must not exist before the edit",
    )?;

    // Run a menu command: it reaches the application and updates the strip.
    invoke_seen(command)?;
    std::thread::sleep(std::time::Duration::from_millis(350));
    let walker = unsafe { automation.ControlViewWalker() }
        .map_err(|error| format!("chrome ControlViewWalker: {error}"))?;
    let mut after = Vec::new();
    let mut dump = String::new();
    unsafe { collect(&walker, root, 0, &mut after, &mut dump)? };
    require(
        status_texts(&after).iter().any(|name| name == "命令 500"),
        format!(
            "the menu command did not reach the status strip: {:?}",
            status_texts(&after)
        ),
    )?;

    // Run a toolbar overflow command too.
    let hidden = after
        .iter()
        .find(|entry| {
            entry.id == "gallery-toolbar" && entry.control == UIA_MenuItemControlTypeId.0
        })
        .ok_or("the toolbar stopped publishing its overflow commands")?;
    require(hidden.invoke, "an overflow command must support Invoke")?;
    invoke_seen(hidden)?;
    std::thread::sleep(std::time::Duration::from_millis(350));
    let mut last = Vec::new();
    unsafe { collect(&walker, root, 0, &mut last, &mut dump)? };
    require(
        status_texts(&last)
            .iter()
            .any(|name| name.starts_with("命令 ") && name != "命令 500"),
        format!(
            "the overflow command did not reach the status strip: {:?}",
            status_texts(&last)
        ),
    )?;

    // A runtime menu edit: invoke the dynamic menu's "添加一项" command and
    // require the new entry to appear in the refreshed tree.
    let add = last
        .iter()
        .find(|entry| {
            entry.id == "gallery-dynmenu"
                && entry.control == UIA_MenuItemControlTypeId.0
                && entry.name == "添加一项"
        })
        .ok_or("the dynamic menu stopped publishing its edit command")?;
    require(add.invoke, "a dynamic menu command must support Invoke")?;
    invoke_seen(add)?;
    std::thread::sleep(std::time::Duration::from_millis(400));
    let mut edited = Vec::new();
    unsafe { collect(&walker, root, 0, &mut edited, &mut dump)? };
    let edited_names = dynamic_menu_children(&edited);
    require(
        edited_names.iter().any(|name| name == "动态项 1"),
        format!("the runtime edit did not reach the tree: {edited_names:?}"),
    )?;
    Ok(())
}

fn dynamic_menu_children(seen: &[Seen]) -> Vec<String> {
    seen.iter()
        .filter(|entry| {
            entry.id == "gallery-dynmenu" && entry.control == UIA_MenuItemControlTypeId.0
        })
        .map(|entry| entry.name.clone())
        .collect()
}

fn status_texts(seen: &[Seen]) -> Vec<String> {
    seen.iter()
        .filter(|entry| {
            entry.id == "gallery-status" && entry.control == UIA_TextControlTypeId.0
        })
        .map(|entry| entry.name.clone())
        .collect()
}

fn invoke_seen(entry: &Seen) -> Result<(), String> {
    let element = entry.element.as_ref().ok_or("element gone")?;
    let pattern: IUIAutomationInvokePattern = unsafe { element.GetCurrentPattern(UIA_InvokePatternId) }
        .map_err(|error| format!("InvokePattern: {error}"))?
        .cast()
        .map_err(|error| format!("InvokePattern cast: {error}"))?;
    unsafe { pattern.Invoke() }.map_err(|error| format!("Invoke: {error}"))
}

/// Open the in-window modal shell through its gallery button, check the focus
/// trap and panel roles, then cancel and prove the shell went away.
fn modal_round(
    automation: &IUIAutomation,
    root: &IUIAutomationElement,
    gallery: &[Seen],
) -> Result<(), String> {
    let open = find(gallery, "open-modal").ok_or("missing open-modal button")?;
    let element = open.element.as_ref().ok_or("open-modal element gone")?;
    let pattern: IUIAutomationInvokePattern =
        unsafe { element.GetCurrentPattern(UIA_InvokePatternId) }
            .map_err(|error| format!("open-modal InvokePattern: {error}"))?
            .cast()
            .map_err(|error| format!("open-modal cast: {error}"))?;
    unsafe { pattern.Invoke() }.map_err(|error| format!("open-modal Invoke: {error}"))?;
    std::thread::sleep(std::time::Duration::from_millis(350));

    let walker = unsafe { automation.ControlViewWalker() }
        .map_err(|error| format!("modal ControlViewWalker: {error}"))?;
    let mut modal = Vec::new();
    let mut dump = String::new();
    unsafe { collect(&walker, root, 0, &mut modal, &mut dump)? };
    check_modal(&modal).map_err(|error| format!("{error}
--- modal tree ---
{dump}"))?;

    let cancel = find(&modal, "modal-cancel").ok_or("missing modal cancel")?;
    let element = cancel.element.as_ref().ok_or("modal cancel gone")?;
    let pattern: IUIAutomationInvokePattern =
        unsafe { element.GetCurrentPattern(UIA_InvokePatternId) }
            .map_err(|error| format!("modal cancel InvokePattern: {error}"))?
            .cast()
            .map_err(|error| format!("modal cancel cast: {error}"))?;
    unsafe { pattern.Invoke() }.map_err(|error| format!("modal cancel Invoke: {error}"))?;
    std::thread::sleep(std::time::Duration::from_millis(350));

    let mut after = Vec::new();
    unsafe { collect(&walker, root, 0, &mut after, &mut dump)? };
    require(
        find(&after, "modal-card").is_none(),
        "the modal panel is still in the tree after Cancel",
    )?;
    require(
        find(&after, "open-modal")
            .map(|button| button.focused)
            .unwrap_or(false),
        "focus did not return to the modal opener",
    )?;
    Ok(())
}

fn check_modal(seen: &[Seen]) -> Result<(), String> {
    let card = find(seen, "modal-card").ok_or("missing modal panel")?;
    require(
        card.control == UIA_GroupControlTypeId.0 && card.name == "删除播放列表？",
        format!("modal card {:?} name {:?}", card.control, card.name),
    )?;

    let ok = find(seen, "modal-ok").ok_or("missing modal confirm button")?;
    require(
        ok.control == UIA_ButtonControlTypeId.0 && ok.name == "确定" && ok.focused,
        format!(
            "modal ok {:?} name {:?} focused={}",
            ok.control, ok.name, ok.focused
        ),
    )?;

    let check = find(seen, "modal-check").ok_or("missing modal checkbox")?;
    require(
        check.control == UIA_CheckBoxControlTypeId.0 && check.name == "不再询问",
        format!("modal checkbox {:?} name {:?}", check.control, check.name),
    )?;
    require(
        !check.focused,
        "the modal checkbox must not steal focus from the default button",
    )?;

    // The page behind the scrim must not keep keyboard focus.
    require(
        find(seen, "gallery-checkbox-random")
            .map(|entry| !entry.focused)
            .unwrap_or(true),
        "background widget still has keyboard focus under the modal",
    )?;
    Ok(())
}

fn check_gallery(seen: &[Seen]) -> Result<(), String> {
    let random = find(seen, "gallery-checkbox-random").ok_or("missing gallery checkbox")?;
    require(
        random.control == UIA_CheckBoxControlTypeId.0 && random.toggled == Some(false),
        format!("random checkbox {:?}/{:?}", random.control, random.toggled),
    )?;

    let shuffle = find(seen, "gallery-checkbox-shuffle").ok_or("missing shuffle checkbox")?;
    require(
        shuffle.control == UIA_CheckBoxControlTypeId.0 && shuffle.toggled == Some(true),
        format!("shuffle checkbox {:?}/{:?}", shuffle.control, shuffle.toggled),
    )?;

    let square = find(seen, "gallery-checkbox-square").ok_or("missing square checkbox")?;
    require(
        square.control == UIA_CheckBoxControlTypeId.0
            && square.toggled == Some(false)
            && square.name == "方框",
        format!(
            "square checkbox {:?}/{:?} name {:?}",
            square.control, square.toggled, square.name
        ),
    )?;

    let radio = find(seen, "gallery-radio").ok_or("missing standalone radio")?;
    require(
        radio.control == UIA_RadioButtonControlTypeId.0 && radio.name == "仅本机",
        format!("standalone radio {:?} name {:?}", radio.control, radio.name),
    )?;

    let group = find(seen, "gallery-radio-group").ok_or("missing radio group")?;
    require(
        group.control == UIA_GroupControlTypeId.0 && group.selection == Some(1),
        format!("radio group {:?} selection {:?}", group.control, group.selection),
    )?;
    require(
        group.name == "循环方式",
        format!("radio group name {:?}", group.name),
    )?;
    let radios: Vec<&Seen> = seen
        .iter()
        .filter(|entry| {
            entry.id == "gallery-radio-group"
                && entry.control == UIA_RadioButtonControlTypeId.0
        })
        .collect();
    require(radios.len() == 3, format!("{} radio children", radios.len()))?;
    require(
        radios.iter().filter(|entry| entry.selected_item).count() == 1,
        "exactly one radio child must be selected",
    )?;

    let search = find(seen, "gallery-search").ok_or("missing gallery search box")?;
    require(
        search.control == UIA_EditControlTypeId.0
            && search.value.as_deref() == Some("")
            && search.name == "搜索曲目",
        format!(
            "gallery search {:?} name {:?} value {:?}",
            search.control, search.name, search.value
        ),
    )?;

    let password = find(seen, "gallery-password").ok_or("missing gallery password box")?;
    require(
        password.control == UIA_EditControlTypeId.0,
        format!("password box {:?}", password.control),
    )?;

    let combo = find(seen, "gallery-combo").ok_or("missing gallery combo box")?;
    require(
        combo.control == UIA_ComboBoxControlTypeId.0
            && combo.name == "播放模式"
            && combo.value.as_deref() == Some("列表循环"),
        format!(
            "combo {:?} name {:?} value {:?}",
            combo.control, combo.name, combo.value
        ),
    )?;

    // The date picker is a Button with a date value and an ExpandCollapse
    // state (D115); it starts collapsed.
    let date = find(seen, "gallery-date").ok_or("missing gallery date picker")?;
    require(
        date.control == UIA_ButtonControlTypeId.0
            && date.name == "发布日期"
            && date.value.as_deref().is_some_and(|value| value.len() == 10)
            && date.expandable
            && !date.expanded,
        format!(
            "date picker {:?} name {:?} value {:?} expandable {} state {:?}",
            date.control, date.name, date.value, date.expandable, date.expand_state
        ),
    )?;

    let time = find(seen, "gallery-time").ok_or("missing gallery time picker")?;
    require(
        time.control == UIA_ButtonControlTypeId.0
            && time.name == "提醒时间"
            && time.value.as_deref() == Some("09:30")
            && time.expandable
            && !time.expanded,
        format!(
            "time picker {:?} name {:?} value {:?} expandable {}",
            time.control, time.name, time.value, time.expandable
        ),
    )?;

    let color = find(seen, "gallery-color").ok_or("missing gallery color picker")?;
    require(
        color.control == UIA_ButtonControlTypeId.0
            && color.name == "标签颜色"
            && color.value.as_deref() == Some("#1971C2")
            && color.expandable
            && !color.expanded,
        format!(
            "color picker {:?} name {:?} value {:?} expandable {}",
            color.control, color.name, color.value, color.expandable
        ),
    )?;

    // The multiline field publishes its whole value, not just the first line.
    let notes = find(seen, "gallery-notes").ok_or("missing multiline text area")?;
    require(
        notes.control == UIA_EditControlTypeId.0
            && notes.value.as_deref().is_some_and(|value| value.starts_with("第一行")),
        format!("notes {:?} value {:?}", notes.control, notes.value),
    )?;

    let banner = find(seen, "gallery-banner").ok_or("missing gallery banner")?;
    require(
        banner.control == UIA_GroupControlTypeId.0
            && banner.name == "有可用更新"
            && banner
                .description
                .as_deref()
                .is_some_and(|body| body.contains("新版本")),
        format!(
            "banner {:?} name {:?} body {:?}",
            banner.control, banner.name, banner.description
        ),
    )?;

    let ring = find(seen, "gallery-ring").ok_or("missing progress ring")?;
    require(
        ring.control == UIA_ProgressBarControlTypeId.0
            && ring.range.is_some_and(|value| (value - 0.72).abs() < 0.001),
        format!("ring {:?} range {:?}", ring.control, ring.range),
    )?;

    let tooltip = find(seen, "gallery-tooltip").ok_or("missing tooltip card")?;
    require(
        tooltip.control == UIA_TextControlTypeId.0 && tooltip.name == "提示卡片",
        format!("tooltip {:?} name {:?}", tooltip.control, tooltip.name),
    )?;

    let spinner = find(seen, "gallery-spinner").ok_or("missing spinner")?;
    require(
        spinner.control == UIA_TextControlTypeId.0,
        format!("spinner {:?}", spinner.control),
    )?;

    let multi = find(seen, "gallery-multi").ok_or("missing multi-select list")?;
    require(
        multi.control == UIA_ListControlTypeId.0
            && multi.name == "多选列表"
            && multi.multi == Some(true)
            && multi.selection == Some(2),
        format!(
            "multi {:?} name {:?} multi {:?} selection {:?}",
            multi.control, multi.name, multi.multi, multi.selection
        ),
    )?;
    let items: Vec<&Seen> = seen
        .iter()
        .filter(|entry| {
            entry.id == "gallery-multi" && entry.control == UIA_ListItemControlTypeId.0
        })
        .collect();
    require(items.len() == 4, format!("{} multi-select rows", items.len()))?;
    require(
        items.iter().filter(|entry| entry.selected_item).count() == 2,
        "two rows must report SelectionItem.IsSelected",
    )?;

    // The accordion publishes a List container whose open section is the
    // selected ListItem; each section also reports its open state through the
    // UIA ExpandCollapse pattern (D98).
    let accordion = find(seen, "gallery-accordion").ok_or("missing accordion")?;
    require(
        accordion.control == UIA_ListControlTypeId.0
            && accordion.name == "播放设置"
            && accordion.selection == Some(1),
        format!(
            "accordion {:?} name {:?} selection {:?}",
            accordion.control, accordion.name, accordion.selection
        ),
    )?;
    let sections: Vec<&Seen> = seen
        .iter()
        .filter(|entry| {
            entry.id == "gallery-accordion" && entry.control == UIA_ListItemControlTypeId.0
        })
        .collect();
    require(sections.len() == 2, format!("{} accordion sections", sections.len()))?;
    let open = sections
        .iter()
        .find(|entry| entry.name == "播放选项")
        .ok_or("missing open accordion section")?;
    require(open.selected_item, "the open section must be selected")?;
    require(
        open.expandable && open.expanded,
        "the open section must expose ExpandCollapse as Expanded",
    )?;
    let closed = sections
        .iter()
        .find(|entry| entry.name == "音效选项")
        .ok_or("missing closed accordion section")?;
    require(
        !closed.selected_item,
        "the closed section must not be selected",
    )?;
    require(
        closed.expandable && !closed.expanded,
        "the closed section must expose ExpandCollapse as Collapsed",
    )?;

    // Breadcrumb: a List with the current (last) crumb selected.
    let crumb = find(seen, "gallery-breadcrumb").ok_or("missing breadcrumb")?;
    require(
        crumb.control == UIA_ListControlTypeId.0
            && crumb.name == "位置"
            && crumb.selection == Some(1),
        format!(
            "breadcrumb {:?} name {:?} selection {:?}",
            crumb.control, crumb.name, crumb.selection
        ),
    )?;
    let crumbs: Vec<&Seen> = seen
        .iter()
        .filter(|entry| {
            entry.id == "gallery-breadcrumb" && entry.control == UIA_ListItemControlTypeId.0
        })
        .collect();
    require(crumbs.len() == 3, format!("{} breadcrumb crumbs", crumbs.len()))?;
    let current = crumbs
        .iter()
        .find(|entry| entry.name == "夜曲")
        .ok_or("missing current breadcrumb")?;
    require(current.selected_item, "the current crumb must be selected")?;

    // Navigation rail: one selected row out of four.
    let rail = find(seen, "gallery-navrail").ok_or("missing nav rail")?;
    require(
        rail.control == UIA_ListControlTypeId.0
            && rail.name == "导航"
            && rail.selection == Some(1),
        format!(
            "nav rail {:?} name {:?} selection {:?}",
            rail.control, rail.name, rail.selection
        ),
    )?;
    let rows: Vec<&Seen> = seen
        .iter()
        .filter(|entry| {
            entry.id == "gallery-navrail" && entry.control == UIA_ListItemControlTypeId.0
        })
        .collect();
    require(rows.len() == 4, format!("{} nav rail rows", rows.len()))?;
    let recent = rows
        .iter()
        .find(|entry| entry.name == "最近播放")
        .ok_or("missing selected nav row")?;
    require(recent.selected_item, "最近播放 must be the selected row")?;

    // Group frame and the splitter separator (no UIA pattern).
    let group = find(seen, "gallery-group").ok_or("missing group box")?;
    require(
        group.control == UIA_GroupControlTypeId.0 && group.name == "设置",
        format!("group box {:?} name {:?}", group.control, group.name),
    )?;
    let splitter = find(seen, "gallery-splitter").ok_or("missing splitter")?;
    require(
        splitter.control == UIA_SeparatorControlTypeId.0 && splitter.name == "拖动调整面板",
        format!(
            "splitter {:?} name {:?}",
            splitter.control, splitter.name
        ),
    )?;

    // Data display: a displayed sparkline, a gauge range and a star rating.
    let sparkline = find(seen, "gallery-sparkline").ok_or("missing sparkline")?;
    require(
        sparkline.control == UIA_TextControlTypeId.0 && sparkline.name == "音量趋势",
        format!(
            "sparkline {:?} name {:?}",
            sparkline.control, sparkline.name
        ),
    )?;
    let gauge = find(seen, "gallery-gauge").ok_or("missing gauge")?;
    require(
        gauge.control == UIA_ProgressBarControlTypeId.0
            && gauge.name == "负载"
            && gauge.range.is_some_and(|value| (value - 0.65).abs() < 1e-3),
        format!(
            "gauge {:?} name {:?} range {:?}",
            gauge.control, gauge.name, gauge.range
        ),
    )?;
    let stars = find(seen, "gallery-stars").ok_or("missing rating stars")?;
    require(
        stars.control == UIA_SliderControlTypeId.0
            && stars.name == "评分"
            && stars.range == Some(3.0),
        format!(
            "stars {:?} name {:?} range {:?}",
            stars.control, stars.name, stars.range
        ),
    )?;
    Ok(())
}

/// Drive an accordion section through the UIA ExpandCollapse pattern: collapse
/// the open one, require both headers to report Collapsed, then expand the
/// other and require the state to follow. This is the pattern a screen reader
/// uses instead of guessing from selection.
fn accordion_round(
    automation: &IUIAutomation,
    root: &IUIAutomationElement,
    seen: &[Seen],
) -> Result<(), String> {
    fn section<'a>(page: &'a [Seen], name: &str) -> Option<&'a Seen> {
        page.iter().find(|entry| {
            entry.id == "gallery-accordion"
                && entry.control == UIA_ListItemControlTypeId.0
                && entry.name == name
        })
    }
    let walker = unsafe { automation.ControlViewWalker() }
        .map_err(|error| format!("accordion ControlViewWalker: {error}"))?;
    let mut page = Vec::new();
    let mut dump = String::new();

    let open = section(seen, "播放选项").ok_or("missing open accordion section")?;
    let pattern = expand_pattern(open)?;
    unsafe { pattern.Collapse() }.map_err(|error| format!("accordion Collapse: {error}"))?;
    std::thread::sleep(std::time::Duration::from_millis(300));
    unsafe { collect(&walker, root, 0, &mut page, &mut dump)? };
    for name in ["播放选项", "音效选项"] {
        let entry = section(&page, name).ok_or("accordion section disappeared")?;
        require(
            entry.expandable && !entry.expanded,
            format!("{name} must report Collapsed after the collapse request"),
        )?;
    }

    let closed = section(&page, "音效选项").ok_or("missing closed accordion section")?;
    let pattern = expand_pattern(closed)?;
    unsafe { pattern.Expand() }.map_err(|error| format!("accordion Expand: {error}"))?;
    std::thread::sleep(std::time::Duration::from_millis(300));
    page.clear();
    unsafe { collect(&walker, root, 0, &mut page, &mut dump)? };
    let sound = section(&page, "音效选项").ok_or("missing expanded accordion section")?;
    require(
        sound.expandable && sound.expanded,
        "音效选项 must report Expanded after the expand request",
    )?;
    let play = section(&page, "播放选项").ok_or("missing accordion section")?;
    require(
        !play.expanded,
        "a single-open accordion must close the other section",
    )?;
    Ok(())
}

unsafe fn collect(
    walker: &IUIAutomationTreeWalker,
    element: &IUIAutomationElement,
    depth: usize,
    seen: &mut Vec<Seen>,
    dump: &mut String,
) -> Result<(), String> {
    let mut entry = Seen {
        element: Some(element.clone()),
        id: unsafe { element.CurrentAutomationId() }
            .map(|id| id.to_string())
            .unwrap_or_default(),
        name: unsafe { element.CurrentName() }
            .map(|name| name.to_string())
            .unwrap_or_default(),
        description: unsafe { element.CurrentHelpText() }
            .ok()
            .map(|text| text.to_string())
            .filter(|text| !text.is_empty()),
        control: unsafe { element.CurrentControlType() }
            .map(|control| control.0)
            .unwrap_or(0),
        ..Seen::default()
    };

    entry.invoke = unsafe { element.GetCurrentPattern(UIA_InvokePatternId) }.is_ok();
    entry.offscreen = unsafe { element.CurrentIsOffscreen() }
        .map(|value| value.as_bool())
        .unwrap_or(false);

    if let Ok(pattern) = unsafe { element.GetCurrentPattern(UIA_ValuePatternId) } {
        if let Ok(pattern) = pattern.cast::<IUIAutomationValuePattern>() {
            entry.value = unsafe { pattern.CurrentValue() }
                .ok()
                .map(|value| value.to_string());
        }
    }
    if let Ok(pattern) = unsafe { element.GetCurrentPattern(UIA_TogglePatternId) } {
        if let Ok(pattern) = pattern.cast::<IUIAutomationTogglePattern>() {
            entry.toggled = unsafe { pattern.CurrentToggleState() }
                .ok()
                .map(|state| state == ToggleState_On);
        }
    }
    if let Ok(pattern) = unsafe { element.GetCurrentPattern(UIA_RangeValuePatternId) } {
        if let Ok(pattern) = pattern.cast::<windows::Win32::UI::Accessibility::IUIAutomationRangeValuePattern>() {
            entry.range = unsafe { pattern.CurrentValue() }.ok();
        }
    }
    if let Ok(pattern) = unsafe { element.GetCurrentPattern(UIA_TextPatternId) } {
        if let Ok(pattern) = pattern.cast::<IUIAutomationTextPattern>() {
            if let Ok(range) = unsafe { pattern.DocumentRange() } {
                entry.text = unsafe { range.GetText(-1) }.ok().map(|text| text.to_string());
            }
            // A closed text field publishes its caret/selection; other editors
            // report SupportedTextSelection_None and an empty selection.
            entry.supported_selection = unsafe { pattern.SupportedTextSelection() }
                .ok()
                .map(|selection| selection.0);
            if let Ok(ranges) = unsafe { pattern.GetSelection() } {
                entry.selection_ranges = unsafe { ranges.Length() }.ok();
                if let Ok(range) = unsafe { ranges.GetElement(0) } {
                    entry.selection_text =
                        unsafe { range.GetText(-1) }.ok().map(|text| text.to_string());
                }
            }
        }
    }
    if let Ok(pattern) = unsafe { element.GetCurrentPattern(UIA_SelectionPatternId) } {
        if let Ok(pattern) = pattern.cast::<IUIAutomationSelectionPattern>() {
            entry.selection = unsafe { pattern.GetCurrentSelection() }
                .ok()
                .and_then(|array| unsafe { array.Length() }.ok());
            entry.multi = unsafe { pattern.CurrentCanSelectMultiple() }
                .ok()
                .map(|value| value.as_bool());
        }
    }
    if let Ok(pattern) = unsafe { element.GetCurrentPattern(UIA_SelectionItemPatternId) } {
        if let Ok(pattern) = pattern.cast::<IUIAutomationSelectionItemPattern>() {
            entry.selected_item = unsafe { pattern.CurrentIsSelected() }
                .map(|selected| selected.as_bool())
                .unwrap_or(false);
        }
    }
    if let Ok(pattern) = unsafe { element.GetCurrentPattern(UIA_GridPatternId) } {
        if let Ok(pattern) = pattern.cast::<IUIAutomationGridPattern>() {
            entry.grid_rows = unsafe { pattern.CurrentRowCount() }.ok();
            entry.grid_columns = unsafe { pattern.CurrentColumnCount() }.ok();
        }
    }
    if let Ok(pattern) = unsafe { element.GetCurrentPattern(UIA_GridItemPatternId) } {
        if let Ok(pattern) = pattern.cast::<IUIAutomationGridItemPattern>() {
            entry.grid_row = unsafe { pattern.CurrentRow() }.ok();
            entry.grid_column = unsafe { pattern.CurrentColumn() }.ok();
        }
    }
    entry.focused = unsafe { element.CurrentHasKeyboardFocus() }
        .map(|focused| focused.as_bool())
        .unwrap_or(false);
    if let Ok(pattern) = unsafe { element.GetCurrentPattern(UIA_ExpandCollapsePatternId) } {
        if let Ok(pattern) = pattern.cast::<IUIAutomationExpandCollapsePattern>() {
            entry.expandable = true;
            if let Ok(state) = unsafe { pattern.CurrentExpandCollapseState() } {
                entry.expand_state = Some(state.0);
                entry.expanded = state == ExpandCollapseState_Expanded;
            }
        }
    }
    if let Ok(pattern) = unsafe { element.GetCurrentPattern(UIA_ScrollPatternId) } {
        if let Ok(pattern) = pattern.cast::<IUIAutomationScrollPattern>() {
            entry.scroll = Some(ScrollState {
                horizontal_scrollable: unsafe { pattern.CurrentHorizontallyScrollable() }
                    .map(|value| value.as_bool())
                    .unwrap_or(false),
                vertical_scrollable: unsafe { pattern.CurrentVerticallyScrollable() }
                    .map(|value| value.as_bool())
                    .unwrap_or(false),
                horizontal_percent: unsafe { pattern.CurrentHorizontalScrollPercent() }
                    .unwrap_or(-1.0),
                vertical_percent: unsafe { pattern.CurrentVerticalScrollPercent() }
                    .unwrap_or(-1.0),
                vertical_view: unsafe { pattern.CurrentVerticalViewSize() }.unwrap_or(-1.0),
            });
        }
    }

    let indent = "  ".repeat(depth);
    let _ = write!(
        dump,
        "{indent}{} id={:?} name={:?}",
        control_name(entry.control),
        entry.id,
        entry.name
    );
    if let Some(description) = &entry.description {
        let _ = write!(dump, " desc={description:?}");
    }
    if let Some(value) = &entry.value {
        let _ = write!(dump, " value={value:?}");
    }
    if let Some(text) = &entry.text {
        let _ = write!(dump, " text={text:?}");
    }
    if let Some(toggled) = entry.toggled {
        let _ = write!(dump, " toggled={toggled}");
    }
    if let Some(range) = entry.range {
        let _ = write!(dump, " range={range}");
    }
    if let Some(selection) = entry.selection {
        let _ = write!(dump, " selected={selection}");
    }
    if let Some(supported) = entry.supported_selection {
        let _ = write!(dump, " text-selection={supported}");
    }
    if let Some(ranges) = entry.selection_ranges {
        let _ = write!(dump, " selection-ranges={ranges}");
    }
    if let Some(text) = &entry.selection_text {
        let _ = write!(dump, " selection-text={text:?}");
    }
    if entry.selected_item {
        let _ = write!(dump, " is-selected");
    }
    if entry.offscreen {
        let _ = write!(dump, " offscreen");
    }
    if entry.expanded {
        let _ = write!(dump, " [expanded]");
    }
    if entry.invoke {
        let _ = write!(dump, " [invoke]");
    }
    if let Some(scroll) = entry.scroll {
        let _ = write!(
            dump,
            " [scroll h={:?} v={:?} h%={:.1} v%={:.1} vview={:.1}]",
            scroll.horizontal_scrollable,
            scroll.vertical_scrollable,
            scroll.horizontal_percent,
            scroll.vertical_percent,
            scroll.vertical_view
        );
    }
    dump.push('\n');

    seen.push(entry);

    let mut child = unsafe { walker.GetFirstChildElement(element) }.ok();
    while let Some(current) = child {
        unsafe { collect(walker, &current, depth + 1, seen, dump)? };
        child = unsafe { walker.GetNextSiblingElement(&current) }.ok();
    }
    Ok(())
}

/// Toggle through UIA and wait for a ToggleState property-change event: a
/// screen reader announces the new state from this notification, not from the
/// focus event (which does not change when the same switch is flipped).
fn toggle_round_trip(automation: &IUIAutomation, seen: &[Seen]) -> Result<(), String> {
    let toggle = find(seen, "toggle").ok_or("missing toggle")?;
    let element = toggle.element.as_ref().ok_or("toggle element gone")?;
    let values = Arc::new(Mutex::new(Vec::new()));
    let spy: IUIAutomationPropertyChangedEventHandler = PropertySpy {
        property: UIA_ToggleToggleStatePropertyId.0,
        values: values.clone(),
    }
    .into();
    let properties = [UIA_ToggleToggleStatePropertyId];
    unsafe {
        automation.AddPropertyChangedEventHandlerNativeArray(
            element,
            TreeScope_Subtree,
            None,
            &spy,
            &properties,
        )
    }
    .map_err(|error| format!("AddPropertyChangedEventHandler: {error}"))?;

    let pattern: IUIAutomationTogglePattern =
        unsafe { element.GetCurrentPattern(UIA_TogglePatternId) }
            .map_err(|error| format!("TogglePattern: {error}"))?
            .cast()
            .map_err(|error| format!("TogglePattern cast: {error}"))?;
    let before = unsafe { pattern.CurrentToggleState() }
        .map_err(|error| format!("CurrentToggleState: {error}"))?;
    unsafe { pattern.Toggle() }.map_err(|error| format!("Toggle: {error}"))?;

    let mut seen_off = false;
    for _ in 0..40 {
        std::thread::sleep(std::time::Duration::from_millis(50));
        if values
            .lock()
            .map(|values| values.iter().any(|value| *value == ToggleState_Off.0))
            .unwrap_or(false)
        {
            seen_off = true;
            break;
        }
    }
    let _ = unsafe { automation.RemovePropertyChangedEventHandler(element, &spy) };
    if !seen_off {
        return Err(format!(
            "no ToggleState property event, states={:?} (before={})",
            values.lock().map(|values| values.clone()).unwrap_or_default(),
            before.0
        ));
    }
    Ok(())
}

/// UIA focus must land on the selected tab/row (not only on the container) so
/// Narrator reads the item and follows arrow-driven selection changes.
fn focus_round_trip(automation: &IUIAutomation, hwnd: isize, seen: &[Seen]) -> Result<(), String> {
    let tabs = find(seen, "tabs").ok_or("missing tabs")?;
    let tabs = tabs.element.as_ref().ok_or("tabs element gone")?;
    retry_focus(automation, hwnd, tabs, "tabs", "概览")?;

    let list = find(seen, "list").ok_or("missing list")?;
    let list = list.element.as_ref().ok_or("list element gone")?;
    // Let the focus request land before selecting; the app answers asynchronously.
    // The showcase narrates rows as "第 N 首，<title>".
    retry_focus(automation, hwnd, list, "list", "第 1 首，曲目 001")?;

    let rows: Vec<&Seen> = seen
        .iter()
        .filter(|entry| entry.id == "list" && entry.control == UIA_ListItemControlTypeId.0)
        .collect();
    let second = rows.get(1).ok_or("need two visible rows")?;
    let element = second.element.as_ref().ok_or("row element gone")?;
    let pattern: IUIAutomationSelectionItemPattern =
        unsafe { element.GetCurrentPattern(UIA_SelectionItemPatternId) }
            .map_err(|error| format!("SelectionItemPattern: {error}"))?
            .cast()
            .map_err(|error| format!("SelectionItemPattern cast: {error}"))?;

    let events = Arc::new(Mutex::new(Vec::new()));
    let spy: IUIAutomationEventHandler = EventSpy {
        events: events.clone(),
    }
    .into();
    unsafe {
        automation.AddAutomationEventHandler(
            UIA_SelectionItem_ElementSelectedEventId,
            list,
            TreeScope_Subtree,
            None,
            &spy,
        )
    }
    .map_err(|error| format!("AddAutomationEventHandler: {error}"))?;

    unsafe { pattern.Select() }.map_err(|error| format!("row Select: {error}"))?;
    let focus = wait_focus(automation, hwnd, "list", &second.name);
    let selected = events
        .lock()
        .map(|events| {
            events
                .iter()
                .any(|event| *event == UIA_SelectionItem_ElementSelectedEventId.0)
        })
        .unwrap_or(false);
    let _ = unsafe {
        automation.RemoveAutomationEventHandler(UIA_SelectionItem_ElementSelectedEventId, list, &spy)
    };
    focus?;
    if !selected {
        return Err(format!(
            "no ElementSelected event for {:?} (events={:?})",
            second.name,
            events.lock().map(|events| events.clone()).unwrap_or_default()
        ));
    }
    Ok(())
}

/// A focus request can be dropped while the window is losing/regaining OS
/// focus on a shared desktop; retry once before failing.
fn retry_focus(
    automation: &IUIAutomation,
    hwnd: isize,
    element: &IUIAutomationElement,
    id: &str,
    name: &str,
) -> Result<(), String> {
    let mut last = String::new();
    for _ in 0..2 {
        unsafe { element.SetFocus() }.map_err(|error| format!("{id} SetFocus: {error}"))?;
        match wait_focus(automation, hwnd, id, name) {
            Ok(()) => return Ok(()),
            Err(error) => last = error,
        }
    }
    Err(last)
}

/// Wait until the window's own provider reports `HasKeyboardFocus` on the
/// expected element. This deliberately does not use `GetFocusedElement`:
/// that resolves the *global* foreground focus, so another window (or a
/// leftover instance of this app) would make the check flaky.
fn wait_focus(
    automation: &IUIAutomation,
    hwnd: isize,
    id: &str,
    name: &str,
) -> Result<(), String> {
    let root = unsafe { automation.ElementFromHandle(HWND(hwnd as *mut core::ffi::c_void)) }
        .map_err(|error| format!("ElementFromHandle: {error}"))?;
    let walker = unsafe { automation.ControlViewWalker() }
        .map_err(|error| format!("ControlViewWalker: {error}"))?;
    let mut last = String::new();
    for _ in 0..40 {
        std::thread::sleep(std::time::Duration::from_millis(50));
        match focused_in(&walker, &root, 0) {
            Some((focused_id, focused_name)) => {
                last = format!("{focused_id:?} {focused_name:?}");
                if focused_id == id && focused_name == name {
                    return Ok(());
                }
            }
            None => last = "none".to_string(),
        }
    }
    Err(format!(
        "focus stayed at {last}, expected {id:?} {name:?}"
    ))
}

fn focused_in(
    walker: &IUIAutomationTreeWalker,
    element: &IUIAutomationElement,
    depth: usize,
) -> Option<(String, String)> {
    if depth > 8 {
        return None;
    }
    // Skip the shell's own menu bars (the native window menu under the title
    // bar and the window menu on the title bar itself): they report keyboard
    // focus whenever the window has focus and would mask the page's ring. The
    // native ones expose the automation id "MenuBar"; the page's painted menu
    // bar carries its own id ("gallery-menubar").
    let native_bar = unsafe { element.CurrentControlType() }
        .map(|control| control == UIA_MenuBarControlTypeId)
        .unwrap_or(false)
        && unsafe { element.CurrentAutomationId() }
            .map(|id| id.to_string() == "MenuBar")
            .unwrap_or(false);
    if native_bar {
        return None;
    }
    if unsafe { element.CurrentHasKeyboardFocus() }
        .map(|focused| focused.as_bool())
        .unwrap_or(false)
    {
        let id = unsafe { element.CurrentAutomationId() }
            .map(|value| value.to_string())
            .unwrap_or_default();
        let name = unsafe { element.CurrentName() }
            .map(|value| value.to_string())
            .unwrap_or_default();
        return Some((id, name));
    }
    let mut child = unsafe { walker.GetFirstChildElement(element) }.ok();
    while let Some(current) = child {
        if let Some(found) = focused_in(walker, &current, depth + 1) {
            return Some(found);
        }
        child = unsafe { walker.GetNextSiblingElement(&current) }.ok();
    }
    None
}

/// Invoke the primary button through UIA and wait for the widget to show the
/// new count: proves the request reached the UI thread and refreshed the tree.
/// The click also shows a toast, which the app pushes as a UIA notification;
/// the probe must receive that notification as a real client.
fn invoke_round_trip(
    automation: &IUIAutomation,
    root: &IUIAutomationElement,
    seen: &[Seen],
) -> Result<(), String> {
    let primary = find(seen, "primary").ok_or("missing primary button")?;
    let element = primary.element.as_ref().ok_or("primary element gone")?;
    let events = Arc::new(Mutex::new(Vec::new()));
    let spy: IUIAutomationEventHandler = EventSpy {
        events: events.clone(),
    }
    .into();
    unsafe {
        automation.AddAutomationEventHandler(
            UIA_NotificationEventId,
            root,
            TreeScope_Subtree,
            None,
            &spy,
        )
    }
    .map_err(|error| format!("AddAutomationEventHandler(notification): {error}"))?;

    let pattern: IUIAutomationInvokePattern =
        unsafe { element.GetCurrentPattern(UIA_InvokePatternId) }
            .map_err(|error| format!("InvokePattern: {error}"))?
            .cast()
            .map_err(|error| format!("InvokePattern cast: {error}"))?;
    let invoked = unsafe { pattern.Invoke() }.map_err(|error| format!("Invoke: {error}"));
    let mut announced = false;
    let mut renamed = false;
    let result = (|| {
        invoked?;
        for _ in 0..40 {
            std::thread::sleep(std::time::Duration::from_millis(50));
            let name = unsafe { element.CurrentName() }
                .map(|name| name.to_string())
                .unwrap_or_default();
            if name.starts_with("主按钮 1") {
                renamed = true;
                break;
            }
        }
        if !renamed {
            return Err("UIA Invoke did not reach the widget".to_string());
        }
        for _ in 0..40 {
            if events
                .lock()
                .map(|events| events.iter().any(|event| *event == UIA_NotificationEventId.0))
                .unwrap_or(false)
            {
                announced = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        if !announced {
            return Err("toast did not raise a UIA notification event".to_string());
        }
        Ok(())
    })();
    let _ = unsafe {
        automation.RemoveAutomationEventHandler(UIA_NotificationEventId, root, &spy)
    };
    result
}

/// Open the popover demo from its trigger, check that the card group and its
/// close control reach a client, then invoke the close button and require both
/// to leave the tree.
fn flyout_round(
    automation: &IUIAutomation,
    root: &IUIAutomationElement,
    seen: &[Seen],
) -> Result<(), String> {
    let walker = unsafe { automation.ControlViewWalker() }
        .map_err(|error| format!("flyout ControlViewWalker: {error}"))?;
    let collect_page = |walker: &IUIAutomationTreeWalker| -> Result<Vec<Seen>, String> {
        let mut page = Vec::new();
        let mut dump = String::new();
        unsafe { collect(walker, root, 0, &mut page, &mut dump)? };
        Ok(page)
    };
    let open = find(seen, "flyout-open").ok_or("missing flyout trigger")?;
    let element = open.element.as_ref().ok_or("flyout trigger element gone")?;
    let invoke: IUIAutomationInvokePattern =
        unsafe { element.GetCurrentPattern(UIA_InvokePatternId) }
            .map_err(|error| format!("flyout trigger InvokePattern: {error}"))?
            .cast()
            .map_err(|error| format!("flyout trigger cast: {error}"))?;
    unsafe { invoke.Invoke() }.map_err(|error| format!("flyout trigger Invoke: {error}"))?;
    std::thread::sleep(std::time::Duration::from_millis(300));
    let opened = collect_page(&walker)?;
    let card = find(&opened, "flyout").ok_or("missing popover card")?;
    require(
        card.control == UIA_GroupControlTypeId.0 && card.name == "弹出层提示",
        format!("popover card {:?} name {:?}", card.control, card.name),
    )?;
    let close = find(&opened, "flyout-close").ok_or("missing flyout close button")?;
    let element = close.element.as_ref().ok_or("flyout close element gone")?;
    let invoke: IUIAutomationInvokePattern =
        unsafe { element.GetCurrentPattern(UIA_InvokePatternId) }
            .map_err(|error| format!("flyout close InvokePattern: {error}"))?
            .cast()
            .map_err(|error| format!("flyout close cast: {error}"))?;
    unsafe { invoke.Invoke() }.map_err(|error| format!("flyout close Invoke: {error}"))?;
    std::thread::sleep(std::time::Duration::from_millis(300));
    let closed = collect_page(&walker)?;
    require(
        find(&closed, "flyout").is_none(),
        "the popover card must leave the tree after closing",
    )?;
    require(
        find(&closed, "flyout-close").is_none(),
        "the flyout content must leave the tree after closing",
    )?;
    Ok(())
}

/// Switch to the "功能区" tab and check the ribbon: a `Tab` with four
/// `TabItem` headers (one selected), the selected tab's commands as `Button`
/// children, a working SelectionItem on a header and an Invoke that runs a
/// command and reaches the application's state label.
fn ribbon_round(
    automation: &IUIAutomation,
    root: &IUIAutomationElement,
) -> Result<(), String> {
    let walker = unsafe { automation.ControlViewWalker() }
        .map_err(|error| format!("ribbon ControlViewWalker: {error}"))?;
    let collect_page = |walker: &IUIAutomationTreeWalker| -> Result<Vec<Seen>, String> {
        let mut page = Vec::new();
        let mut dump = String::new();
        unsafe { collect(walker, root, 0, &mut page, &mut dump)? };
        Ok(page)
    };
    let (page, _) = select_tab(automation, root, "功能区")?;

    let ribbon = find(&page, "gallery-ribbon").ok_or("missing ribbon")?;
    require(
        ribbon.control == UIA_TabControlTypeId.0
            && ribbon.name == "功能区"
            && ribbon.selection == Some(1),
        format!(
            "ribbon {:?} name {:?} selection {:?}",
            ribbon.control, ribbon.name, ribbon.selection
        ),
    )?;
    let headers: Vec<&Seen> = page
        .iter()
        .filter(|entry| {
            entry.id == "gallery-ribbon" && entry.control == UIA_TabItemControlTypeId.0
        })
        .collect();
    require(headers.len() == 4, format!("{} ribbon tabs", headers.len()))?;
    require(
        headers.iter().filter(|entry| entry.selected_item).count() == 1,
        "exactly one ribbon tab must be selected",
    )?;
    let names: Vec<&str> = headers.iter().map(|entry| entry.name.as_str()).collect();
    require(
        names.iter().any(|name| name.starts_with("开始")),
        format!("ribbon tab names: {names:?}"),
    )?;
    let commands: Vec<&Seen> = page
        .iter()
        .filter(|entry| {
            entry.id == "gallery-ribbon" && entry.control == UIA_ButtonControlTypeId.0
        })
        .collect();
    require(
        commands.iter().any(|entry| entry.name == "粘贴"),
        format!(
            "ribbon commands: {:?}",
            commands.iter().map(|entry| &entry.name).collect::<Vec<_>>()
        ),
    )?;

    // Select the 插入 header through SelectionItem: its commands publish.
    let insert = headers
        .iter()
        .find(|entry| entry.name.starts_with("插入"))
        .ok_or("missing 插入 tab")?;
    let element = insert.element.as_ref().ok_or("insert tab element gone")?;
    let pattern: IUIAutomationSelectionItemPattern =
        unsafe { element.GetCurrentPattern(UIA_SelectionItemPatternId) }
            .map_err(|error| format!("insert tab SelectionItemPattern: {error}"))?
            .cast()
            .map_err(|error| format!("insert tab cast: {error}"))?;
    unsafe { pattern.Select() }.map_err(|error| format!("insert tab Select: {error}"))?;
    std::thread::sleep(std::time::Duration::from_millis(300));
    let picked = collect_page(&walker)?;
    let commands: Vec<&Seen> = picked
        .iter()
        .filter(|entry| {
            entry.id == "gallery-ribbon" && entry.control == UIA_ButtonControlTypeId.0
        })
        .collect();
    let command = commands
        .iter()
        .find(|entry| entry.name == "添加封面")
        .ok_or("the 插入 tab did not publish 添加封面")?;
    let element = command.element.as_ref().ok_or("command element gone")?;
    let invoke: IUIAutomationInvokePattern =
        unsafe { element.GetCurrentPattern(UIA_InvokePatternId) }
            .map_err(|error| format!("ribbon command InvokePattern: {error}"))?
            .cast()
            .map_err(|error| format!("ribbon command cast: {error}"))?;
    unsafe { invoke.Invoke() }.map_err(|error| format!("ribbon command Invoke: {error}"))?;
    std::thread::sleep(std::time::Duration::from_millis(300));
    let after = collect_page(&walker)?;
    let state = find(&after, "ribbon_state").ok_or("missing ribbon state label")?;
    require(
        state.name == "最近命令：添加封面",
        format!("ribbon state after invoke: {:?}", state.name),
    )?;
    Ok(())
}

/// Switch to the "属性" tab and check the property grid: a `List` over the
/// visible rows with exactly one selected and a Scroll pattern, the key/value
/// rows as `ListItem`s, and a SelectionItem on the editable row that runs the
/// application edit message and changes the value through typed access.
fn props_round(
    automation: &IUIAutomation,
    root: &IUIAutomationElement,
) -> Result<(), String> {
    let walker = unsafe { automation.ControlViewWalker() }
        .map_err(|error| format!("props ControlViewWalker: {error}"))?;
    let collect_page = |walker: &IUIAutomationTreeWalker| -> Result<Vec<Seen>, String> {
        let mut page = Vec::new();
        let mut dump = String::new();
        unsafe { collect(walker, root, 0, &mut page, &mut dump)? };
        Ok(page)
    };
    let (page, _) = select_tab(automation, root, "属性")?;

    let grid = find(&page, "gallery-props").ok_or("missing property grid")?;
    require(
        grid.control == UIA_ListControlTypeId.0
            && grid.name == "曲目属性"
            && grid.selection == Some(1),
        format!(
            "property grid {:?} name {:?} selection {:?}",
            grid.control, grid.name, grid.selection
        ),
    )?;
    let scroll = grid
        .scroll
        .ok_or("the property grid must publish a Scroll pattern")?;
    require(
        scroll.vertical_scrollable && scroll.vertical_percent < 1.0,
        format!("property grid scroll state: {scroll:?}"),
    )?;
    let rows: Vec<&Seen> = page
        .iter()
        .filter(|entry| {
            entry.id == "gallery-props" && entry.control == UIA_ListItemControlTypeId.0
        })
        .collect();
    require(rows.len() >= 10, format!("{} property rows", rows.len()))?;
    require(
        rows.iter()
            .any(|entry| entry.name == "名称: Demo Track.flac")
            && rows.iter().any(|entry| entry.name == "收藏: 是"),
        format!(
            "property row names: {:?}",
            rows.iter().map(|entry| &entry.name).collect::<Vec<_>>()
        ),
    )?;
    require(
        rows.iter().filter(|entry| entry.selected_item).count() == 1,
        "exactly one property row must be the cursor",
    )?;

    // Select a row through SelectionItem: the cursor moves and the
    // application's state line follows; a published row is one logical
    // element, so selecting it never edits the value.
    let target = rows
        .iter()
        .find(|entry| entry.name == "收藏: 是")
        .ok_or("the 收藏 row did not publish")?;
    let element = target.element.as_ref().ok_or("property row element gone")?;
    let select: IUIAutomationSelectionItemPattern =
        unsafe { element.GetCurrentPattern(UIA_SelectionItemPatternId) }
            .map_err(|error| format!("property row SelectionItemPattern: {error}"))?
            .cast()
            .map_err(|error| format!("property row cast: {error}"))?;
    unsafe { select.Select() }.map_err(|error| format!("property row Select: {error}"))?;
    std::thread::sleep(std::time::Duration::from_millis(300));
    let after = collect_page(&walker)?;
    let selected: Vec<&Seen> = after
        .iter()
        .filter(|entry| {
            entry.id == "gallery-props"
                && entry.control == UIA_ListItemControlTypeId.0
                && entry.selected_item
        })
        .collect();
    require(
        selected.len() == 1 && selected[0].name == "收藏: 是",
        format!(
            "selected property rows after Select: {:?}",
            selected.iter().map(|entry| &entry.name).collect::<Vec<_>>()
        ),
    )?;
    let state = find(&after, "props_state").ok_or("missing property state label")?;
    require(
        state.name.contains("收藏"),
        format!("property state after Select: {:?}", state.name),
    )?;

    // A text row publishes an editable Value: SetValue applies the label the
    // way Enter does and the application mirrors it (D103).
    let name_row = after
        .iter()
        .find(|entry| {
            entry.id == "gallery-props"
                && entry.control == UIA_ListItemControlTypeId.0
                && entry.name == "名称: Demo Track.flac"
        })
        .and_then(|entry| entry.element.clone())
        .ok_or("missing editable name row")?;
    let value: IUIAutomationValuePattern =
        unsafe { name_row.GetCurrentPattern(UIA_ValuePatternId) }
            .map_err(|error| format!("property row ValuePattern: {error}"))?
            .cast()
            .map_err(|error| format!("property row Value cast: {error}"))?;
    let current = unsafe { value.CurrentValue() }
        .map(|value| value.to_string())
        .map_err(|error| format!("property row CurrentValue: {error}"))?;
    require(
        current == "Demo Track.flac",
        format!("property value before SetValue: {current:?}"),
    )?;
    let readonly = unsafe { value.CurrentIsReadOnly() }
        .map(|flag| flag.as_bool())
        .unwrap_or(true);
    require(!readonly, "an editable property row must accept SetValue")?;
    unsafe { value.SetValue(&BSTR::from("Demo 2.flac")) }
        .map_err(|error| format!("property row SetValue: {error}"))?;
    std::thread::sleep(std::time::Duration::from_millis(300));
    let edited = collect_page(&walker)?;
    require(
        edited.iter().any(|entry| {
            entry.id == "gallery-props"
                && entry.control == UIA_ListItemControlTypeId.0
                && entry.name == "名称: Demo 2.flac"
        }),
        "the SetValue label did not reach the tree",
    )?;
    let state = find(&edited, "props_state").ok_or("missing property state after SetValue")?;
    require(
        state.name.contains("修改 名称：Demo 2.flac"),
        format!("property state after SetValue: {:?}", state.name),
    )?;
    Ok(())
}

fn find<'a>(seen: &'a [Seen], id: &str) -> Option<&'a Seen> {
    seen.iter().find(|entry| entry.id == id)
}
fn require(condition: bool, message: impl Into<String>) -> Result<(), String> {
    if condition { Ok(()) } else { Err(message.into()) }
}

/// Select a catalog tab by name from a fresh snapshot and return the refreshed
/// tree plus its dump. Every round starts here, so an element captured before
/// an earlier round cannot go stale (the application rebuilds the accessibility
/// tree on a page switch).
fn select_tab(
    automation: &IUIAutomation,
    root: &IUIAutomationElement,
    name: &str,
) -> Result<(Vec<Seen>, String), String> {
    let walker = unsafe { automation.ControlViewWalker() }
        .map_err(|error| format!("tab ControlViewWalker: {error}"))?;
    let snapshot = |walker: &IUIAutomationTreeWalker| -> Result<(Vec<Seen>, String), String> {
        let mut page = Vec::new();
        let mut dump = String::new();
        unsafe { collect(walker, root, 0, &mut page, &mut dump)? };
        Ok((page, dump))
    };
    let (page, _) = snapshot(&walker)?;
    let tab = page
        .iter()
        .find(|entry| {
            entry.id == "tabs"
                && entry.control == UIA_TabItemControlTypeId.0
                && entry.name == name
        })
        .and_then(|entry| entry.element.clone())
        .ok_or_else(|| format!("missing {name} tab"))?;
    let pattern: IUIAutomationSelectionItemPattern =
        unsafe { tab.GetCurrentPattern(UIA_SelectionItemPatternId) }
            .map_err(|error| format!("{name} tab SelectionItemPattern: {error}"))?
            .cast()
            .map_err(|error| format!("{name} tab cast: {error}"))?;
    unsafe { pattern.Select() }.map_err(|error| format!("{name} tab Select: {error}"))?;
    std::thread::sleep(std::time::Duration::from_millis(400));
    snapshot(&walker)
}

fn check(seen: &[Seen]) -> Result<(), String> {
    let root = seen.first().ok_or("empty UIA tree")?;
    require(
        root.control == UIA_WindowControlTypeId.0 || root.control == UIA_PaneControlTypeId.0,
        format!("root control type {:?} is not a window/pane", root.control),
    )?;
    require(
        root.name == TITLE,
        format!("root name {:?}, expected {TITLE:?}", root.name),
    )?;

    let primary = find(seen, "primary").ok_or("missing primary button")?;
    require(
        primary.control == UIA_ButtonControlTypeId.0 && primary.invoke,
        "primary button is not an invokable button",
    )?;
    require(
        primary.name.starts_with("主按钮"),
        format!("primary name {:?}", primary.name),
    )?;

    let toggle = find(seen, "toggle").ok_or("missing toggle")?;
    require(
        toggle.control == UIA_CheckBoxControlTypeId.0 && toggle.toggled == Some(true),
        "toggle is not a checked checkbox",
    )?;

    let volume = find(seen, "volume").ok_or("missing slider")?;
    require(
        volume.control == UIA_SliderControlTypeId.0
            && volume.range.is_some_and(|value| (value - 0.6).abs() < 0.001),
        format!("slider range {:?}", volume.range),
    )?;

    let search = find(seen, "search").ok_or("missing search field")?;
    require(
        search.control == UIA_EditControlTypeId.0 && search.value.as_deref() == Some(SEARCH_TEXT),
        format!("search value {:?}", search.value),
    )?;
    require(
        search.text.as_deref() == Some(SEARCH_TEXT),
        format!("search text range {:?}", search.text),
    )?;
    // The closed field models its caret at the end of the value: one
    // selection range that is degenerate (empty text), and a single-selection
    // text pattern.
    require(
        search.supported_selection == Some(SupportedTextSelection_Single.0),
        format!("search SupportedTextSelection {:?}", search.supported_selection),
    )?;
    require(
        search.selection_ranges == Some(1) && search.selection_text.as_deref() == Some(""),
        format!(
            "search selection {:?}/{:?}",
            search.selection_ranges, search.selection_text
        ),
    )?;

    let segmented = find(seen, "segmented").ok_or("missing segmented control")?;
    require(
        segmented.control == UIA_GroupControlTypeId.0 && segmented.selection == Some(1),
        format!("segmented selection {:?}", segmented.selection),
    )?;

    let tabs = find(seen, "tabs").ok_or("missing tabs")?;
    require(
        tabs.control == UIA_TabControlTypeId.0 && tabs.selection == Some(1),
        format!("tabs selection {:?}", tabs.selection),
    )?;

    let list = find(seen, "list").ok_or("missing list")?;
    require(
        list.control == UIA_ListControlTypeId.0 && list.selection == Some(1),
        format!("list selection {:?}", list.selection),
    )?;
    let rows: Vec<&Seen> = seen
        .iter()
        .filter(|entry| entry.id == "list" && entry.control == UIA_ListItemControlTypeId.0)
        .collect();
    require(
        rows.len() >= 5,
        format!("only {} list rows published", rows.len()),
    )?;
    require(
        rows.len() < 40,
        format!("{} rows published; only the viewport should be exposed", rows.len()),
    )?;
    require(
        rows.iter().any(|row| row.selected_item),
        "no list row reports SelectionItem.IsSelected",
    )?;
    require(
        rows.iter().any(|row| row.offscreen),
        "the look-ahead row must report IsOffscreen",
    )?;
    require(
        rows.iter().filter(|row| !row.offscreen).count() >= 5,
        "IsOffscreen marked the whole viewport offscreen",
    )?;
    require(
        rows.iter()
            .all(|row| row.name.starts_with("第 ") && row.name.contains("首，")),
        "row narration did not reach the UIA names",
    )?;

    // Per-instance properties reach the client: the secondary button uses a
    // custom AutomationId and a description.
    let secondary = find(seen, "secondary-action").ok_or("missing secondary-action")?;
    require(
        secondary.control == UIA_ButtonControlTypeId.0
            && secondary.description.as_deref() == Some("次操作：只弹出提示"),
        format!(
            "secondary id/description {:?}/{:?}",
            secondary.id, secondary.description
        ),
    )?;

    // Display widgets stay out of the tree unless the instance opts in: the
    // heading is focusable and narrated, the swatch is a button with a
    // description, the progress bar is an exposed range value.
    let heading = find(seen, "list_title").ok_or("missing narrated heading")?;
    require(
        heading.control == UIA_TextControlTypeId.0,
        format!("heading control type {:?}", heading.control),
    )?;
    require(
        heading.description.as_deref() == Some("虚拟列表：只发布当前视口内的行"),
        format!("heading description {:?}", heading.description),
    )?;

    let swatch = find(seen, "color-swatch").ok_or("missing clickable swatch")?;
    require(
        swatch.control == UIA_ButtonControlTypeId.0,
        format!("swatch control type {:?}, expected button", swatch.control),
    )?;
    require(
        swatch.name == "竹绿" && swatch.description.as_deref().is_some_and(|text| text.contains("#7FD3A8")),
        format!("swatch name/description {:?}/{:?}", swatch.name, swatch.description),
    )?;

    let progress = find(seen, "progress").ok_or("missing exposed progress bar")?;
    require(
        progress.control == UIA_ProgressBarControlTypeId.0
            && progress.range.is_some_and(|value| (value - 0.6).abs() < 0.001),
        format!("progress bar {:?} range {:?}", progress.control, progress.range),
    )?;


    Ok(())
}
/// Switch to the "详情" tab and check the text/content widgets. They are
/// decoration by default; these instances opted in, so each reaches the client
/// as a text element whose name is the painted string (the link's name comes
/// from narration).
fn text_round(automation: &IUIAutomation, root: &IUIAutomationElement) -> Result<(), String> {
    let walker = unsafe { automation.ControlViewWalker() }
        .map_err(|error| format!("text ControlViewWalker: {error}"))?;
    let mut initial = Vec::new();
    let mut dump0 = String::new();
    unsafe { collect(&walker, root, 0, &mut initial, &mut dump0)? };
    let tab = initial
        .iter()
        .find(|entry| {
            entry.id == "tabs"
                && entry.control == UIA_TabItemControlTypeId.0
                && entry.name == "详情"
        })
        .and_then(|entry| entry.element.clone())
        .ok_or("missing 详情 tab")?;
    let pattern: IUIAutomationSelectionItemPattern =
        unsafe { tab.GetCurrentPattern(UIA_SelectionItemPatternId) }
            .map_err(|error| format!("text tab SelectionItemPattern: {error}"))?
            .cast()
            .map_err(|error| format!("text tab cast: {error}"))?;
    unsafe { pattern.Select() }.map_err(|error| format!("text tab Select: {error}"))?;
    std::thread::sleep(std::time::Duration::from_millis(400));
    let mut page = Vec::new();
    let mut dump = String::new();
    unsafe { collect(&walker, root, 0, &mut page, &mut dump)? };
    check_text(&page).map_err(|error| format!("{error}
--- text tree ---
{dump}"))
}

fn check_text(seen: &[Seen]) -> Result<(), String> {
    // Text and content widgets. They are decoration by default; these
    // instances opted in, so each reaches the client as a text element whose
    // name is the painted string (the link's name comes from narration).
    let divider = find(seen, "content-divider").ok_or("missing labeled divider")?;
    require(
        divider.control == UIA_TextControlTypeId.0 && divider.name == "文本与内容",
        format!("divider {:?} name {:?}", divider.control, divider.name),
    )?;

    let wrap = find(seen, "wrap-label").ok_or("missing wrapping label")?;
    require(
        wrap.control == UIA_TextControlTypeId.0 && wrap.name.starts_with("自动换行"),
        format!("wrap label {:?} name {:?}", wrap.control, wrap.name),
    )?;

    let badge = find(seen, "badge-lossless").ok_or("missing exposed badge")?;
    require(
        badge.control == UIA_TextControlTypeId.0 && badge.name == "无损",
        format!("badge {:?} name {:?}", badge.control, badge.name),
    )?;

    let kbd = find(seen, "kbd-search").ok_or("missing shortcut hint")?;
    require(
        kbd.control == UIA_TextControlTypeId.0 && kbd.name == "Ctrl+K",
        format!("kbd {:?} name {:?}", kbd.control, kbd.name),
    )?;

    let avatar = find(seen, "avatar").ok_or("missing avatar")?;
    require(
        avatar.control == UIA_TextControlTypeId.0 && avatar.name == "竹韵",
        format!("avatar {:?} name {:?}", avatar.control, avatar.name),
    )?;

    let link = find(seen, "link-label").ok_or("missing link label")?;
    require(
        link.control == UIA_TextControlTypeId.0 && link.name == "打开歌词页",
        format!("link label {:?} name {:?}", link.control, link.name),
    )?;

    let copy = find(seen, "copy-text").ok_or("missing copyable text")?;
    require(
        copy.control == UIA_TextControlTypeId.0 && copy.name == "Bamboo Native UI",
        format!("copyable text {:?} name {:?}", copy.control, copy.name),
    )?;

    let marquee = find(seen, "marquee").ok_or("missing marquee")?;
    require(
        marquee.control == UIA_TextControlTypeId.0
            && marquee.name == "正在播放：竹韵 · 夜曲 · 无损音质",
        format!("marquee {:?} name {:?}", marquee.control, marquee.name),
    )?;

    // Rich text publishes its plain concatenation, not one node per run.
    let rich = find(seen, "rich-text").ok_or("missing rich text")?;
    require(
        rich.control == UIA_TextControlTypeId.0 && rich.name == "内联样式：重点，次要，详情",
        format!("rich text {:?} name {:?}", rich.control, rich.name),
    )?;

    let avatar_image = find(seen, "avatar-image").ok_or("missing image avatar")?;
    require(
        avatar_image.control == UIA_TextControlTypeId.0 && avatar_image.name == "夜曲",
        format!(
            "image avatar {:?} name {:?}",
            avatar_image.control, avatar_image.name
        ),
    )?;

    Ok(())
}

/// The `布局` page (showcase `m7` feature): the layout skin is a normal widget
/// tree, including the third-party meter whose `x-*` vocabulary the library
/// never sees. It publishes a named ProgressBar with the value from the file.
#[cfg(feature = "m7")]
fn layout_round(automation: &IUIAutomation, root: &IUIAutomationElement) -> Result<(), String> {
    let (seen, dump) = select_tab(automation, root, "布局")?;
    let meter = find(&seen, "m7_meter")
        .ok_or_else(|| format!("missing m7_meter\n--- UIA tree ---\n{dump}"))?;
    require(
        meter.control == UIA_ProgressBarControlTypeId.0,
        format!(
            "meter control {} ({})",
            control_name(meter.control),
            dump
        ),
    )?;
    require(
        meter.name == "信号表",
        format!("meter name {:?}", meter.name),
    )?;
    require(
        meter.value.as_deref() == Some("72 dB"),
        format!("meter value {:?}", meter.value),
    )?;
    require(
        meter.range == Some(72.0),
        format!("meter range {:?}", meter.range),
    )?;
    Ok(())
}
