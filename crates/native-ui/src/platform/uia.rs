// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! UI Automation provider for the retained widget tree.
//!
//! The provider is a snapshot: `Ui` builds [`A11yNode`]s on the UI thread and
//! the host refreshes them in place on focus changes and actions, so a client
//! holding a provider sees current names, values and bounds. Pattern methods
//! never touch the application directly — they post an [`A11yRequest`] back to
//! the window, so actions run on the UI thread through the widget input path.

use std::sync::{Arc, Mutex};

use windows::core::{BOOL, BSTR, IUnknown, PCWSTR, Result, implement};
use windows::Win32::Foundation::{E_FAIL, E_INVALIDARG, E_NOTIMPL, HWND, LPARAM, WPARAM};
use windows::Win32::System::Com::SAFEARRAY;
use windows::Win32::System::Ole::{SafeArrayCreateVector, SafeArrayDestroy, SafeArrayPutElement};
use windows::Win32::System::Variant::{
    InitVariantFromDoubleArray, VARIANT, VT_BOOL, VT_BSTR, VT_I4, VT_R8, VT_UNKNOWN,
};
use windows::Win32::UI::Accessibility::{
    ExpandCollapseState, ExpandCollapseState_Collapsed, ExpandCollapseState_Expanded,
    ExpandCollapseState_LeafNode, HeadingLevel2, IExpandCollapseProvider_Impl,
    IGridItemProvider_Impl, IGridProvider_Impl,
    IInvokeProvider_Impl, IRangeValueProvider_Impl,
    IRawElementProviderFragmentRoot_Impl, IRawElementProviderFragment_Impl,
    IRawElementProviderSimple, IRawElementProviderSimple_Impl, IScrollProvider_Impl,
    ISelectionItemProvider_Impl,
    ISelectionProvider_Impl, ITextProvider_Impl, ITextRangeProvider, ITextRangeProvider_Impl,
    IToggleProvider_Impl, IValueProvider_Impl, NavigateDirection_FirstChild,
    NavigateDirection_LastChild, NavigateDirection_NextSibling, NavigateDirection_Parent,
    NavigateDirection_PreviousSibling, NotificationKind_ActionCompleted,
    NotificationProcessing_ImportantMostRecent, NotificationProcessing_MostRecent, ProviderOptions,
    ProviderOptions_ServerSideProvider,
    ScrollAmount, ScrollAmount_LargeDecrement, ScrollAmount_NoAmount,
    ScrollAmount_SmallDecrement, ScrollAmount_SmallIncrement,
    SupportedTextSelection_None, SupportedTextSelection_Single, TextPatternRangeEndpoint, TextUnit,
    ToggleState_Off, ToggleState_On, UiaAppendRuntimeId, UiaClientsAreListening,
    UiaHostProviderFromHwnd,
    UiaRaiseAutomationEvent, UiaRaiseAutomationPropertyChangedEvent, UiaRaiseNotificationEvent,
    UiaRect, UIA_AutomationIdPropertyId, UIA_BoundingRectanglePropertyId,
    UIA_ButtonControlTypeId, UIA_CheckBoxControlTypeId, UIA_ComboBoxControlTypeId,
    UIA_ControlTypePropertyId, UIA_DataGridControlTypeId, UIA_DataItemControlTypeId,
    UIA_EditControlTypeId, UIA_ExpandCollapseExpandCollapseStatePropertyId,
    UIA_ExpandCollapsePatternId,
    UIA_FullDescriptionPropertyId, UIA_GroupControlTypeId,
    UIA_HeaderItemControlTypeId,
    UIA_GridColumnCountPropertyId, UIA_GridItemColumnPropertyId,
    UIA_GridItemColumnSpanPropertyId,
    UIA_GridItemPatternId, UIA_GridItemRowPropertyId, UIA_GridItemRowSpanPropertyId,
    UIA_GridPatternId, UIA_GridRowCountPropertyId,
    UIA_HasKeyboardFocusPropertyId, UIA_HeadingLevelPropertyId, UIA_HelpTextPropertyId,
    UIA_InvokePatternId, UIA_IsContentElementPropertyId, UIA_IsControlElementPropertyId,
    UIA_IsEnabledPropertyId, UIA_IsKeyboardFocusablePropertyId, UIA_IsOffscreenPropertyId,
    UIA_ListItemControlTypeId, UIA_ListControlTypeId, UIA_MenuBarControlTypeId,
    UIA_MenuControlTypeId, UIA_MenuItemControlTypeId, UIA_NamePropertyId, UIA_PATTERN_ID,
    UIA_PaneControlTypeId, UIA_ProgressBarControlTypeId, UIA_PROPERTY_ID,
    UIA_RadioButtonControlTypeId, UIA_RangeValueMaximumPropertyId, UIA_RangeValueMinimumPropertyId,
    UIA_RangeValuePatternId, UIA_RangeValueValuePropertyId,
    UIA_ScrollHorizontalScrollPercentPropertyId, UIA_ScrollHorizontalViewSizePropertyId,
    UIA_ScrollHorizontallyScrollablePropertyId, UIA_ScrollPatternId, UIA_ScrollPatternNoScroll,
    UIA_ScrollVerticalScrollPercentPropertyId, UIA_ScrollVerticalViewSizePropertyId,
    UIA_ScrollVerticallyScrollablePropertyId,
    UIA_SelectionItemIsSelectedPropertyId,
    UIA_SelectionItemPatternId, UIA_SelectionItem_ElementSelectedEventId, UIA_SelectionPatternId,
    UIA_SeparatorControlTypeId, UIA_SliderControlTypeId, UIA_StatusBarControlTypeId,
    UIA_TabControlTypeId,
    UIA_TabItemControlTypeId, UIA_TextControlTypeId, UIA_ToolBarControlTypeId,
    UIA_TreeControlTypeId, UIA_TreeItemControlTypeId,
    UIA_TextPatternId, UIA_TogglePatternId, UIA_ToggleToggleStatePropertyId,
    UIA_ValueIsReadOnlyPropertyId, UIA_ValuePatternId, UIA_ValueValuePropertyId,
};
use windows::Win32::UI::WindowsAndMessaging::PostMessageW;

use crate::a11y::{A11yNode, A11yRole, A11yScrollAxis, AnnouncementPriority};
use crate::geometry::Rect;
use crate::widget::WidgetId;

/// Posted by a pattern method; `LPARAM` owns a boxed [`A11yRequest`].
pub(crate) const WM_APP_A11Y_ACTION: u32 = 0x8000 + 2;

/// Work an assistive technology asked for, carrying the element's stable
/// identity (widget id + option/row index) rather than a snapshot index. A
/// refresh between the client's walk and the call then cannot redirect the
/// action to a different control.
pub(crate) enum A11yRequest {
    Invoke(WidgetId, Option<usize>),
    Toggle(WidgetId, Option<usize>),
    Expand(WidgetId, Option<usize>, bool),
    Select(WidgetId, Option<usize>),
    Focus(WidgetId, Option<usize>),
    SetText(WidgetId, Option<usize>, String),
    SetNumber(WidgetId, Option<usize>, f64),
    /// Absolute target offsets in DIPs; `None` leaves the axis alone.
    Scroll(WidgetId, Option<usize>, Option<f32>, Option<f32>),
}

struct A11yState {
    origin_x: i32,
    origin_y: i32,
    scale: f32,
    root_name: String,
    root_rect: Rect,
    nodes: Vec<A11yNode>,
}

/// Accessibility snapshot shared by every provider it produces. The tree is
/// refreshed in place so long-lived provider objects stay current.
pub(crate) struct A11yTree {
    hwnd: isize,
    state: Mutex<A11yState>,
}

impl A11yTree {
    pub fn new(
        hwnd: HWND,
        origin_x: i32,
        origin_y: i32,
        scale: f32,
        root_name: String,
        root_rect: Rect,
        nodes: Vec<A11yNode>,
    ) -> Arc<Self> {
        Arc::new(Self {
            hwnd: hwnd.0 as isize,
            state: Mutex::new(A11yState {
                origin_x,
                origin_y,
                scale,
                root_name,
                root_rect,
                nodes,
            }),
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn refresh(
        &self,
        origin_x: i32,
        origin_y: i32,
        scale: f32,
        root_name: String,
        root_rect: Rect,
        nodes: Vec<A11yNode>,
    ) -> Vec<A11yChange> {
        let Ok(mut state) = self.state.lock() else {
            return Vec::new();
        };
        let changes = diff(&state.nodes, &nodes);
        *state = A11yState {
            origin_x,
            origin_y,
            scale,
            root_name,
            root_rect,
            nodes,
        };
        changes
    }

    fn hwnd(&self) -> HWND {
        HWND(self.hwnd as *mut core::ffi::c_void)
    }

    pub(crate) fn node(&self, index: usize) -> Option<A11yNode> {
        self.state.lock().ok()?.nodes.get(index).cloned()
    }

    pub(crate) fn node_count(&self) -> usize {
        self.state.lock().map(|state| state.nodes.len()).unwrap_or(0)
    }

    fn root_name(&self) -> String {
        self.state
            .lock()
            .map(|state| state.root_name.clone())
            .unwrap_or_default()
    }

    fn root_rect(&self) -> Rect {
        self.state.lock().map(|state| state.root_rect).unwrap_or_default()
    }

    /// Node that represents keyboard focus for `id`: the selected option/row
    /// of a focused container, otherwise the widget node itself. Composite
    /// controls publish focus on the item so screen readers announce it.
    pub(crate) fn focus_index(&self, id: WidgetId) -> Option<usize> {
        let state = self.state.lock().ok()?;
        state
            .nodes
            .iter()
            .position(|node| node.focused && node.id == id)
            .or_else(|| {
                state
                    .nodes
                    .iter()
                    .position(|node| node.id == id && node.child.is_none())
            })
    }

    fn focused(&self) -> Option<usize> {
        let state = self.state.lock().ok()?;
        state.nodes.iter().position(|node| node.focused)
    }

    fn hit(&self, x: f32, y: f32) -> Option<usize> {
        let state = self.state.lock().ok()?;
        state
            .nodes
            .iter()
            .rposition(|node| node.rect.contains(x, y))
    }

    fn sibling(&self, index: usize, step: isize) -> Option<usize> {
        let state = self.state.lock().ok()?;
        let parent = state.nodes.get(index)?.parent;
        let siblings: Vec<usize> = state
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| node.parent == parent)
            .map(|(index, _)| index)
            .collect();
        let position = siblings.iter().position(|candidate| *candidate == index)?;
        siblings
            .get((position as isize + step) as usize)
            .copied()
    }

    fn first_child(&self, index: Option<usize>) -> Option<usize> {
        let state = self.state.lock().ok()?;
        match index {
            None => state.nodes.iter().position(|node| node.parent.is_none()),
            Some(index) => state.nodes.get(index)?.children.first().copied(),
        }
    }

    fn last_child(&self, index: Option<usize>) -> Option<usize> {
        let state = self.state.lock().ok()?;
        match index {
            None => state.nodes.iter().rposition(|node| node.parent.is_none()),
            Some(index) => state.nodes.get(index)?.children.last().copied(),
        }
    }

    /// Screen pixel rectangle `[left, top, width, height]` for a client DIP
    /// rectangle.
    fn screen_rect(&self, rect: Rect) -> [f64; 4] {
        let state = match self.state.lock() {
            Ok(state) => state,
            Err(_) => return [0.0; 4],
        };
        let scale = state.scale as f64;
        [
            state.origin_x as f64 + rect.left as f64 * scale,
            state.origin_y as f64 + rect.top as f64 * scale,
            rect.width() as f64 * scale,
            rect.height() as f64 * scale,
        ]
    }
}

/// Property change detected while refreshing the snapshot. Screen readers
/// announce a toggle or slider only when they receive these notifications;
/// the focus event alone does not describe the new state.
pub(crate) enum A11yChange {
    Toggled { index: usize, old: bool, new: bool },
    Value { index: usize, old: String, new: String },
    Range { index: usize, old: f64, new: f64 },
    Name { index: usize, old: String, new: String },
    Selected { index: usize },
}

/// Compare two snapshots by element identity (widget id + option/row index).
fn diff(old: &[A11yNode], new: &[A11yNode]) -> Vec<A11yChange> {
    let mut changes = Vec::new();
    for (index, node) in new.iter().enumerate() {
        let Some(previous) = old
            .iter()
            .find(|previous| previous.id == node.id && previous.child == node.child)
        else {
            continue;
        };
        if matches!(node.role, A11yRole::Toggle | A11yRole::MenuItem)
            && previous.checked != node.checked
            && let (Some(old), Some(new)) = (previous.checked, node.checked)
        {
            changes.push(A11yChange::Toggled { index, old, new });
        }
        if matches!(node.role, A11yRole::TextField | A11yRole::NumberField)
            && previous.value != node.value
            && let (Some(old), Some(new)) = (&previous.value, &node.value)
        {
            changes.push(A11yChange::Value {
                index,
                old: old.clone(),
                new: new.clone(),
            });
        }
        if matches!(node.role, A11yRole::Slider | A11yRole::ProgressBar)
            && previous.range.map(|range| range.2) != node.range.map(|range| range.2)
            && let (Some(old), Some(new)) = (previous.range, node.range)
        {
            changes.push(A11yChange::Range {
                index,
                old: old.2,
                new: new.2,
            });
        }
        if previous.name != node.name {
            changes.push(A11yChange::Name {
                index,
                old: previous.name.clone(),
                new: node.name.clone(),
            });
        }
        if node.child.is_some() && previous.selected != Some(true) && node.selected == Some(true) {
            changes.push(A11yChange::Selected { index });
        }
    }
    changes
}

fn toggle_state(on: bool) -> i32 {
    if on { ToggleState_On.0 } else { ToggleState_Off.0 }
}

/// Raise UIA notifications for the property changes found during a refresh.
pub(crate) fn raise_changes(tree: &Arc<A11yTree>, changes: &[A11yChange]) {
    if changes.is_empty() || unsafe { !UiaClientsAreListening().as_bool() } {
        return;
    }
    for change in changes {
        let (index, property, old, new, selected) = match change {
            A11yChange::Toggled { index, old, new } => (
                *index,
                UIA_ToggleToggleStatePropertyId,
                unsafe { variant_i4(toggle_state(*old)) },
                unsafe { variant_i4(toggle_state(*new)) },
                false,
            ),
            A11yChange::Value { index, old, new } => (
                *index,
                UIA_ValueValuePropertyId,
                unsafe { variant_bstr(old) },
                unsafe { variant_bstr(new) },
                false,
            ),
            A11yChange::Range { index, old, new } => (
                *index,
                UIA_RangeValueValuePropertyId,
                unsafe { variant_f64(*old) },
                unsafe { variant_f64(*new) },
                false,
            ),
            A11yChange::Name { index, old, new } => (
                *index,
                UIA_NamePropertyId,
                unsafe { variant_bstr(old) },
                unsafe { variant_bstr(new) },
                false,
            ),
            A11yChange::Selected { index } => (
                *index,
                UIA_SelectionItemIsSelectedPropertyId,
                unsafe { variant_bool(false) },
                unsafe { variant_bool(true) },
                true,
            ),
        };
        let provider: IRawElementProviderSimple = UiaProvider::at(tree.clone(), Some(index)).into();
        unsafe {
            let _ = UiaRaiseAutomationPropertyChangedEvent(&provider, property, &old, &new);
            if selected {
                let _ = UiaRaiseAutomationEvent(&provider, UIA_SelectionItem_ElementSelectedEventId);
            }
        }
    }
}

/// Raise a UIA notification for transient status text (toasts, background
/// completion). Unlike focus events this is not tied to an element; the root
/// provider carries it, and screen readers announce it when they are
/// processing notifications. Polite keeps only the newest message; assertive
/// interrupts the current utterance.
pub(crate) fn announce(
    tree: &Arc<A11yTree>,
    text: &str,
    priority: AnnouncementPriority,
) {
    if text.is_empty() || unsafe { !UiaClientsAreListening().as_bool() } {
        return;
    }
    let processing = match priority {
        AnnouncementPriority::Polite => NotificationProcessing_MostRecent,
        AnnouncementPriority::Assertive => NotificationProcessing_ImportantMostRecent,
    };
    let provider: IRawElementProviderSimple = UiaProvider::at(tree.clone(), None).into();
    let display = BSTR::from(text);
    let activity = BSTR::new();
    unsafe {
        let _ = UiaRaiseNotificationEvent(
            &provider,
            NotificationKind_ActionCompleted,
            processing,
            &display,
            &activity,
        );
    }
}

/// Root element (index `None`) or one node of the snapshot.
#[implement(
    IRawElementProviderSimple,
    windows::Win32::UI::Accessibility::IRawElementProviderFragment,
    windows::Win32::UI::Accessibility::IRawElementProviderFragmentRoot,
    windows::Win32::UI::Accessibility::IInvokeProvider,
    windows::Win32::UI::Accessibility::IToggleProvider,
    windows::Win32::UI::Accessibility::IRangeValueProvider,
    windows::Win32::UI::Accessibility::IValueProvider,
    windows::Win32::UI::Accessibility::ISelectionItemProvider,
    windows::Win32::UI::Accessibility::ISelectionProvider,
    windows::Win32::UI::Accessibility::ITextProvider,
    windows::Win32::UI::Accessibility::IExpandCollapseProvider,
    windows::Win32::UI::Accessibility::IScrollProvider,
    windows::Win32::UI::Accessibility::IGridProvider,
    windows::Win32::UI::Accessibility::IGridItemProvider
)]
pub(crate) struct UiaProvider {
    tree: Arc<A11yTree>,
    index: Option<usize>,
}

impl UiaProvider {
    pub fn root(tree: Arc<A11yTree>) -> IRawElementProviderSimple {
        UiaProvider { tree, index: None }.into()
    }

    pub fn at(tree: Arc<A11yTree>, index: Option<usize>) -> Self {
        Self { tree, index }
    }

    fn node(&self) -> Option<A11yNode> {
        self.index.and_then(|index| self.tree.node(index))
    }

    /// One axis of this element's published scroll state.
    fn scroll_axis(&self, vertical: bool) -> Option<A11yScrollAxis> {
        scroll_axis(&self.node()?, vertical)
    }

    /// Stable identity of this element. Pattern calls carry it back to the UI
    /// thread so they resolve against the current tree by id, not by the
    /// snapshot index captured when the element object was created.
    fn target(&self) -> Option<(WidgetId, Option<usize>)> {
        self.node().map(|node| (node.id, node.child))
    }

    fn post(&self, request: A11yRequest) {
        let boxed = Box::into_raw(Box::new(request));
        let result = unsafe {
            PostMessageW(
                Some(self.tree.hwnd()),
                WM_APP_A11Y_ACTION,
                WPARAM(0),
                LPARAM(boxed as isize),
            )
        };
        if result.is_err() {
            drop(unsafe { Box::from_raw(boxed) });
        }
    }

    fn provider_at(&self, index: Option<usize>) -> IRawElementProviderSimple {
        UiaProvider {
            tree: self.tree.clone(),
            index,
        }
        .into()
    }

    fn fragment_at(
        &self,
        index: Option<usize>,
    ) -> windows::Win32::UI::Accessibility::IRawElementProviderFragment {
        UiaProvider {
            tree: self.tree.clone(),
            index,
        }
        .into()
    }
}

fn control_type(role: A11yRole) -> i32 {
    let id = match role {
        A11yRole::Button => UIA_ButtonControlTypeId,
        A11yRole::Toggle => UIA_CheckBoxControlTypeId,
        A11yRole::Slider => UIA_SliderControlTypeId,
        A11yRole::TextField | A11yRole::NumberField => UIA_EditControlTypeId,
        A11yRole::RadioGroup => UIA_GroupControlTypeId,
        A11yRole::RadioItem => UIA_RadioButtonControlTypeId,
        A11yRole::TabList => UIA_TabControlTypeId,
        A11yRole::TabItem => UIA_TabItemControlTypeId,
        A11yRole::List | A11yRole::MultiSelectList => UIA_ListControlTypeId,
        A11yRole::ListItem => UIA_ListItemControlTypeId,
        A11yRole::Tree => UIA_TreeControlTypeId,
        A11yRole::TreeItem => UIA_TreeItemControlTypeId,
        A11yRole::Grid => UIA_DataGridControlTypeId,
        A11yRole::GridCell => UIA_DataItemControlTypeId,
        A11yRole::ColumnHeader => UIA_HeaderItemControlTypeId,
        // UIA has no heading control type; a heading is text with a level.
        A11yRole::Heading | A11yRole::Text => UIA_TextControlTypeId,
        A11yRole::Group => UIA_GroupControlTypeId,
        A11yRole::Separator => UIA_SeparatorControlTypeId,
        A11yRole::ProgressBar => UIA_ProgressBarControlTypeId,
        A11yRole::ComboBox => UIA_ComboBoxControlTypeId,
        A11yRole::MenuItem => UIA_MenuItemControlTypeId,
        A11yRole::Menu => UIA_MenuControlTypeId,
        A11yRole::MenuBar => UIA_MenuBarControlTypeId,
        A11yRole::ToolBar => UIA_ToolBarControlTypeId,
        A11yRole::StatusBar => UIA_StatusBarControlTypeId,
    };
    id.0
}

fn supports_pattern(role: A11yRole, pattern: UIA_PATTERN_ID) -> bool {
    match role {
        A11yRole::Button => pattern == UIA_InvokePatternId,
        A11yRole::Toggle => pattern == UIA_TogglePatternId,
        A11yRole::Slider | A11yRole::ProgressBar => pattern == UIA_RangeValuePatternId,
        A11yRole::TextField | A11yRole::NumberField => {
            pattern == UIA_ValuePatternId || pattern == UIA_TextPatternId
        }
        A11yRole::RadioItem | A11yRole::TabItem | A11yRole::ListItem => {
            pattern == UIA_SelectionItemPatternId
        }
        // A tree row is selectable and expands/collapses; a leaf reports
        // `LeafNode` through the state property.
        A11yRole::TreeItem => {
            pattern == UIA_SelectionItemPatternId || pattern == UIA_ExpandCollapsePatternId
        }
        // A grid cell is selectable, reports its row/column coordinates and
        // its text as a Value.
        A11yRole::GridCell => {
            pattern == UIA_SelectionItemPatternId
                || pattern == UIA_GridItemPatternId
                || pattern == UIA_ValuePatternId
        }
        A11yRole::RadioGroup
        | A11yRole::TabList
        | A11yRole::List
        | A11yRole::MultiSelectList
        | A11yRole::Tree => pattern == UIA_SelectionPatternId,
        // The grid itself is a selectable container that also publishes its
        // row/column extent (IGridProvider).
        A11yRole::Grid => {
            pattern == UIA_SelectionPatternId || pattern == UIA_GridPatternId
        }
        A11yRole::ComboBox => pattern == UIA_ValuePatternId,
        A11yRole::MenuItem => pattern == UIA_InvokePatternId,
        A11yRole::ToolBar | A11yRole::StatusBar => false,
        // A painted menu opens/closes (ExpandCollapse) and toggles when
        // invoked; its items are separate MenuItem elements.
        A11yRole::Menu => {
            pattern == UIA_InvokePatternId || pattern == UIA_ExpandCollapsePatternId
        }
        A11yRole::MenuBar => pattern == UIA_ExpandCollapsePatternId,
        A11yRole::Heading
        | A11yRole::Group
        | A11yRole::Text
        | A11yRole::Separator
        | A11yRole::ColumnHeader => false,
    }
}

/// One axis of a node's published scroll state.
fn scroll_axis(node: &A11yNode, vertical: bool) -> Option<A11yScrollAxis> {
    let scroll = node.scroll?;
    if vertical { scroll.vertical } else { scroll.horizontal }
}

/// DIPs one `SmallIncrement`/`SmallDecrement` moves: one wheel notch, the same
/// travel the wheel gives a scroll view (D65a).
const SCROLL_SMALL_STEP: f64 = 48.0;

/// Where a UIA `Scroll` amount sends an axis (`None` = leave it alone).
fn scroll_target(axis: Option<A11yScrollAxis>, amount: ScrollAmount) -> Option<f32> {
    let axis = axis?;
    if amount == ScrollAmount_NoAmount || !axis.scrollable() {
        return None;
    }
    let small = amount == ScrollAmount_SmallIncrement || amount == ScrollAmount_SmallDecrement;
    let view = axis.viewport as f64;
    // A page leaves a little content in view, so the old edge stays visible.
    let step = if small {
        SCROLL_SMALL_STEP.min(view)
    } else {
        (view * 0.9).max(SCROLL_SMALL_STEP)
    };
    let backwards =
        amount == ScrollAmount_SmallDecrement || amount == ScrollAmount_LargeDecrement;
    let target = if backwards {
        axis.offset as f64 - step
    } else {
        axis.offset as f64 + step
    };
    Some(target.clamp(0.0, axis.max_offset() as f64) as f32)
}

unsafe fn variant_i4(value: i32) -> VARIANT {
    let mut variant: VARIANT = unsafe { core::mem::zeroed() };
    unsafe {
        let inner = &mut variant.Anonymous.Anonymous;
        inner.vt = VT_I4;
        inner.Anonymous.lVal = value;
    }
    variant
}

unsafe fn variant_bool(value: bool) -> VARIANT {
    let mut variant: VARIANT = unsafe { core::mem::zeroed() };
    unsafe {
        let inner = &mut variant.Anonymous.Anonymous;
        inner.vt = VT_BOOL;
        inner.Anonymous.boolVal =
            windows::Win32::Foundation::VARIANT_BOOL(if value { -1 } else { 0 });
    }
    variant
}

unsafe fn variant_f64(value: f64) -> VARIANT {
    let mut variant: VARIANT = unsafe { core::mem::zeroed() };
    unsafe {
        let inner = &mut variant.Anonymous.Anonymous;
        inner.vt = VT_R8;
        inner.Anonymous.dblVal = value;
    }
    variant
}

unsafe fn variant_bstr(value: &str) -> VARIANT {
    let mut variant: VARIANT = unsafe { core::mem::zeroed() };
    unsafe {
        let inner = &mut variant.Anonymous.Anonymous;
        inner.vt = VT_BSTR;
        // The VARIANT owns the BSTR from here; UIA clears it after reading.
        inner.Anonymous.bstrVal = std::mem::ManuallyDrop::new(BSTR::from(value));
    }
    variant
}

unsafe fn variant_rect(values: [f64; 4]) -> VARIANT {
    unsafe { InitVariantFromDoubleArray(&values).unwrap_or_else(|_| core::mem::zeroed()) }
}

unsafe fn variant_empty() -> VARIANT {
    unsafe { core::mem::zeroed() }
}

/// Create a one-dimensional SAFEARRAY; the caller owns the result.
unsafe fn new_array(vt: windows::Win32::System::Variant::VARENUM, count: usize) -> Option<*mut SAFEARRAY> {
    let array = unsafe { SafeArrayCreateVector(vt, 0, count as u32) };
    (!array.is_null()).then_some(array)
}

/// `VT_I4` vector used as an automation runtime id.
unsafe fn runtime_array(values: &[i32]) -> *mut SAFEARRAY {
    let Some(array) = (unsafe { new_array(VT_I4, values.len()) }) else {
        return std::ptr::null_mut();
    };
    for (index, value) in values.iter().enumerate() {
        let position = index as i32;
        let data = unsafe { SafeArrayPutElement(array, &position, value as *const i32 as *const core::ffi::c_void) };
        if data.is_err() {
            unsafe {
                let _ = SafeArrayDestroy(array);
            }
            return std::ptr::null_mut();
        }
    }
    array
}

/// `VT_UNKNOWN` vector holding COM interface pointers. `SafeArrayPutElement`
/// takes its own reference on each interface, so the caller keeps ownership of
/// `items`.
unsafe fn interface_array<I: windows_core::Interface>(items: &[I]) -> *mut SAFEARRAY {
    let Some(array) = (unsafe { new_array(VT_UNKNOWN, items.len()) }) else {
        return std::ptr::null_mut();
    };
    for (index, item) in items.iter().enumerate() {
        let position = index as i32;
        // VT_UNKNOWN elements are interface pointers: pass the pointer
        // itself, not its address (SafeArrayPutElement AddRefs it).
        let raw = windows_core::Interface::as_raw(item);
        let data = unsafe { SafeArrayPutElement(array, &position, raw) };
        if data.is_err() {
            unsafe {
                let _ = SafeArrayDestroy(array);
            }
            return std::ptr::null_mut();
        }
    }
    array
}

/// `VT_R8` vector (text range bounding rectangles).
unsafe fn double_array(values: &[f64]) -> *mut SAFEARRAY {
    let Some(array) = (unsafe { new_array(VT_R8, values.len()) }) else {
        return std::ptr::null_mut();
    };
    for (index, value) in values.iter().enumerate() {
        let position = index as i32;
        let data = unsafe { SafeArrayPutElement(array, &position, value as *const f64 as *const core::ffi::c_void) };
        if data.is_err() {
            unsafe {
                let _ = SafeArrayDestroy(array);
            }
            return std::ptr::null_mut();
        }
    }
    array
}

/// Stable 32-bit FNV-1a hash of a widget id; runtime ids must not change while
/// the widget exists.
fn stable_id(value: &str) -> i32 {
    let mut hash: u32 = 2_166_136_261;
    for byte in value.as_bytes() {
        hash ^= *byte as u32;
        hash = hash.wrapping_mul(16_777_619);
    }
    hash as i32
}

impl IRawElementProviderSimple_Impl for UiaProvider_Impl {
    fn ProviderOptions(&self) -> Result<ProviderOptions> {
        Ok(ProviderOptions_ServerSideProvider)
    }

    fn GetPatternProvider(&self, patternid: UIA_PATTERN_ID) -> Result<IUnknown> {
        let supported = self
            .node()
            .map(|node| {
                supports_pattern(node.role, patternid)
                    // The Scroll pattern follows the published scroll state,
                    // whatever the role: a list that fits advertises nothing,
                    // and it appears once the content overflows.
                    || (patternid == UIA_ScrollPatternId
                        && node.scroll.is_some_and(|scroll| scroll.scrollable()))
                    // A child that publishes an open state can expand and
                    // collapse (an accordion section), whatever its role.
                    || (patternid == UIA_ExpandCollapsePatternId && node.expanded.is_some())
                    // A child that publishes a value exposes the Value pattern
                    // (the renaming tree row), whatever its role.
                    || (patternid == UIA_ValuePatternId && node.value.is_some())
            })
            .unwrap_or(false);
        if !supported {
            return Err(E_FAIL.into());
        }
        let provider = UiaProvider::at(self.tree.clone(), self.index);
        let unknown: IUnknown = provider.into();
        Ok(unknown)
    }

    fn GetPropertyValue(&self, propertyid: UIA_PROPERTY_ID) -> Result<VARIANT> {
        let id = propertyid.0;
        let value = match self.node() {
            None => {
                // Fragment root: the window itself.
                if id == UIA_NamePropertyId.0 {
                    unsafe { variant_bstr(&self.tree.root_name()) }
                } else if id == UIA_AutomationIdPropertyId.0 {
                    unsafe { variant_bstr("BambooNativeUiRoot") }
                } else if id == UIA_ControlTypePropertyId.0 {
                    unsafe { variant_i4(UIA_PaneControlTypeId.0) }
                } else if id == UIA_IsEnabledPropertyId.0
                    || id == UIA_IsKeyboardFocusablePropertyId.0
                    || id == UIA_IsContentElementPropertyId.0
                    || id == UIA_IsControlElementPropertyId.0
                {
                    unsafe { variant_bool(true) }
                } else if id == UIA_IsOffscreenPropertyId.0
                    || id == UIA_HasKeyboardFocusPropertyId.0
                {
                    unsafe { variant_bool(false) }
                } else if id == UIA_BoundingRectanglePropertyId.0 {
                    unsafe { variant_rect(self.tree.screen_rect(self.tree.root_rect())) }
                } else {
                    unsafe { variant_empty() }
                }
            }
            Some(node) => {
                if id == UIA_NamePropertyId.0 {
                    unsafe { variant_bstr(&node.name) }
                } else if id == UIA_AutomationIdPropertyId.0 {
                    unsafe {
                        variant_bstr(node.automation_id.as_deref().unwrap_or(node.id.0))
                    }
                } else if id == UIA_HelpTextPropertyId.0
                    || id == UIA_FullDescriptionPropertyId.0
                {
                    match node.description.as_deref() {
                        Some(description) => unsafe { variant_bstr(description) },
                        None => unsafe { variant_empty() },
                    }
                } else if id == UIA_HeadingLevelPropertyId.0 {
                    if node.role == A11yRole::Heading {
                        unsafe { variant_i4(HeadingLevel2.0) }
                    } else {
                        unsafe { variant_empty() }
                    }
                } else if id == UIA_ControlTypePropertyId.0 {
                    unsafe { variant_i4(control_type(node.role)) }
                } else if id == UIA_IsEnabledPropertyId.0 {
                    unsafe { variant_bool(node.enabled) }
                } else if id == UIA_IsKeyboardFocusablePropertyId.0 {
                    unsafe { variant_bool(node.focusable) }
                } else if id == UIA_HasKeyboardFocusPropertyId.0 {
                    unsafe { variant_bool(node.focused) }
                } else if id == UIA_IsControlElementPropertyId.0
                    || id == UIA_IsContentElementPropertyId.0
                {
                    unsafe { variant_bool(true) }
                } else if id == UIA_IsOffscreenPropertyId.0 {
                    unsafe { variant_bool(node.offscreen) }
                } else if id == UIA_BoundingRectanglePropertyId.0 {
                    unsafe { variant_rect(self.tree.screen_rect(node.rect)) }
                } else if id == UIA_ToggleToggleStatePropertyId.0 {
                    let state = if node.checked == Some(true) {
                        ToggleState_On.0
                    } else {
                        ToggleState_Off.0
                    };
                    unsafe { variant_i4(state) }
                } else if id == UIA_ExpandCollapseExpandCollapseStatePropertyId.0 {
                    match node.expanded {
                        Some(true) => unsafe {
                            variant_i4(ExpandCollapseState_Expanded.0)
                        },
                        Some(false) => unsafe {
                            variant_i4(ExpandCollapseState_Collapsed.0)
                        },
                        None => unsafe { variant_empty() },
                    }
                } else if id == UIA_SelectionItemIsSelectedPropertyId.0 {
                    unsafe { variant_bool(node.selected == Some(true)) }
                } else if id == UIA_GridItemRowPropertyId.0 {
                    unsafe { variant_i4(node.grid_row.unwrap_or(0) as i32) }
                } else if id == UIA_GridItemColumnPropertyId.0 {
                    unsafe { variant_i4(node.grid_column.unwrap_or(0) as i32) }
                } else if id == UIA_GridItemRowSpanPropertyId.0 {
                    unsafe { variant_i4(1) }
                } else if id == UIA_GridItemColumnSpanPropertyId.0 {
                    unsafe { variant_i4(1) }
                } else if id == UIA_GridRowCountPropertyId.0 {
                    unsafe { variant_i4(node.grid.map(|grid| grid.0 as i32).unwrap_or(0)) }
                } else if id == UIA_GridColumnCountPropertyId.0 {
                    unsafe { variant_i4(node.grid.map(|grid| grid.1 as i32).unwrap_or(0)) }
                } else if id == UIA_ValueValuePropertyId.0 {
                    unsafe { variant_bstr(node.value.as_deref().unwrap_or("")) }
                } else if id == UIA_ValueIsReadOnlyPropertyId.0 {
                    unsafe { variant_bool(node.read_only) }
                } else if id == UIA_RangeValueValuePropertyId.0 {
                    unsafe { variant_f64(node.range.map(|range| range.2).unwrap_or(0.0)) }
                } else if id == UIA_RangeValueMinimumPropertyId.0 {
                    unsafe { variant_f64(node.range.map(|range| range.0).unwrap_or(0.0)) }
                } else if id == UIA_RangeValueMaximumPropertyId.0 {
                    unsafe { variant_f64(node.range.map(|range| range.1).unwrap_or(0.0)) }
                } else if id == UIA_ScrollHorizontallyScrollablePropertyId.0 {
                    unsafe { variant_bool(scroll_axis(&node, false).is_some_and(|axis| axis.scrollable())) }
                } else if id == UIA_ScrollVerticallyScrollablePropertyId.0 {
                    unsafe { variant_bool(scroll_axis(&node, true).is_some_and(|axis| axis.scrollable())) }
                } else if id == UIA_ScrollHorizontalScrollPercentPropertyId.0 {
                    unsafe {
                        variant_f64(
                            scroll_axis(&node, false)
                                .and_then(|axis| axis.percent())
                                .unwrap_or(UIA_ScrollPatternNoScroll),
                        )
                    }
                } else if id == UIA_ScrollVerticalScrollPercentPropertyId.0 {
                    unsafe {
                        variant_f64(
                            scroll_axis(&node, true)
                                .and_then(|axis| axis.percent())
                                .unwrap_or(UIA_ScrollPatternNoScroll),
                        )
                    }
                } else if id == UIA_ScrollHorizontalViewSizePropertyId.0 {
                    unsafe {
                        variant_f64(
                            scroll_axis(&node, false)
                                .and_then(|axis| axis.view_percent())
                                .unwrap_or(UIA_ScrollPatternNoScroll),
                        )
                    }
                } else if id == UIA_ScrollVerticalViewSizePropertyId.0 {
                    unsafe {
                        variant_f64(
                            scroll_axis(&node, true)
                                .and_then(|axis| axis.view_percent())
                                .unwrap_or(UIA_ScrollPatternNoScroll),
                        )
                    }
                } else {
                    unsafe { variant_empty() }
                }
            }
        };
        Ok(value)
    }

    fn HostRawElementProvider(&self) -> Result<IRawElementProviderSimple> {
        if self.index.is_some() {
            return Err(E_FAIL.into());
        }
        unsafe { UiaHostProviderFromHwnd(self.tree.hwnd()) }
    }
}

impl IRawElementProviderFragment_Impl for UiaProvider_Impl {
    fn Navigate(
        &self,
        direction: windows::Win32::UI::Accessibility::NavigateDirection,
    ) -> Result<windows::Win32::UI::Accessibility::IRawElementProviderFragment> {
        let fail = || windows::core::Error::from(E_FAIL);
        if direction == NavigateDirection_Parent {
            if self.index.is_none() {
                return Err(fail());
            }
            let parent = self.node().and_then(|node| node.parent);
            return Ok(self.fragment_at(parent));
        }
        let index = if direction == NavigateDirection_NextSibling {
            self.index.and_then(|index| self.tree.sibling(index, 1))
        } else if direction == NavigateDirection_PreviousSibling {
            self.index.and_then(|index| self.tree.sibling(index, -1))
        } else if direction == NavigateDirection_FirstChild {
            self.tree.first_child(self.index)
        } else if direction == NavigateDirection_LastChild {
            self.tree.last_child(self.index)
        } else {
            None
        };
        index
            .map(|index| self.fragment_at(Some(index)))
            .ok_or_else(fail)
    }

    fn GetRuntimeId(&self) -> Result<*mut SAFEARRAY> {
        // [UiaAppendRuntimeId, ...]: the root is identified by its window and
        // each element by a stable hash of its widget id plus its option/row
        // index, so the id survives snapshot refreshes and list scrolling.
        let values = match self.node() {
            None => vec![UiaAppendRuntimeId as i32, self.tree.hwnd as i32],
            Some(node) => {
                let child = node.child.map(|child| child as i32 + 1).unwrap_or(0);
                vec![
                    UiaAppendRuntimeId as i32,
                    stable_id(node.id.0),
                    child,
                ]
            }
        };
        Ok(unsafe { runtime_array(&values) })
    }

    fn BoundingRectangle(&self) -> Result<UiaRect> {
        let rect = match self.node() {
            Some(node) => node.rect,
            None => self.tree.root_rect(),
        };
        let rect = self.tree.screen_rect(rect);
        Ok(UiaRect {
            left: rect[0],
            top: rect[1],
            width: rect[2],
            height: rect[3],
        })
    }

    fn GetEmbeddedFragmentRoots(&self) -> Result<*mut windows::Win32::System::Com::SAFEARRAY> {
        Ok(std::ptr::null_mut())
    }

    fn SetFocus(&self) -> Result<()> {
        if let Some((id, child)) = self.target() {
            self.post(A11yRequest::Focus(id, child));
        }
        Ok(())
    }

    fn FragmentRoot(
        &self,
    ) -> Result<windows::Win32::UI::Accessibility::IRawElementProviderFragmentRoot> {
        let root: windows::Win32::UI::Accessibility::IRawElementProviderFragmentRoot =
            UiaProvider {
                tree: self.tree.clone(),
                index: None,
            }
            .into();
        Ok(root)
    }
}

impl IRawElementProviderFragmentRoot_Impl for UiaProvider_Impl {
    fn ElementProviderFromPoint(
        &self,
        x: f64,
        y: f64,
    ) -> Result<windows::Win32::UI::Accessibility::IRawElementProviderFragment> {
        let scale = self.tree.state.lock().map(|state| state.scale).unwrap_or(1.0) as f64;
        let (origin_x, origin_y) = self
            .tree
            .state
            .lock()
            .map(|state| (state.origin_x, state.origin_y))
            .unwrap_or((0, 0));
        let dx = ((x - origin_x as f64) / scale) as f32;
        let dy = ((y - origin_y as f64) / scale) as f32;
        Ok(self.fragment_at(self.tree.hit(dx, dy)))
    }

    fn GetFocus(&self) -> Result<windows::Win32::UI::Accessibility::IRawElementProviderFragment> {
        Ok(self.fragment_at(self.tree.focused()))
    }
}

impl IInvokeProvider_Impl for UiaProvider_Impl {
    fn Invoke(&self) -> Result<()> {
        if let Some((id, child)) = self.target() {
            self.post(A11yRequest::Invoke(id, child));
        }
        Ok(())
    }
}

impl IToggleProvider_Impl for UiaProvider_Impl {
    fn Toggle(&self) -> Result<()> {
        if let Some((id, child)) = self.target() {
            self.post(A11yRequest::Toggle(id, child));
        }
        Ok(())
    }

    fn ToggleState(&self) -> Result<windows::Win32::UI::Accessibility::ToggleState> {
        let on = self.node().and_then(|node| node.checked).unwrap_or(false);
        Ok(if on { ToggleState_On } else { ToggleState_Off })
    }
}

impl IExpandCollapseProvider_Impl for UiaProvider_Impl {
    fn Expand(&self) -> Result<()> {
        if let Some((id, child)) = self.target() {
            self.post(A11yRequest::Expand(id, child, true));
        }
        Ok(())
    }

    fn Collapse(&self) -> Result<()> {
        if let Some((id, child)) = self.target() {
            self.post(A11yRequest::Expand(id, child, false));
        }
        Ok(())
    }

    fn ExpandCollapseState(&self) -> Result<ExpandCollapseState> {
        let node = self.node().ok_or_else(|| windows::core::Error::from(E_FAIL))?;
        Ok(match node.expanded {
            Some(true) => ExpandCollapseState_Expanded,
            Some(false) => ExpandCollapseState_Collapsed,
            None => ExpandCollapseState_LeafNode,
        })
    }
}

impl IScrollProvider_Impl for UiaProvider_Impl {
    fn Scroll(&self, horizontalamount: ScrollAmount, verticalamount: ScrollAmount) -> Result<()> {
        let (id, child) = self.target().ok_or_else(|| windows::core::Error::from(E_FAIL))?;
        let horizontal = scroll_target(self.scroll_axis(false), horizontalamount);
        let vertical = scroll_target(self.scroll_axis(true), verticalamount);
        // NoAmount must be used for the axis that should not move; a requested
        // axis with nothing to scroll is an error, as the platform contract
        // asks. A request at the end of the range is a successful no-op.
        let wanted_h = horizontalamount != ScrollAmount_NoAmount;
        let wanted_v = verticalamount != ScrollAmount_NoAmount;
        if (!wanted_h && !wanted_v)
            || (wanted_h && horizontal.is_none())
            || (wanted_v && vertical.is_none())
        {
            return Err(E_FAIL.into());
        }
        self.post(A11yRequest::Scroll(id, child, horizontal, vertical));
        Ok(())
    }

    fn SetScrollPercent(&self, horizontalpercent: f64, verticalpercent: f64) -> Result<()> {
        let no = UIA_ScrollPatternNoScroll;
        let valid = |percent: f64| percent == no || (0.0..=100.0).contains(&percent);
        if !valid(horizontalpercent) || !valid(verticalpercent) {
            return Err(E_INVALIDARG.into());
        }
        let (id, child) = self.target().ok_or_else(|| windows::core::Error::from(E_FAIL))?;
        let node = self.node().ok_or_else(|| windows::core::Error::from(E_FAIL))?;
        let target = |vertical: bool, percent: f64| {
            scroll_axis(&node, vertical)
                .filter(|axis| axis.scrollable())
                .map(|axis| axis.target(percent))
        };
        let horizontal = (horizontalpercent != no)
            .then(|| target(false, horizontalpercent))
            .flatten();
        let vertical = (verticalpercent != no)
            .then(|| target(true, verticalpercent))
            .flatten();
        if horizontal.is_none() && vertical.is_none() {
            return Err(E_FAIL.into());
        }
        self.post(A11yRequest::Scroll(id, child, horizontal, vertical));
        Ok(())
    }

    fn HorizontalScrollPercent(&self) -> Result<f64> {
        Ok(self
            .scroll_axis(false)
            .and_then(|axis| axis.percent())
            .unwrap_or(UIA_ScrollPatternNoScroll))
    }

    fn VerticalScrollPercent(&self) -> Result<f64> {
        Ok(self
            .scroll_axis(true)
            .and_then(|axis| axis.percent())
            .unwrap_or(UIA_ScrollPatternNoScroll))
    }

    fn HorizontalViewSize(&self) -> Result<f64> {
        Ok(self
            .scroll_axis(false)
            .and_then(|axis| axis.view_percent())
            .unwrap_or(UIA_ScrollPatternNoScroll))
    }

    fn VerticalViewSize(&self) -> Result<f64> {
        Ok(self
            .scroll_axis(true)
            .and_then(|axis| axis.view_percent())
            .unwrap_or(UIA_ScrollPatternNoScroll))
    }

    fn HorizontallyScrollable(&self) -> Result<BOOL> {
        Ok(BOOL::from(
            self.scroll_axis(false).is_some_and(|axis| axis.scrollable()),
        ))
    }

    fn VerticallyScrollable(&self) -> Result<BOOL> {
        Ok(BOOL::from(
            self.scroll_axis(true).is_some_and(|axis| axis.scrollable()),
        ))
    }
}

impl IGridItemProvider_Impl for UiaProvider_Impl {
    fn Row(&self) -> Result<i32> {
        self.node()
            .and_then(|node| node.grid_row)
            .map(|row| row as i32)
            .ok_or_else(|| windows::core::Error::from(E_FAIL))
    }

    fn Column(&self) -> Result<i32> {
        self.node()
            .and_then(|node| node.grid_column)
            .map(|column| column as i32)
            .ok_or_else(|| windows::core::Error::from(E_FAIL))
    }

    // Cells cover exactly one row and one column; merged cells would publish
    // their span on the node.
    fn RowSpan(&self) -> Result<i32> {
        Ok(1)
    }

    fn ColumnSpan(&self) -> Result<i32> {
        Ok(1)
    }

    fn ContainingGrid(&self) -> Result<IRawElementProviderSimple> {
        let parent = self.node().and_then(|node| node.parent);
        Ok(self.provider_at(parent))
    }
}

impl IGridProvider_Impl for UiaProvider_Impl {
    fn GetItem(&self, row: i32, column: i32) -> Result<IRawElementProviderSimple> {
        let node = self.node().ok_or_else(|| windows::core::Error::from(E_FAIL))?;
        if row < 0 || column < 0 {
            return Err(E_INVALIDARG.into());
        }
        // Only realized cells exist in the snapshot; a virtualized grid
        // returns the ones inside its viewport (plus the look-ahead row).
        let found = node.children.iter().copied().find(|&child| {
            self.tree.node(child).is_some_and(|state| {
                state.grid_row == Some(row as usize) && state.grid_column == Some(column as usize)
            })
        });
        match found {
            Some(child) => Ok(self.provider_at(Some(child))),
            None => Err(E_INVALIDARG.into()),
        }
    }

    fn RowCount(&self) -> Result<i32> {
        Ok(self
            .node()
            .and_then(|node| node.grid)
            .map(|grid| grid.0 as i32)
            .unwrap_or(0))
    }

    fn ColumnCount(&self) -> Result<i32> {
        Ok(self
            .node()
            .and_then(|node| node.grid)
            .map(|grid| grid.1 as i32)
            .unwrap_or(0))
    }
}

impl IRangeValueProvider_Impl for UiaProvider_Impl {
    fn SetValue(&self, val: f64) -> Result<()> {
        let node = self.node().ok_or_else(|| windows::core::Error::from(E_FAIL))?;
        // A progress bar reports progress; it is not a control a client may
        // drive. Read-only sliders behave the same way.
        if node.read_only || node.role == A11yRole::ProgressBar {
            return Err(windows::core::Error::from(E_FAIL));
        }
        if let Some((id, child)) = self.target() {
            self.post(A11yRequest::SetNumber(id, child, val));
        }
        Ok(())
    }

    fn Value(&self) -> Result<f64> {
        Ok(self
            .node()
            .and_then(|node| node.range)
            .map(|range| range.2)
            .unwrap_or(0.0))
    }

    fn IsReadOnly(&self) -> Result<BOOL> {
        Ok(BOOL(
            self.node()
                .map(|node| {
                    !node.enabled || node.read_only || node.role == A11yRole::ProgressBar
                })
                .unwrap_or(true)
                .into(),
        ))
    }

    fn Maximum(&self) -> Result<f64> {
        Ok(self
            .node()
            .and_then(|node| node.range)
            .map(|range| range.1)
            .unwrap_or(0.0))
    }

    fn Minimum(&self) -> Result<f64> {
        Ok(self
            .node()
            .and_then(|node| node.range)
            .map(|range| range.0)
            .unwrap_or(0.0))
    }

    fn LargeChange(&self) -> Result<f64> {
        let (min, max) = self
            .node()
            .and_then(|node| node.range)
            .map(|range| (range.0, range.1))
            .unwrap_or((0.0, 0.0));
        Ok((max - min) / 10.0)
    }

    fn SmallChange(&self) -> Result<f64> {
        let (min, max) = self
            .node()
            .and_then(|node| node.range)
            .map(|range| (range.0, range.1))
            .unwrap_or((0.0, 0.0));
        Ok((max - min) / 100.0)
    }
}

impl IValueProvider_Impl for UiaProvider_Impl {
    fn SetValue(&self, val: &PCWSTR) -> Result<()> {
        let Some((id, child)) = self.target() else {
            return Ok(());
        };
        let text = unsafe { val.to_string() }?;
        self.post(A11yRequest::SetText(id, child, text));
        Ok(())
    }

    fn Value(&self) -> Result<BSTR> {
        let value = self
            .node()
            .and_then(|node| node.value)
            .unwrap_or_default();
        Ok(BSTR::from(&value))
    }

    fn IsReadOnly(&self) -> Result<BOOL> {
        Ok(BOOL(
            self.node().map(|node| node.read_only).unwrap_or(true).into(),
        ))
    }
}

impl ISelectionItemProvider_Impl for UiaProvider_Impl {
    fn Select(&self) -> Result<()> {
        if let Some((id, child)) = self.target() {
            self.post(A11yRequest::Select(id, child));
        }
        Ok(())
    }

    fn AddToSelection(&self) -> Result<()> {
        self.Select()
    }

    fn RemoveFromSelection(&self) -> Result<()> {
        Ok(())
    }

    fn IsSelected(&self) -> Result<BOOL> {
        Ok(BOOL(
            self.node()
                .map(|node| node.selected == Some(true))
                .unwrap_or(false)
                .into(),
        ))
    }

    fn SelectionContainer(&self) -> Result<IRawElementProviderSimple> {
        let parent = self.node().and_then(|node| node.parent);
        Ok(self.provider_at(parent))
    }
}

impl ISelectionProvider_Impl for UiaProvider_Impl {
    fn GetSelection(&self) -> Result<*mut SAFEARRAY> {
        let Some(node) = self.node() else {
            return Ok(unsafe { interface_array(&[] as &[IRawElementProviderSimple]) });
        };
        // Containers are single-select: at most one child is selected.
        let selected: Vec<IRawElementProviderSimple> = node
            .children
            .iter()
            .filter_map(|&child| self.tree.node(child).map(|state| (child, state)))
            .filter(|(_, state)| state.selected == Some(true))
            .map(|(child, _)| self.provider_at(Some(child)))
            .collect();
        Ok(unsafe { interface_array(&selected) })
    }

    fn CanSelectMultiple(&self) -> Result<BOOL> {
        let multiple = self
            .node()
            .map(|node| node.role == A11yRole::MultiSelectList)
            .unwrap_or(false);
        Ok(BOOL(multiple.into()))
    }

    fn IsSelectionRequired(&self) -> Result<BOOL> {
        // Radio groups and tab lists always have one option selected; a queue
        // may have no current row.
        let required = self
            .node()
            .map(|node| {
                !matches!(
                    node.role,
                    A11yRole::List | A11yRole::MultiSelectList
                )
            })
            .unwrap_or(false);
        Ok(BOOL(required.into()))
    }
}

/// Text range over the current value of a closed edit/number field.
///
/// The range covers the widget's own text (the native editor exposes its own
/// UIA while open). Only full-value navigation is modelled for movement:
/// `ExpandToEnclosingUnit` selects everything and movement returns 0. A closed
/// text field additionally publishes its caret/selection, so `GetSelection`
/// and `SupportedTextSelection` behave like a real text control.
#[implement(ITextRangeProvider)]
struct UiaTextRange {
    tree: Arc<A11yTree>,
    index: usize,
    start: usize,
    end: usize,
}

impl UiaTextRange {
    fn document(tree: Arc<A11yTree>, index: usize) -> Self {
        let end = tree
            .node(index)
            .and_then(|node| node.value)
            .map(|value| value.len())
            .unwrap_or(0);
        Self {
            tree,
            index,
            start: 0,
            end,
        }
    }

    /// Range over byte offsets `start..end` of the node's current value.
    fn at(tree: Arc<A11yTree>, index: usize, start: usize, end: usize) -> Self {
        Self {
            tree,
            index,
            start,
            end,
        }
    }

    fn text(&self) -> String {
        self.tree
            .node(self.index)
            .and_then(|node| node.value)
            .unwrap_or_default()
    }

    /// Byte range clamped to the current value and to character boundaries
    /// (the value may have been refreshed since this range was created).
    fn span(&self) -> (usize, usize) {
        let text = self.text();
        let boundary = |mut offset: usize| {
            offset = offset.min(text.len());
            while offset > 0 && !text.is_char_boundary(offset) {
                offset -= 1;
            }
            offset
        };
        let start = boundary(self.start);
        let end = boundary(self.end).max(start);
        (start, end)
    }
}

impl ITextRangeProvider_Impl for UiaTextRange_Impl {
    fn Clone(&self) -> Result<ITextRangeProvider> {
        let range = UiaTextRange {
            tree: self.tree.clone(),
            index: self.index,
            start: self.start,
            end: self.end,
        };
        Ok(range.into())
    }

    fn Compare(&self, range: windows::core::Ref<ITextRangeProvider>) -> Result<BOOL> {
        // Ranges are compared by content: a closed field publishes only its
        // document range, so equal text means the same range for a client.
        let ours = self.text();
        let Ok(range) = range.ok() else {
            return Ok(BOOL(0));
        };
        let other = unsafe { range.GetText(-1) }?;
        Ok(BOOL((other.to_string() == ours).into()))
    }

    fn CompareEndpoints(
        &self,
        _endpoint: TextPatternRangeEndpoint,
        _targetrange: windows::core::Ref<ITextRangeProvider>,
        _targetendpoint: TextPatternRangeEndpoint,
    ) -> Result<i32> {
        Ok(0)
    }

    fn ExpandToEnclosingUnit(&self, _unit: TextUnit) -> Result<()> {
        // Every range handed out already covers the whole value.
        Ok(())
    }

    fn FindAttribute(
        &self,
        _attributeid: windows::Win32::UI::Accessibility::UIA_TEXTATTRIBUTE_ID,
        _val: &VARIANT,
        _backward: BOOL,
    ) -> Result<ITextRangeProvider> {
        Err(E_NOTIMPL.into())
    }

    fn FindText(
        &self,
        text: &BSTR,
        backward: BOOL,
        ignorecase: BOOL,
    ) -> Result<ITextRangeProvider> {
        let needle = text.to_string();
        let (start, end) = self.span();
        let value = self.text();
        let window = &value[start..end];
        let search = if ignorecase.as_bool() {
            window.to_ascii_lowercase()
        } else {
            window.to_string()
        };
        let pattern = if ignorecase.as_bool() {
            needle.to_ascii_lowercase()
        } else {
            needle.clone()
        };
        let found = if backward.as_bool() {
            search.rfind(&pattern)
        } else {
            search.find(&pattern)
        };
        let (start, end) = match found {
            Some(offset) => (start + offset, start + offset + needle.len()),
            None => (start, start),
        };
        Ok(UiaTextRange {
            tree: self.tree.clone(),
            index: self.index,
            start,
            end,
        }
        .into())
    }

    fn GetAttributeValue(
        &self,
        _attributeid: windows::Win32::UI::Accessibility::UIA_TEXTATTRIBUTE_ID,
    ) -> Result<VARIANT> {
        Ok(unsafe { variant_empty() })
    }

    fn GetBoundingRectangles(&self) -> Result<*mut SAFEARRAY> {
        let Some(node) = self.tree.node(self.index) else {
            return Ok(std::ptr::null_mut());
        };
        let rect = self.tree.screen_rect(node.rect);
        Ok(unsafe { double_array(&rect) })
    }

    fn GetEnclosingElement(&self) -> Result<IRawElementProviderSimple> {
        Ok(UiaProvider {
            tree: self.tree.clone(),
            index: Some(self.index),
        }
        .into())
    }

    fn GetText(&self, maxlength: i32) -> Result<BSTR> {
        let (start, end) = self.span();
        let value = self.text();
        let text = &value[start..end];
        let text: String = if maxlength >= 0 {
            text.chars().take(maxlength as usize).collect()
        } else {
            text.to_string()
        };
        Ok(BSTR::from(&text))
    }

    fn Move(&self, _unit: TextUnit, _count: i32) -> Result<i32> {
        Ok(0)
    }

    fn MoveEndpointByUnit(
        &self,
        _endpoint: TextPatternRangeEndpoint,
        _unit: TextUnit,
        _count: i32,
    ) -> Result<i32> {
        Ok(0)
    }

    fn MoveEndpointByRange(
        &self,
        _endpoint: TextPatternRangeEndpoint,
        _targetrange: windows::core::Ref<ITextRangeProvider>,
        _targetendpoint: TextPatternRangeEndpoint,
    ) -> Result<()> {
        Ok(())
    }

    fn Select(&self) -> Result<()> {
        Ok(())
    }

    fn AddToSelection(&self) -> Result<()> {
        Ok(())
    }

    fn RemoveFromSelection(&self) -> Result<()> {
        Ok(())
    }

    fn ScrollIntoView(&self, _aligntotop: BOOL) -> Result<()> {
        Ok(())
    }

    fn GetChildren(&self) -> Result<*mut SAFEARRAY> {
        Ok(std::ptr::null_mut())
    }
}

impl ITextProvider_Impl for UiaProvider_Impl {
    fn GetSelection(&self) -> Result<*mut SAFEARRAY> {
        // A closed text field keeps its own caret/selection model; other
        // editors (number fields) publish no selection.
        let Some(index) = self.index else {
            return Ok(unsafe { interface_array(&[] as &[ITextRangeProvider]) });
        };
        let Some((start, end)) = self.tree.node(index).and_then(|node| node.text_selection) else {
            return Ok(unsafe { interface_array(&[] as &[ITextRangeProvider]) });
        };
        let range: ITextRangeProvider =
            UiaTextRange::at(self.tree.clone(), index, start, end).into();
        Ok(unsafe { interface_array(&[range]) })
    }

    fn GetVisibleRanges(&self) -> Result<*mut SAFEARRAY> {
        let Some(index) = self.index else {
            return Ok(std::ptr::null_mut());
        };
        let range: ITextRangeProvider = UiaTextRange::document(self.tree.clone(), index).into();
        Ok(unsafe { interface_array(&[range]) })
    }

    fn RangeFromChild(
        &self,
        _childelement: windows::core::Ref<IRawElementProviderSimple>,
    ) -> Result<ITextRangeProvider> {
        Err(E_NOTIMPL.into())
    }

    fn RangeFromPoint(
        &self,
        _point: &windows::Win32::UI::Accessibility::UiaPoint,
    ) -> Result<ITextRangeProvider> {
        let Some(index) = self.index else {
            return Err(E_FAIL.into());
        };
        Ok(UiaTextRange::document(self.tree.clone(), index).into())
    }

    fn DocumentRange(&self) -> Result<ITextRangeProvider> {
        let Some(index) = self.index else {
            return Err(E_FAIL.into());
        };
        Ok(UiaTextRange::document(self.tree.clone(), index).into())
    }

    fn SupportedTextSelection(
        &self,
    ) -> Result<windows::Win32::UI::Accessibility::SupportedTextSelection> {
        let supports = self
            .index
            .and_then(|index| self.tree.node(index))
            .is_some_and(|node| node.text_selection.is_some());
        Ok(if supports {
            SupportedTextSelection_Single
        } else {
            SupportedTextSelection_None
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Rect;

    fn node(role: A11yRole) -> A11yNode {
        A11yNode {
            id: WidgetId::new("w"),
            role,
            child: None,
            name: "w".to_string(),
            automation_id: None,
            description: None,
            value: None,
            enabled: true,
            focusable: true,
            focused: false,
            checked: None,
            selected: None,
            range: None,
            read_only: false,
            text_selection: None,
            rect: Rect::from_xywh(0.0, 0.0, 10.0, 10.0),
            offscreen: false,
            expanded: None,
            scroll: None,
            grid_row: None,
            grid_column: None,
            grid: None,
            parent: None,
            children: Vec::new(),
        }
    }

    #[test]
    fn diff_reports_toggle_value_and_range_changes() {
        let old_toggle = A11yNode {
            checked: Some(true),
            ..node(A11yRole::Toggle)
        };
        let new_toggle = A11yNode {
            checked: Some(false),
            ..node(A11yRole::Toggle)
        };
        let changes = diff(&[old_toggle], &[new_toggle]);
        assert!(matches!(
            changes.as_slice(),
            [A11yChange::Toggled { index: 0, old: true, new: false }]
        ));

        let old_text = A11yNode {
            value: Some("a".to_string()),
            ..node(A11yRole::TextField)
        };
        let new_text = A11yNode {
            value: Some("b".to_string()),
            ..node(A11yRole::TextField)
        };
        let changes = diff(&[old_text], &[new_text]);
        assert!(matches!(
            changes.as_slice(),
            [A11yChange::Value { old, new, .. }] if old == "a" && new == "b"
        ));

        let old_slider = A11yNode {
            range: Some((0.0, 1.0, 0.2)),
            ..node(A11yRole::Slider)
        };
        let new_slider = A11yNode {
            range: Some((0.0, 1.0, 0.5)),
            ..node(A11yRole::Slider)
        };
        let changes = diff(&[old_slider], &[new_slider]);
        assert!(matches!(
            changes.as_slice(),
            [A11yChange::Range { old, new, .. }] if *old == 0.2 && *new == 0.5
        ));

        // A progress bar reports range changes the same way a slider does.
        let old_progress = A11yNode {
            range: Some((0.0, 1.0, 0.1)),
            ..node(A11yRole::ProgressBar)
        };
        let new_progress = A11yNode {
            range: Some((0.0, 1.0, 0.9)),
            ..node(A11yRole::ProgressBar)
        };
        let changes = diff(&[old_progress], &[new_progress]);
        assert!(matches!(
            changes.as_slice(),
            [A11yChange::Range { old, new, .. }] if *old == 0.1 && *new == 0.9
        ));
    }

    #[test]
    fn diff_reports_new_selection_only() {
        let mut old_row = node(A11yRole::ListItem);
        old_row.child = Some(1);
        old_row.selected = Some(false);
        let mut new_row = old_row.clone();
        new_row.selected = Some(true);
        let changes = diff(&[old_row], &[new_row]);
        assert!(matches!(
            changes.as_slice(),
            [A11yChange::Selected { index: 0 }]
        ));

        // Values on roles without the pattern (a pager label) stay silent.
        let old_pager = A11yNode {
            value: Some("1/5".to_string()),
            ..node(A11yRole::Button)
        };
        let new_pager = A11yNode {
            value: Some("2/5".to_string()),
            ..node(A11yRole::Button)
        };
        assert!(diff(&[old_pager], &[new_pager]).is_empty());
    }

    #[test]
    fn diff_ignores_unchanged_and_newly_scrolled_rows() {
        let unchanged = node(A11yRole::Button);
        assert!(diff(&[unchanged.clone()], &[unchanged]).is_empty());
        // A row that was not published before has no previous state to diff.
        let mut scrolled = A11yNode {
            role: A11yRole::ListItem,
            ..node(A11yRole::Button)
        };
        scrolled.child = Some(9);
        scrolled.selected = Some(true);
        assert!(diff(&[], &[scrolled]).is_empty());
    }
}
