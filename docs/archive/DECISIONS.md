# Decisions

Architecture decisions that later milestones must not silently undo.

## D1. The application computes rectangles

The framework never lays out widgets. A `Host` maps state to rectangles and
calls `WindowHandle::invalidate(rect)` for exactly the regions that changed.
`WM_SIZE` / DPI / page switch are the only places allowed to recompute layout,
and they invalidate the whole window once.

Reason: this is what makes "hover repaints two rectangles" possible. An
immediate-mode or widget-tree-with-layout design cannot keep that guarantee.

## D2. One back buffer per window

`GdiDoubleBuffer` lives for the window lifetime and is resized only when the
client size changes. A paint pass draws into it and blits the bounding box of
the merged dirty rectangles. More than 16 rectangles or 55% of the window area
collapse into one full repaint.

## D3. Backend traits, GDI implementation

The public API uses only neutral types (`Rect`, `Color`, `Event`, `KeyCode`,
`TextStyle`). `Canvas` abstracts drawing; the GDI implementation is the only
module that touches Win32. A future software-raster or GPU backend implements
`Canvas` without changing call sites. Dispatch overhead is per draw call, not
per widget.

## D4. Caches belong to the backend

Brushes, fonts, pens and converted image pixels live in `GdiResources` for the
lifetime of the window. They are keyed by color / font spec / image id. A theme
font change calls `clear_fonts()`; DPI changes do the same.

## D5. Accessibility is derived, not filled in

Widget type + props determine the UIA role and value (`toggle` → Toggle + bool,
`slider` → Slider + range, `list` → ListItem). `role()` / `name()` remain
one-line overrides. A11y strings are built only on `WM_GETOBJECT` and focus
events, never during paint.

## D6. Handlers return messages

`Ui<M>` handlers do not take `&mut App`. They return `M`, and the application
drains messages after the event. This avoids retained-tree borrow conflicts and
matches the player's "UI reads snapshots, emits commands" rule.

## D7. XML layout is a skin, not a constructor

The M7 opt-in XML feature assigns rectangles to existing widget ids. It does not
create widgets, so hot reload keeps runtime state (scroll, text, playback) and
cannot introduce types the Rust code does not know about. The schema is
deliberately undecided until M7 starts; see `TODO.md`.

## D8. Opt-in debug features are compiled out by default

`layout-xml` and `debug-form` are Cargo features. Default builds do not parse
XML and do not ship the debug window.

## D9. Repository wiring

The framework is an independent git repository. The player links it as a git
submodule plus a Cargo `path` dependency, so `git clone --recursive` yields a
buildable tree and GitHub shows the sub-repository link. It must not be added to
the parent `.gitignore`.

## D10. License: Apache-2.0

Permissive with attribution: commercial and closed-source use is allowed, but
distributions must keep `LICENSE` and `NOTICE`, and significant modifications
must be stated. This is deliberately not copyleft, so it does not conflict with
the player's own non-commercial license: a combined distribution keeps this
project's `LICENSE`/`NOTICE` alongside the player's terms.

Earlier commits declared MIT in `Cargo.toml` metadata (no LICENSE file existed);
those snapshots remain MIT-licensed. Both are permissive, so the change is
compatible for downstream users.

## D11. Widget-level dynamic dispatch is allowed

`Ui<M>` stores `Box<dyn Widget<M>>`. Dispatch happens once per visible widget
(around 20 calls per frame), never per list row: `ListView` paints its rows from
a plain `Vec<Row>` and later publishes only the visible ones to accessibility.
This keeps the paint path allocation-free while preserving a simple, extensible
widget contract.

## D12. Invalidation goes through `Invalidator`

`Ui` never touches Win32. Every state change calls
`Invalidator::invalidate(rect)`; `WindowHandle` implements it in production and
unit tests use a recording sink. This is what makes "hover invalidates exactly
two rectangles" a testable invariant instead of a claim.

## D13. Text editing uses a native child, driven through `UiHost`

Text fields are not reimplemented in the framework. A `TextEdit` paints its own
frame and text (so pages never depend on the child), and on click/Enter asks
the host for a borderless `EDIT` child aligned to the widget rectangle. The
request crosses the platform boundary as `EditRequest` through `UiHost`, which
`WindowHandle` implements; `Host` receives `edit_changed/committed/canceled`
callbacks. Reasons:

- Native editing gives IME, selection, clipboard, caret and accessibility for
  free; a hand-written GDI editor would have to rebuild all of it.
- The child is inset 1 DIP inside the widget, and the window already has
  `WS_CLIPCHILDREN`, so the dirty-rect blit never paints over it.
- Committing on kill-focus/Enter/Tab and committing when `Ui` moves focus keep
  one source of truth: the session ends exactly when keyboard focus leaves.

Edit work is queued (`EditOp`) and applied after the current `Host` callback
returns. Creating, focusing or destroying a window synchronously re-enters the
window procedure; doing that inside `Host::event` would dispatch from a taken
host and alias the window state. The child is hidden immediately on close and
destroyed from a posted message, so `WM_KILLFOCUS` never destroys the control
that is running it.

Future direction (TODO, after the M6.5 groups): a framework-painted editor
could remove the seam this design creates between the closed paint and the
native child — the text strip appearing on open/close, the child's inset
top-aligned text, a second caret/selection and UIA text shape while editing,
forwarded padding clicks/context menu, and system scrollbar chrome. That
editor must own IME composition (including candidate-window placement),
clipboard, undo, scrolling and UIA text before it can replace the child; until
then this decision stands and the native child remains the implementation.

## D14. Layout owns editor alignment

`Ui::sync_edit(window)` moves an open editor to the widget's current rectangle.
The application calls it once at the end of `Host::resize`; layout code (which
has no window handle) stays unchanged. A DPI change detected there recreates
the control font.

## D15. Resize messages are chained to `DefWindowProc`

The window procedure must not swallow `WM_WINDOWPOSCHANGED`: `DefWindowProc`
is what turns it into `WM_SIZE` / `WM_MOVE`. Handling the message without
chaining left the client size stale after every resize and maximize, so the
layout and paints covered only the old area. `WM_MOVE` is handled separately
for the visibility re-scan.

## D16. Cursors belong to the window under the pointer

`WM_SETCURSOR` is delivered to this window with `wParam` set to the window
under the cursor. Only when that is the window itself does the stored shape
apply; for a child (the native editor) the message goes to `DefWindowProc`,
which lets the child keep its class cursor (I-beam). Overriding child cursors
made the input field show an arrow inside its own editor.

## D17. The focus ring is keyboard-only

The accent ring is an accessibility indicator: pointer focus does not draw it,
keyboard focus (Tab/arrows/Enter, modify mode) does. `Ui::show_focus_ring`
lets an a11y layer force it. Overriding `Widget::focus_ring_rect` keeps the
ring around what the widget actually paints (a switch's track, a list's rows)
so painted content never shows outside the frame — the ring rect, not an inset
of the layout rect, is the single source of truth.

## D18. Content controls use a modify mode, not permanent arrow capture

Arrow keys are focus navigation by default. `Enter` on a slider, list,
segmented control or tab strip enters **modify mode**: arrows then operate the
control, the ring turns into a solid 2 DIP frame, `Enter` on a list activates
the row, and `Esc`/`Backspace` leave the mode; `Tab` always leaves. This avoids
three problems seen with permanent arrow capture: a focused list trapping the
keyboard, accidental selection changes when the user only wanted to move focus,
and arrow keys that mean different things in different widgets. Backspace
outside modify mode releases keyboard focus (inside a text field it never
reaches the framework and still deletes text).

## D19. UIA providers are snapshots, actions are posted

A screen reader may call provider methods from any thread, so the provider
never touches application state directly. `WM_GETOBJECT` (with
`UiaRootObjectId` or `OBJID_CLIENT`) builds an owned `A11yNode` snapshot; the
provider holds it behind a lock and is refreshed in place on focus changes and
actions, so long-lived provider objects stay current. Pattern calls
(Invoke/Toggle/Select/SetValue/Focus) post a boxed request back to the window
and run on the UI thread through `Ui::apply_a11y`, i.e. the same widget input a
user produces.

## D20. Runtime ids hash the widget id, not the snapshot position

Container children could otherwise be identified by their vector position, but
snapshot order changes when list rows scroll and a stable id lets a client keep
a reference across refreshes. Elements use
`[UiaAppendRuntimeId, FNV-1a(widget id), option/row index + 1]`; the fragment
root uses its window handle. This is also why `A11yChild` carries the
widget-level index: the published list rows start at the scroll offset, and
choosing one must select that row, not the first row in the buffer.

## D21. Closed edit fields expose a read-only TextPattern

ValuePattern alone lets Narrator read a field, but text navigation and bounding
rectangles need TextPattern. The closed widget publishes a document range over
its current text: `GetText`/`FindText`/`GetBoundingRectangles` work, movement is
a no-op and `ExpandToEnclosingUnit` selects the whole value. A `TextEdit` also
publishes its closed-field caret/selection model (D31) through `GetSelection`
and `SupportedTextSelection_Single`; number fields report `None`. `Compare`
compares range text rather than endpoints; that is sufficient because a closed
field only ever publishes whole-value or selection ranges over the same text.

## D22. Composite controls focus the item, not the container

A focused `Segmented`/`Tabs`/`ListView` publishes `HasKeyboardFocus` on its
selected child and `GetFocus` returns that child; the container itself reports
`focused = false`. Native list and tab controls behave this way, and it is what
Narrator needs: with focus on the container it only announced "contains N
items" and ignored arrow-driven selection changes. Focus events therefore carry
the selected item's provider (`[widget id, absolute option/row index]`), and
the host deduplicates them: hosts call `a11y_focus` after every event including
pointer moves, and re-announcing the same element made a screen reader repeat
itself. Only viewport rows are published, so this never reads the whole list.

## D23. State changes are announced as property-change events

Focus events do not fire when the same element changes state, so Narrator
stayed silent when a switch was flipped or a slider moved. Every snapshot
refresh now diffs the old and new trees by element identity (widget id plus
option/row index) and reports: `ToggleState` changes on switches,
`RangeValue.Value` on sliders, `Value.Value` on edit/number fields, `Name`
changes, and `SelectionItem.IsSelected` + `ElementSelected` for newly selected
options/rows. Only properties that the element's role actually supports are
raised (a Pager label change is not a Value event). The diff runs on the UI
thread before the provider lock is released, so clients read the new state when
they handle the notification.

## D24. The container owns visibility, focus order and the cursor

Per-widget storage is not enough: only Toast tracked a visible flag, so a
hidden widget still took hits, and the container had no idea what was on top.
`Ui` now keeps per-instance visibility, focusability, focus weight and cursor
overrides next to the widget list, while `Widget` exposes `visible`,
`interactive` and `cursor` as defaults. Hit-testing walks top-down (last added
wins), skips invisible/non-interactive widgets, and display-only widgets are
pass-through by default. Hiding or disabling the focused widget relocates focus
to the next widget in the *new* order and invalidates both rings; hidden
widgets leave the a11y tree. Tab order is ascending `focus_order` with
insertion order as tie-break (default 0), replacing the previous spatial sort
so the declared order is authoritative.

## D25. Triggers run before widget input; focus hooks bracket the move

A binding is consulted inside the same dispatch path the pointer, keyboard and
UIA already use (`Ui::interact`), so there is no second event route to keep in
sync. The handler returns an optional message and calls `ctx.default()` to keep
the widget's behavior; the custom message is queued first, then the default one.
Keyboard activation and UIA `Invoke`/`Toggle` share `Trigger::Activate`, so a
screen reader triggers exactly what Enter triggers. `Click` fires on
press-and-release; a text field still opens its editor on pointer-down, so the
editor exists before a `Click` binding runs.

Focus changes go through `focus_move`, which commits an open native editor,
runs the lost hook (which may `Cancel` or `Redirect` once), moves focus, then
runs the gained hook (which may `Cancel` to revert). Relocation caused by
hiding/disabling/making a widget non-focusable uses `FocusReason::Relocated`;
if a hook vetoes relocation, focus is cleared anyway so a hidden widget cannot
keep keyboard focus.

## D26. Instance properties live in the container; narration feeds the tree

Per-instance presentation and accessibility details (`WidgetProps`) do not go
into widget structs and do not require a rebuild. `Ui::add_with` applies them
before the first layout; `Ui::set_props` applies them on a live page and
relocates focus when the focused widget becomes invisible or non-focusable.
Focus policy, cursor and visibility are folded into the existing container
tables (`focus_policies`, `cursors`, `visibility`), so there is still one
source of truth for Tab order, hit-testing and the a11y tree.

`Narration` overrides (fixed or lazy name/description, per-option/row
formatters) are applied inside `Ui::a11y_nodes` after the widget reports its
derived `A11yInfo`. The provider tree, `--a11y-dump` and `--self-test` all read
that one result, so a renamed control cannot read differently to a screen
reader and to the automated check. Unset fields keep the derived values.

Layout properties (margin, grow, alignment, min/max size) stay out: per D1 the
application computes rectangles, and a retained layout engine would undo the
"hover repaints two rectangles" guarantee. Font overrides stay with the
painter's cache (D4) for the same reason the palette override is per-widget
only for colors.

## D27. Display widgets are opt-in accessibility citizens

Titles, cards, stat blocks, progress bars and toasts paint decoration, so
exposing them by default would make Narrator read every label while tabbing.
`A11yInfo::display_only` marks them, and `Ui::a11y_nodes` skips such a node
unless that instance opts in: it is focusable, it can be hit (`mouse(true)`),
it carries narration or a role override, or `expose(true)` is set. Once in the
tree the role comes from the widget type (heading, group, text, progress bar)
and can be overridden per instance (`role(A11yRole::Button)` makes a clickable
card announce as a button). `expose(false)` forces a widget out.

Transient status text uses `WindowHandle::a11y_announce`: it raises a UIA
notification on the root provider instead of moving focus, which is what a
screen reader needs for a toast. `a11y_announce_with` takes
`AnnouncementPriority`: `Polite` maps to `MostRecent` (a burst of status
updates does not queue up) and `Assertive` to `ImportantMostRecent` (errors
interrupt the current utterance). A display widget that explicitly sets
`mouse(true)` also gets container-drawn hover/press feedback (a tinted outline,
accent while pressed); widgets that paint their own hover state never set that
flag, so the two never double up.

## D28. Widget-specific behavior goes through a typed seam

The retained tree stores `Box<dyn Widget<M>>` (D11), which is what keeps
painting and input dispatch uniform. Widget-specific operations (list scroll
and selection, slider range, text insertion, pager pages) still need the
concrete type, so `Widget` exposes `as_any`/`as_any_mut` and `Ui` provides two
methods:

- `with_widget::<W, R>(id, f)` reads; unknown id or wrong type returns `None`.
- `edit_widget::<W>(id, f)` writes; the closure returns whether it changed
  anything, and only then is the widget marked for repaint (D120 defers that
  repaint to `Ui::flush_damage`; it used to invalidate the rectangle at once).

A wrong-type edit is a no-op that repaints nothing, so lookups are safe to
probe. These methods do not emit messages: the application already owns its
data and decides whether a change becomes one. They also cannot know whether a
screen reader can see the change, so the caller marks the snapshot stale
(`WindowHandle::a11y_refresh`) when it matters — the same contract as the
`Ui::set_*` setters. `Any` requires `Self: 'static`, so generic widget impls
carry an `M: 'static` bound; `Ui::add` already required that for every added
widget.

## D29. UIA pattern requests carry stable identity

Pattern methods used to post the snapshot index into the tree. A refresh
between the client's walk and the call could then resolve that index against a
different node, and a `SetFocus` could land on the wrong widget — the failure
showed up as focus on an unrelated control on a busy desktop. `A11yRequest`
now carries `(WidgetId, Option<usize> child)` taken from the element the client
called, and the UI thread builds the `A11yAction` directly from it. Indices are
still used inside one snapshot for focus-event resolution
(`A11yTree::focus_index` is id-based), but no action depends on an index that
outlived the snapshot it came from.

The automated client's focus check reads `HasKeyboardFocus` from this window's
own subtree instead of the global `GetFocusedElement`, so another focused
window (or a leftover instance of the app) cannot make it flaky.

## D30. Extended triggers: platform double click, right click, drag, value change

The trigger path from D25 was extended without adding a second input route:

- Double click uses `CS_DBLCLKS` and `WM_LBUTTONDBLCLK`, normalized as
  `Event::PointerDoubleClick`. Windows sends the ordinary down/up pair first,
  so a double click means one `Click` plus one `DoubleClick`; widgets keep no
  default double-click behavior.
- The secondary button is tracked separately from the primary press: a
  matching right down/up raises `RightClick` on release and does not focus
  ordinary controls. An editable text field is the exception: a secondary
  click focuses it, opens its native session and shows the `EDIT` context
  menu, because paste/undo live there; a secondary click inside an open
  session's field forwards `WM_CONTEXTMENU` to the child instead of committing
  it. The opening click forwards the menu after the deferred child exists, and
  the host drops its mouse capture first so the modal menu owns the mouse. The
  app-level context-menu host is still M4.
- A primary press becomes a drag after 4 DIPs of travel. `DragStart` fires
  once from `Ui::pointer_move`, the widget keeps receiving `DragTo`, and the
  release sends `ReleaseAt` (so a slider still commits) followed by `DragEnd`;
  `Click` is suppressed because a drag is not a click.
- `ValueChange` is derived, not reported by widgets: when a binding exists and
  the input can change a value, `interact` compares a small accessible
  fingerprint (checked / value text / selected flag / range / selected child)
  before and after the widget input and runs the binding afterwards. The
  fingerprint allocates only for that bound widget on those inputs, so the
  paint path and unbound controls are unaffected. The message is queued after
  the default one. `Outcome` carries the three message slots (custom, default,
  after).

Editor commits (`Ui::commit_edit`) and application setters are deliberately
outside `ValueChange`: the application already knows it changed the value.

## D31. Closed text fields carry their own caret model

The native `EDIT` control reports only committed text, so anything the
application wants to do with a selection (select-all, insert-replace, a screen
reader asking for `GetSelection`) used to require opening a session. `TextEdit`
now keeps `caret` and an optional `anchor` in **characters** while closed:

- typed access mutates the model (`set_caret`, `set_selection`, `select_all`,
  `insert_text` replaces the selection) and invalidates one rectangle;
- the closed caret/selection is painted only for keyboard/a11y focus
  (`WidgetState::focus_visible`) and never while a native session is open
  (`PaintCtx::editing` names the widget). Pointer focus after a click or
  right-click leaves no stripe, and our caret cannot be doubled under the
  native child, which only covers a text-height strip;
- offsets are converted to **bytes only at the accessibility boundary**, and
  against the *published* value — for a password field that is the mask, so a
  UIA range always lands on a character boundary and never leaks the secret;
- `A11yInfo::text_selection` is `Some` for text fields (a degenerate range is
  a caret) and `None` for number fields, which keeps
  `SupportedTextSelection` honest (`Single` vs `None`).

`TextPattern` movement is still whole-value only; the native editor continues
to publish its own UIA (with a real editing caret) while a session is open.

## D32. Offscreen is reported by the widget that virtualizes

UIA `IsOffscreen` used to be hardcoded `false`, which meant the list's
look-ahead row reported a rectangle extending past the viewport and into the
pager. The platform layer cannot know a virtualized container's viewport, so
the publisher decides: `ListView` clips each published row to its viewport and
sets `A11yChild::offscreen` when no intersection remains. `Ui::a11y_nodes`
copies it into `A11yNode::offscreen`, and the provider maps that one flag to
`IsOffscreen`. A partially visible row reports bounds clipped to the viewport;
a fully offscreen row keeps its geometric rectangle so a client that still
reads it sees where it would be. This keeps the rule from D5 (accessibility is
derived from widget state) while avoiding a second layout pass at the platform
layer.

## D33. Hit-testing is a widget hook inside container rules

Non-rectangular visuals (a round swatch, a rounded card) should not swallow
clicks in empty corners. `Widget::hit_test(pos)` defaults to the layout
rectangle, and `Ui` consults it only after its own filters (visible, enabled,
interactive, z-order) have selected a candidate. So a widget narrows *where*
its own rectangle accepts a press and cannot re-enable itself or steal input
from a widget above it. The container still owns hover/press/focus and
invalidation.

## D34. Tooltips are host-owned, targets are container-computed

The tooltip text lives on `WidgetProps` (so it follows the widget instance),
but everything with a clock and a window lives in the host:

- `Ui` computes *which* widget owns the tip (hovered wins even when that widget
  has no tip; otherwise keyboard/a11y focus when the ring is visible) and hands
  the text, a stable widget identity and the anchor to `UiHost::set_tooltip`.
  The text comes from `WidgetProps::tooltip_at(item)` when the hovered/focused
  row or option has one, falling back to the default `tooltip`; an explicit
  empty entry suppresses the tip. A held button suppresses it.
- The anchor follows native tooltip behavior: a hover tip sits at the pointer
  (lower right, flipping near the window edge) and follows the pointer until it
  is shown; a keyboard/a11y tip hangs below the focused *element* —
  `Widget::tooltip_rect` returns the selected list row / segmented/tab cell,
  falling back to the widget rectangle — and does not move once shown.
- The host deduplicates the widget identity plus the hovered sub-item
  (`Widget::child_at`: row/option/cell), so pointer-move spam inside one item
  cannot restart the dwell, while moving to another row/cell counts as a new
  target and hides the tip. A dismissed/auto-hidden tip does not pop back until
  the target changes. The dwell (450 ms) and auto-hide (5 s) are host timers.
- The popup is created from the dwell timer, not inside a `Host` callback, for
  the same re-entrancy reason the native editor is deferred (D15).
- The popup is owned, topmost, `WS_EX_NOACTIVATE | WS_EX_TRANSPARENT`, with
  `WM_NCHITTEST -> HTTRANSPARENT`, so it never takes focus and hovering it
  cannot fake a mouse-leave on the host. It paints with the same painter style
  and palette as the page, so it follows the theme without new styling knobs.
  `WS_EX_LAYERED` alpha plus a ~15 ms host timer fades it in and out over about
  95 ms, matching the native tooltip feel while keeping the framework palette.
- Showing it uses `SetWindowPos(..., SWP_SHOWWINDOW | SWP_NOACTIVATE |
  SWP_NOZORDER | SWP_NOOWNERZORDER)`, never `ShowWindow(SW_SHOWNOACTIVATE)`.
  For an owned popup the latter can activate the owner (the reason SDL switched
  to this flag set); without that, hovering a tooltip over an inactive window
  brought the application to the foreground after the dwell.
- Dismissal is explicit: click, pointer leave, focus loss, hide, resize/move,
  DPI change and theme change hide the popup; leaving and re-entering (or
  hovering another tip) re-arms it.

## D35. Composite hover is owned by the composite, not the container

`Ui` keeps one hovered *widget* and dispatches `HoverAt(pos)`; deriving a
sub-item there would need a second hit-test layer and per-widget cell tables in
the container. Instead the composite owns its hover index the same way
`ListView` owns `hover_row`:

- `Segmented`/`Tabs` store `hover: Option<usize>` from `HoverAt`/`HoverLeave`,
  return `true` from `repaint_on_move` and paint only the hovered cell. The
  selected cell wins over the hover tint.
- `set_enabled(false)` clears the hover, and the setters now actually update
  the field for these controls.
- Hover colors stay a palette concern (`WidgetProps::palette` per instance,
  `ThemeSettings` globally); hover events remain trigger bindings
  (`HoverEnter`/`HoverLeave`). There is deliberately no third hover-painting
  path.

## D36. Popups and the exposed set

The occlusion scan treats any opaque window above this one as an occluder,
including a popup menu. While the menu is open its area leaves `visible`, and
ordinary invalidations are clipped to `visible`; when it closes there is no
queued repaint for the newly visible pixels, so the menu's pixels stay on
screen. Fixing only the paint path was not enough — the invalidations
themselves were being clipped away. Three rules now hold:

- `refresh_visibility` compares the exposed *area* with the previous scan and
  forces a full frame when it grows (a popup or another window moved away). A
  shrinking set (something moved on top) repaints nothing, so opening a menu
  does not trigger a wasted full frame.
- Menu windows (class `#32768`) are never treated as occluders: they are
  transient, and their dismissal invalidates what they covered. Excluding them
  keeps `visible` stable while the menu is open, so the invalidation clip does
  not throw away repaint requests under it.
- The native `EDIT` owns its context menu, so `WM_UNINITMENUPOPUP` on the
  subclassed child (and on the parent, because the owner depends on USER32)
  calls the forced, unclipped invalidation. It is deferred to the message
  loop — painting synchronously there would re-enter the parent's window
  procedure inside the menu's nested loop.
- `WM_PAINT` treats `rcPaint` as input: if the update rectangle reaches beyond
  the queued dirty bounding box (allowing the 1 px `InvalidateRect`
  inflation), an unqueued exposure happened and the frame does one full
  repaint. Normal hover/invalidation frames keep the precise rectangles.

## D37. Modal shell loops run from posted messages

`TrackPopupMenu` owns a nested message loop. Showing a menu inside
`Host::event` would dispatch `WM_PAINT` and `WM_TIMER` while the host is taken
and the window state is borrowed by the callback — the same re-entrancy that
D13 avoids for the native editor, but harder to see because the menu has no
window handle of its own. So `Ui` only describes the menu
(`UiHost::show_context_menu`, a borrowed `Menu` plus a client-space anchor);
the host clones it, posts `WM_APP_SHOW_MENU`, and opens the popup from the
message loop. The selected command is replayed to `Host::menu_command` after
the popup closes, which is why a handler may change labels, rebuild menus or
open an editor. The model stays application-defined (`u32` ids): the framework
never needs to know the application's message type to show a menu.

## D38. Dialogs run on a worker thread; the owner decides modality

A native dialog owns a nested message loop on the thread that calls it. Running
that loop inside `Host::event` would re-enter the window procedure with the
window state borrowed twice (the same reason D13 defers edit work and D37
defers menus). Every dialog therefore runs on its own thread:

- **Modal** (the default) disables the owner before showing and re-enables it
  from a drop guard; the owner handle is also passed to the dialog so it stays
  above the application. A cross-thread owner attaches the two input queues,
  so both threads must pump while the dialog is up — the UI thread always
  does, and the dialog thread is inside the native loop.
- **Parallel** passes no owner. The dialog is a normal top-level window, the
  application stays enabled and the user can keep working.

Either way the UI thread never runs the dialog's loop: it paints, ticks and
dispatches normally, and the result arrives as a posted message that becomes
`Host::dialog_finished`. `DialogId` correlates overlapping dialogs; a
dismissed or unshowable dialog reports `None` so the application is never left
waiting.

## D39. The tray icon is a shell registration owned by the host

The notification area is not a widget: `Shell_NotifyIconW` registers a window
with the shell and delivers clicks as messages to that window. The API is
therefore `WindowHandle::set_tray_icon(Option<TrayIcon>)` — one icon per
window, update and remove are the same call — and the host owns everything
else:

- `NOTIFYICON_VERSION_4` makes the notification carry `(event << 16) | id` in
  `wParam`, so left click/`NIN_SELECT` and `WM_CONTEXTMENU` are distinguished.
  The handler also accepts the legacy form (`lParam` is the mouse message) for
  the first notification, before `NIM_SETVERSION` takes effect.
- Right click shows the shared `Menu` at the cursor. It runs from the
  notification message, outside every `Host` callback, so the popup may run its
  own loop; `SetForegroundWindow` before and `WM_NULL` after are required for
  the popup to dismiss when the user clicks elsewhere.
- Menu commands are reported as `TrayEvent::Command { icon, command }`; icon
  actions as `TrayEvent::Action { icon, action }` (D96 refined the original
  single `Activate` into per-button actions). The framework never learns the
  application's message type, exactly like widget menus.
- A file icon loaded by the host is destroyed only after the shell received
  the replacement handle; `TaskbarCreated` re-adds the icon after an Explorer
  restart and `WM_NCDESTROY` removes it.

## D40. Hotkeys are host-owned registrations; capture is a modal input mode

`RegisterHotKey` is a system-wide registration tied to an integer id that the
window owns and that Windows delivers as `WM_HOTKEY`. Applications must not
have to manage those ids, so the host allocates them per window:

- A `Hotkey` is plain data (modifiers + virtual-key code). Registering the same
  value twice returns `true` without a second OS call; a chord owned by another
  process returns `false`. All registrations are released in `WM_NCDESTROY`.
- `parse` rejects a modifier-less printable key: a global `A` would swallow
  ordinary typing. Bare function keys are allowed. Capture enforces the same
  rule by continuing to wait instead of returning an unusable chord.
- Capture runs in the window procedure (`WM_KEYDOWN`/`WM_SYSKEYDOWN`) before
  the event reaches `Host`, and drops the translated `WM_CHAR`/`WM_SYSCHAR` so
  the captured chord cannot also type into a focused field. Modifiers are
  sampled with `GetKeyState` because the event flags may lag a modifier that
  was pressed in the same chord. Esc, focus loss or
  `cancel_hotkey_capture()` reports `None`.
- The captured chord is not registered automatically; the application decides,
  exactly like a menu command carries an application id.

## D41. Text content widgets are decoration with explicit opt-ins

The text/content group (`Label`, `Icon`, `Badge`, `Divider`, `Avatar`, `Kbd`)
follows the display-widget rule from D27: each instance is out of Tab and the
accessibility tree unless the application exposes it. Two behaviors are
inherent to the widget rather than prop overrides:

- A link label is pointer-interactive (`interactive() == true`) so hit-testing
  reaches it and `Click`/`Activate` bindings fire. It is still not a Tab stop by
  default; an app that needs keyboard access sets `focusable(true)`. The hand
  cursor comes from the widget, not per-instance event code.
- A removable badge owns a close target, so it is pointer-interactive even
  without `mouse(true)`, and `Delete`/`Backspace` remove it while focused. The
  close zone is the square at the trailing edge (height-clamped), not the whole
  chip; clicking the label area emits nothing, and the `×` hover highlight
  follows the close target (`repaint_on_move`), not the chip.

Wrapping is computed by the backend-free `text::wrap_ranges` (byte ranges,
word breaks with a character fallback for CJK, hard `\n`), and `Label` caches
the ranges keyed by width, effective font size and text revision. The cache is
what keeps the "steady-state frames allocate nothing" rule: a wrapped, idle
label reuses its range vector instead of re-wrapping. A line that does not fit
the rectangle height at all is skipped rather than painted clipped.

Single-line edge behavior is a backend primitive (`Canvas::text_overflow` +
`TextOverflow`), not a pre-truncated string: pre-truncation would allocate per
paint and would break centered/right-aligned text. GDI maps it to
`DT_END_ELLIPSIS`; other backends inherit the plain `text` default.

`Avatar` derives initials in the widget (Latin words use the first letter of
the first two words, a single ASCII word uses two letters, CJK uses the first
character) and picks a tint from a six-color table by a stable hash of the
name, so the same artist keeps one color across pages. `.image(ImageId)` uses
the image seam from D42 and falls back to the initials when the id is not
registered.

## D42. Images are page-owned, referenced by id

A widget knows an `ImageId`, not the pixels: the application decodes/creates
owned RGBA and registers it once with `Ui::set_image(UiImage)`. The page passes
the `ImageStore` to every `WidgetPaint`, and `WidgetPaint::image` hands the
borrowed pixels to the backend (which still converts and caches them). This
keeps the widget contract allocation-free and lets several widgets (a cover
wall, an avatar, a now-playing card) share one copy. Replacing pixels under the
same id keeps all references valid; an unknown id paints nothing so the widget
can fall back. `Ui::clear` drops the store with the widgets. The clipboard is a
host call (`WindowHandle::copy_text`) for the same reason: widgets stay
platform-neutral.

## D43. In-window popups are painted last and hit first

A drop-down/flyout does not create a second HWND. The owning widget reports
three facts: `paint_bounds` (layout rect plus the open popup, used for
invalidation and dirty culling), `overlay()` (true while open; `Ui::paint` runs
a second pass after all ordinary widgets so the popup is never covered, and
hit-testing checks overlay widgets first) and `dismiss()` (Escape, Alt+Up/Down delivery,
a press outside the widget, or focus moving away). The bounding-box blit
covers the union, so opening a popup invalidates one rectangle. Keeping the popup in the page also keeps its
theme, scaling and accessibility in the one tree; the tooltip and native menu
hosts stay separate windows because they must escape the page's rectangle.

## D44. Multi-selection is a role, not a list mode

`MultiSelectList` publishes `A11yRole::MultiSelectList`, which maps to the UIA
List control type but, unlike `List`, reports `CanSelectMultiple` true and
requires no selection. Children stay `ListItem` + SelectionItem. The provider
derives both facts from the role, so a client sees a single-selection queue and
a checkable list as the same tree shape with honest selection capabilities.
The list toggles a row per click/Space directly; Shift-range/Ctrl-toggle
modifier selection is separate interaction infrastructure (TODO M6.5).

## D45. Translucency is a Canvas primitive with an opaque fallback

A scrim or tinted overlay needs alpha, and GDI's solid brushes cannot express
it. `Canvas::fill_translucent` is therefore part of the drawing contract: GDI
stretches a 1x1 premultiplied DIB through `AlphaBlend`, and any other backend
inherits a default that paints the color opaquely. That default is honest (a
missing feature looks solid, not invisible) and keeps the trait implementable
by a future software rasterizer. Only the scrim uses it so far; the tooltip
and menus keep their own layered-window alpha because they are separate HWNDs.

The 1x1 source DC/bitmap is cached in `GdiResources`, and the host collapses
the dirty rectangles to the blit bounding box and passes it as
`WidgetPaint::dirty`. `Scrim` fills only those pieces: the back buffer already
holds the dimmed pixels elsewhere, so content below can update a small region
and only that region is re-blended. `Widget::paints_pointer_state` lets the
backdrop opt out of hover/press/wheel invalidation entirely. Before this, every
pointer transition over the scrim invalidated its full-window rectangle and
repainted the page (~65 ms per hover in the debug showcase at 1479x930 DIPs);
now the dimmed page stays static while the pointer moves.

## D46. The in-window modal is a scrim plus a focus scope

Native dialogs already trap focus at the OS level (D38). An in-window modal
shell does not get that for free, so it is assembled from two seams:

- `Scrim` is an interactive display widget added **after** the page widgets and
  **before** the panel. Insertion order gives the painting and hit-testing the
  right z-order without an overlay level: the scrim covers the page, the panel
  covers the scrim.
- `Ui::set_focus_scope(scope)` filters `focus_order`, refuses `set_focus`
  outside the scope, moves focus to the first member when the current owner
  falls out, and is removed on close. It is keyboard-only by design; pointer
  inertness comes from the scrim.

The background stays in the accessibility tree, so a screen reader can still
walk it; a real modal-provider/`IsModal` implementation is future work. The
scrim uses `fill_translucent`, whose non-GDI fallback is opaque (D45).

The scrim reports `paints_pointer_state() == false` and paints only the pass's
dirty pieces. Opening the panel still needs one repaint of the covered page
(it has to be dimmed), but hovering the backdrop, clicking it, or moving
between the panel's own widgets only repaints the small affected box; the
dimmed page is otherwise static. A future overlay compositor (keep a page
snapshot and dim it without repainting the widgets) would remove even the
one-time opening hitch on very large windows.

## D47. Closed multiline layout mirrors the native control

A multiline field must not look like one line while it is not being edited.
`EditRequest` therefore carries `wrap` and `ScrollBars`; the host maps them to
`ES_MULTILINE` + `ES_AUTOHSCROLL`/`ES_AUTOVSCROLL` + `WS_HSCROLL`/`WS_VSCROLL`
(no `ES_AUTOHSCROLL` means the classic control wraps). The closed paint lays
out hard lines when wrapping is off and wrapped byte ranges when it is on,
using the same width and font, caches the ranges by width/font/text revision,
clips to the content area, and draws themed scrollbars when the content
overflows. The wheel scrolls the closed view; the native editor takes over on
edit. The closed caret is not painted for multiline (the native editing caret
is the only one), but the caret/selection model still feeds UIA. Native
scrollbar chrome follows the system colors (documented limitation); the closed
bars follow the page palette. Line breaks are normalized at the platform
boundary: the classic `EDIT` control round-trips CRLF, while the widget stores
and paints `\n` (`text::normalize_newlines`/`to_crlf`).

## D48. The painted editor owns input, IME and its menu

`.painted()` keeps the text field in the framework instead of hosting the
native `EDIT` child. The reasons are concrete: single-line text can center
vertically (the classic control top-aligns), margins, line spacing and every
editor color are configurable, the palette-true selection/caret are painted by
the same code path as the closed view, and the field no longer has a native
child that appears and disappears mid-interaction. The trade is that the
framework must own what Windows used to provide:

- **Input.** `Widget` gained `inline_edit`/`inline_edit_at`, `editing`,
  `begin_inline_edit`, `finish_edit`, `insert_text`, `set_composition`,
  `take_clipboard`, `caret_rect`, `edit_menu` and `edit_command`. `Ui` routes
  every key to an open editor (Tab/Escape/Enter have commit/cancel semantics)
  and translated characters through `Ui::char`; the application forwards
  `Event::Char` because a native child would otherwise consume it.
- **Clipboard.** `UiHost::clipboard_text`/`copy_to_clipboard`; `WindowHandle`
  reads and writes `CF_UNICODETEXT` without re-entering a modal loop.
- **IME.** The window handles `WM_IME_STARTCOMPOSITION/COMPOSITION/END` and
  `WM_IME_SETCONTEXT`. It hides the system composition window (the field draws
  the composition inline, underlined), positions the candidate list and
  composition font at the caret through IMM, and forwards the strings through
  `Host::ime_compose`/`ime_commit` to `Ui`. Composition does not mutate the
  value until the result string arrives.
- **Menu.** `Widget::edit_menu` supplies Undo/Redo/Cut/Copy/Paste/Delete/Select
  All with ids in the reserved `widget::edit_command` range. `Ui` shows it when
  the application installed no menu, and `Ui::menu_command` dispatches the id
  back to the widget. Applications forward menu ids to it before their own
  commands. The secondary button only focuses and opens the menu; it never
  moves the caret, so a selection survives for Copy/Cut.
- **Caret.** The host blinks it with the system blink time, reports the
  rectangle to the IME, and reads the width from the accessibility setting
  (`SPI_GETCARETWIDTH`); `TextEdit::caret_width` overrides it. The open session
  is painted with an active border plus an inner ring and a small tint so
  keyboard and assistive-technology users can tell it apart from a merely
  focused field.

The native child stays as the default and as the fallback for `NumberInput` and
`SearchBox`; no existing field changes behavior unless it opts into `.painted()`.
UIA still sees one Edit node with a live value and text selection. Touch
gestures and drag-and-drop are not part of this decision.

## D49. The accordion is a List of expandable sections

`Accordion` paints its own header rows and the body of each open section, so
the section count is fixed at construction (`new` takes the titles; bodies are
set with `.body`). Open bodies add `body_height` of layout space, which is why
the application recomputes the rectangles it cares about after a toggle (D1) —
the widget only repaints its own rectangle and never lays out siblings.

Expansion is widget state, not a container property: clicks and
Enter/Space-in-adjust-mode run the widget's own toggle path (the same path
`a11y_select_child` and the typed `set_expanded` use, minus the message), and
`single_open(true)` closes the previous section before opening the next. Hover
is per header (D35) and a press inside an open body is not a toggle.

UIA has an `ExpandCollapse` pattern, but the framework's provider is
role-mapped and has no expander role yet. The first version therefore publishes
the accordion as a `List` whose open section is the selected `ListItem`:
SelectionItem `Select` toggles the section, and the state is announced as
selection rather than as expansion/collapse. The widget's shape (a header strip
with children) is unchanged when a dedicated `Expander` role and
`IExpandCollapseProvider` are added; that is tracked in `TODO.md`.

## D50. Overlay bounds are measured before the state change

`Widget::paint_bounds()` is the invalidation union for a widget that paints
outside its layout cell, and an open combo drop-down is the first user. But
the call that closes the overlay also changes `paint_bounds()`: after
`input()`/`dismiss()` the widget reports only the closed box, so invalidating
that rectangle leaves the drop-down pixels behind on the back buffer. Every
path that can close an overlay therefore captures the old bounds first and
invalidates the union of before/after:

- widget input (`Ui::interact`),
- Escape, a press elsewhere and focus loss (`dismiss()` paths),
- a11y `Invoke`/`Toggle`/`Select`/`SetValue` and the typed `edit_widget` seam,
- `set_visible`, `set_enabled` and `set_props`.

Hiding or disabling also calls `dismiss()`, so an open list does not reappear
with its parent. The same rule covers a future flyout or popover.

An open overlay also owns the navigation keys. `Widget::owns_navigation_keys()`
(ComboBox: `open`) makes `Ui` route Up/Down/Home/End/PageUp/PageDown and
Enter/Space to the widget instead of moving focus, which is what lets the
arrow keys move the open list's highlight. Tab still moves focus, and Escape
still closes the overlay first.

## D51. Navigation widgets stay flat; the splitter only reports positions

`Breadcrumb` and `NavRail` are leaves like `ListView`/`Tabs`: they paint their
children themselves, publish them as `A11yChild` rows with the current item
selected, and emit the clicked/selected index. The app owns the destination
logic; there is no navigation stack inside the framework.

The breadcrumb measures its crumb labels during paint and caches the rectangles
by `(width, font size, revision)`; `Widget::input` has no canvas, so hit-testing
and hover use that cache. `set_rect`/`set_items` bump the revision. A click
before the first paint (the widget was just made visible and no frame has run
yet) finds an empty cache and does nothing; the normal paint-then-input order
makes this a non-issue in practice, and it avoids a second measurement API on
the widget contract.

`Splitter` owns no pane geometry. It reports the divider centre in client DIPs
on every drag (preserving the press grab offset) so the application can clamp
to its pane minimums and set the pane/divider rectangles plus the invalidations
(D1). `Ui::set_rect` is silent by design. The widget is pointer-interactive but
not a Tab stop, and publishes as `A11yRole::Separator`
(`UIA_SeparatorControlTypeId`, no pattern). `GroupBox` is pure decoration: card
fill, border, title and rule; it becomes a named `Group` only through the D27
opt-ins.

## D52. Data widgets: display by default, one real control

`Sparkline` and `Gauge` are decoration (D27): they paint from their current
data/value and stay out of the tree unless the instance opts in, then as
`Text` (samples + range) and `ProgressBar` (RangeValue). The sparkline scales
bars from a zero baseline so a series is comparable across repaints; the gauge
reuses the `ProgressRing` dot-arc primitive with a 270° open sweep.

`RatingStars` is a real control: click/hover map a star cell to a value, a
clearable rating can be reset by clicking its current value, and arrows in
adjust mode move it. It publishes the `Slider` role with a 0..=stars range, so
the UIA RangeValue/SetValue path (and Narrator) works without a new pattern.
Hover paints a preview but never changes the stored value; read-only instances
paint but ignore input.

## D53. One scrollbar helper, one state per bar

Scrollbar geometry, hit-testing and drag math live in `widgets::scrollbar`
instead of being copied per widget. A scrollable widget stores a
`ScrollBarState` (hover, active, press grab offset) and a `ScrollBarStyle`
(thickness, insets, minimum thumb, page-vs-jump clicks, optional colors) and
calls the helper from paint/input. Colors default to the widget palette at
paint time, so a per-widget or per-theme palette keeps working; the palette
has no active color, so the helper derives one by mixing the hover color with
the accent.

`ListView` replaces its hand-written geometry/drag (identical defaults: 10 DIP
track, 2 DIP thumb inset, 24 DIP minimum) and gains a real hover state.
`MultiSelectList` had no bar at all; it now shows the shared bar whenever the
rows overflow, reserves the bar strip in hit-testing and the label width, and
guards `ClickAt` on the bar so a press there never toggles a row. Bar hover
clears the row hover.

The bar repaints with the whole widget because row-hover changes already
repaint it on pointer movement; per-part invalidation for a bar-only hover is
a later optimization. The painted/native `TextEdit` and `ComboBox` drop-down
keep their local bars until they are moved onto the helper.

## D54. Rounded corners are an anti-aliased cached mask

`Canvas::fill_round_rect` is now the rounded primitive; `Painter::round_rect`
only delegates to it. The trait default keeps the old portable composition
(three rectangles plus four ellipses), so test canvases and a future backend
still work without changes. The GDI backend overrides it with a mask:

- 4×4 supersampling computes per-pixel coverage of the rounded rectangle
  (center band + corner circles); radius is clamped to half the shorter side
  and snapped to device pixels.
- The mask is stored premultiplied BGRA in a DIB and drawn with one
  `AlphaBlend`, replacing seven GDI calls and the seams between bands and
  ellipses. The DIB cache is keyed `(pixel width, pixel height, radius,
  colorref)`; since D60 the expensive supersampled coverage is cached
  separately by geometry only (`width/height/radius`, 1 byte per pixel,
  4 MB/256 entries) and colorized per mask, so a new color for the same shape
  does not repeat the supersampling. The DIB cache is LRU-limited to 8 MB and
  128 entries; shapes above 1024 px per side use the composition fallback.
- Nested `round_rect` calls (a border then an inset fill) blend their AA edges,
  which removes the hard corner seam; the two-call border stays for now.
- `theme::radius` names the common radii (`SMALL` 4, `MEDIUM` 6, `LARGE` 10,
  `pill(height)`), and the existing rounded widgets use them. The scrollbar
  thumb is now a pill through the same path.

Per-corner radii, a real stroke primitive and an anti-aliased circle path are
still open; the mask key and generator are written so per-corner radii only add
fields.

## D55. The text field shares the scrollbar helper in both modes

`TextEdit` keeps one `ScrollBarState` per axis and a `ScrollBarStyle` (7 DIP
track, 1 DIP thumb inset, 12 DIP minimum, 5 DIP pill thumb) and calls the
shared scrollbar for geometry, hit-testing, painting, track clicks and thumb
drags. This applies to the painted editor and the closed multiline view, so
both look and behave the same.

A closed multiline field now excludes its active bar stripes from
`wants_edit_at`: a press there is dispatched to the widget (which starts a bar
drag/jump) and no native `EDIT` session opens. `scrollbar_hit` also feeds
`inline_edit_at`, so the painted editor's caret is not moved by a bar press.
The bar's hover flag drives `repaint_on_move`, which keeps hover feedback live
at the cost of repainting the field on pointer motion while a bar is hovered;
the entry event's highlight appears on the following move because the
container samples `repaint_on_move` before delivering the hover. Track clicks
jump (not page) by default.

## D56. The combo drop-down shares the scrollbar helper

The open `ComboBox` list uses the shared scrollbar with a slim style (6 DIP
track, 1 DIP insets) instead of its hand-written 4 DIP track. The bar is the
`scroll` field read as a row offset: a thumb drag maps to lines and rounds, a
track click jumps, and `close_list` resets the bar state. `visible_row_at`
and `ClickAt` both exclude the track rectangle, so a press on the bar neither
selects the row under it nor closes the list; `PressAt`/`DragTo`/`ReleaseAt`
now drive the bar (they were unused before). Wheel and keyboard highlighting
still scroll the list and move the thumb.

## D57. Application chrome opens native menus; the toolbar has fixed cells

`MenuButton`, `SplitButton`, `MenuBar` and the `Toolbar` overflow do not paint
menus. A widget returns a `NativeMenuRequest { menu, anchor, avoid }` from
`Widget::take_native_menu`; `Ui` drains it after the widget handled the input
and calls `UiHost::show_context_menu`, so the chrome reuses the context-menu
host: the model is cloned, the popup is shown from a posted message (never
inside a `Host` callback), and the command is replayed through
`Host::menu_command(widget, command, …)`. An in-window painted menu looked and
behaved like a second menu system, repainted the page around it and had to
reimplement keyboard walk, submenus and accessibility; the native popup is
instant, follows the system menu colors and is accessible by the shell.

Trade-off, documented in "Known limitations": chrome menus use the system
colors, not the page palette. The page-themable path stays possible for an
application that wants it, but it is not the default.

**Placement.** A widget menu's `avoid` rectangle (the control) is passed on.
The host computes the menu's approximate height, converts the anchor and the
control to screen pixels, and uses the **client area** (intersected with the
monitor work area) as the fit boundary: the popup goes below the control
(top-left at the anchor) and, when it does not fit, flips above with its
*bottom* aligned to the control's top (`TPM_BOTTOMALIGN`). That is what makes
a top-docked menu bar drop down and a bottom-docked one open upward, and the
button that opened the menu is never covered — handing Windows the button's
bottom point and letting it flip produced a menu sitting exactly on the bar.
Pointer context menus pass no `avoid`: they anchor at the click point and are
only kept on the monitor.

**The toolbar uses fixed-width cells** (`item_width`, default 104 DIPs) rather
than measured text. The overflow split is then pure geometry, identical for
paint, hit-testing, the accessibility tree and a UIA action before the first
frame; a measured cache made those disagree when a snapshot was built between
a layout change and the next paint. Labels ellipsize inside their cell, which
is how icon toolbars behave anyway. Plain `Button` widgets stacked with an
`HStack` remain a valid alternative for an application that wants per-item
widgets; the composite is the convenience with built-in overflow.

**Keyboard ownership is per key.** `Widget::owns_navigation_key` (defaulting to
`owns_navigation_keys`) lets `MenuBar`/`Toolbar` keep `Left`/`Right` for their
item cursor only while the cursor can move that way. At the ends, and for
`Up`/`Down`, the key moves focus, so arrows can never trap the user inside a
strip; `Tab` always leaves. `Alt+key` access keys are parsed once
(`MnemonicLabel`) and routed by the container: a widget may claim a per-item
key (`Widget::mnemonic_key`), otherwise the first matching `mnemonic()` is
focused and activated.

## D58. Pointer motion repaints only a real hover change

`Widget::repaint_on_move_at(pos)` (defaulting to `repaint_on_move`) lets the
container ask "would this pointer position change what you paint?" before
delivering the hover. Composite widgets compare the cell/row under the pointer
with their stored hover: `Toolbar`, `MenuBar`, `SplitButton`, `ListView`,
`Segmented`, `Tabs`, `Accordion`, `Breadcrumb` and `NavRail` now repaint only
when the hovered item changes, instead of on every mouse event. Moving the
pointer inside one row of a long list or one cell of a toolbar no longer
invalidates the whole rectangle, which was the observed CPU spike while
"hovering" a control that was already hovered. Widgets whose whole look
depends on the pointer (a slider preview, a star rating) still return `true`
unconditionally.

## D59. Native popups follow the page's dark/light decision

Win32 menus do not follow the page: a dark page got a light popup, because
Windows only themes menus dark for applications that opt in. The framework now
opts in through the undocumented `uxtheme.dll` exports, resolved dynamically
(so an OS without them keeps its default menus):

- `SetPreferredAppMode` (ordinal 135) with `ForceDark`/`ForceLight` — the
  process app mode decides how the shell draws menus and the controls that
  follow it. Forcing it makes the page decision win over the system setting,
  which is what "the menu follows the window theme" means for this framework.
- `FlushMenuThemes` (ordinal 136) after every change, so the next popup uses
  the new mode instead of a cached theme.
- `AllowDarkModeForWindow` (ordinal 104) on the main window (its native
  scrollbars follow) and on each `#32768` popup. `TrackPopupMenu` creates the
  menu window itself, so a thread-local `WH_CBT` hook themes the popup at
  `HCBT_CREATEWND`; the hook only compares the class name and calls two FFI
  functions, so it cannot allocate or panic.

The call sequence is: `follow_theme(dark)` before the first window exists (the
shell themes its menu class from the app mode) and again from
`WindowHandle::set_theme`, followed by `apply_to_window` for the main window.
`--a11y-dump` and the self-test still pass; `BAMBOO_UI_DEBUG=1` prints whether
the exports were found and which mode was set. The exact popup colors remain
the shell's (an owner-drawn menu would be needed for page-palette colors), and
a system without the exports shows the system menu colors.

## D60. Page-open cost: bounded draw counts, shared coverage masks

Opening a page with many widgets was a single full repaint of everything, so
every cold resource on that page was paid at once. Two costs dominated, both
measurable with `BAMBOO_UI_PAINT_LOG=1` (the log now also prints each widget
slower than 1.5 ms with its rectangle):

- `ring_dots` interpreted its `from`/`to` arguments as a 0..1 fraction and
  multiplied the step count by 60, while both callers pass degrees. A full
  `ProgressRing` therefore painted ~21,600 ellipses and a `Gauge` ~16,200:
  38–52 ms per widget per frame. The dots are now spaced by about one dot
  diameter along the arc, derived from the radius (clamped to 1.5–12°, at most
  360 dots): a 56 DIP ring draws ~50 dots and a 270° gauge ~45, and a larger
  ring keeps its density without ever approaching the old count. A regression
  test counts ellipses and requires fewer than 200.
- Rounded fills keyed their cached mask by `(width, height, radius, color)`,
  so every new color rebuilt the 4×4 supersampled coverage (measured 21 ms for
  a 420×64 DIP banner at 175%). The coverage is now cached by geometry only
  (`width, height, radius`, one byte per pixel, 4 MB) and shared by every
  color; a color change is a colorize pass (0.9 ms cold, 40 µs warm). The
  supersampler also skips a pixel whose four corners are all inside — a
  rounded rectangle is convex — so only the anti-aliased corners sample.

Measured on the showcase gallery (debug, 175% DPI): the worst full-window
paint dropped from ~172 ms to ~18 ms, typical full repaints to 1–6 ms, and the
banner from 18 ms to 5.7 ms. The remaining first-paint costs are one-off font
and glyph-cache builds per page.

## D61. One menu model, one lifecycle hook, one themed native bar

**The model is mutable between opens.** `Menu` gained item-level edits
(`insert`, `remove`, `remove_command`, `item_mut`, `set_label`, `set_checked`,
`set_enabled`, `set_command`), `MenuEntry`/`MenuItem`/`Submenu` setters, a
`MenuPath` (index or label steps) for submenu addressing, and
`MenuBarModel` for the window bar. Editing the stored model is the documented
alternative to rebuilding it; commands keep their ids, so a handler does not
change. A popup keeps the snapshot it was built from (the host clones the
model), so an edit applies to the next open; D86 later added an explicit
close-and-rebuild path (`edit_open_menu` / an edited `Opened` model) for live
updates. `Ui::set_menu`/`remove_menu`/`menu` install the
registry entry; `Ui::set_menu` also writes a menu button's / split button's own
model through `Widget::set_native_menu`, so the click chord, the secondary
click and a programmatic open cannot disagree. `MenuBar`/`Toolbar` item edits
(`set_item_menu`, `insert_item`, `remove_item`, `set_item_label`) are typed
per-widget operations, not a second tree.

**Imperative triggers are deferred, like every other menu path.** `Ui::open_menu`
shows the registered menu at a client-DIP anchor; `Ui::open_menu_for` shows the
widget's own menu for its current state (`Widget::native_menu`: a menu button's
model, the menu bar's cursor cell, the toolbar overflow) and falls back to the
registry; `Ui::expand_menu` reopens a submenu at a `MenuPath` as its own root
(the row of a live popup cannot be measured from the UI layer, so this is an
explicit reopen, not a hover). `WindowHandle::open_menu` takes a model directly;
`WindowHandle::menu_click` replays a selection *without* a popup. Everything
goes through the posted-message `TrackPopupMenu` path: a `Host` callback never
runs a menu loop, and `menu_click` cannot dispatch a command from inside
`Host::timer`/`Host::event` (the host is borrowed there), so it posts
`WM_APP_MENU_CLICK`. `WindowHandle::menu_active`/`is_menu_open`,
`active_menu_widget`, `active_menu_source` and `dismiss_menu` expose the
controller state; `dismiss_context_menu` stays as an alias.

**One `Host::menu_event` hook sees every menu.** `MenuEvent::{Opening, Opened,
Selected, Closed}` carries `Option<WidgetId>` (the tray has none), a
`MenuSource` (context, chrome, programmatic, tray, menu-bar) and a decision:
`Handled` cancels an `Opening` (or an empty model is never shown) and consumes a
`Selected` command so the default replay is skipped. `Opening` hands out the
model mutably — a rebuild-before-show hook — and `Selected` hands out the
command mutably (menu aliases). Ordering: `Opening` → `Opened` → popup →
`Selected` → `Closed` → `Host::menu_command` / `Host::tray_event`. `Opened` is
now dispatched from the owner's message loop while the popup is up and carries
the live model (D86); the original implementation reported it immediately
before the blocking call. A synthetic
`menu_click` reports only `Selected` (there is no popup to close). The hook runs
with the host taken out and with the same post-callback work as a timer tick,
so it may invalidate, edit the page or open another menu.

**The window menu bar is a native `HMENU` with owner-drawn top-level items.**
`WindowBuilder::menu_bar(MenuBarModel)` / `WindowHandle::set_menu_bar` build a
root menu with one popup per model entry and `MF_OWNERDRAW | MF_POPUP` bar
items. The shell has no per-app dark mode for the non-client menu bar (Windows
keeps it in the shell theme even when the app mode is forced dark, D59), so the
bar items are painted by the framework with the page palette: `WM_MEASUREITEM`
sizes each item from the label (minus the check column the shell reserves even
though a title never carries a checkmark), `WM_DRAWITEM` fills it with
`panel`/`nav_hover`, centers the label (equal margins whatever the shell adds)
and underlines the `&` key unless the system sends `ODS_NOACCEL` (so the
underline only appears while Alt is held). The shell still paints the bar
background and a light hairline; `WM_NCPAINT` and `WM_NCACTIVATE` run a fill pass
after `DefWindowProc` that covers the area outside the items and the bottom
edge, and draws the `divider` line. The divider is also painted inside a hovered
item, because hover repaints only that item. The strip spans the full window
width so neither edge shows a light sliver, and the fill covers the frame row.
The menu font comes from `SystemParametersInfoForDpi(SPI_GETNONCLIENTMETRICS)`
and is rebuilt on DPI change; the theme switch calls `DrawMenuBar`, which
repaints the bar with the new palette in both directions.

**Keyboard activation:** the owner-drawn titles keep their label text through
`SetMenuItemInfoW(MIIM_STRING)` — the drawing is still `WM_DRAWITEM`, but the
stored string is what the shell's mnemonic matcher and menu mode read, so
`Alt+key`, the Alt tap followed by a letter, arrow/Enter navigation and the
cycling between titles that share a letter are the shell's own behavior.
`Alt`/`F10`, the menu-mode keys and the drop-downs' keyboard handling are the
default window procedure's; a key the page did *not* consume is handed to
`DefWindowProc` for `WM_SYSKEYDOWN` when a bar exists (which also restored
`Alt+F4`/`Alt+Space`), and while the shell reports menu mode
(`GetGUIThreadInfo` flags `GUI_*MENUMODE`) keys go straight there so the menu
keeps them instead of the focused widget.

Because the drawing is owner-drawn, the host keeps one safety net: after an
Alt *tap* (the release arms it, or the shell's menu mode is active) a
letter/digit whose `&` key matches a title is opened by the host
(`MenuBarModel::mnemonic_indices`, repeated letters cycle) through a posted
`WM_APP_SHOW_BAR_MENU`, which shows that title's existing popup at its screen
rectangle and replays the selection through `dispatch_menu_bar_command`. A
chord **held** with Alt (`Alt+P`) is deliberately not intercepted: it keeps its
application meaning and only reaches the shell through the normal
`DefWindowProc` fallback, so the bar cannot swallow an application shortcut.
The fallback opens the popup directly (no title highlight first) and does not
re-implement Left/Right traversal between titles; the shell's own menu mode
still provides those.

**Accessibility is the shell's.** The native bar appears in the UIA tree as a
`MenuBar` with `MenuItem` children (name "应用程序" on a Chinese system) with
Invoke, so no page-side publishing is needed. `--a11y-dump`'s focus walk skips
system `MenuBar`-class elements: they report `HasKeyboardFocus` whenever the
window has focus and would mask the page's own focus ring.

## D62. The flat widget vector is the tree; removal and z-order are container operations

`Ui` stores `Vec<Box<dyn Widget<M>>>`. That order is the paint order, the
hit-test order (reversed) and the Tab tie-break for equal focus weights, so
structural edits are operations on the vector, never on a second tree.

`Ui::remove(id, sink)` drops every per-id side table the container owns — focus
policy, cursor override, visibility, `WidgetProps` (including the context menu),
trigger bindings, focus hooks and focus-scope membership — and keeps the
index-based pointer/focus state coherent by shifting it (a stale index would
otherwise point at the next widget). An open editor is closed first, the same
as a focus loss: a painted draft commits and returns its message through
`drain_messages`, and the native child is ended with commit only when it
belongs to the removed widget (`sink.editing_id()`) — its text then reaches
`Host::edit_committed` after the widget is gone, so an application that needs
it reads it there rather than through `Ui::commit_edit`; removing a neighbour
never closes an unrelated session. The focused widget's lost hook runs (its
outcome is ignored — the widget is disappearing), then focus moves to the
widget at the same Tab-order position, i.e. the next row slides up, and the
removed rectangle (overlay included) is invalidated. The accessibility snapshot
is still host-owned: the application calls `WindowHandle::a11y_refresh` when a
client must see the change.

`insert(at)`/`insert_before(anchor)` shift the container's indices and leave the
widget's own state alone. Ids are `&'static str` and must stay unique, so a
dynamic instance uses a fixed pool (the showcase's two-slot panel) until the
M7 id-allocation decision. `raise`/`lower` move an entry to the end/start and
remap the indices between the two positions; nothing is invalidated because
z-order alone changes no pixels unless widgets overlap, so the caller repaints
the affected union. `move_rect` is the layout-safe convenience `set_rect`
deliberately is not: old ∪ new invalidation, a native-editor `move_edit` when
that widget owns the session, and a tooltip-target refresh. `rebuild(area, build)`
runs the whole-page sequence once: commit an open inline edit (returning its
message), end a native session, `clear` (page images survive so covers are not
re-uploaded), rebuild, invalidate `area` and refresh the tooltip.

## D63. The table is a virtualized leaf; sorting stays application-owned

`Table` paints its own header band and rows from `Vec<TableRow>`; it creates no
child widgets, so a thirty-row table costs a viewport's worth of work and a
thousand-row table costs the same. Columns are geometry only: `TableColumn`
carries a title, a fixed DIP width or `flex` (shares what the fixed columns
leave), and an alignment. The body excludes the shared scrollbar strip, and the
header cell always reserves room for the sort arrow so a title never shifts when
the sort column changes.

Sorting is application-owned. A header click emits the column index through
`on_header_click`; the application re-sorts its data, pushes the new rows
(`set_rows`) and paints the indicator (`set_sort(column, ascending)`). The
widget cannot know whether a column holds numbers, dates or locale-collated
text, and guessing would produce silently wrong orderings. `set_sort` only
validates the column and paints a hollow triangle (an up arrow widens downward).

Rows behave like `ListView`'s cursor: arrows/PageUp/PageDown/Home/End in adjust
mode, Enter or a double click activates the selected row, a click selects it and
emits `on_select`, and a disabled row neither selects nor activates. Wheel,
thumb drag, track click and thumb hover use the shared scrollbar helper.

Fixed columns can be resized from their header divider: a press within 4 DIPs of
a fixed column's trailing divider starts a drag (the `SizeWE` cursor shows it),
the drag updates the width in place with a 24 DIP floor, and the release emits
`on_column_resize(column, width)` so the application can persist the layout.
A flex column has no divider of its own (its width is the leftover). Header
clicks never fire for a divider press.

A header press that moves more than 6 DIPs horizontally becomes a column move
instead of a click: the widget paints an accent insertion line across the
header and body, and the release applies `remove(from)` + `insert(to)` to its own
column vector (the sort indicator follows its column through the same remap)
before emitting `on_column_reorder(from, to)` for persistence. A smaller move
stays a header click. Columns are widget geometry, so the widget applies the
move itself, the way it owns widths; the message only records the order.

When the fixed widths exceed the viewport the columns scroll horizontally: a
second shared bar appears under the body, the row area shrinks, the header and
cells share one horizontal offset, and hit-testing/`column_span` follow it. The
two bars resolve together (each shrinks the room the other axis has) in a
three-pass loop. A11y publishes both axes through the Scroll pattern, so a UIA
`SetScrollPercent` moves the columns like a bar drag.

A11y publishes the visible rows (plus one look-ahead row marked offscreen with
clipped bounds) as a `List` whose `ListItem` names join the cells with ` · `;
per-cell navigation needs a native Table/grid pattern and is future work, as are
column reordering, inline cell editing and a header-cell child role.

## D64. The tree flattens on structural change, never on paint

`TreeView` owns `Vec<TreeItem>` (label, children, `expanded`, `disabled`) and a
cached `Vec<FlatRow>` of the *visible* nodes. The cache is rebuilt on
`set_items`, on `set_expanded`/a chevron/Space toggle and on a parent collapse,
never during paint, so a frame only indexes rows. A flat row stores its path
(`Vec<usize>`) rather than its label: the paint and the a11y publisher read the
label through the path (cheap for a small tree and correct after a rebuild),
and messages carry the path (`&[usize]`), which stays valid across structural
changes — unlike the flat index.

The cursor is a flat index plus a path restore: `rebuild_rows` remembers the
selected row's path, maps it through the new flatten and falls back to the
nearest surviving ancestor when the node disappeared (collapsing a parent puts
the cursor on the parent). Selection defaults to the first row; disabled nodes
are painted dim, cannot be selected or activated and are skipped by arrow
movement, but their chevron still toggles (their children are data, not
actions).

Input mirrors the shell's tree view: a click on the chevron column toggles (and
emits `on_toggle`), a click on the row selects (emits `on_select`), a double
click or Enter activates, and in adjust mode Right expands or walks to the
first child, Left collapses or walks to the parent, Up/Down/Page/Home/End move
the cursor, Space toggles. Only the intersecting rows are painted, hit-tested
and published; the publisher adds one look-ahead row marked offscreen with
clipped bounds, and the shared scrollbar handles wheel/thumb/track input.

A11y publishes a `Tree` with one `TreeItem` per visible row (label, selected,
enabled, offscreen, expanded on expandable rows) — see D81 for the per-node
ExpandCollapse.

A row may carry an MDL2 glyph and secondary text (`TreeItem::glyph`,
`TreeItem::secondary`). Both are pure paint: the chevron column stays at the
indent, the glyph column (18 DIP) and the right-hand secondary column (96 DIP)
are reserved per row in the flatten, so the label starts and ends at the
column its row actually uses (a row without a glyph gives that width to the
label). The label ellipsizes into its column and secondary text is right
aligned, muted and ellipsized; neither affects hit-testing, selection or the
accessibility name, which stays the label. The tree still has no drag reorder
or inline rename, and the node count is not virtualized *inside* a collapsed
branch (flattening walks the expanded part only).

## D65. A scroll view is a container region, not a widget that owns children

`ScrollView` (`scroll.rs`) is plain data: a viewport, the axes, a
`ScrollBarStyle`, the children (`WidgetId` + rectangle in *content*
coordinates) and the offset. `Ui` keeps a `Vec<ScrollView>`; the application
registers the panel once per layout with `Ui::set_scroll_view` and lays its
children out in content coordinates, where the viewport's top-left corner is
the origin.

The container applies the second coordinate space in exactly one place:
`scroll_layout` writes each child's rectangle as
`viewport.origin + content - offset` into the retained widget. Every other
subsystem (hit-testing, painting, typed access, the native editor, tooltips,
focus hooks, the accessibility tree) therefore keeps seeing ordinary client
rectangles, which is what the M6.5 scroll-panel note asked for — a container
widget owning its children would have broken `with_widget`/`edit_widget`,
`focus_order`, tooltip targets and overlay paint order.

Container rules, all keyed on membership in a panel (`Ui::scroll_owner`):

- **Display area, padding and range**: the *clip* rectangle is the whole
  viewport — the drawing area never shrinks, so content is shown at full size
  (mid-scroll it may slide under an overlay bar, which paints last anyway).
  `ScrollView::padding` (uniform or per side) is the content margin: children
  start at `viewport.origin + padding`, and the *range* ends one padding
  inside the visible bars (`max_offset = content - (viewport - 2·padding -
  bar thickness)`). At the end of the range the last row is fully visible,
  stops `padding` before the bar's inner edge and never reaches the edge — the
  "let the axis scroll a little further" the showcase asked for. Two earlier
  drafts were wrong in opposite directions: subtracting the bar from the clip
  area made the content look permanently clipped at the right, and leaving the
  range at `content - viewport` let the row run under the bar at the end.
  The same visible size must feed the clamp, the thumb ratio, the track-jump
  and the drag math (they disagreed once, so the pointer could only drag the
  thumb part of the way while the programmatic setter reached the end).
- **Paint**: a child is skipped when it does not intersect the viewport, and
  otherwise painted under `Canvas::push_clip(viewport)`, including its overlays
  and the opted-mouse outline. `Ui::bounds` returns the clipped rectangle, so
  an invalidation never dirties pixels outside the viewport (nothing else can
  change there).
- **Bars, junction and pointer**: a press over one of the panel's bars is
  consumed by the container (thumb drag with the grab offset, track click
  page-or-jump that then keeps dragging, hover tint, arrow cursor), never
  reaches a widget and never moves focus. A bar's display decision is
  independent per axis (content vs the *padded* viewport), so content sized to
  the viewport does not grow a horizontal bar the moment a vertical one shows.
  A shown bar's track stops where the other shown bar starts; the square where
  the two meet belongs to neither, so it is filled with the track color and
  treated as chrome by hit-testing — otherwise the content showed and clicked
  through the junction.
  `hit_index` reports nothing over a bar strip. A child that does not intersect
  the viewport is not under the pointer even when its geometric rectangle
  covers the click — that rectangle lives in content space below the fold.
- **Wheel**: the innermost wheel-scrolling widget wins (`Widget::scrolls_with_wheel`,
  true for lists, tables, trees, multiline fields and an open combo list), so a
  notch never double-applies; otherwise the panel under the pointer scrolls by
  `wheel_step` DIPs. `Ui::wheel_xy` carries both axes; `wheel` is the vertical
  wrapper.
- **Focus**: Tab order includes scrolled-out children (a panel's content is a
  page, not a virtualized list), and a keyboard/a11y/programmatic focus move
  reveals the new child (least-distance scroll). Pointer focus never scrolls:
  the user clicked the visible part, and moving the content under the cursor
  would undo the click (a real bug report from the showcase).
- **Accessibility**: a named panel publishes one `Group` node for the viewport
  with the children nested under it. A fully scrolled-out child keeps its
  geometric rectangle and reports `offscreen` (the same rule as a virtualized
  list's look-ahead row); a partially visible one reports its viewport-clipped
  on-screen rectangle.
- **Visibility** is derived: a panel is shown while at least one child is
  visible, so a page switch that hides the children hides the bars too.

The panel owns no pixels of its own: the application paints the viewport
background (a `Card`, a page fill) below the children. One level of scrolling
is supported (no nested panels); the content extent is derived from the
children unless `ScrollView::content` supplies it; a child's rectangle is
panel-managed, so an application re-registers the panel after a layout change
instead of moving a scrolled widget with `Ui::set_rect`/`Ui::move_rect`.

## D65a. Scrolling is in DIPs everywhere, including row/line lists

`MultiSelectList` and the painted/closed `TextEdit` used to scroll by whole
rows/lines (`scroll: usize`, `scroll_line: usize`): the wheel moved one step and
the thumb only had as many positions as there were rows, so a short list showed
two or three thumb stops while the content itself could sit anywhere. Both keep
a fractional DIP offset now (`scroll: f32`, `scroll_y: Cell<f32>`):

- rows/lines are painted from `floor(offset / step)`, with one partial row/line
  at each end; hit-testing adds the offset back; the caret reveal scrolls the
  least pixel distance that shows the caret line.
- a wheel notch follows the **system setting** (`SPI_GETWHEELSCROLLLINES`, 3 by
  default, `platform::wheel_lines`; a "scroll a page" setting reports the
  `PAGE_LINES` marker and clamps to the range): row/line widgets multiply their
  row/line height by it, and the scroll view panel scales its 48 DIP notch by
  the same ratio. The wheel therefore feels like the rest of Windows — the
  text field used to move less per notch than the overview list. (An earlier
  draft capped the notch at a quarter of the range, which made short fields
  crawl; the jump it was compensating for was the thumb-unit bug below.)
- the scrollbar's `content`/`visible`/`offset` are all in DIPs. Two unit
  leftovers made the thumb jump to the end as soon as the content moved a
  little: the painted editor's vertical bar fed a *line* count with a DIP
  offset, and `MultiSelectList::thumb_rect` multiplied the DIP offset by the
  row height (a remainder of the row-index days). Both are covered by tests
  now — the editor paints the thumb at two offsets, the list requires the thumb
  to start at the track's top, end at its bottom and sit halfway in between.

## D66. Modified clicks reach only widgets that opted into selection

`Event::PointerDown`/`PointerUp` carry a `PointerModifiers` (Ctrl/Shift, sampled
from `GetKeyState` in the window procedure — the message flags lag a modifier
pressed in the same chord). `Ui::pointer_up_with` turns a modified release into
`WidgetInput::SelectWith(pos, modifiers)` **only** for a widget whose
`multi_selectable()` returns true; every other widget still receives
`ClickAt`, so a Ctrl+click on a button is an ordinary click. The plain
`pointer_up`/`pointer_up_button` wrappers pass `PointerModifiers::NONE`, so
tests and hosts that do not track modifiers keep working. `SelectWith` maps to
`Trigger::Click` for bindings.

`MultiSelectList` is the first consumer. It keeps an `anchor` (the last row
toggled without Shift): a plain click toggles one row and moves the anchor,
Ctrl+click toggles one row (accepted for muscle memory, and it moves the
anchor), Shift+click selects the inclusive range from the anchor and keeps it.
Because this is a *checked* list, the plain click is a toggle, not a
replace-selection — the modifier adds the range operation, which is the part a
check list cannot express otherwise. `set_rows` clamps the anchor; a Shift
click with no anchor yet behaves like a plain toggle.

The bar hit-test in `widgets::scrollbar` follows the same "the widget owns its
real target" idea: a press anywhere across the track within the thumb's axis
span grabs the thumb. The pill is inset inside the track, and treating that
2 DIP strip as a track click made the bar jump instead of drag on exactly the
edge the user aims at ("the scrollbar only drags sometimes"). A track click
also starts a drag (grabbing the thumb's centre where it lands) and the
release clears every held bar, so a press can never leave a thumb stuck in the
active style.

## D67. Scroll reaches a screen reader as published DIP axis state

The UIA Scroll pattern needs a percentage and a view size per axis, while the
widgets keep a DIP offset and a content extent. Instead of teaching the
provider each widget's internals, a widget publishes its geometry once per
snapshot and the provider does the arithmetic:

- `A11yInfo::scroll` carries an `A11yScroll` with one optional
  `A11yScrollAxis` per axis: `offset`, `viewport` (the scroll range's viewport)
  and `content`, all DIPs. `percent()`, `view_percent()` and `target()` are the
  only place a percentage appears.
- Publishers: `ListView`, `MultiSelectList` and `TreeView` use their list rect,
  `Table` its body (the header never scrolls), and a named `ScrollView` pane
  publishes both axes from `content_rect()` against the resolved content
  extent, so its percentages agree with `max_offset()`. Each widget also
  implements `Widget::a11y_scroll` and clamps a target to its own range; the
  scroll view is a container region, so `Ui::apply_a11y` moves it directly.
- The pattern is advertised only while an axis overflows
  (`A11yScroll::scrollable`), independent of the role: a list that fits has no
  Scroll pattern, and one whose rows grow past the viewport gains it on the
  next refresh. All six scroll property ids are answered for clients that read
  properties instead of the pattern.
- `Scroll` amounts map to DIP travel: a small step is one wheel notch (48 DIP,
  capped at the viewport) and a large step is 90 % of the viewport, so a page
  keeps a strip of the old content visible. `NoAmount` leaves its axis alone,
  a request at the end of the range succeeds as a no-op, and asking for an axis
  with no range fails the call, as the platform contract expects.
  `SetScrollPercent` accepts `0..=100`, treats `-1` (`NoScroll`) as "leave this
  axis" and rejects anything else with `E_INVALIDARG`; the provider converts
  the percentage to an absolute offset from the snapshot, so nothing is lost to
  rounding.
- The request (`A11yRequest::Scroll` → `A11yActionKind::Scroll`) carries
  absolute DIP offsets and is posted to the window like every other pattern
  call, so it runs on the UI thread and invalidates through the same path as
  the wheel or a thumb drag.
- Text fields keep `TextPattern` instead: their viewport is a text-editing
  concern (caret reveal, inline IME), not a container a client should page.
  The scrollbar's own thumb drag remains the pointer path.

## D68. The cover wall is a virtualized grid over page images

`CoverWall` is the player's browse surface. It is a leaf widget like the table
and the tree, not a container over placed children:

- Columns come from a target tile width: `cols = floor((available + gap) /
  (tile + gap))` and every tile takes an equal share of the row, so the grid
  fills the widget at any width without the application computing a layout. A
  shown bar takes its 10 DIP strip out of the row width; the decision is made
  in two steps (full width, then the strip when that content overflows), and
  because fewer columns only make the content taller the step is stable.
- Artwork is page-owned (`Ui::set_image`, D42) and a missing id paints a
  deterministic gradient placeholder with the title's first letter, so the wall
  is complete before the artwork pipeline exists. Covers are square crops; a
  rounded image mask is still open (rounded-corner item).
- Only intersecting cells are painted and published, with one look-ahead row
  marked offscreen, so a large library stays cheap. The shared scrollbar and
  the wheel (one row per notch at the default system line count) move it.
- Selection is a current-cover cursor: a click selects, a double click or
  Enter opens (`on_open`), so a single click never starts playback. A pointer
  click deliberately does **not** reveal-scroll: the tile was visible where it
  was clicked, and scrolling the few DIPs that finish a partially visible row
  clipped the row above and read as the grid jumping. Keyboard and typed
  selection still reveal the row; that is the same pointer/keyboard split the
  container uses for focus in a scroll view (D65).
- The selection frame is painted in a 2 DIP band reserved around every tile,
  not over the artwork. A frame drawn on the image edge makes the selected
  tile's picture look smaller than its neighbours; the reserved band keeps
  every tile's geometry identical in every state, which is also what the
  selection tests assert.

## D69. Chrome-only input reports a narrow repaint region

D58 stopped pointer motion from repainting on every event, but a bar hover
tint or a thumb press still invalidated the whole widget, repainting every
visible row of a 600-row list. `Widget::take_repaint_hint` closes that gap:

- After every `Widget::input`, the container takes the widget's hint. A widget
  that changed only chrome returns the chrome rectangle (a scrollbar strip);
  `None` keeps the old rule (the union of the paint bounds before and after,
  which is also what an overlay needs). The hint is consumed even when the
  input does not repaint, so it can never leak into a later event.
- Scrollable widgets also implement the position-aware
  `Widget::repaint_on_move_at`: moving inside one row, or along an
  already-hovered bar, repaints nothing; a thumb drag returns `true` because
  the content moves under it. This is what D58 asked for on composite widgets.
- A click that lands on the bar after a bar press must not repaint the rows:
  the row activation already ran on the release, so the widgets guard their
  `ClickAt` path with the same hit test and return the bar hint.

## D70. The waveform seek bar paints from amplitudes; progress stays app-owned

`WaveformSeekBar` is a leaf like the slider, but its value is playback time, so
the application owns it:

- The samples are normalized amplitudes (`0..=1`) supplied by the decoder; the
  widget repeats or truncates them to one bar per slot across its width. Without
  samples it paints a flat line, so a seek target exists before the waveform is
  decoded. The played/muted split and the playhead read from the same position,
  and a drag shows a preview instead of the last committed value.
- A press seeks and every drag move emits (`0..=1`), so the player scrubs
  continuously; the release commits the preview locally. Keyboard steps are 2 %
  and update optimistically; the application's next `set_progress` (or playback
  tick) confirms the value without emitting.
- UIA is the Slider shape: RangeValue 0..=1 with `SetValue` (a `%` suffix is a
  convenience), so a client seeks the same path the pointer uses. The optional
  `duration` only formats the accessible value text (`m:ss`), it never changes
  the range.
- Painting is bars plus a line/handle, no track fill: the widget sits on the
  player's surface and reads as a waveform, not a form control.

## D71. Panel chrome is repainted when a scroll-view child changes visibility

A scroll view's bars span the whole viewport, but its children cover only the
content. `Ui::set_visible` and `Ui::remove` therefore invalidate the owning
panel's viewport in addition to the child's own rectangle. Without that, hiding
the children of a page's panel (a tab switch) repainted only the rows: the part
of the bar below the last child stayed on screen and appeared over the next
page's widgets. The invalidation is exact (one viewport rectangle per call), so
a tab switch still costs one repaint per panel rather than a full-window frame.

## D72. The transport strip is one widget with an application-owned state

`PlaybackTransport` looks like five buttons, but it is one retained widget with
a cursor, like a segmented control with glyphs:

- Splitting it into five `Button`s would give five focus stops and five Tab
  targets for one player control, and the page would have to lay them out. One
  rectangle, five equal cells (the application sizes the strip) keeps the
  layout contract (D1) and one focus stop.
- The action is the only thing the widget decides. `playing`, `shuffle` and
  `repeat` stay application state: the strip emits `TransportAction` and the
  application pushes back what actually happened (`set_playing`, ...). An
  optimistic toggle would lie whenever the player rejects a command, the same
  reason `Toggle` is app-owned.
- Keyboard: the strip claims Left/Right/Home/End and Enter/Space through
  `Widget::owns_navigation_key`, so the arrows move an internal cursor and
  Enter runs the cursor cell. This avoids adjust mode's two-step entry (one
  Enter to enter, another to run), which reads badly on a play button; Up/Down
  and Tab still leave, so navigation is not trapped.
- UIA publishes a `ToolBar` whose children are the actions, so a screen reader
  sees five named controls (播放/暂停 follows playback, 循环播放 names the
  mode). Shuffle/repeat carry Toggle state; the play cell carries the action
  name rather than a "pressed" state because pressing it is momentary.
- Painting is a line of glyphs, not a boxed control: the play cell is the only
  filled shape (accent while playing), and the on-state of shuffle/repeat is
  the accent glyph color. The strip therefore sits flat on the player surface
  next to the waveform (D70).

## D73. PlaylistView reports reorders; the application owns the order

The playlist is the first widget that mutates order, so the ownership line is
explicit:

- The widget keeps a copy only for painting and hit-testing, like every other
  retained list. A drop reports `(from, to)` where `to` is the destination in
  the list *after* the removal (`rows.remove(from); rows.insert(to, track)`).
  The application applies the move to its data and pushes the result back with
  `set_rows`; the widget never reorders its copy, so the two cannot drift when
  the player rejects or filters a move.
- Dragging is widget-local instead of the generic drag-and-drop layer. The
  insertion line, the half-row auto-scroll near the viewport edges and the
  no-reveal click are the row semantics a list needs; the cross-widget drop
  infrastructure (open in TODO) can reuse the `DragStart`/`DragTo`/`Release`
  protocol the container already delivers. `Alt+Up/Down` runs the same message,
  so keyboard and pointer share one code path.
- `Row` is reused unchanged: `current` paints the playing marker (accent bar +
  accent title), `trailing` carries the duration and `disabled` rows cannot be
  selected, played or dragged. A separate `Track` type would duplicate the
  list row contract for no gain.
- The cursor follows the moved track (the widget sets `selected = to` before
  reporting), and a drop that lands back in place emits nothing. Pointer
  selection happens on release without reveal-scrolling, like the cover wall.

## D74. The flyout is a card plus sibling content widgets

The container keeps one flat widget vector with ids (D1/D62), so a popover
cannot own children without introducing a second tree:

- The card paints only the surface (rounded border, notch) and the application
  adds its content widgets **after** it. Later widgets paint above the card and
  hit-test first, so the card's light-dismiss hit area only catches presses
  that miss the content. This keeps the popover a normal widget with props,
  visibility and typed access like everything else.
- Placement is a pure helper (`place_popover`) because layout is the
  application's job. Auto placement tries below, then above, then the sides,
  and every mode clamps into the viewport; the widget stores the anchor only to
  draw the notch.
- Light dismissal is a press outside the card while the card's hit area covers
  the client. The press is swallowed, so the flyout behaves like a transient
  overlay: one click closes it and does not also activate the page. The
  dismissal message closes both the card and the content; visibility stays
  application-owned.
- Escape is not built in. The container's `dismiss` path belongs to
  one-widget overlays (a combo drop-down) and returns no message, so an app
  handles Escape itself or puts a close control inside the card (the showcase
  does both). A future container-level outside-press notification could remove
  that wiring.
- The notch is a row-by-row filled triangle (8 DIPs deep, 6 half-width) rather
  than a polygon primitive, clamped away from the rounded corners. A square tab
  with a border ring read as a dark blob at 175 %.

## D75. Chrome cells are sized by kind; Alt is a host-tracked tap

The toolbar, status bar and access-key underlines grew the last chrome
refinements without measurement or a second tree:

- A toolbar cell's extent depends on its item kind: a text cell is
  `item_width`, an icon-only cell is the bar's thickness (square), a separator
  is 9 DIPs, and a vertical bar uses `item_height` as its extent. The overflow
  split walks the extents instead of dividing by a fixed width, so separators
  and icon-only cells need no text measurement and cannot disagree with the
  painted layout.
- Separators stay items in the vector: the application's indices are stable
  across edits, but they are skipped by the cursor targets, hover, activation
  and the overflow menu. The cursor is normalized onto a command after any
  structural change.
- An icon-only cell cannot take its accessible name from a label, so
  `ToolbarItem::name` supplies it for the a11y tree, tooltips and the overflow
  menu (`label()` remains the painted text).
- A vertical toolbar is the same geometry with the axis flipped: cells stack
  from the top, the overflow button sits at the bottom, and Up/Down own the
  cursor while Left/Right move focus. This avoids a second layout path.
- The status bar reserves the size grip and the optional progress segment out
  of the segments' width, so text never runs under them. The progress segment
  publishes a `ProgressBar` child; the grip is paint only.
- Alt underlines are a host-tracked *tap*, not "while Alt is held": the host
  records an Alt press without a chord, shows the mode on release, and hides it
  on the next key, pointer press or focus change (`Event::Alt(bool)`). The
  container forwards the flag through
  `Widget::set_mnemonic_underlines`, which returns whether the widget's visuals
  changed, so only label-painting chrome repaints. The default stays on for
  hosts that ignore the event.

## D76. The ribbon is one widget with fixed cells and no overflow

The ribbon is the chrome surface above the toolbar, built from the same
principles:

- Fixed geometry, no text measurement: tabs are 88 DIPs wide (titles clip with
  an ellipsis), a large tile is 58 DIPs wide and fills the body height, and a
  small column is 96 DIPs wide with three 22 DIP rows. The application sizes
  the ribbon; there is no horizontal overflow or collapse (a mini-toolbar mode
  is a detail, not a functional need). Geometry is computed on demand and
  cached by `(rect, revision, selected tab)` so paint allocates nothing.
- The model is application-owned (`RibbonTab` → `RibbonGroup` → `RibbonItem`),
  like the toolbar and status bar. Large items are tiles (glyph above the
  label); consecutive small items fill three-row columns; group dividers and
  bottom labels are paint only. Checked items get the active background;
  disabled items cannot run.
- Keyboard: the ribbon is one Tab stop in adjust mode. The cursor lives on the
  tab band (`None`) or on a flat item index; Left/Right switch tabs on the band
  and step between items in the body, Down enters the body, Up uses rectangle
  adjacency and returns to the band at the top row, Enter runs. This avoids one
  Tab stop per command.
- The accessibility tree is a `TabList`: one `TabItem` per header (selection
  state) plus an explicit `Button` role per command child (the parent's child
  role would otherwise make commands TabItems). `a11y_select_child` selects a
  tab or runs a command through the same path as a click.
- Access keys are per tab title (`Alt+K`), claimed through `mnemonic_key`; item
  labels still paint their `&` underline in the Alt mode but are not routed as
  chords (an app-level detail).

## D77. Native menu columns and bar separators keep the shell's drawing

The native popup stays the shell's control; these additions only feed it:

- Shortcut text is the standard `label\tAccelerator` convention: the shell
  draws the accelerator in its own right-aligned column, at the system menu
  font and DPI. A separate column API (`MFT_RIGHTJUSTIFY`/owner-draw) would
  mean painting the popup ourselves, which the owner-drawn menu decision
  already rejected.
- Icons are `hbmpItem` bitmaps built from caller RGBA. The menu does not own
  item bitmaps, so the platform tracks every handle it sets and frees them
  after `TrackPopupMenu` returns (or with the replaced window menu bar). The
  app supplies theme-appropriate pixels; the shell scales 16×16 sources.
- A top-level `MF_SEPARATOR` in the window bar is a plain shell row: it owns
  no popup, no access key and no mnemonic. Because every model entry occupies
  exactly one `HMENU` position (menu entries and separators alike), the
  existing model-index → position mapping, hit rectangles and mnemonic picks
  stay valid; only `build_bar` skips the popup for separators. The strip
  painter later dropped the vertical rule for these rows: the group break is
  the gap they leave (D80).


## D78. PropertyGrid: application-owned rows with typed in-grid editors

A property inspector is data plus a small editor per value; the application
owns the data and the grid owns the editing surface.

- The model is `PropertySection` → `PropertyRow` (`key`, `value`, a
  `PropertyKind`, a per-row `read_only` flag), the same application-owned shape
  as `Table`/`TreeView`. Kinds are `ReadOnly`, `Text`, `Number{min,max,step,
  decimals}`, `Choice(options)`, `Check{on,off}` and `Rating{stars}`; the value
  stays a `String` so the app stores whatever it displays. Sections flatten into
  a scrolling entry list (header, then its rows) rebuilt on any structural
  change; the cursor and the scroll offset survive a rebuild by key.
- The grid edits the value itself and reports the committed value through
  `on_change(key, value)`; `on_activate` is for read-only rows the app inspects
  elsewhere. Text/number rows use the container's inline session, so typing,
  IME composition, clipboard paste, the built-in edit menu, Tab/Enter commit
  and Escape cancel are the framework's existing paths, not a second editor.
  The draft is a character-indexed buffer with caret + anchor: clicks place the
  caret from the advances cached by the last paint, a drag selects a range,
  Shift+arrows/Ctrl+A extend or select, and typing replaces the selection. The
  number draft filters to digits/`.``-`, steps with Up/Down and clamps/formats
  on commit. A choice opens an overlay list inside the grid (flip/clamp,
  arrows+Enter, press-outside/Escape close) because a native popup would hide
  the themed value; check and rating edit in place.
- Editability is two levels: `PropertyRow::read_only` locks one row while the
  rest stay editable, and the widget-level lock (`.read_only(true)` or
  `WidgetProps::read_only`) locks all of them. `WidgetProps::read_only` reaches
  the widget through the new `Widget::set_read_only` push (the container ORs it
  with `Widget::read_only` in `effective_read_only`), because the container
  cannot otherwise stop a composite that edits its own contents from doing so.
  Locked rows keep painting, selection and scrolling; they open no editor and
  paint muted.
- Rows are virtualized like the table (viewport + one look-ahead), with the
  shared themed scrollbar and wheel.
- The accessibility tree is an `A11yRole::List` (read-only when the grid is
  locked) with one `ListItem` per visible row (name `key: value`, cursor
  selected, locked rows disabled) and a `Text`/`Heading` child per section
  title, plus the vertical Scroll pattern. Selecting a row only moves the
  cursor: UIA `ListItem` has SelectionItem but no Invoke in this provider, and
  the grid's editors are pointer/keyboard surfaces. A real grid pattern with
  per-cell navigation and value editing through UIA is deferred with the same
  table work.

## D79. Window accelerators come from the menu bar labels

The shortcut column of a native menu is only decoration until something
matches it; a global `RegisterHotKey` is the wrong mechanism (it conflicts with
other processes, needs no focus, and cannot be disabled with the window).

- Accelerators are window-scoped: the window procedure matches
  `WM_KEYDOWN`/`WM_SYSKEYDOWN` against an exact `Hotkey` (modifiers + virtual
  key) before the page sees the key, drops the translated character, and
  dispatches the command through the existing native-menu path
  (`MenuEvent::Selected` with source `MenuBar`, then `Host::menu_bar_command`).
  A shortcut therefore behaves like a menu click, and an application that
  already handles bar commands gets live accelerators for free.
- The bar's accelerator labels are the single source of truth:
  `MenuBarModel::hotkeys()` parses each enabled item's `shortcut` text and the
  host re-derives the table on every `set_menu_bar`/`edit_menu_bar`. Disabled
  items and submenus are skipped; the first item wins a repeated chord. Labels
  that are not usable chords (a bare `Q`) still paint but do not bind.
- Bare printable keys and navigation keys without a modifier are rejected
  (`Hotkey::is_global_safe`), because a window accelerator would swallow
  ordinary typing; function keys are allowed alone. The shell owns the keys
  while its own menu loop is up, and shortcut capture owns them while it is
  active, so neither path can be stolen.
- Applications can add commands that have no menu row
  (`WindowBuilder::window_hotkey`, `WindowHandle::register_window_hotkey`) and
  remove them again. The application table is checked before the bar-derived
  one, so an explicit binding overrides a menu label, and both dispatch
  through `menu_bar_command`.
- Not covered: the painted `MenuBar` widget keeps its own Alt+letter model
  (no command accelerators), and bindings are not persisted — the application
  stores the `Hotkey` and re-registers it.

## D80. The window menu bar strip is painted whole, in one pass

Owner-drawn titles alone are not enough for a themed bar over a dark page: the
shell paints the bar's own light background, a light rule above and below the
strip, and rounded hot/focus decorations around an owner item. Leaving those
pixels visible produced light arcs near the titles after a full frame repaint
(they disappeared only when a hover repainted the item), and `SetMenu` on a
theme switch left the whole bar in the old palette.

- Whenever the shell asks for one bar pixel (`WM_DRAWITEM`, after the NC paint
  in `WM_NCPAINT`/`WM_NCACTIVATE`), the host repaints the entire strip: panel
  background, the hot cell (tracked from `ODS_SELECTED`), every label with its
  `&` underline (visibility from `WM_QUERYUISTATE`), and the theme's divider
  color as the strip's top and bottom rules. Nothing the shell drew in the
  strip survives that pass.
- The band comes from the client origin and `SM_CYMENU`, not from the item
  rects, so the very first frame can paint it before the shell lays the menu
  items out. `DrawMenuBar` plus one strip paint right after `ShowWindow` makes
  the first visible frame themed; a full repaint or hover then repaints the same
  strip, so the initial state and later updates cannot disagree.
- Top-level separators keep their `HMENU` slot as a group gap but draw no
  vertical rule (the page-theme bar reads cleaner without one). Their slot
  still carries the model index, so hit rectangles and mnemonics are unchanged.
- Access-key underlines keep the owner-drawn contract: the shell decides when
  accelerators are shown and reports it per item as `ODS_NOACCEL`. The strip
  painter stores that flag from the last `WM_DRAWITEM` and uses it for every
  label, so underlines are hidden by default, appear while the menu mode shows
  accelerators, and disappear with the shell's next ordinary draw — no custom
  tap state to go stale. The menu-closed notification (`WM_MENUSELECT` with
  `0xFFFF`) clears the flag and repaints once as a safety net for the case
  where the shell swallowed Escape; any later shell draw updates it again.
  The window-hotkey matching and dispatch are independent of this and only
  read the item rects.
- `set_theme` redraws the bar and repaints the strip explicitly: the old code
  relied on `DrawMenuBar`, which only repaints what the shell thinks changed,
  and the bar could keep the previous palette until the next hover.

## D81. Tree rows are TreeItems with per-node ExpandCollapse

A tree published as a flat `List` of `ListItem` rows told a screen reader
nothing about the hierarchy: a client could not ask a node to expand, and the
selected node's children appeared and disappeared without semantics. The
hierarchy is now visible in the accessibility tree per row.

- `A11yRole::Tree` / `A11yRole::TreeItem` map to the matching UIA control
  types. `Tree` advertises Selection; every `TreeItem` advertises
  SelectionItem, and rows with children also advertise ExpandCollapse. Leaf
  rows still answer the pattern's state as `LeafNode` (`A11yChild::expanded`
  stays `None`), which is what UIA expects from a node that cannot open.
- `A11yChild` gained `expanded: Option<bool>` and the child nodes in
  `Ui::a11y_nodes` copy it, so the per-row state reaches the provider without
  a second lookup table. Widget-level `A11yInfo::expanded` is unchanged for
  menus and menu bars.
- A pattern call carries the row index end to end: `A11yRequest::Expand(id,
  child, expanded)` → `A11yAction { kind: Expand/Collapse, child }` →
  `Widget::a11y_expand(child, expanded)`. The child is the published flat row
  index, the same identity selection and runtime ids already use, so a
  snapshot refresh between the client's walk and the call cannot redirect the
  request to another row.
- `TreeView` routes the request through `toggle_index`, the same path the
  chevron click and Right/Left keys use: the application's `on_toggle` fires,
  the flat rows rebuild, and the call is idempotent (expanding an open node
  emits nothing). A leaf or an out-of-range index is a no-op.
- The menu widgets keep their widget-level expand (they ignore `child`), and
  `PropertyGrid`/lists keep their List roles; the change does not alter them.

## D82. The date/time picker is a DTP control in a hand-built dialog

> **Superseded by D115**: the native date/time dialog is removed in favour of
> the in-window `DatePicker`/`TimePicker` components.

Windows has file, font and color choosers but no standard date dialog, so the
picker is a small modal dialog around the `SysDateTimePick32` common control.

- `DateTimeDialog` carries a mode (`Date`, `Time`, `DateAndTime`), an optional
  initial value (default: `DateTime::now()`), an app-controlled caption and the
  usual modal/parallel switch. The mode chooses the control format
  (`yyyy-MM-dd`, `HH:mm:ss`, `yyyy-MM-dd HH:mm:ss`) and the label. OK reads
  `DTM_GETSYSTEMTIME` (`GDT_VALID`) into `DialogResult::DateTime(Some(..))`;
  Cancel, the close box and a failed creation all report `None`.
- The dialog comes from an in-memory `DLGTEMPLATE` built on the dialog worker
  thread: header, zero menu/class words, the UTF-16 title, then item templates
  that end with a **zero creation-data WORD** before the DWORD padding. That
  word is mandatory: without it a two-item template still parses (the padding
  happens to be read as the size), but the third item starts two bytes early
  and `DialogBoxIndirectParamW` fails with no last error. The template parser
  test walks every item so this cannot regress.
- `InitCommonControlsEx(ICC_DATE_CLASSES)` runs before creation; the DTP is
  created by the dialog manager, so it gets the dialog font and tab order. The
  proc stores its state through `DWLP_USER`, which is pointer-size dependent
  (16 on 64-bit, not 8) — writing 8 clobbers the dialog procedure pointer.
- The picker uses the shared `show_dialog` path, so it runs on its own worker
  thread, disables the owner when modal, and returns through
  `Host::dialog_finished` like every other dialog. The self-test opens it
  (parallel) and closes it through `WM_CLOSE`; the showcase menu command opens
  it in date-and-time mode.

## D83. Circles and outlines share the rounded-rect coverage cache

The masks behind `fill_round_rect` described one shape, a uniformly rounded
rectangle. Avatars, radio buttons, the spinner and the progress ring all draw
circles with GDI `Ellipse`, which is not anti-aliased, and the TODO's stroke
primitive had no implementation.

- `CoverageShape` now names what a coverage map describes: `Corners([tl, tr,
  br, bl])` (per-corner radii in pixels; a sharp corner is 0), `Ring { radius,
  thickness }` (a rounded outline: outer shape minus the inset) and `Circle`
  (the inscribed circle). Coverage is cached per pixel size and shape; the color
  mask is still one premultiplied BGRA bitmap per size/shape/color under the
  existing 8 MB + 128-entry budget. A repeated dot therefore costs one
  `AlphaBlend`, not a rasterization.
- Coverage generation keeps the convex fast path (a pixel whose four corners
  agree is fully covered) and only supersamples edge pixels 4×4. Corner radii
  are clamped to half the shorter side, which also prevents overlapping corner
  regions.
- `Canvas` gained `fill_round_rect_corners`, `stroke_round_rect` and
  `fill_circle` with portable fallbacks (largest radius / four straight bands /
  `ellipse`). `Painter` exposes `round_rect_corners`, `round_rect_stroke` and
  `circle`. GDI overrides all three.
- Adoption is deliberately small: `Avatar` (initials fill and ring),
  `RadioButton` (ring, inner fill and dot), `Spinner` dots and
  `ProgressRing::ring_dots` now use `fill_circle`. Shape, color and text stay
  unchanged; image avatars remain square until an image mask exists.
- **Performance (measured, 6×6 dots, 60 frames):** anti-aliased circles cost
  3.5 µs per dot versus 1.6 µs for `Ellipse`; a progress ring's 45 dots are
  0.16 ms per frame and a spinner's 12 dots 0.04 ms per frame. The spinner's 12
  tint levels cache 12 masks; the coverage itself is shared. No per-frame
  rasterization happens, so a batched sprite compositor is not needed at these
  counts. If a future widget paints hundreds of *different* circle sizes per
  frame, reuse one mask or composite the dots into one sprite first.

## D84. Table cells edit in place and publish as a UIA grid

The table was rows-only: the UIA tree published one `ListItem` per visible row
with the cells joined into one name, and there was no way to edit a value.

- **One cell cursor, not a second tree.** `Table` keeps `(selected,
  selected_col)`; `selected_row()` still works and `selected_cell()` returns the
  pair. Left/Right move the column (previously unbound) while Up/Down/Page/Home/
  End keep moving rows. A header reorder remaps the cursor column with the same
  `remap_index` used for the sort indicator. The active cell paints a one-pixel
  accent frame only while the table has focus, so the row highlight still works
  for pointer selection.
- **Editing rides the container's painted session** (`Widget::inline_edit` +
  char/key/clipboard/IME routing), like `PropertyGrid`. The draft is a
  self-contained `CellDraft` in the widget (text, caret/anchor, composition,
  drag) because a table cell has no number/choice kinds to share with the
  property grid's draft. A committed draft is applied to the row first and then
  reported through `on_cell_edit(row, column, text)`; an unchanged draft reports
  nothing. Enter/F2 and a double click open a session, Enter/Tab commit, Escape
  reverts, and a click on another cell commits and moves the cursor. The widget
  never re-sorts: the application owns ordering, exactly like `on_header_click`.
- **UIA**: the table is a `Grid` (DataGrid) whose children are `GridCell`s
  (DataItem) on a `row * columns + column` widget-level index. Each cell carries
  row/column metadata and SelectionItem; the container publishes the full
  extent and implements IGridProvider (`GetItem`, RowCount, ColumnCount).
  Virtualized rows mean only realized cells exist — `GetItem` returns
  `E_INVALIDARG` for a row the viewport has not published, which is the normal
  contract for a virtualized grid. The Selection pattern reports the single
  selected cursor cell.
- The row-joined name disappears; a screen reader now reads one cell at a time
  with its coordinates. Narration formatters (`Narration::rows`) still apply per
  published child, so an application can name cells itself.

## D85. The tree renames in place and moves nodes by drag

The tree had expand/collapse and selection but no editing, and the TODO's
"drag reorder, inline rename" needed a drop model that respects the hierarchy.

- **Rename** is the container's painted session again (`Widget::inline_edit`
  + key/char/clipboard/IME routing), started by F2 on the cursor row or
  `begin_rename(path)`. The draft is the shared `TextDraft` (now extracted to
  `widgets/inline.rs` and also used by `Table` and `PropertyGrid`), so caret,
  selection, composition, deletion and the edit menu behave identically in all
  three. The widget applies the label first and reports it through
  `on_rename(path, label)`; an empty label is ignored (the old text stays) and
  Escape reverts. `WidgetProps::read_only`, the widget lock and disabled rows
  refuse to open a session.
- **Drag** is a tree move, not just a sibling reorder: the top/bottom quarter
  of a row inserts before/after it at that row's depth, and the middle drops
  *inside* it (the target row is outlined). Travel starts the drag after 6 DIPs
  (below that the release is still a click/chevron toggle). A slot inside the
  dragged node's own subtree is rejected — the drag keeps its last valid hint
  so the pointer never shows an impossible drop. Holding the pointer outside the
  viewport scrolls one row per move; staying inside never scrolls, so a row
  near the edge stays droppable.
- The widget applies the move itself (remove, insert, expand the new parent,
  rebuild the flat rows and select the node's new path) and reports
  `on_move(from, to)` with the **new path after the move**, because the paths
  are the application's persistence key. `move_item` is the same operation
  through typed access. Replacing the items (`set_items`) drops an open
  rename/drag rather than re-targeting it at a different node.
- Accessibility is unchanged: the tree stays a UIA `Tree` of `TreeItem` rows.
  Rename is a keyboard/typed operation today; exposing an in-place editor through
  UIA would need a ValuePattern on the row, which the TODO does not ask for.

## D86. The open popup can be rebuilt from its live model

D61 deliberately froze a popup to the snapshot it was built from and reported
`Opened` just before `TrackPopupMenu`. The TODO asked for the opposite: an
`Opened` that fires while the popup is up and a way to rebuild the items into
it. A native popup cannot have its `HMENU` edited while its loop runs, so the
only honest implementation is close-and-reopen; the mechanism now exists.

- **The model lives in the window state while the popup is up**
  (`WindowState::open_menu`). The placement, owner and rebuild count travel
  with it, so both the widget/programmatic path and the tray path share one
  loop (`run_open_menu_loop`).
- **`Opened` is posted, not called inline.** Each pass posts
  `WM_APP_MENU_OPENED` immediately before `TrackPopupMenu`; the popup's own
  message loop dispatches it to the owner window, where `fire_menu_opened`
  hands the live model to `MenuEvent::Opened { menu }`. If the popup closes
  before that message is dispatched, the event is raised once after the call
  so the lifecycle stays complete.
- **A changed model means a rebuild.** The host clones the model around the
  `Opened` callback and compares; a difference (or
  `WindowHandle::edit_open_menu`, which applies a closure to the live model
  from a timer and compares the same way) sets `dirty` and posts
  `WM_CANCELMODE`. The popup closes with no selection, the loop rebuilds the
  `HMENU` from the edited model and shows it at the same placement, and the
  hook sees another `Opened`. Edits are therefore applied item-for-item with a
  visible close/reopen, not mutated under the cursor.
- **Bounded and no-op-safe.** An unchanged model never reopens, and at most 8
  rebuilds happen per open so a hook that edits on every `Opened` cannot spin.
  A selection made after a rebuild still follows the normal
  `Selected`/`Closed`/`Host::menu_command` path.
- `edit_open_menu` is a no-op inside the `Opened` callback itself (the model is
  borrowed for the callback); edit the `menu` argument there instead.

## D87. A modal dims the cached page instead of repainting it

Opening the in-window modal shell (a `Scrim` plus a panel) invalidated the whole
window, so the next frame repainted every page widget *under* the scrim and then
blended the scrim over it — the ~60 ms open in the handoff. The page was already
in the persistent back buffer and had not changed, so repainting it was pure
waste.

- **The split is structural.** `Widget::is_backdrop` (only `Scrim` returns
  true) names the first widget of the overlay layer; everything before it is
  the page and everything from it on is the overlay. This matches the modal
  shell contract already documented for `Scrim` ("add it after the page's
  widgets and before the panel"). `Ui::overlay_active`/`overlay_split` expose
  the same split.
- **The host opts into the composite** with `Host::overlay_paint()`. When it
  returns true the window keeps the page pixels in the back buffer, forces the
  dirty box to the full window (the backdrop must dim everything once) and
  sets `PaintCtx::overlay` / `overlay_first`. The application's `Host::paint`
  must skip its own page background/sidebar fills when `ctx.overlay`;
  `Ui::paint` then paints only the backdrop and the widgets after it, and skips
  the scroll-view bars (page chrome).
- **One blend, then incremental.** The first overlay frame blends the scrim
  across the cached page; later frames skip the scrim (those pixels are already
  dimmed in the buffer) and paint only the overlay widgets. Layout and the
  a11y tree are unchanged; a full-window blit replaces the dirty-bbox blit.
- **Close repaints the page**, as the TODO allowed: hiding the scrim invalidates
  the full window and the normal path restores it. No second full-size buffer
  is allocated (the TODO's two-layer option would cost ~17 MB at 1479×930 @175 %
  DPI); the trade-off is that the page must not change while the overlay is up.
  `BAMBOO_UI_NO_OVERLAY=1` in the showcase disables the composite for exactly
  that case and for A/B measurement.
- **Measured (debug build, 175 % DPI, `--a11y-dump` opening the shell):** the
  first overlay frame is 8.2 ms against 16.8 ms for the same frame on the old
  path in the same run (the full page paint plus the scrim), and closing costs
  one normal page repaint (~7 ms). That is inside the 5–12 ms target.

## D88. Image avatars use a real circle mask

Initials avatars were round (AA `fill_circle`) but an image avatar fell back to
a square crop because `Canvas` had no way to mask a bitmap; its ring was drawn
as four straight edges to match. The rounded-corner work left "an image mask for
round image avatars" open.

- **`Canvas::image_circle`** draws an image clipped to the circle inscribed in
  the target rectangle. The default is `image` (a square) — an honest fallback
  for a backend without a mask. The GDI backend resamples the square crop into a
  premultiplied BGRA buffer whose alpha is `image_alpha * circle_coverage`, then
  `AlphaBlend`s it, so the corners are genuinely transparent rather than painted
  over with an unknown background. The coverage is the same cached `Circle`
  path used by `fill_circle`.
- The resize is a nearest-neighbour sample into the destination pixel grid
  (the target is a 40–80 DIP avatar, so the cost is a few thousand pixels per
  paint) and no bitmap is cached: unlike a solid mask, the result depends on the
  image and size, and caching one per avatar would grow with the page.
- `WidgetPaint::image_circle` is the widget seam. `Avatar` uses it for the
  image path and draws its `.ring` with `stroke_round_rect` (a circular
  outline), so the image and initials forms now look identical apart from the
  fill.

## D89. A gamepad is the keyboard, not a second input model

`Ui` already routes every key through one navigation model (Tab/arrows/Enter/
modify mode). Adding a pad as a parallel event source (a second focus walker,
its own action enum, its own a11y story) would duplicate that model and every
special case in it.

- **`gamepad::Gamepad` translates pad samples into `KeyEvent`s** (XInput
  `wButtons`/sticks): D-pad and left stick → arrows, A/Start → Enter, B/Back →
  Escape, X → Space, Y → Shift+F10, RB → Tab, LB → Shift+Tab. The application
  forwards them to `Ui::key`, so focus movement, modify mode, menus, editors
  and the a11y announcements are exactly the keyboard's. The stick therefore
  walks the same focus order as the arrows; there is no spatial focus guesser.
- **Held directions repeat** (400 ms, then 90 ms) because a stick is continuous
  where a key is discrete; face buttons stay edge-triggered. Releasing and
  re-pressing always fires immediately.
- **Focus-gated and switchable.** `poll(now, active)` takes the window's
  foreground state (`WindowHandle::is_foreground`) and a disabled `Gamepad`
  (`set_enabled`) resets its held state, so a background window or a turned-off
  pad never navigates. This mirrors a console dashboard: only the front window
  owns the pad.
- The mapping is pure (`update(state, now)`) and unit-tested without hardware;
  `read_state(slot)` is the only platform call (XInput slot 0–3). The showcase
  starts polling only when a pad is connected at launch and mirrors the switch
  as a checked 手柄导航 context-menu item.

### D84 addendum: grid cells expose Value / SetValue

`A11yChild` gained `value` and `read_only` (default read-only), so a child that
advertises the Value pattern can report and accept text. `Widget::a11y_set_value_at`
carries the published child index (the container used to discard it), and the
table implements it for cells: `SetValue` applies the text exactly like a
committed draft (clamp-free, `set_cell`, cursor moved) and reports the change
through `on_cell_edit`. Editable cells report `IsReadOnly = false`; locked
grids, disabled rows and column headers refuse. The UIA round drives this end to
end (read 晨雾, SetValue 自检曲, re-read the tree).

## D90. Composite inline editors are the painted field, not a second draft

`Table`, `TreeView` and `PropertyGrid` used to run a small private
`inline::TextDraft`: a character buffer with caret/anchor indices, its own
key handling and a per-widget glyph-width cache for pointer placement. The
standalone `TextEdit::painted()` had meanwhile grown layout caching, word
navigation, undo/redo, clipboard, the built-in edit menu, inline IME,
caret reveal/scroll and a UIA text model — everything a cell editor needs and
none of which the draft had.

- **One implementation: the composite embeds a real `TextEdit`.** Each widget
  owns one `PaintedField<M>` (`widgets/inline.rs`) for its lifetime, wrapping a
  painted `TextEdit` created with `TextEdit::inline_field` (no commit handler,
  no scrollbar chrome). The composite opens it over the cell/row/value
  rectangle, forwards keys, pointer motion, wheel, IME composition and
  clipboard text through crate-internal shims (`TextEdit::inline_*`), and
  applies the committed value through its own callback (`on_cell_edit`,
  `on_rename`, `on_change`). Painting asks the field to paint itself, so the
  editor looks identical to a standalone painted field, including the frame,
  selection, composition strip and blinking caret. The UIA caret rect the host
  receives is now the field's caret (previously the whole cell).
- **Multiline is data-driven.** A value containing a newline opens with
  `TextEdit`'s multiline model (soft wrap, per-line layout, Up/Down across
  lines, internal scrolling and caret reveal); a value without one stays
  single-line. The commit contract: multiline → Enter inserts a newline,
  Ctrl+Enter/Tab/focus loss commits, Escape cancels; single-line → Enter
  commits. The row/cell height stays application-owned: a multiline value
  scrolls inside the fixed row rectangle rather than growing it.
- **The container stayed the routing layer.** Tab/Escape/Enter/Ctrl+Enter are
  still decided in `Ui::key_for_editor` from `Widget::multiline_edit`, so all
  three widgets and the standalone field share one key policy. Blur already
  committed through `focus_move`.
- **Live UIA text.** While a session is open the editing cell/row publishes
  the field's current text (not the last committed value), so a screen reader
  follows the typing; neighbours and a closed widget publish committed data.
  A UIA `SetValue` on the same cell closes the session first, so the applied
  value and the snapshot agree.
- **Widget motion while editing is suppressed.** The wheel over an open
  session goes to the field (scrolling a multiline value inside its row), and
  a press/drag/release inside the field only moves the caret and selection, so
  rows never slide out from under the editor.

## D91. Search and number fields are painted sessions too

After the composite widgets moved onto the painted field (D90), `SearchBox`
and `NumberInput` were the last text inputs still asking the host for a native
`EDIT` child. Both draw decorations the native control hid (a magnifier and a
clear button; a prefix, a suffix and stepper zones), so the question was where
the painted text should sit.

- **`SearchBox` embeds a painted field over its text area** with the field's
  own frame off (`chrome = false`): the box keeps its frame, magnifier and
  clear button, and the field paints the live text, selection, IME strip and
  caret between them. Typing from the keyboard starts the session; Enter/Tab
  and focus loss commit through `on_commit`; Escape reverts. The clear target
  is outside the session and clears + reports `""`. The a11y value is live
  while typing.
- **`NumberInput` embeds a painted field over the text area only**, also with
  its own chrome off. The stepper arrows, the prefix and the suffix keep
  painting while typing, and the painted text area is measured by the paint
  (cached in a `Cell<Rect>`) so the closed display and the editor share exactly
  the same width — the arrows never cover the selection and the layout never
  jumps when a session opens. A click on the arrows *during* a session
  commits the typed text first and then steps, so the final value is what the
  application sees. The typed text is filtered to digits, one dot and (only
  for a signed range) a minus; committing parses it back to the raw integer,
  clamps and reports `on_change`; Escape keeps the value.
- **The seam additions are session-scoped**: `PaintedField::begin_styled`
  carries the alignment, the own-chrome flag and `max_length`, so a widget
  that decorates its own field does not inherit the field frame. No public API
  changes: both widgets keep their builders, typed access (`text`,
  `is_editing`, `display`, `value`) and commit messages.

## D92. A scroll view can sit inside another scroll view

A nested list inside a scrolling page needs both offsets — the page's and the
list's — without the application re-laying out children on every page scroll.

- **A panel names its parent** (`ScrollView::nested_in(panel)`). Its configured
  viewport is then a rectangle in the parent's *content* coordinates; the
  container resolves the client viewport as `parent.content_rect - parent.offset`
  and the clip as that viewport intersected with every ancestor's viewport, so
  a nested panel moves, scrolls and clips with the page (D92). Root panels keep
  the old contract: configured viewport == client viewport.
- **Layout cascades.** Laying a panel out positions its children, then recurses
  into the panels whose parent it is, so one `set_scroll_offset` on the page
  moves the inner viewport and its content with it. The panel's *bars* paint
  clipped to the resolved clip (the intersection with its ancestors), never to
  the raw viewport: a nested panel that extends past its parent shows only the
  bar part that is actually visible. A chain guard (depth 32)
  makes a malformed parent cycle fall back to a root panel instead of looping.
- **Visibility is subtree content.** A panel is shown while its own children or
  a nested panel have visible widgets. Hiding the page hides the widgets, so
  the nested panel disappears with it; no separate ancestor check is needed
  (and the naive parent-chain check recursed forever between two panels that
  each only contained the other).
- **Wheel bubbles.** Under the pointer, the deepest panel scrolls first; when
  it is already at its end the notch moves the next ancestor (a nested list
  hands over to the page it sits in). The chain is depth-ordered, then by
  registration order.
- **Focus reveals through the chain**: the innermost panel scrolls first, then
  each ancestor moves just enough to show the widget, so Tab reaches a nested
  child inside a scrolled page.
- Removing a panel reparents its nested panels to roots (their configured
  viewport becomes client space again). The showcase's data page has a small
  nested list inside the scroll demo; the self-test wheels it and checks the
  hand-over.

## D93. A window can own several tray icons

Players want more than one notification-area entry (the player itself and a
download/transfer indicator), and one shared icon with a mutating menu cannot
express two independent tooltips or two menus.

- **Icons are keyed by an application id.** `WindowHandle::set_tray_icon`
  manages the primary icon (id 0); `set_tray_icon_at(id, Some(..)/None)`
  installs, updates or removes any other id in place. Each entry owns its
  tooltip, image and `Menu`, and the same id is the shell's
  `NOTIFYICONDATAW.uID`.
- **Events carry the id.** `TrayEvent::Action { icon, action }` (per-button
  actions, D96) and `TrayEvent::Command { icon, command }` let the application
  route per icon.
  Version-4 notifications put `(notification << 16) | id` in `wParam`, so the
  low word is the icon id; the legacy pre-`NIM_SETVERSION` form also carries
  the id there, and the high-word check (as before) tells the two apart.
- **Menus and balloons are per icon.** A right click opens the menu of the
  icon that was clicked; `show_tray_balloon` targets the primary icon and
  `show_tray_balloon_on(id, ..)` any other.
- **Lifecycle is per window, over all icons.** Explorer's `TaskbarCreated`
  re-adds every icon, a DPI change reloads every icon file at the new
  small-icon size, and window destruction removes all of them. The self-test
  installs and removes a second icon to prove the id path.

## D94. Application-labelled message-dialog buttons use a task dialog

`MessageBox` can only show the system button sets, so a dialog whose buttons
are the application's ("保存"/"放弃") was impossible without a custom window.

- **`MessageDialog::button(label, command)` switches the dialog to
  `TaskDialogIndirect`** (comctl32 v6). The labels carry `&` access keys, the
  selected id comes back as `MessageAnswer::Custom(command)`, and
  `.instruction(text)` / `.icon_file(path)` map to the task-dialog main
  instruction and a custom main icon (loaded with `LoadImageW`, destroyed
  after the dialog). A Cancel common button plus `TDF_ALLOW_DIALOG_CANCELLATION`
  always give the user a way out; Esc and the close box report `Cancel`.
- **The standard path stays `MessageBox`.** A dialog without custom buttons or
  an icon file is unchanged, so existing look, default buttons and the
  self-test's modal/parallel checks keep working.
- **Missing comctl32 v6 is not fatal.** Task dialogs need the version-6
  activation context (an application manifest); if `TaskDialogIndirect` fails
  the call shows a standard OK box instead of returning nothing, and the
  limitation is documented (the custom labels cannot be represented there).
- The custom icon is a file path rather than an `HICON` because the dialog
  runs on a worker thread, where an application-owned icon handle would need
  cross-thread lifetime rules; loading and freeing it inside the dialog thread
  keeps that simple.

## D95. Tree drags are policy-gated, and a drop never loses a node

Two problems surfaced together: an application could not refuse a drag (or a
drop kind), and a detach-then-reattach sequence left a blank "ghost" row and a
childless parent with no expander.

- **Master switch + per-kind rules + per-gesture veto.** `draggable(bool)` turns
  press-drag moves off entirely; `TreeDragRules { reorder, nest, detach,
  reparent }` decides *where* a drag may land (`TreeDropKind`: same-parent
  before/after, drop-into-a-row, root-level before/after from a nested node,
  before/after under a different non-root parent); `on_drag_start(|from| bool)`
  is asked once when the pointer crosses the drag threshold and can veto that
  gesture (the press stays a click). All are runtime-settable
  (`set_draggable`, `set_drag_rules`); changing the rules cancels a drag in
  flight.
- **Three hooks bracket a gesture.** `on_drag_start(from)` runs once at the
  threshold and can refuse the whole drag; `on_drag_over(from, to)` runs
  whenever the *prospective path* `to` changes under the pointer (not per mouse
  move — the last offer is cached) and can invalidate that target; and
  `on_drag_end(from, to)` runs once on a valid release *before* the tree
  changes and can cancel the drop. `to` is the path the node would have, so the
  same value identifies an accepted over-target and its release.
- **A rejected slot is not a move.** The drag state always carried a target;
  when no legal slot was under the pointer it kept an initial placeholder at
  (root, index 0). Releasing there applied that placeholder, so a drag over a
  disallowed target could move the node to the root. `TreeDrag` now records
  `valid`, set only when `can_drop` passes; releasing an invalid drag changes
  nothing.
- **The target path is remapped after the removal.** Removing the node shifts
  the sibling indices of its parent; a target path that runs through that
  sibling list (dropping onto a *later* sibling — the common "detach to just
  before the old parent, then put it back" sequence near the end of a list)
  addressed a different node or none at all. The old code silently lost the
  node and returned before rebuilding the row cache, leaving stale ghost rows
  and a missing expander. `apply_move` now decrements the shifted component
  first, and restores the node if a target is somehow still unresolvable.

## D96. Window-level lifecycle is a set of Host hooks

The Host only had `started`, `close` and `destroy`; an application could not
learn that the window was shown/hidden, activated, minimized/maximized, moved,
received files, or that the first frame was on screen, and a close request did
not say who asked.

- **One hook per question, same callback rules.** All new callbacks run on the
  UI thread through the existing "take the host out of the state" pattern
  (`call_host`), so they may touch the UI, open dialogs and call
  `WindowHandle` setters:
  - `close_requested(reason, window) -> bool` replaces `close()` and reports
    `CloseReason::User` (title-bar close / Alt+F4 / `SC_CLOSE`, and a bare
    `WM_CLOSE` from Task Manager), `System` (`WM_QUERYENDSESSION`, whose answer
    is still the veto) or `Programmatic` (`WindowHandle::request_close`).
    Returning `false` keeps the window open and no destroy callback follows.
  - `loaded(window)` once after the first paint settles on screen.
  - `visibility_changed(visible, ..)` on `WM_SHOWWINDOW` after `started`,
    `activated(active, ..)` on `WM_ACTIVATE`, and
    `visual_state_changed(WindowVisualState, ..)` only when `IsIconic`/
    `IsZoomed` really changed (not on every resize).
  - `moved(position, ..)` with the frame's top-left in screen DIPs,
    `files_dropped(paths, ..)` for `WM_DROPFILES` (`DragAcceptFiles` is on from
    creation), and `destroying(window)` in `WM_DESTROY`, before the handle
    dies. The old `destroy()` keeps its after-the-handle meaning.
- **State is exposed, not inferred.** `WindowHandle` gained `is_visible`,
  `is_active`, `visual_state`, `frame_position`, and the actions `hide`,
  `minimize`, `maximize`, `restore`, `request_close`; the callbacks fire when
  the shell reports the change, so tracking code and `is_*` agree.
- **Tray actions are explicit.** `TrayEvent::Action { icon, action }` with
  `TrayAction::{Click, DoubleClick, RightClick, MiddleClick, KeySelect}`
  replaces the single `Activate`: a right click notifies the application
  *before* the icon's menu is shown, so a hook can refresh the model in place.
- The self-test drives the whole path: it waits for `loaded`, hides/shows the
  window and requires both visibility callbacks, requests a close that the app
  vetoes (window stays), then minimizes and restores while requiring the
  state callbacks.

## D97. The on-screen keyboard belongs to the pad session, not the text field

A controller cannot type, so activating a text field with a pad has to raise the
Windows keyboard — but a desktop window is not a touch app and has no
text-input host of its own. Three decisions shape the feature:

- **Only a pad-raised keyboard exists.** Opening the system keyboard whenever a
  text field gains focus would flash it at every mouse user. `Ui::pad_key` wraps
  `Ui::key`, marks the session it opened, and reports it through the
  `UiHost::set_soft_keyboard` seam; `UiHost` keeps a no-op default, so tests and
  plain invalidators are unaffected. Every way a session can end (commit,
  cancel, Tab, focus loss, removal, rebuild) runs the same dismissal, and
  `WindowHandle` additionally hides on native-editor close, window deactivation
  and destruction — a keyboard can never outlive the field it was opened for.
- **The shell is asked the way Windows asks itself.** `soft_keyboard::show`
  tries `CoreInputView::TryShowWithKind(Gamepad)` (the controller layout on
  Windows 11 24H2+ handhelds), then the touch layout, then `ITipInvocation` on
  the undocumented `CLSID_UIHostNoLaunch`, then a `TabTip.exe`/`osk.exe` launch
  for the first-ever use. `Toggle` flips state unconditionally, so visibility is
  measured instead: `IFrameworkInputPane::Location` returns the keyboard's
  screen rectangle (empty while hidden) in an ordinary desktop process, and the
  input view's `GetCoreInputViewOcclusions` covers a hosted window. A short
  grace window after an accepted request bridges the shell raising its window.
  Window-class probing was tried and rejected: Windows 11's input service keeps
  a *visible* full-screen `Windows.UI.Core.CoreWindow` whether or not the
  keyboard is up, and the title is localized ("Windows 输入体验"), so a process
  probe reads "open" forever and traps the pad in swallow mode. (The first
  build did exactly that, which is why `is_open` is API-measured now.)
- **While the keyboard is up, the pad belongs to it.** Windows' gamepad layout
  is selected with the controller and inserts text into the focused field; our
  raw XInput polling would also see those presses and **A** would commit the
  field under the user's selection. `pad_key` therefore swallows the pad in a
  pad-raised session and adds the two escapes the user needs: **A** re-raises a
  keyboard dismissed with its own close button (instead of committing), **B**
  closes it and leaves the field with a commit, matching a click-out. A platform
  that refuses the keyboard (or a dismissed one where the user presses another
  key, including a tab away) drops back to ordinary key routing, so a machine
  without a working touch keyboard can never trap the pad in swallow mode.

The showcase polls the pad from its animation timer, so that timer now stays
alive while the window is live (33 ms with a pad, 40 ms while a toast/marquee/
gallery animation runs, a 500 ms watch tick when idle — it never paints). The
first build still stopped it on toast expiry and page switches, which killed
controller input mid-session until an unrelated click started another toast;
`sync_timer` is the one place that decides the interval now.

Tested without hardware: the fake `UiHost` records `set_soft_keyboard` calls, so
the open/re-raise/swallow/dismiss transitions are pinned by unit tests; the
Windows probes only run in the real host.

## D98. An accordion section expands through UIA ExpandCollapse, not selection

The accordion first published an open section as the *selected* `ListItem`
(D49), because no widget-level ExpandCollapse pattern existed. That reads wrong
to a screen reader: a list row is not "selected" when a section is open, and the
SelectionItem pattern was the only way to toggle it.

- **Every section carries an open state.** `A11yChild::expanded` was already
  the seam the tree uses per node; the accordion now sets it for each header, so
  the provider advertises the ExpandCollapse pattern for the section and the
  state property reports Expanded/Collapsed. The new rule in `GetPatternProvider`
  is role-independent: *any* child that publishes an open state can expand and
  collapse.
- **The request runs the click path.** `Widget::a11y_expand(child, expanded)`
  maps the published section index through `set_expanded` (single-open sections
  close the others silently) and emits the same `on_toggle` message a header
  click does, so an assistive-technology expand and a mouse click cannot
  diverge.
- **Selection stays.** The open section is still the selected `ListItem`, so
  list-shaped navigation keeps working; the two properties describe the same
  state from different angles. A request for the state a section already has
  changes nothing and emits nothing.

## D99. Lifecycle notifications raised inside a callback are queued, not dropped

`hide()`, `minimize()`, `maximize()`, `restore()` and window moves are executed
by the shell *synchronously*: `ShowWindow` sends `WM_SHOWWINDOW`/`WM_SIZE`
before it returns. When the application calls one of them from inside a `Host`
callback (the natural place — a tray click, a timer, `loaded`), the framework
still holds the host: `Host` is taken out of the window state for the duration
of the callback, so the nested dispatch found no host and returned silently.
The observable effect was that the *first* hide/minimize never produced
`visibility_changed`/`visual_state_changed` (the state flag had already been
updated, so the change was lost forever). The self-test caught it as a
`window_api` failure: `hidden=false shown=false min=false normal=false` after a
hide/minimize/restore cycle.

- **Dispatch defers when the host is busy.** `dispatch_visibility`,
  `dispatch_visual_state` and `dispatch_moved` push a small `LifecycleEvent`
  when `self.host` is `None` instead of dropping it.
- **The message loop flushes after every dispatched message.** By then the
  callback that caused the event has returned and the host is installed again;
  `flush_lifecycle` replays the queue through the ordinary dispatch path. Order
  is preserved, and a callback that hides the window again just queues another
  event for the next flush.
- **Only these three are queued.** `close_requested` keeps its
  "proceed unless vetoed" fallback, and file drops arrive from the shell while
  no callback runs.

## D100. Masked images go through the coverage cache, not a region clip

Rounded cover art could be painted by clipping the DC to a rounded region, but
`CreateRoundRectRgn` is a hard region edge: the corners would be visibly jagged
next to the anti-aliased cards around them. It could also be painted by
round-tripping through GDI+ or D2D, which the GDI backend deliberately avoids.

- **The alpha channel is the mask.** `Canvas::image_round` resamples the image
  into premultiplied BGRA and multiplies its alpha by the same anti-aliased
  coverage map `fill_round_rect` uses (cached by pixel size and corner radii),
  then `AlphaBlend`s it. The corners really are transparent, so whatever the
  widget painted behind the picture — a card, a gradient — shows through.
- **One scratch surface, no per-draw GDI objects.** The masked pixels are
  assembled in a reused buffer and blended from a reused DIB/DC
  (`GdiResources::image_surface`), recreated only when the pixel size changes.
  The old avatar path created and destroyed a DC and bitmap per draw; avatars
  and covers now share the same route, and the defaults of both trait methods
  paint the plain square image for a backend without an alpha mask.
- **The placeholder follows.** A cover without artwork paints a gradient, and a
  gradient cannot be clipped by an alpha mask after the fact, so
  `Canvas::vertical_gradient_round` lerps the gradient per row, premultiplies
  it with the coverage and blends it the same way. Without it a rounded tile
  with art would sit next to a square one without.
- **Opt-in, per instance.** `CoverWall::corner_radius(dips)` is the wall
  default and `Cover::corner_radius(dips)` overrides one tile (`0` is square);
  the default stays 0, because the generic corner-radius adoption is still a
  separate user decision.

## D101. The renderer seam keeps the GDI back buffer; Direct2D is a second Canvas

The queued paint-surface work could have replaced the whole frame path with a
DXGI swap chain. It does not: the window keeps its persistent back buffer, its
paint exactly the dirty bounding box, and its dirty-rect blit in every mode.
Only the half of the frame that draws into that buffer changes.

- **One `Canvas`, three renderers.** `Renderer::{Gdi, D2dSoftware, D2dHardware}`
  is selected by `WindowBuilder::renderer` or `BAMBOO_UI_RENDERER`. GDI stays
  the default and the fallback. The paint path boxes the selected canvas and
  hands it to `PaintCtx` as `&mut dyn Canvas`; widgets, `Ui`, the painter, the
  overlay compositor, the owner-drawn menu bar and the native editors never
  learn which one is active. A Direct2D mode that cannot start switches the
  window to GDI for good and says so under `BAMBOO_UI_DEBUG`.
- **Direct2D renders into the same DC.** An `ID2D1DCRenderTarget` is bound to
  the back buffer each frame with `D2D1_RENDER_TARGET_TYPE_HARDWARE` or
  `…_SOFTWARE` (Direct2D falls back to its software rasterizer on its own). Its
  DPI is `96 × scale`, so every coordinate the widgets pass is already a DIP.
  Nothing about the two-layer overlay snapshot changes: the page stays in the
  buffer and the next overlay frame draws over it.
- **The coverage cache is GDI's, not D2D's.** D2D anti-aliases geometry itself,
  so it caches shapes instead: rounded-rectangle and ellipse geometries,
  per-corner paths and ring paths keyed by the rectangle's position, radius and
  thickness, plus a DirectWrite text format per style/alignment/trimming.
  Solid brushes are cached per color (with alpha) and rebuilt with the target.
  Masked images (avatars, rounded cover art) push a layer with the cached
  geometry as its mask, so they are anti-aliased like the GDI path; images are
  premultiplied once into `ID2D1Bitmap`s with the same 16 MB budget.
- **Text is DirectWrite grayscale AA.** The buffer is premultiplied, so ClearType
  cannot apply; the format is created with trimming only in the ellipsis mode,
  no wrapping and centered paragraph alignment (the same vertical centering
  GDI's `DT_VCENTER` gives). A test renders a mixed ASCII/CJK run through both
  backends and keeps the ink amount, the extent and the measured width close.
- **Device loss retires the target.** `EndDraw` is the frame's commit; a failure
  drops the device-dependent target (brushes, bitmaps, layer) so the next frame
  rebuilds it, while the factory, text formats and geometries survive.

Measured on the reference machine with `--bench 2` (debug, 175 % DPI, hard
occlusion varies): GDI ≈ 2.3 ms/frame, Direct2D software ≈ 0.44 ms, Direct2D
hardware ≈ 0.42 ms (private working set 44 MB for the D3D device). The bench
line now reports the active renderer, which is the gate for comparing runs.

## D102. A renaming tree row publishes an editable UIA Value

The tree already published the session's draft as the row's *name*, so a screen
reader followed what was being typed, but a client had no way to read it as a
value or replace it — the documented "rename has no ValuePattern" gap.

- **The renaming row is the only row with a value.** `A11yChild::value` plus
  `editable(true)` is set for the row whose path matches the session; every
  other row stays value-less and read-only. The name still tracks the draft.
- **Role-independent availability.** `GetPatternProvider` now advertises the
  Value pattern for *any* child that publishes a value (the same rule the open
  state and the scroll state already use). Grid cells keep it through their
  role, and the tree row gains it while renaming.
- **`SetValue` is the Enter commit.** `TreeView::a11y_set_value_at` writes the
  requested text into the painted field and runs `finish_rename(true)`, so the
  `on_rename` message, the empty-value rule and the label application are
  identical to the keyboard path. A row that is not renaming (or is disabled)
  refuses.

## D103. A property row is a ListItem with a settable Value

`PropertyGrid` publishes one logical element per row (a `ListItem`, D78), which
kept list-shaped navigation but left a screen reader unable to change a value.
The row is not split into key/value cells; instead the value travels through
the generic Value pattern.

- **Editable kinds publish a value.** `Text`, `Number`, `Choice`, `Check` and
  `Rating` rows set `A11yChild::value` (the displayed text, or the rename-style
  live draft while an inline session is open) and `editable(true)` unless the
  row or the whole grid is locked. A `ReadOnly` kind stays value-less; a locked
  row keeps its value but reports read-only, so a client can still read it.
- **`SetValue` is the commit, not a new editor.** `a11y_set_value_at` applies
  the same normalisation the inline editor uses and reports through the same
  `on_change` message: numbers parse, clamp to `min..=max` and format with the
  row's `decimals`; a choice must name one of its options; a check accepts its
  on/off texts or `true`/`false`; a rating parses to a number clamped to its
  star count. A draft the kind would refuse changes nothing and emits nothing.
- **The List structure stays.** SelectionItem still moves the cursor, so the
  list/selection pattern and the row count a client sees do not change; only
  the value affordance was added. Per-cell navigation and focus for the single
  editing row remain separate (the inline field owns them while it is open).

## D104. Read-only locks mutations, not the selection session

`WidgetProps::read_only` used to keep a painted field from opening its session
at all, so a locked field had no caret, could not be selected with the pointer
or Shift+arrows, and could not be copied — the one thing a locked field is for.
The container now opens the session for every enabled painted field and the
widget decides what a locked session may do.

- **One flag, two questions.** `TextEdit::read_only` (pushed by
  `Widget::set_read_only`) refuses `insert_impl` through the user paths (typed
  characters, IME, paste, the Enter newline), `delete_back`/`delete_forward`,
  `undo`/`redo`, cut, and every mutating `edit_command` id. Navigation,
  selection, `select_all`, copy, caret reveal and the blink are unaffected. The
  application's own setters (`set_text`, inherent `insert_text`, `clear`) are
  not input and keep working.
- **The container stopped gating.** The pointer press, right press and Enter
  paths open a painted session regardless of the lock; the widget's guards
  refuse the mutations. Typing (`Ui::char`/`ime_*`) and native `EDIT` sessions
  still keep the container-side lock, because those do not pass through the
  widget.
- **The menu and the a11y state follow.** The built-in edit menu disables
  Cut/Paste/Delete/Undo/Redo and the Unicode-control inserts, keeps Copy and
  Select All, and the `TextPattern`/read-only state reports the lock so
  `SetValue` is refused.
- **No on-screen keyboard.** `Ui::text_entry_active` ignores a locked session,
  so a gamepad opening a locked field never raises the shell keyboard.

## D105. Cue text is painted on the first line's rectangle

A multiline field top-aligns its lines, but the cue was drawn across the whole
content rectangle and the canvas centers single-line text vertically, so a tall
empty field showed its hint in the middle while the caret sat on the first
line. Both paint paths (`paint_painted` and the closed `paint_multiline`) now
build the cue rectangle from `line_top(0)`: one line tall, full content width,
aligned left for a multiline field and with the field's own alignment for a
single line. `vertical_center(false)` (documented but previously unused) now
also moves a single line and its cue to the content top.

## D106. Edit-box observers run before the built-in behavior

The painted field had only a commit callback, so an application could not react
to typed characters, live changes, clicks or individual keys. `TextEdit` gained
optional observers: `on_change(&str)`, `on_char(char)`, `on_click` /
`on_right_click` / `on_double_click(Point)` and `on_key_down` /
`on_key_up(KeyEvent)`. Each is an observation hook: it runs before the built-in
action, the action still runs, and returning `None` stays silent. Because they
only add a notification and never replace the default, they take no
`TriggerContext`-style default handle; suppressing or replacing a default stays
the job of the `Ui::bind` trigger seam, which already has `ctx.default()`. A
future callback that would take over a default action should carry that handle
— or a return value that decides whether the built-in action runs, whichever
fits the component — so the application does not reimplement it (tracked in
`TODO.md`).

- **One message per input.** `Widget::input` returns one message, so the hooks
  are ordered and the first `Some` wins. `on_char`/`on_key_down` precede the
  `on_change` check; a change is tracked by a pending flag set only by user
  mutations (insertion, deletion, cut, undo/redo), and an observer that
  answered consumes it so it cannot leak into a later input.
- **Direct insertions are observed too.** `Ui::char`, the Ctrl+V/menu paste
  and `ime_commit` do not pass through `Widget::input`; the container calls
  `Widget::observe_char` before the insertion and
  `Widget::take_change_message` after it (the edit menu's cut/delete/undo drain
  the same way), so the observers fire on the real keyboard route. `on_char`
  is the keyboard path; an IME result reports through `on_change`. Programmatic
  setters never set the pending flag.
- **Key releases are a new normalized event.** `Event::KeyUp` →
  `Ui::key_up` forwards to the focused widget's `WidgetInput::KeyUp`; the
  container gives releases no navigation meaning and does not repaint for them
  (`Ui::key_up` pushes only the widget's message). The window procedure raises
  it from `WM_KEYUP`/`WM_SYSKEYUP` after the existing Alt-tap handling. A
  native `EDIT` session owns its own key releases, so a native-mode field sees
  none.

## D107. A widget may report a second message per input

`Widget::input` returns one `Option<M>`, but the slider drag lifecycle needs
`on_change` *and* `on_drag_end` on the same release, and the upcoming widget
observers add a notification beside a control's own action. Rather than widen
the return type (a churn across every widget), the trait gained an additive
`take_extra_message` slot, drained right after `input`:

- **One primary, one extra.** `Outcome` now carries `extra` between the
  widget's own message and the `ValueChange` trigger. `Ui::emit` queues
  `custom` (trigger), `default` (widget), `extra`, then `after` (value change).
- **Drained every input.** Like `take_repaint_hint`, the container takes the
  slot on every input, so it can never leak into a later event.
- **Who uses it.** `Slider`/`WaveformSeekBar`: the press may report
  `on_drag_start`, every move reports `on_drag`, and the release reports
  `on_drag_end`; the built-in `on_seek`/`on_change` stays primary. Additive
  observers will use the same slot.

The drag lifecycle itself is additive: the press, move and release still run
the built-in preview and value commit. Typed access (`is_dragging`,
`drag_preview`) exposes the live preview without waiting for the release.

## D108. A trigger handler can run the default itself

`TriggerContext::default()` only set a flag checked after the handler
returned, so a handler could not run the widget's own action and then
post-process its message. The context now carries the default as an invocable
closure.

- **Two forms, one suppression rule.** `ctx.default()` keeps the old flag
  form (the default runs after the handler). `ctx.run_default()` runs the
  default immediately and returns its message; the default does not run again
  (a second call in the same handler returns `None`). Calling neither still
  suppresses the built-in action.
- **The handler owns an in-handler default's message.** When it calls
  `run_default()`, the container does not also emit the widget's message: the
  handler decides whether to return it, wrap it, or drop it. Snapshotting of
  the value-change state, repaint intent and old paint bounds happens *before*
  the handler, so a default run inside it still repaints and still feeds a
  `ValueChange` binding.
- **Type parameter with a default.** `TriggerContext<'a, M = ()>` holds the
  callable. The default keeps existing `TriggerContext` spellings valid;
  handler closures infer `M`, so application code does not change.
- **ValueChange has no default.** It is a notification with no widget input
  behind it, so its handler gets a context whose default returns `None`.

## D109. Widget-level observers are additive and container-routed

`TextEdit` had per-instance observers (D106), but every other control exposed
click/focus/key only through the per-id `Ui::bind` seam. The standard controls
gained fluent, additive observers of their own:

- **What.** `Button`, `Toggle`, `Segmented`, `Tabs`, `Slider`, `Checkbox`,
  `RadioButton`, `RadioGroup`, `ListView`, `ComboBox`, `MultiSelectList`,
  `MenuButton`, `SplitButton`, `MenuBar` and `Toolbar` expose `.on_click`,
  `.on_right_click`, `.on_double_click`, `.on_focus_gained`, `.on_focus_lost`,
  `.on_key_down` and `.on_key_up`, each returning `Option<M>`. A widget stores
  one `Observers<M>` field; the builder and forwarding code is a macro
  (`observer_builders!` / `observer_methods!`) so the surface stays identical.
- **Additive, never replacing.** The built-in action always runs and the
  observer message is queued beside it through `Outcome.extra` (D107), after
  the widget's own message. The observer takes no default handle because it
  cannot suppress or replace anything; suppression stays the `Ui::bind` seam,
  which now offers an invocable default (D108). This is the same rule the
  `TextEdit` observers follow (D106).
- **Container-routed.** The widget does not sniff `WidgetInput`: `Ui::interact`
  asks `observe_click`/`observe_right_click`/`observe_double_click` before
  dispatching the input, `Ui::key` asks `observe_key_down` once for the focused
  widget *before* navigation (so it sees Tab, arrows outside adjust mode and
  ordinary characters), `Ui::key_up` falls back to `observe_key_up`, and
  `Ui::focus_move` asks `observe_focus_gained`/`observe_focus_lost` only when
  the move is committed (after the container hooks). `TextEdit` keeps its own
  fields and input path (D106) and leaves the trait methods at their defaults.
- **Ordering.** An observer message from the current input wins the `extra`
  slot over a widget-reported one (they cannot come from the same input). The
  click observer also fires for a modified click (`SelectWith`).

## D110. One surface style for radius, fill override and transparency

Widgets had ad-hoc background fills: a plain `fill_rect`, a themed `frame`, or
a hard-coded radius token. A scheduled user request asks every common component
to take a corner radius and a background override/transparency, so the paint
path now shares one value.

- **`SurfaceStyle` (public).** `corners: Corners`, `background:
  Option<Color>` (`None` = the widget's theme fill) and `transparent: bool`.
  `fill(rect, default)`, `shape(rect, color)` (sub-shapes like a progress fill
  that always show) and `frame(rect, border, background)` (a 1 DIP border plus
  an inner fill, both rounded; `transparent` suppresses only the inner fill so
  a field keeps its focus border) are the whole paint vocabulary. A negative
  radius clamps to square; a mixed corner set fills per-corner and leaves the
  focus ring rectangular (the ring stroke is single-radius).
- **Builders.** `surface_builders!` (generic `M`) and `surface_builders_plain!`
  (non-generic) generate `.corner_radius(dips)`, `.corners(Corners)`,
  `.background(color)`, `.background_option(Option<Color>)`,
  `.transparent_background(bool)` and `surface()`.
- **Adopted by** `Button`, `Label`, `Card`, `Toast`, `Banner`, `TextEdit`
  (native and painted frames), `NumberInput`, `SearchBox`, `Slider` (track and
  fill), and `ProgressBar` (track and fill). A button's focus ring follows a
  uniform radius; a text field's inner editing ring uses `stroke_round_rect`.
- **Bounds.** Hit-testing stays rectangular (the layout rect is the target).
  The GDI/D2D clip stack is rectangular, so an explicit rounded *content* clip
  is not part of this pass; the components that hold images already round them
  through the masked-image path, and text sits inside its padding. This keeps
  the change to the fill vocabulary without a new clip primitive.

## D111. Dynamic ids intern; a preserved session survives remove/insert

Two leftovers of the dynamic widget tree (D62): creating widget ids from data,
and keeping a live editor session across a remove/re-insert of the same id.

- **Interning, not a pool.** `WidgetId::intern(&str)` interns the text once for
  the process (a `OnceLock<Mutex<HashSet<&'static str>>>`, one leaked string per
  *distinct* name) and returns the stable `&'static str`. This was chosen over a
  fixed pool because it preserves `WidgetId`'s `Copy`/`Eq`/`Hash`/`&'static str`
  contract, so every container table, the a11y runtime id hash and the M7
  schema keep working unchanged; a pool would change the type for all of them.
  The trade-off is the bounded leak: draw dynamic names from a bounded set
  (`row-{index}` over a bounded row count, or a fixed id pool) rather than a
  fresh UUID per rebuild. `WidgetId::name()` reads the string back.
- **Session capture is a plain value.** `SessionSnapshot` holds the draft, caret,
  anchor, the pre-session original (for a later Escape) and whether the session
  was open. `Widget::take_session` *closes* the draft without committing or
  reverting (so the container's normal removal has nothing left to finish);
  `Widget::restore_session` writes it back and re-opens the session. Both default
  to `None`/`false`, so only widgets that opt in participate.
- **Container plumbing.** `Ui::remove_preserving_session` captures before the
  normal `remove`, which keeps every other cleanup rule; the snapshot waits in
  `pending_sessions` and is consumed by the next `add`/`insert` whose widget id
  matches. A fresh removal of the id, `clear`/`rebuild`, or
  `discard_pending_sessions` drops it. The re-inserted widget is not focused
  automatically (the container has no host handle there); the application
  restores focus if it wants the caret to keep blinking.
- **What can be handed over.** A framework-painted `TextEdit` (the documented
  case). A native `EDIT` session owns its control and a composite
  (`Table`/`TreeView`/`PropertyGrid`) keeps its embedded field internal, so both
  still commit on removal as before.

## D112. Dialog templates are data, built into a DLGTEMPLATE at runtime

> **Superseded by D115**: the application dialog template is removed. A native
> template cannot follow the page theme; an application that needs a custom
> dialog builds its own window with the normal widgets.

The date/time picker already hand-assembled a Win32 `DLGTEMPLATE`, but there
was no way for an application to ask for its *own* small dialog without a
resource script. `DialogTemplate` adds that as pure data.

- **A Rust-side builder, not a raw template.** `DialogTemplate::new(title)`
  plus `.intro`, `.label`, `.text(id, label, value)`, `.password`,
  `.number(id, label, value, range)`, `.check`, `.choice(id, label, options,
  selected)`, `.separator`, `.ok_label`, `.cancel_label`, `.modal/.parallel`.
  Field ids are the caller's `u16` handles (1/2 are the OK/Cancel commands);
  the same ids read the result back through `TemplateValues`.
- **Built on the worker thread.** `platform::dialog::template_bytes` lays the
  fields out top to bottom in dialog units and appends OK/Cancel; the dialog
  procedure seeds the controls in `WM_INITDIALOG` and reads every field in
  `WM_COMMAND`/IDOK. Text/number values go through `WM_SETTEXT`/`WM_GETTEXT`
  (not the ambient `GetDlgItemText` shims), checks through
  `BM_SETCHECK`/`BM_GETCHECK`, and choices through
  `CB_ADDSTRING`/`CB_SETCURSEL`/`CB_GETCURSEL`. Numbers are parsed and clamped
  to their range; a choice's index is clamped to its option count.
- **Result shape.** `DialogResult::Template(Option<TemplateValues>)`; `None` on
  cancel/close/failure. `TemplateValues` stores per-kind id → value pairs with
  `text`/`number`/`checked`/`choice` lookups (a missing id means that kind was
  not present). The self-test opens one in parallel mode and closes it, which
  exercises the real template and procedure.
- **Scope.** One column of labels + controls and OK/Cancel; no grids, tabs or
  custom control classes. An application that needs more builds the dialog
  itself; the value is a dependency-free, resource-free small form.

## D113. Cross-widget drag and drop is a container session

> Superseded by D132: the payload and the gesture are now the system's (OLE),
> with the same widget-level routing; the message payload `M` is gone.

The tree, playlist and table had their own internal drags, but there was no way
to move a payload between two unrelated widgets. The container now owns a
minimal drag-and-drop session.

- **Three registrations, keyed by widget id.** `set_drag_source(id, || Option<M>)`
  produces the payload when a drag starts on that widget (`None` falls back to
  the ordinary `Trigger::DragStart` path), `set_drop_target(id, |M, Point|
  Option<M>)` consumes a payload released over the widget and returns the
  application message, and `set_drop_hover(id, |&M, Point, bool| bool)` gives a
  target enter/leave feedback (returning `true` asks for a repaint). Each has a
  `clear_*` and replaces a previous registration. The payload is the
  application message type `M` — no `dyn Any` downcasting.
- **The session owns the gesture.** Once the 4 DIP threshold is crossed on a
  source, `Ui` skips the source's `DragStart`/`DragTo`/`ReleaseAt`/`DragEnd`/
  `Click` and instead tracks the widget under the pointer (`hit_index`, so
  clipping and overlays apply), notifies the old/new target's hover callback on
  a change, and on release calls the target's drop handler under the release
  position. The source and target rectangles are repainted; the payload is
  dropped if no target accepts it.
- **Aborts are explicit.** Escape or `Ui::cancel_drag` ends the session without
  a drop (the hover sees `false`); removing the source or the current target
  aborts it too, and `clear`/`rebuild` drop every registration.
- **Typed access.** `is_dragging`, `drag_payload`, `drag_source`, `drag_target`.
- **Boundaries.** A widget that already owns an internal drag (`TreeView`,
  `PlaylistView`, the table header) should not also be a drag source; the
  container cannot stop their internal behavior and the two would compete. A
  source/target must be in the tree when registered (like `Ui::bind`).

## D114. The showcase is a scrollable, page-per-category catalog

The showcase had grown to eight categories of demos in one 1080×900 window with
hand-computed rectangles; on a smaller window (or a taller widget) components
were clipped, and there was no demonstration of the functional API surface.

- **One `ScrollView` page per tab.** Every page is registered with
  `Ui::set_scroll_view`; its demos are laid out once in *content* coordinates
  by a small wrapping flow (`crates/showcase/src/catalog.rs`). The container
  positions them at `content - offset`, clips and scrolls them, so nothing is
  clipped at the window minimum (now 760×560) and a page can grow without a
  window resize. Pages are shown/hidden by flipping each child's visibility,
  the same mechanism the container already uses.
- **Headings are app-painted captions, not widgets.** A section heading and its
  API note are static strings stored with the page and painted at
  `caption - offset`, clipped to the viewport and tested against the dirty
  box. Notes are wrapped at layout time with a character-width estimate; a
  dozen headings therefore cost neither widgets nor accessibility nodes. The
  one-line sidebar hint and the bottom "last action" strip are painted the
  same way.
- **A page switch parks focus on the tab strip.** Hiding the page's focused
  widget makes the container relocate focus, and relocation reveals the new
  widget — which used to scroll the freshly opened page to an arbitrary row.
  `Msg::Tab` focuses the always-visible tabs first, so a page keeps its live
  scroll offset across switches.
- **The nav badge is responsive.** Eight tabs fit the minimum width without
  the "新" badge; `Tabs::set_badge(index, Option<String>)` (new public setter)
  lets the showcase drop it below 640 DIP and restore it above, instead of
  rebuilding the widget.
- **The functional APIs get their own page.** Native dialogs, programmatic and
  context menus, tray icons/balloons, hotkeys, window controls, a11y
  announce/refresh/copy, theme, renderer and pad status, and a real
  cross-widget drag (D113) are ordinary buttons whose result lands in the
  bottom strip. The self-test drags a card onto a zone; the a11y probe checks
  the text/content page through a fresh snapshot.
- **Trade-offs.** The catalog is taller than a screen, so a demo may start
  below the fold; that is intended (the page scrolls, and focus reveal still
  works). Two-column sections (data, covers, properties) place their headings
  with `caption_block` so a heading never sits below its demo.

## D115. Native date/template dialogs become in-window picker components

`DateTimeDialog` (a `SysDateTimePick32` in a native dialog) and the
application `DialogTemplate` (a generated `DLGTEMPLATE`) are removed. They
could not follow the page theme (fonts and colors need `WM_SETFONT` /
`WM_CTLCOLOR*` / `DS_SETFONT`, which drag backend details into the API and only
partly work under visual styles), and the template form duplicated the widget
layer with a second, non-themeable control set. An application that needs a
custom full dialog builds its own window with the normal widgets; the native
shell/host dialogs that remain are message, file, font and color pickers.

In their place, three ordinary retained widgets share one picker surface:

- **`DatePicker` / `Date`.** A field showing `YYYY-MM-DD` and a month-grid
  popup (Monday-first, six weeks, today accent, selected fill, cursor ring).
  Click the field or press Enter to open; click a day or Enter to choose; arrows
  move, PageUp/PageDown change month, Home/End jump to the week's start/end;
  Escape or an outside press closes. `Date::new` clamps month/day,
  `Date::today()`, `add_days`, `monday_index`, `display()`.
- **`TimePicker` / `Time`.** An `HH:MM` field and two popup columns (hours,
  minutes) in a seven-row window; Up/Down adjust the active unit, Left/Right
  switch, PageUp/PageDown jump five, Enter commits, Escape reverts the value the
  popup opened with. Clicking an hour keeps the popup open and moves to the
  minutes; clicking a minute commits and closes.
- **`ColorPicker`.** A field with a swatch and hex plus an 8×6 palette popup;
  arrows move the swatch cursor, Enter selects, Escape closes unchanged.
- **Ranges (date/time).** `.value_range(min, max)`, `.min_date`/`.max_date`,
  `.clear_range`, `set_range`, `range()`. Out-of-range days/minutes paint
  disabled and cannot be chosen; `set_value` clamps into the range; the cursor
  clamps at the boundaries; a reversed range is normalized.
- **Common API and events.** All three: `new(id, value, on_change)` (the event),
  `value()`/`set_value()`, `is_open()`/`open()`/`close()`, the standard
  observer builders (D109), the surface builders (D110), and a UIA `Button`
  with the value and `ExpandCollapse` (open state). Date/time keep
  `date()/set_date()` and `time()/set_time()`, color keeps `color()/set_color()`.
- **Showcase.** The gallery page demos all three with a range on time; the
  self-test opens/moves/commits each; the UIA round checks the Button value and
  ExpandCollapse state.

## D116. A backdrop blocks the wheel; opening a modal resyncs the page

The in-window modal composites over a cached page. Two paths could leave stale
page pixels under the scrim: the wheel scrolled the page under the backdrop
(`Ui::wheel_xy` sent the notch straight to the panels, bypassing hit-testing),
and the first overlay frame assumed the cached page was complete even when it
was not (a scroll during the modal, a partial paint).

- **The backdrop owns the wheel.** When `Ui::overlay_split()` is `Some` (a
  visible `Scrim`), `wheel_xy` delivers the notchy only to widgets at or after
  the split and never scrolls a page panel. A test pins that the page offset
  does not move under a scrim.
- **One resync frame on open.** `WindowHandle::resync_overlay()` marks the next
  frame as a normal page paint (the host already uses the same flag after a
  buffer recreate): the page and the visible scrim paint, `overlay_painted` is
  set, and later frames return to overlay-only compositing. The showcase calls
  it (plus `invalidate_all_force`) when opening the modal, so the cached page is
  complete and the scrim is blended exactly once. Hiding the full-window scrim
  on close already invalidates the whole window, so the dimmed page is restored.

## D117. Picker follow-ups: wheel/order, RGBA, and the backdrop's widget order

Four defects from the first week of the in-window pickers (D115) and the modal
shell (D87/D116).

- **The color field's swatch filled the window.** `field_swatch_rect` passed the
  swatch's *right/bottom* edges to `Rect::from_xywh`, which takes
  width/height: the rounded fill inherited the field's absolute coordinates as
  a size, so the "20 × 20 swatch" covered everything below and right of the
  field in the selected color. It now builds the rect from the top-left and the
  size, and a test pins the swatch inside the field.
- **The time picker's direction.** The hour/minute columns render increasing
  top-to-bottom; Up/Down did the opposite (spinner convention). Down now steps
  to the larger value, Up to the smaller one, PageDown/PageUp by five, and the
  wheel keeps the same order. A list-style cursor that follows the paint is the
  a11y-correct mapping.
- **Wheel scrolling the page under a picker.** A picker popup is not a
  scrollable widget, so `Ui::wheel_xy` stepped the value *and* scrolled the
  page it sits in — the popup moved with the field and the change was invisible.
  The open `TimePicker` now overrides `scrolls_with_wheel()` (the `ComboBox`
  rule), so the notch belongs to the popup. `DatePicker` and `ColorPicker` do
  the same: their popups must not drag the page under them.
- **Colors are RGBA.** `color::Rgba` (a `Color` plus an alpha byte) is the
  picker's value type: `new_rgba`/`on_change_rgba`/`rgba()`/`set_rgba()`.
  `allow_alpha(true)` adds an eight-step transparency strip under the palette
  (painted over a checkerboard, applied without closing the popup so the color
  can be tuned in place); the property only controls whether the *user* can
  pick an alpha — a value the application stored keeps its alpha. The field
  shows `#RRGGBB` while opaque and `#RRGGBBAA` otherwise.
- **More colors.** `on_more(|| M)` appends a "更多颜色…" row (a keyboard stop
  after the palette) that closes the popup and reports the handoff; the
  showcase opens the library's own color window (D118) seeded with the
  picker's color and writes the result back through `set_rgba`. The system
  `ColorDialog` is unchanged (it keeps its RGB-only chooser and is still the
  API page's native-dialog demo).
- **The overlay split is structural, so the scrim must be added last.** The
  showcase inserted its modal shell in the middle of the widget list; every page
  widget added after it (`gallery_empty`, the API page's labels, ...) then
  belonged to the overlay layer and painted *over* the panel — the "page text
  bleeding through the dialog". The shell (scrim, panel, buttons) now comes
  after every page widget in insertion order, with the toast after it so the
  snackbar still shows over the modal. `Scrim`'s contract ("add it after the
  page's widgets and before the panel") is the rule; the app broke it.
- **No startup tray icons.** The showcase installs none; the API page's
  安装/移除按钮 own the tray demo, the play-row tooltip update only runs when an
  icon is already installed, and the self-test installs and removes its own
  icon.

## D118. The color window is ours: an owned, themed RGBA picker

The color picker's "更多颜色…" opened the system chooser, which has no alpha
control (the classic `ChooseColor` dialog cannot express one) and cannot follow
the page theme. The user decided the library should maintain its own color
window instead of extending or replacing the system dialog.

- **A new window, not a panel.** `dialog::ColorWindow` is shown through
  `WindowHandle::show_dialog`, so it reuses the existing worker-thread dialog
  plumbing (`DialogId`, `DialogResult`, the owner disabled while it runs) and
  the ordinary `Ui`/widget stack inside. `WindowBuilder::owner(raw_hwnd)` makes
  it an owned dialog window (caption + close, no resize frame) centered on the
  owner, so it behaves like a modal chooser without a nested message loop on
  the UI thread.
- **Themes follow the page.** `WindowState::show_dialog` captures
  `Painter::settings()` and the new `Painter::dark()` and passes them to the
  dialog body; the color window is built with `WindowBuilder::theme`, so its
  palette, fonts and native caption follow the application's light/dark
  decision.
- **Contents.** An HSV field (`ColorField`: saturation/value square + hue bar,
  both live-dragged), a hex editor with `#RRGGBB`/`#RRGGBBAA` parsing, an
  RGB/HSV switch (`Segmented`), one row per channel (label + `Slider` +
  `NumberInput`, 0..=255), the built-in presets as clickable dots
  (`SwatchBoard`) and OK/Cancel. Channel sliders report through `on_drag` too,
  so the field, the hex text and the sibling editors follow the knob live.
- **Alpha is a property.** The window only grows the transparency row when the
  opening picker allows an alpha (`ColorPicker::allows_alpha()` →
  `ColorWindow::allow_alpha`); otherwise it reports an opaque color. The
  result is `DialogResult::ColorWindow(Option<Rgba>)`.
- **Hue does not wrap.** The hue bar keeps its position when the color it
  wrote back has an equivalent hue (0° and 360° are the same red): `set_color`
  compares the hue modulo 360 and only moves the thumb for a real change. A
  drag ending at the bottom of the bar used to jump back to the top.
- **Repaint after a code change.** The `Ui::set_*` setters do not invalidate
  (the application owns repaint policy). The window's `sync` therefore
  invalidates exactly the controls it wrote — field, hex, presets, the channel
  sliders and every number editor — so a slider drag updates the alpha editor
  in the same frame instead of waiting for an unrelated repaint.
- **Who wrote the value.** A channel message carries whether it came from the
  slider: a slider already shows its own value, so `sync` leaves it alone while
  the user drags; a number editor reports a value its slider does not have yet,
  so `sync` writes the slider back. Without the flag an edited RGB/HSV box left
  the slider where it was.
- **Supporting additions.** `NumberInput::set_value(i64)` (clamped; ignored
  while the user types), `Painter::dark()`, `WindowBuilder::owner`. The window
  is described in `docs/API.md`; the showcase's picker "更多颜色…" opens it
  through `WindowHandle::show_color_window(rgba, allow_alpha)` (the thin
  `show_dialog(ColorWindow::new(..))` form keeps `.parallel()` and the rest of
  the dialog options available).
- **One Enter path.** The focused control gets the key first (`Ui::key`): Enter
  invokes a focused button — the same action Narrator's Invoke performs — and
  commits an open hex/number editor. Only a key no control consumed falls back
  to the dialog defaults (Enter = OK, Escape = Cancel), so an accessibility
  action and the window's confirm never both fire for one keystroke.

## D119. Content changed by code is silent, self-repainting and coalesced

The widget change callbacks (`TextEdit::on_change`, `NumberInput`/`Slider`'s
`on_change`, `TextEdit::on_commit`) report *user* edits. Callbacks fire for
typing, paste (Ctrl+V and the built-in menu), IME commits, cut, delete and
undo/redo — every path goes through `user_insert`/`remove_range`/`restore`,
which mark the pending change — and never for `set_text`/`set_number`/
`set_value` or the typed `insert_text`/`clear` API. A screen reader's
`SetValue` is a programmatic set, so it is silent for the same reason.

A programmatic change still has to repaint, and a dialog that syncs a dozen
controls at once must not queue a dozen repaints. `Ui::set_text`/`set_number`/
`set_value`/`set_toggle`/`set_selected`/`set_progress`/`set_label`/`set_rows`
therefore record the changed widget's paint bounds in one union
(`Ui::has_damage`), and `Ui::flush_damage(sink)` invalidates that union once and
clears it. The setters cannot invalidate themselves — they have no sink — so
the host flushes at the end of the event that changed the content; the color
window flushes once per event after applying its whole message batch, and the
showcase flushes at the end of `Host::event`/`apply`. Invisible widgets are
skipped (showing one invalidates it through `set_visible`). `Ui::edit_widget`
kept its immediate single-rect invalidation in this pass because it already
reported whether the closure changed anything; **D120 folded it into the same
batch.**

Tests pin the contract both ways: a text field, a number editor and a slider
set through code emit nothing and produce exactly one flush rect covering all
three; typing, Backspace, Ctrl+X and Ctrl+V each report `change:…`, while the
Enter that opens the session and Ctrl+A do not.

## D120. Every display-property change marks its own widget, one flush repaints

D119 coalesced the container-level `set_*` setters. A page still had a second,
faster path: the widget-specific setters (`Tabs::set_badge`,
`ColorPicker::set_rgba`, `StatusBar::set_segment`, `Table::set_cell`,
`PlaybackTransport::set_playing`, `MarqueeText::advance`, …) are reached
through `Ui::edit_widget`, and it invalidated per call. A dialog syncing a
dozen typed-access widgets, a page switch that flips several, or an animation
tick that advances two therefore queued one `InvalidateRect` per change; the
host later unions them into one blit, so nothing was painted twice, but the
per-call z-order intersection and invalidation work grew with the burst.

The rule is now uniform: **a display-property change records the widget's own
rectangle; the host flushes once.** `Ui::edit_widget` drops its sink argument
and stores the before ∪ after paint bounds (a setter that moves an overlay must
repaint where it was) in the same pending-damage union as `set_text`.
`set_props`, `set_enabled`, `set_message` and `set_mnemonic_underlines` join
it: each records the rectangle it changed instead of invalidating, and
`set_message` now marks at all (a toast written through code used to rely on
the application's own manual invalidate). `Ui::mark_dirty(id)` exposes the mark
for an application-owned change the container cannot observe (an app-drawn
overlay beside the widget); hidden or zero-sized widgets record nothing.

`set_visible` stays immediate on purpose: it is a structural change (hit
testing, focus, a11y, a scroll panel's bar chrome), and hiding a widget must
repaint where it was even though its own visibility check would skip the mark.
`select_child` and the user-input paths (`commit_edit`, `char`, `ime_commit`,
`menu_command`) also keep their immediate rectangle: they are actions, not
property syncs.

Because no setter can reach the host, the host is responsible for the flush.
The showcase calls `Ui::flush_damage` at the end of every callback that can
touch widget display state — `event`, `apply` (so `drain` batches its messages),
`timer` (poll report and animation tick), `menu_event`, `menu_command` (through
`apply`), `a11y_action`, `dialog_finished` (through `show_toast`),
`layout_chrome` and `started`; `show_toast` flushes its own message batch since
a background callback (tray, hotkey) may not end in an event. Redundant manual
`window.invalidate` calls that existed only because setters did not paint
(menu command 101/104, the dynamic-panel label, the drag-state label) are gone.

Tests: two widgets edited through `edit_widget` record nothing until one flush
covers both; `set_props` + `set_enabled` share one rect; `mark_dirty` returns
false for unknown/hidden ids and schedules a repaint otherwise; a `set_message`
toast is marked and flushed; typed access that reports no change marks nothing.
The self-test's painted-frame count dropped (44 → 31 on the reference desktop)
because the probes' manual batches now coalesce; the bench still paints once
per animation tick.

## D121. Touch input selects a touch model in the painted editor

Windows promotes touch and pen input to ordinary mouse messages unless a window
registers for `WM_TOUCH`/`WM_POINTER`, which is why a touch screen "works" today
as a mouse: tap places the caret, drag selects. That is not how a mobile text
field behaves, and a painted field is not a real edit control, so the shell
never raises the touch keyboard for it either.

The window host now classifies each pointer message with
`GetMessageExtraInfo`: touch and pen are tagged with `MI_WP_SIGNATURE`
(`(extra & 0xFFFFFF00) == 0xFF515700`), and bit `0x80` of the low byte separates
touch (set) from pen (clear). When the device changes the host dispatches
`Event::PointerKind(PointerKind::{Mouse, Touch, Pen})` once, and the showcase
forwards it to `Ui::set_pointer_kind`. A plain mouse sends nothing, so desktop
behavior is unchanged.

For a touch press the host also owns the long-press dwell: a left press under a
touch device arms a 500 ms timer and records the position; the timer dispatches
`Event::LongPress { pos }` if the finger stays put, while travel past 8 DIP, the
release, leaving the window or losing capture cancels it. The dwell is a shell
policy like the tooltip dwell (the app has no timer seam for a spontaneous
event), so the host owns it and the application only forwards the event to
`Ui::long_press(pos, sink)`.

The container keeps the device (`pointer_kind`) and applies it in three places:

- A touch/pen press that opens a painted session asks the host for the touch
  keyboard (`UiHost::set_touch_keyboard(true)`, implemented through
  `soft_keyboard::show(Layout::Touch)`). Read-only sessions do not ask, because
  there is nothing to type into. The keyboard is dismissed with the other
  soft keyboards (`end_soft_keyboard`) whenever the session ends — focus loss,
  commit, removal, rebuild — so it never outlives its field.
- `Ui::long_press` hands the position to the widget's new `Widget::long_press`
  hook. The painted editor selects the word under the finger and turns on
  `touch_handles`.
- While `touch_handles` is set and a selection exists, the editor paints two
  handles above the content clip: a stem from the line edge to a circle
  (accent fill with a background ring), the start handle above its line and the
  end handle below. The circles are clamped inside the widget rectangle, since
  the container repaints exactly that rectangle. `Widget::grab_selection_handle`
  is offered to the container before it opens/positions a session, so a press
  within 20 DIP of a handle drags that edge instead of collapsing the
  selection; `WidgetInput::DragTo` then moves the anchor (start) or caret (end),
  and crossing the other edge flips the selection. A mouse press clears the
  touch model on the next session start.

Tests: a touch press records `set_touch_keyboard(true)`, a read-only press and a
mouse press record nothing, leaving the field records `false`, `Ui::long_press`
selects the word `(0, 5)` and survives the release; the editor's handle geometry
sits inside the field with the start above and the end below the line, painting
adds the handle strokes, a handle drag moves one edge only, and a password field
never hands its selection out. The showcase forwards both events (there is no
touch hardware in the self-test, so it covers the forwarding only).

## D122. Text is a second drag payload, between fields only

> Superseded by D132: text is one `DragData` kind served by `IDataObject`, and
> the target registry is no longer split into message/text listeners.

D113's cross-widget drag carries one application message (`M`) and reaches
`set_drop_target` handlers. Dragging a selection out of a field and dropping
text into one is a different kind of payload, so the session holds
`DragPayload<M>::Message(M)` or `DragPayload<M>::Text(String)`. The two never
mix: a message drag only notifies `set_drop_hover` and only runs a
`set_drop_target` handler; a text drag only notifies `set_text_drop_hover` and
only runs `set_text_drop_target`. `drag_payload()` therefore returns `None` for
a text drag and `drag_text()` returns the text (and vice versa), so a handler
can peek without guessing.

Sources. `Ui::set_text_drag_source(id)` registers a painted field; at the drag
threshold the container asks the widget for `Widget::take_drag_text(pos)`. The
editor returns the selection (a copy — the field keeps it) when a session is
open, the selection is non-empty, the field is not a password field, and no
touch handle is being dragged. For the drag to be possible the press must land
inside the selection: `TextEdit::begin_edit_impl(Some(pos))` now keeps a
selection the press lands in (so a drag can start) and the click that follows
without a drag collapses it to the pointer. A press outside, or on a collapsed
selection, places the caret exactly as before.

Drops. `Ui::set_text_drop_target(id, |text, pos| Some(Msg))` receives the text
and release point. Only a registered target takes the text (dropping on any
other widget, even another field, is a no-op). When the target is a painted
field the container first calls `Widget::insert_dropped_text`, which opens a
session if needed, puts the caret under the release point and inserts through
the normal user path (undo entry, `max_length`, `on_change`), refusing
read-only/disabled fields; the pending change message is queued before the drop
handler's message. A non-editor target
only gets the handler. `set_text_drop_hover(id, |text, pos, over| bool)` is the
matching hover feedback (the showcase paints the same accent frame the card
targets use). Removal of a widget drops its text registrations and aborts a drag
that involves it, like the message ones.

The showcase's API page gained a "文本拖放" section: a source field, a target
field, a state label and the accent frame on hover; the self-test selects the
source text, presses inside it, drags to the second field and checks the text
arrived (`text_drag` probe field). Ui tests cover the copy drag end to end, a
message target never seeing a text payload, and the widget-level hooks
(handle drag refuses a drag-out, password refuses, read-only refuses a drop).

## D123. The XML layout is a versioned, strict, flat skin document

M7's `layout-xml` feature reads one UTF-8 file whose root is
`<layouts version="1">`. The document contains a small set of breakpoint
variants (`<layout min-width min-height>`), and each variant is a flat list of
`<scroll>` and `<widget>` entries keyed by the existing `WidgetId` name. There
is no hierarchy in the model: entries are looked up by id and never reparent,
reorder or create a widget. Document order does not affect paint, hit-test or
focus order.

Parsing is pure and separate from application. Parsing validates syntax and
the document schema; `Ui::apply_layout` first resolves every id (widget, scroll
panel, scroll child), every range and every expression, and only then mutates.
Any error rejects the whole document and leaves the previous layout untouched;
warnings (a style token on a widget without a surface, an open editing session
skipped, a clamped value) are reported and the rest applies. Diagnostics carry
a stable code and a line/column.

Reason: D7 already fixed "a skin, not a constructor", and hot reload needs an
all-or-nothing failure mode. A flat entry list mirrors `Ui`'s flat widget
vector (D11) exactly, so the XML cannot invent parentage the container would
have to undo. Versions let the schema grow without guessing at old files.
Reference: `docs/M7-XML.md`.

## D124. Geometry is anchor expressions over a client or content space

Every entry determines its rectangle from expressions over the space it lives
in: `left`, `top`, `right`, `bottom`, `cw`, `ch`, `cx`, `cy`, numeric DIPs,
nested parentheses, `+ - * /` and `N%` (percent of the expression's axis).
Horizontal geometry is exactly one of `x`+`w`, `right`+`w` (inset from the
right) or `x`+`right` (stretch); vertical geometry is the same with `y`,
`bottom`, `h`. Over- and under-determined rectangles are errors. Constants
(`<const name value>`) are literal-only and may reference earlier constants.

A widget's space comes from ownership, not a flag: `Ui::scroll_owner_of` says
whether it lives in client space or in a scroll panel's content space. A root
panel's `<scroll>` viewport is client space; a nested one is in its parent's
content space.

Reason: the user chose anchors/percent over flat absolute coordinates, so one
document can survive a resizable window. The grammar is deliberately smaller
than CSS (no selectors, no cascade, no auto layout): the framework has no
measure pass (D1), and adding one would break the dirty-rectangle contract.
The 80 % case (`x`/`y`/`w`/`h`) is still a single plain line, and the writer
never inlines expressions. Reference: `docs/M7-XML.md` section 3.

## D125. Content attributes are static initial values, one write per generation

`text`, `label`, `value`, `progress`, `number`, `checked` and `selected` are
allowed in an entry as literal initial values. They are written only when the
document *generation* changes (first install, or a successful reload whose
canonical model differs), through the silent D119 setters. Resizing, switching
breakpoints, a theme change and re-applying the same document never rewrite
content, so a user's typing is not lost to a resize. A reload deliberately
overwrites; that is what "initial value" means and the debug form says so.

If two variants declare content for the same id, the parsed values must be
equal, which makes the one write independent of the active breakpoint. An open
painted or native editing session on the target makes the applier skip that
entry with a warning instead of committing or discarding the draft.

Reason: the user asked for editable static defaults in the file (useful for
the new `布局` demo page and for iterating on real text), but content still
belongs to the application at runtime (D7). Writing per breakpoint would make
the result order-dependent; writing on every apply would clobber input.
Reference: `docs/M7-XML.md` section 6.

## D126. The XML parser is an optional quick-xml dependency; the library owns no IO

`layout-xml` enables the parser with `quick-xml` at
`default-features = false` (UTF-8 only). D128 factors the parser into
`bamboo-native-ui-layout-core`; the feature still gates it, and the default
build compiles no XML code and links neither crate.
`debug-form` implies `layout-xml` because reload/save are part of the form.
The library never reads, watches or writes a file: the application reads the
text, calls `LayoutDocument::parse`, and handles reload however it likes (the
showcase polls mtime on its existing timer). A missing or malformed file is a
diagnostic, never a panic.

Reason: hand-writing an XML parser means owning entity decoding, BOMs,
attribute normalisation and positions; quick-xml is small, mature and only
present under the feature, so the dependency is cheaper than the bugs. D8
already requires the default build to ship none of this, and keeping IO out
of the library preserves the `Ui`/host seam (`Ui` never touches Win32 or the
file system). Reference: `docs/M7-XML.md` sections 8.5 and 12.

## D127. Widget-specific XML support is a widget-declared skin protocol (design, TODO)

The shared XML vocabulary (geometry, generic properties, surface/font tokens,
`text`/`label`/`value`/`progress`/`number`/`checked`/`selected`) is fixed by
the parser. Everything a single widget type needs beyond that is published by
the widget itself: `skin_attrs() -> &'static [SkinAttrDecl]` plus a
side-effect-free `validate_skin_attr` and a display-only `apply_skin_attr`,
and `skin_children()`/`validate_skin_children`/`apply_skin_children` for data
lists (rows, options, nodes, tracks). The shared names are reserved; a widget's
own names live in its namespace, and names starting with `x-` are guaranteed
for user code.

The applier validates every declared attribute and child list before it mutates
anything, which is what keeps D123's atomic promise intact: a handler cannot
half-apply and then fail. Local attributes and children are content-scoped
(written once per document generation, variant-invariant), never restored by
`clear_layout`, and applied after the data and before selection/state. The
types (`SkinAttrDecl`, `SkinValue`, `SkinElement`) live in `crate::skin` and
are compiled unconditionally, so a third-party widget can declare support
without enabling `layout-xml`; the debug form can list the vocabulary per
widget.

Reason: a central `match` on widget types would grow with every component and
would be closed to third parties; a registry keyed by `TypeId` needs a
downcast seam for every widget. Provided trait methods keep the support local
(add one impl block to a widget), let built-ins adopt incrementally (v1.1
scalars, v2 child lists), and give the writer and the debug form a schema to
introspect. `docs/M7-XML.md` §6.1–6.4 is the contract; the implementation is
TODO.

## D128. The XML has a dynamic and a compiled consumption mode

The same document can be consumed at runtime (`layout-xml`: `quick-xml`,
`LayoutDocument::parse`, `Ui::apply_layout`, hot reload, debug form) or
compiled into plain Rust at build time (`layout-codegen`: a small CLI or the
optional `layout_xml!` proc macro, with `layout-compiled` providing the
runtime helpers for claims, variant selection and application). The parser,
model, validation and writer move into `bamboo-native-ui-layout-core`, a
host-side crate with no Win32 and no `Ui`, so both paths share one schema and
the runtime build with only `layout-compiled` links no `quick-xml`.

The compiled path emits ordinary Rust that can be `include!`d or expanded in
place; geometry and constants are folded by `rustc`, and a parity test applies
the same file through both paths and compares rects, properties and content.
Inline Rust (`rust:` on scalar attribute values; later `<rust target=...>` for
data bindings) is allowed only in the compiled mode, where it is type-checked
by the compiler and can see a user-supplied layout context. The dynamic parser
rejects it so a runtime file stays portable data and is never executed from
disk. Both paths keep D123's strict atomic validation, D125's content timing
and the same diagnostic codes; the compiled path has no hot reload by nature.

Reason: the user asked for a dynamically loaded XML and for a compile-time
translation for speed, with Rust inlining if possible. Compile-time generation
removes parsing and validation from startup and gives the optimizer a
straight-line function; the dynamic path keeps iteration and the debug form.
Factoring the model into `layout-core` avoids duplicating the schema and avoids
a proc-macro dependency cycle (a macro crate cannot depend on the crate that
optionally depends on it). `docs/M7-XML.md` §16 is the contract; implementation
is TODO.

## D129. The layout skin is `.buixml` and the model is canonical

The M7 document uses XML syntax but a dedicated `.buixml` extension, so a skin
is never confused with a generic XML data file in editors, globs or file
association. The library reads a string; the application owns the path (the
showcase uses `layouts/layout-demo.buixml`).

Three grammar details were fixed while implementing D123/D124:

- **Constant names are `[a-z][a-z0-9_]*`** — no hyphen. The quick tour's
  `x="cw-32"` must always read as a subtraction, never as a name; a hyphen in
  the name grammar made `cw-32` ambiguous. A name violation is
  `xml.const.invalid`.
- **Expressions are printed canonically** (`Expr::to_source`): shortest
  numbers, no whitespace, minimal parentheses. The writer and the generation
  hash both use it, so reformatting an expression in the file never counts as
  a new generation, and `parse(to_xml(parse(x)))` reproduces the same model.
- **Geometry is evaluated at apply time**, so a division by zero in an
  attribute is an apply-time `xml.expr.div-zero`, not a parse error; constant
  definitions are folded at parse time, where the same code applies.

`xml.attr.value` (a malformed attribute value) and `xml.const.invalid` (a bad
constant name) join the §9 code table.

## D130. A claim is per declared field, not per mentioned id

§8.2 said the applier captures the whole baseline for every id the document
mentions; taken literally, the file would own `visible`, `enabled` and every
`WidgetProps` field of an id even when no variant declares them. That fights
the showcase's page-visibility pass, which hides every page but the active
one: a resize would restore `visible` to the install-time baseline and show
pages that should be hidden.

The applier therefore claims a *field* only when at least one variant declares
it for that id:

- `visible`, `enabled`, `focusable`, `focus-order`, `cursor`, each `WidgetProps`
  field, the font, the corner shape and the background are separate claims.
- On every apply each claimed field is restored to its baseline and then
  overwritten by the active variant's value (or the baseline, when the active
  variant does not declare it). Unclaimed fields keep whatever the
  application last set.
- `focusable`/`focus-order`/`cursor` restore their container tables explicitly,
  because `WidgetProps` treats `None` as "no override", not "clear it".
- Capturing happens on a generation change, after restoring the previous
  document's claims, so an id dropped from a reloaded file returns to the
  application. `clear_layout` performs the same restore.

The `<scroll>` entries are validated but never claimed: viewports are the
application's layout (D1) and `clear_layout` leaves them where they are.
Content stays an initial value (D125) and is never restored.

## D131. Repaint damage is a set of pieces, and id lookups are hashed

D120 coalesced a burst of display changes into one pending rectangle, but it
still stored that rectangle as the *bounding box* of every touched widget.
A timer that updated a label near the top and a spinner near the bottom
therefore repainted the whole page between them; the window's paint pass then
unioned its dirty queue into the same box. The same shape of cost sat in the
event path: `Ui::index_of` was a linear scan, and it is called once per widget
from `scroll_shown`, `hit_index` and `scroll_owner`, so a pointer move over a
catalog with nine scroll panels cost O(widgets × children) before anything
painted.

- **Pending damage is a `Vec<Rect>`.** `mark_damage`/`mark_damage_rect` push
  through `geometry::merge_damage`: a new piece merges with every piece it
  overlaps or touches (and merging can cascade), and the list collapses to one
  bounding box only past `MAX_DAMAGE_PIECES` (32). `flush_damage` invalidates
  each piece; the host keeps those pieces as separate dirty rectangles.
- **The window paints and blits each piece.** `WindowState::paint` no longer
  reduces dirty to a bounding box: `Ui::paint` culls per widget against the
  piece list, and the back buffer is copied once per piece. A pixel inside a
  piece is covered only by widgets that intersect that piece, so the pieces
  stay correct without every widget clipping itself. The full-window path is
  now only the area threshold (more than 55 % of the client) or an exposure.
- **Id lookups are maps.** `widget_index` (id → insertion index) is rebuilt on
  an insert/remove/reorder; `visibility`, `props` and `cursors` are
  `HashMap<WidgetId, _>`; and `scroll_children` (child id → owning panel) is
  rebuilt when a panel changes. `scroll_owner`, `effective_visible`,
  `effective_interactive` and the cursor lookup are therefore O(1), including
  inside the per-widget hit-test loop.
- **Measured** (debug, 1000 posted moves, no paints): 0.97 s → 0.34 s CPU for a
  stationary pointer and 0.55 s for a full-row sweep. A real 8 s horizontal
  hover trail fell from 7.4 % of one core to 3.1 %. `--bench 2` stayed at
  ~0.71 ms average paint (full frame), so the change is a routing win, not a
  paint trade.

The `damage` batch remains one flush per host callback (D120); only its
representation changed.

## D132. Drag and drop is an OLE system drag

D113's in-window payload was the application's message type `M`, which cannot
leave the process. Dragging a row out to Explorer or dropping a file in from
another application needs the shell's drag protocol, so the drag surface now
carries `DragData` — `Text(String)` (`CF_UNICODETEXT`) or `Files(Vec<PathBuf>)`
(`CF_HDROP`) — and both halves are OLE objects:

- **Source.** `Ui::set_drag_source(id, || Option<DragData>)` and
  `set_text_drag_source` build the data at the threshold; `UiHost::start_drag`
  hands it to the host, which queues it. `WindowState::run_pending_drag` starts
  `DoDragDrop` from the message loop, *after* the application callback returns:
  OLE's modal loop calls the drop target back, and that callback must be able to
  enter `Host` again. `IDropSource` answers the standard button/escape
  protocol; `IDataObject` serves Unicode text or a `DROPFILE` structure.
- **Target.** The window registers an `IDropTarget` at creation (OLE is
  initialized per window thread; a conflicting apartment leaves the in-window
  fallback). `DragEnter`/`DragOver`/`DragLeave`/`Drop` are converted to client
  DIPs and delivered as `Host::drag_enter`/`drag_over`/`drag_leave`/`drag_drop`;
  `Host::drag_finished` follows the source's `DoDragDrop` with the effect it
  received, so the application clears its drag state (OLE consumes the button
  release, so no `PointerUp` arrives).
- **Routing stays in the container.** `Ui::drag_enter`/`drag_over`/`drag_leave`/
  `drag_drop` find the widget under the position, call its `set_drop_hover`
  callback on a target change only, and run the `set_drop_target` handler on
  release. A text drop on a registered painted field inserts at the release
  point through the normal user-edit path first. `set_text_drop_target` and
  `set_files_drop_target` are type filters over the same registry.
- **Fallback.** A host that cannot or may not use OLE (`UiHost::start_drag`
  returns `false`; `WindowHandle::set_system_drag_enabled(false)`) keeps the
  in-window session: the same `DragData`, hover and drop routing, without the
  shell. The showcase disables it for `--self-test`/`--a11y-dump`/`--bench`,
  where synthetic pointers have no physical button release to end OLE's loop.
- **Typed access.** `drag_data`, `drag_text`, `drag_source` (the source id, or
  `None` for a drag that entered from somewhere else), `drag_target`,
  `is_dragging`; `DropEffect::{None, Copy, Move, Link}` is what a target
  reports and what `drag_finished` receives.

## D133. A rounded frame is an outline, and a frame mask carries both colors

`SurfaceStyle::frame` used to fill the whole rounded rectangle with the border
color and then fill the inset with the background. With an opaque background
the second pass hides the first, but with `transparent_background` there is no
second pass: the entire shape — corners included — painted in the border color.
That is the "the circle shows" artifact on a transparent rounded edit box.

The frame is now a real outline:

- **Transparent:** `stroke_round_rect` (uniform corners) or
  `stroke_round_rect_corners` (mixed corners) paints only the 1 DIP ring. A
  zero-radius surface strokes four bands. Nothing inside the ring is touched.
- **Opaque:** `Canvas::fill_round_frame` paints the inset fill and the outline
  as one shape. The default implementation draws the two pieces; the GDI
  backend bakes both colors into one cached BGRA mask, so a rounded field or
  card costs **one** AlphaBlend instead of a full-shape border blend covered
  by a second inner blend. The mask is keyed by size, corner set, thickness,
  border color and fill color; the inner coverage uses
  `CoverageShape::CornersInset`, which is the inset rounded rectangle rather
  than a full-size one.
- **Per-corner rings.** GDI gained `CoverageShape::RingCorners` (outer rounded
  coverage minus the inset rounded coverage, both supersampled) and Direct2D
  the `ring_corners_path` geometry, so cut-corner transparent surfaces keep
  their exact outline instead of falling back to the largest radius.

Tests render a transparent frame into a DIB and assert the interior keeps the
page pixels while the rim is the border; the opaque variant asserts fill inside
and border on the rim; Direct2D checks the same through its software target.
The ignored `frame_bench` (cargo test --lib frame_bench -- --ignored
--nocapture) times 1000 cached 220×36 frames at 175 %: **one-mask 14.4 ms vs
two-pass 27.1 ms** (1.9x).

## D134. Hover routing visits only the visible page, and only the changed cells

A page catalog keeps every page's widgets in one tree and hides them with the
container visibility override. Hit-testing and painting still looped
`0..widgets.len()`, so a pointer move over an idle page scanned every hidden
page and every hidden widget. `Tabs` and `Segmented` also returned
`repaint_on_move() == true` unconditionally, so moving inside one tab repainted
the whole strip on every mouse event (a 1100 DIP-wide, eight-cell widget).

- **Visible index list.** `Ui` keeps `visible_indices` (ascending paint order)
  rebuilt lazily after a visibility/structural change (`invalidate_visible`,
  set from `set_visible`, `set_props`, `set_message`, `add`/`insert`/`remove`/
  `raise`/`lower`, and a typed-access edit that changes `Widget::visible`).
  `hit_index` walks that list instead of all widgets, and `Ui::paint` iterates
  it too. Hidden pages cost one list entry, not one dynamic dispatch each.
- **One overlay probe.** A page with no overlay does one hit-test pass; the
  two-pass overlay order is only used when a visible widget reports `overlay()`.
- **Scroll chrome once per move.** `scroll_chrome_at` finds the bar and the
  corner junction in one pass; `scroll_hover_to` touches only the previous and
  the new bar (only one is hovered at a time), and `hover_cursor` reuses the
  cached result. `scroll_shown` (has visible content) is cached per panel and
  rebuilt only after a visibility or panel change.
- **Per-cell hover repaint.** `Tabs`/`Segmented` override `repaint_on_move_at`
  (`cell_at(pos) != hover`) and remember the old ∪ new cell as the repaint
  hint, so a crossing repaints two cells, not the whole strip; moving inside
  one cell repaints nothing.
- Measured on a debug build: `pointer_move` over an empty area fell from
  ~110 µs to ~37 µs (the full host event from ~140 µs to ~40 µs). A 5 s
  60 Hz sweep across the tab strip produced 9 paints (one per crossed tab,
  each ~two cells) instead of ~300 whole-strip paints.

## D135. System notifications are WinRT toasts with a tray-balloon fallback

The tray icon could already show an `NIF_INFO` balloon, but a notification
should not require the application to keep an icon in the notification area,
and Windows 10/11 routes a real toast to the Action Center with its own
history. The window host now owns a notification path:

- `Notification` (title, body, optional per-notification `app_id`, `tag`,
  `group`, `launch` arguments and `silent`) builds a `ToastGeneric` XML
  document with escaped text.
- `WindowBuilder::app_id` pushes the AppUserModelID to the process
  (`SetCurrentProcessExplicitAppUserModelID`) and is the default toast
  identity; a per-notification id wins. The window thread calls
  `RoInitialize(RO_INIT_SINGLETHREADED)` next to `OleInitialize`
  (`RoUninitialize` at destroy), because WinRT is a separate apartment from
  COM.
- `WindowHandle::register_notification_shortcut(display_name)` writes (or
  refreshes) a Start-menu shortcut pointing at the executable with
  `System.AppUserModel.ID` set, which is what an unpackaged desktop app needs
  for the shell to display anything. It is explicit (one settings/first-run
  call), not an implicit startup side effect. `WindowHandle::notify(&Notification)` calls
  `ToastNotificationManager::CreateToastNotifierWithId` (or the process
  notifier) and reports `NotificationResult::{Toast, Balloon, Failed}`. When
  the shell refuses the toast — an unpackaged process without a registered
  AUMID, or the platform is unavailable — it falls back to the primary tray
  icon's balloon when one is installed. Applications that want the icon in the
  notification area can keep using `show_tray_balloon` directly.
- There is no in-process activation callback: clicking a toast starts the
  application through the shell's normal activation path, and
  `Notification::launch` supplies the arguments the next launch reads. A COM
  activation server is a possible later extension; it needs registration and
  a second command-line handling path, which is the application's decision.

The showcase's API page gained a 系统通知 section; the self-test does not
assert the toast (it depends on shell policy), but an explicit notification
test run produced `Toast` on Windows 11 with the showcase AppUserModelID.

## D136. A color-field drag owns its marker, and dark windows get a dark frame

The color window's HSV field had three related live-preview defects:

1. `ColorField` never opted into `repaint_on_move`, so `DragTo` updated the
   saturation/value/hue but the container skipped the invalidation; the hue
   thumb did not follow the pointer until the release.
2. Every drag message called `on_change`, the host wrote the quantized RGB
   color back through `ColorField::set_color`, and that function's 0.004
   saturation/value threshold treated a one-byte RGB step (≈0.0039) as "no
   change". The marker advanced in irregular jumps and the paint was skipped.
3. The whole 64-column gradient / 36-band hue strip was repainted even for a
   12 DIP marker move, so the drag could not keep up with the pointer.

The field now owns the gesture: `set_color` ignores writes while `drag` is
`Some` (the pointer-derived HSV is exact; the host's quantized write-back is
for the other controls), `repaint_on_move_at` returns true during a drag, and
`take_repaint_hint` narrows the invalidation to the old ∪ new marker for a
square drag (a hue drag changes the square's colors, so it keeps the full
field). `ColorField::paint` also skips gradient columns and hue bands outside
`ctx.dirty`, so the narrowed hint costs two columns instead of sixty-four.
The result follows the pointer at the message rate (60 Hz and above; 30 Hz is
the floor when the shell coalesces).

The "old white bar" remnant was geometry, not refresh timing: the hue thumb
was stroked from `bar.left - 3` to `bar.right + 3` and the SV ring could paint
six pixels outside the square. Pixels outside the widget rectangle are outside
every damage hint, so moving the marker left the old white tips behind.
`cursor_center` now clamps the ring a full radius inside the square and
`hue_thumb_y` plus the inset thumb bar keep the whole white mark inside the
hue strip; a regression test checks all four square corners and the bar ends.

The "white shadow" around the color dialog was the default DWM frame in a
dark window: `dark_theme::apply_to_window` now sets
`DWMWA_USE_IMMERSIVE_DARK_MODE` (20) for dark pages and `DWMWA_BORDER_COLOR`
(34) to black, so the 1 DIP light border no longer glows around a dark dialog.
The change applies to every window the framework creates, not only the color
window.

## D137. The skin protocol stores owned child data and validates before it mutates

`crate::skin` is implemented as designed in D127 with one deliberate change to
the value types: `SkinElement` owns its name, attributes and children (plus the
source line/column for diagnostics) instead of borrowing slices from the source
text. A parsed `LayoutDocument` must be self-contained — the source buffer is
dropped after parsing and a hot reload replaces the document — so a borrowed
`&'a [SkinElement<'a>]` tree would need either a self-referential document or a
per-apply arena. Owned nodes cost one allocation per element, and a data list
ends up as the widget's own `Vec` anyway; `SkinValue`/`SkinAttrDecl` keep the
designed shapes. A helper `SkinElement::same_data` compares structure without
the positions, which is what the variant-invariance check needs (two variants
that declare the same list at different offsets are not a conflict).

The applier reads the target widget's declarations (`skin_attrs`,
`skin_children`) and validates every name, kind and value plus both widget
hooks before it mutates anything, so D123's atomic promise survives a bad
local value. Declared attributes and child elements are content: they are
written only when the document generation changes, in three passes (children,
then the shared content including `selected`, then attributes), each widget
marked once; `clear_layout` does not restore them. An open edit session skips
the write with the existing `xml.content.session-open` warning. The writer emits
local attributes after the shared ones sorted by name and the child elements
after them in source order, which keeps `parse(to_xml(parse(x)))` byte-stable;
the generation hash therefore covers local data too.

New diagnostics: `xml.skin.reserved` (a widget declaration reuses a shared
content name), `xml.skin.unknown` (an attribute or child name the widget does
not declare), `xml.skin.value` (a value fails its kind or the widget rejects it)
and `xml.skin.conflict` (variants disagree on one id's local data). A widget
declares `x-*` names and the library never claims the namespace, so the
showcase's meter (`crates/showcase/src/meter.rs`) is an ordinary third-party
widget: it implements the six provided methods and nothing in the library knows
its vocabulary.

## D138. The rounded-mask cap is per area, and the portable stroke draws arcs

`MAX_ROUND_MASK_PX` limited a cached rounded mask to 1024 pixels *per side*.
Any widget wider than that — an edit field in a wide window at 175 % DPI is
~1900 px — silently took the portable composer instead: `fill_round_rect` fell
back to band + ellipse fills and `stroke_round_rect` to four bands plus, per
corner, a *filled* corner disc. That drew a solid border blob at each corner
over the rounded frame, and the artifact appeared and disappeared with the
widget's width (the user's screenshot: the `m7_edit` field on the layout page).

The cap is now `MAX_ROUND_MASK_AREA = 1 << 20` pixels (4 MB premultiplied): a
wide, short shape is cheap, and a mask is refused only when the *area* is
unreasonable (a 1900×56 field is 106k). `blend_mask` checks the cap itself, so
`fill_round_frame` cannot bypass it either.

The portable stroke fallback also became correct: its corners are quarter rings
walked one device row at a time (the outer and inner radius give the span), not
filled discs, so a shape too large to mask — or a canvas without masks — still
gets an outline. Regression tests render a 1500×40 rounded fill/stroke (clear
corner, anti-aliased arc) and the fallback composer's corner (inner quarter
untouched, arc painted).

## D139. Built-in local scalars, and one setter name per declared name

M7.3 turns D127's protocol on the library's own widgets. Each built-in that has
state outside the shared vocabulary implements the three scalar methods
(`skin_attrs` + `validate_skin_attr` + `apply_skin_attr`) with a static
`SkinAttrDecl` table and a `match` over its own names. Nothing central changed:
the parser, applier and writer are exactly as D137 left them, which is the
point of the protocol — supporting a component is a local block.

The vocabulary was chosen so a declared name always maps 1:1 onto an existing
silent setter, and the missing setters were added alongside the declarations
(`Card::set_title`/`set_body`/`set_emphasis`, `Kbd::set_text`, `ColorSwatch::
set_color`/`set_label`, `Avatar::set_initials`/`set_tint`/`set_ring`,
`Icon::set_tint`, `Toast::set_text`/`set_error`, `Banner::set_body`,
`Spinner::set_animating`, `TextEdit`/`SearchBox::set_cue`, `NumberInput::
set_prefix`/`set_suffix`, `MarqueeText::set_offset`, `Date::parse`/`Time::parse`
and `RepeatMode::parse`). A `Text` declaration may validate its domain in
`validate_skin_attr` (an `Icon` glyph is exactly one character, `Badge`'s tone
and `PlaybackTransport`'s repeat are a fixed enum), so a bad value is still an
`xml.skin.value` before anything mutates.

Two deliberate boundaries:

- **Shared names are not redeclared.** `Badge` text, `Kbd`/`Avatar` label and
  `RatingStars` value stay on `content_caps`; only names the schema does not own
  appear in `skin_attrs()`.
- **Images are not addressable.** `Avatar::image` is Rust-owned because a page
  references pixels by `ImageId`, not by name (D42). A named-image registry is a
  separate follow-up (`TODO.md`), so the attribute is not declared rather than
  declared with a misleading type.

Coverage is a test, not a promise: `builtin_local_scalars_reach_the_widget`
writes every declared name through the real applier and asserts the typed
accessor, and `a_builtin_rejects_an_unparsable_local_value_atomically` pins the
validate-before-mutate path for the enum/date/range cases. The default build has
a smaller `skin.rs` test over a representative sample, so the
declaration-to-setter link cannot drift without `layout-xml` either.

## D140. Data child lists are row-shaped and reject unknown attributes

M7.4 begins with the widgets that already own a runtime data setter, so a child
list is a replacement and not a construction path: `ListView` `<item>` and
`PlaylistView` `<track>` both feed `Row`, `ComboBox` `<option>` feeds
`set_items`, `CoverWall` `<cover>` feeds `set_covers` and `StatusBar`
`<segment>` feeds the new `set_segments`. No widget is created, no structure
changes, and the applier's existing order (children → shared content →
attributes) already gives the rule "data before `selected`" for free.

The vocabulary is deliberately small and strict:

| Child | Attributes |
|---|---|
| `<item>`, `<track>` | `title` (required), `subtitle`, `trailing`, `current`, `disabled` |
| `<option>` | `label` (required) |
| `<cover>` | `title` (required), `subtitle`, `image`, `corner-radius` |
| `<segment>` | `text` (required), `icon`, `width`, `align` |

A row is parsed once in `Row::from_skin`/`Row::valid_skin_row` so the two
list widgets cannot drift. Child attributes are validated as a fixed set
(`SkinElement::attrs_are`), so a typo (`colour`, `disabled="maybe"`) is an
`xml.skin.value` instead of a silently ignored field; the optional bool/number
checks use the new `attr_bool_ok`/`attr_number_ok` helpers. `<cover image>` is
the raw page `ImageId` as a number because a page references pixels by id, not
by name (D42); named covers wait for an image-name registry, the same follow-up
as `Avatar::image`. `align` accepts `left`/`center`/`right` and defaults to
left.

Atomicity is unchanged: every child list validates before anything mutates, so
one bad row leaves the old model intact (tested for missing title, unknown
attribute, bad bool, and empty option/cover/segment). Structural lists (tabs,
table columns, toolbar cells) still declare nothing and are rejected as
unknown children until those widgets own their data at runtime.

## D141. Structural child lists map one data model each, and Pager gains labels

M7.4 step 2 covers the four widgets whose data setter replaces a structural
model rather than row text: `Table`, `TreeView`, `PropertyGrid` and `Pager`.

| Widget | Child | Vocabulary |
|---|---|---|
| `Table` | `<column>` | `title` (required), `width`, `flex`, `align`; no nested children |
| `Table` | `<row>` | `disabled`; nested `<cell text>` (`text` present, may be empty) |
| `TreeView` | `<node>` | `label` (required), `glyph` (one character), `secondary`, `expanded`, `disabled`; nested `<node>` |
| `PropertyGrid` | `<section>` | `title`; nested `<row>` |
| `PropertyGrid` | `<row>` | `key` (required), `value`, `kind`, `read-only`, plus the kind's parameters |
| `Pager` | `<page>` | `label` (required) |

Rules that follow from the design (§6.3):

- **A table's child list is the whole table.** Columns and rows are two lists of
  one model; a declared child list replaces both, so a document that changes one
  declares both. A `<row>` owns only cells; its own attribute is `disabled`
  (the row cursor and `selected` stay shared content).
- **Property rows select an editor kind.** `kind` is one of `read-only`
  (default), `text`, `number`, `choice`, `check`, `rating`. The optional
  parameter set is the kind's own: `min`/`max`/`step`/`decimals` (number; step
  positive, decimals a 0..=10 integer), `options` (choice, `|`-separated and
  non-empty), `on`/`off` (check, both required and non-empty), `stars` (rating,
  1..=10 integer). An attribute outside the kind's set is a typo and
  `xml.skin.value`, not a silently ignored value.
- **Tree nesting is validated by the widget.** `TreeItem::valid_skin_node`
  walks the subtree (name, label, one-character glyph, fixed attributes,
  booleans); applying a `<node>` builds the whole subtree.
- **Pager needed a small widget-side change.** A `<page label>` list is
  meaningless without per-page data, so `Pager` gained `labels: Vec<String>`,
  `set_page_labels` (installs the list, sets `pages` to its length, clamps the
  page, drops a custom format) and `page_labels()`. `refresh_label` prefers the
  current page's label and falls back to the format/built-in text for pages
  past the list; `set_pages` truncates the labels; `set_selected` clamps to the
  last page. `label_with`/`set_label_format` clear the list, so there is one
  label mechanism at a time.
- **`PropertyKind` is re-exported from `widgets`.** Applications and tests
  already needed to inspect a row's kind; it was `pub` in the module but not
  re-exported.

Atomicity and write timing are unchanged (validate everything, then children →
shared content → local attributes). Coverage: the feature test applies all four
lists and asserts every field including the selection; malformed lists (empty
title/label/key, unknown alignment, multi-character glyph, cell without `text`,
bad step/options/kind, a stray kind parameter, an unlabeled page) reject
atomically with no data replaced. Default-build tests cover each widget's
vocabulary without `layout-xml`. The showcase's `布局` page drives these four
widgets from `layout-demo.buixml` in both variants; the `m7` self-test checks
the applied data and both variants' rectangles.

## D142. Label-owning lists finish the v2 child vocabulary

M7.4 step 3 covers the last six built-in lists. Two of them needed the widget
to own its labels; the other four already had an owned model with a silent
setter.

| Widget | Child | Vocabulary |
|---|---|---|
| `Tabs`, `Segmented` | `<option>` | `label` (required) |
| `Breadcrumb` | `<step>` | `label` (required) |
| `NavRail` | `<step>` | `label` (required), `icon` (one character) |
| `RichText` | `<run>` | `text` (required), `style` (`normal`/`strong`/`muted`/`link`, default `normal`), `link` (`u32`, only on a link run) |
| `Sparkline` | `<point>` | `value` (required, finite number) |

- **Tabs/Segmented became runtime data.** Their fields were
  `Vec<&'static str>` (the XML stores owned strings), so both moved to
  `Vec<String>`; the constructors still take `&[&'static str]` and copy, and the
  new silent `set_labels`/`set_options` clamp the selection, clear the hover and
  keep per-option/badge state by index. `labels()`/`options()` are the typed
  accessors. Per-option disabled flags (Segmented) and badges (Tabs) stay
  application-owned: the file speaks labels only.
- **NavRail needed one public type.** Its rows were a private `NavItem`; it is
  now public (`label`/`icon` fields, `new`/`icon`/`label()`/`icon_glyph()`) and
  `NavRail::set_items`/`items` replace/read them. `Breadcrumb` already owned
  `Vec<String>` and `set_items`.
- **RichText and Sparkline already owned their models** (`set_runs`,
  `set_data`); the new code is only the vocabulary plus a `RichText::runs_ref`
  accessor for tests. `link` on a non-link run is a typo, matching the strict
  child-attribute rule.
- **One `<option>` validator**, `valid_skin_option`, is shared by
  `Tabs`/`Segmented`; the combo box keeps its own equivalent declaration.
- **The structural-list rule is now satisfied** for every widget listed in the
  design except construction-only chrome: `Toolbar`, `Ribbon` and `MenuBar` may
  stay Rust-owned.

Coverage mirrors the earlier rounds: the feature test applies all six lists and
asserts every label/style/point plus the selection; malformed lists (blank
option/step, an icon on a breadcrumb step or a two-character rail icon, an
unknown run style, a `link` on a non-link run, a missing/non-numeric point)
reject atomically; default-build tests cover each vocabulary. The showcase's
`布局` page drives the six widgets from `layout-demo.buixml` in both variants and
the `m7` self-test checks the data and both variants' rectangles, plus the wide
and base sparkline rects.

## D143. The debug form is an API plus an application menu, not a built-in panel

The M7.5 design assumed a painted in-app inspector with its own tree, frame
and log views. The user redirected it: the library exposes read-only debug
**data**, the application decides how to show it, and the showcase adds a
`调试` menu that formats snapshots into ordinary dialogs. The default build
still contains no debug code (`debug-form` implies `layout-xml`).

`crate::debug` (feature `debug-form`) is a value-only module:

- `Ui::debug_snapshot() -> DebugSnapshot` walks the widget tree once and
  returns owned data: the installed layout (generation, variant, client size,
  entries with rect + space, claims with baseline rects and field names, scroll
  panels with viewport/offset/content, warnings), one `DebugWidget` per widget
  (effective visible/enabled/focusable/read-only, focus/hover/press, overlay,
  scroll parent, automation id, the accessibility info **with narration
  resolved**, and the declared `skin_attrs`/`skin_children` vocabulary), the
  interaction ids, pending and last-flushed damage, paint measurements and the
  diagnostics ring.
- The snapshot owns everything, so an application can keep it while the tree
  changes. Narration is resolved in the snapshot with the same
  `resolve_name`/`resolve_description`/`apply_rows` path the accessibility
  tree uses, so the debug name is what a screen reader would hear.
- `Ui::debug_event(kind, text)` appends an application note (`note`,
  `layout`, `warning`, `error`); `Ui::clear_debug_log()` drops the ring. The
  applier records one summary per apply, one event per warning, and the first
  diagnostic of a failed apply; `clear_layout` records its own event. Capacity
  is 128 entries (a recent window, not a log file).
- Paint measurements are collected **only** with `debug-form`: every widget
  paint is timed, per-frame and cumulative totals are kept, and the snapshot
  lists per-widget paints/microseconds longest first. `BAMBOO_UI_PAINT_LOG`
  still prints slow widgets in every build; with `debug-form` the same timing
  feeds the queryable stats instead of a second clock.
- Damage is exposed, not drawn: `pending_damage` is the batch waiting for
  `flush_damage`, `last_damage` is what the last flush handed to the host. The
  invalidation overlay the old design sketched is application paint work; the
  library only guarantees the pieces.
- `Ui::hover_id()` joins `focus_id()`/`press_id()` as the interaction query.

The showcase's `调试(&D)` menu (commands 840–847, compiled only with `m7`)
drives the API: `布局快照…`, `控件树…` (first 12 widgets), `帧统计…`,
`诊断日志…` open information dialogs; `重载布局` forces a re-read and apply;
`保存布局` writes `document.to_xml()` after copying the file to
`layout-demo.buixml.bak` and refreshes the mtime so the hot-reload poll does not
re-apply its own write; `重置布局` calls `clear_layout` and drops the in-memory
document (the next file change reloads it); `完整控件树到控制台` prints every
widget line to stdout for a developer with a terminal.

The `m7` self-test probes the API directly (variant, entries, log event,
`m7_list`'s `item` child list and `m7_meter`'s `x-level` scalar); the menu
items are presentation code, so the automated run does not open modal dialogs.
Tracing-event capture into the ring stays a follow-up: the container's tracing
uses the process subscriber, and the layout diagnostics already cover the skin.

## D144. Appearance and interaction locals for the data widgets

M7.3 gave display/input widgets their own scalars, but the data widgets could
only supply rows, columns, nodes and sections. This round adds their
appearance and interaction knobs to the same protocol; **event handlers stay
Rust-only** (user decision) and no callback, focus or widget-creation path is
reachable from a file.

| Widget | New local attributes |
|---|---|
| `Table` | `row-height`, `header-height`, `cell-padding`, `striped`, `grid-lines`, `editable`, `empty-text` |
| `TreeView` | `row-height`, `indent`, `renameable`, `draggable`, `empty-text` |
| `PropertyGrid` | `row-height`, `header-height`, `key-width`, `grid-lines`, `empty-text` |
| `ListView` | `row-height`, `alternating`, `empty-text` |
| `PlaylistView` | `row-height`, `alternating`, `empty-text` |
| `CoverWall` | `gap`, `corner-radius` (wall default; `<cover corner-radius>` still overrides), `caption-height`, `empty-text` |
| `NavRail` | `row-height` |
| `Tabs` | `label-align` (`left`/`center`/`right`) |
| `RichText` | `wrap`, `align` (`left`/`center`/`right`), `text-style` (`body`/`small`/`title`) |
| `Sparkline` | `color` (the one color grammar, palette tokens included), `gap` |
| `Breadcrumb` | `separator` (exactly one character) |

Rules:

- **Values are validated, not silently clamped.** Each numeric attribute has
  the widget's real minimum (`row-height` 16 for table/tree/property, 24 for
  the lists, 18 for the rail; `key-width` 40; `indent` 8; the rest 0), so
  `row-height="4"` is `xml.skin.value` instead of a surprising layout.
- **Shared names stay shared.** `read-only` is still the generic property; the
  grid lock and the per-row locks use it, so no local duplicate exists.
- **Setters, not field pokes.** Every attribute has a silent runtime setter:
  the derived state follows it (`PropertyGrid` rebuilds its entry table and
  keeps the cursor key; `ListView`/`PlaylistView`/`Table` clamp scroll and
  reveal the selection; `Table::set_editable(false)` closes a draft;
  `TreeView::set_renameable(false)` closes a rename; `RichText` bumps its
  cache revision; `Breadcrumb::set_separator` invalidates the measured trail).
  Typed getters (`*_value`, `is_*`, `empty_text_ref`, `separator_char`) make
  the applied state testable; the builders are unchanged.
- **No events.** `editable`/`renameable`/`draggable` only enable the
  widget-side interaction; the application still installs every handler in
  Rust. A file cannot make a widget call back into application code.
- **Ordering is unchanged**: children → shared content → local attributes, one
  damage mark, written once per generation and variant-invariant like every
  other local value.

Coverage: the feature test applies every new name through the real applier and
asserts each typed getter; a malformed case per attribute (below the minimum,
unknown alignment/style, a two-character separator, a negative gap) rejects
atomically with no installed layout. A default-build `skin.rs` test checks a
sample without `layout-xml`. The showcase's `布局` page now declares these on
the table/tree/property/list/tabs/rail/rich/spark/crumb entries in both
variants, and the `m7` self-test checks the typed values.

## D145. The compiled mode is static data plus the dynamic applier's mutation half

D128 promised a second consumption mode and two crates. Both landed in M7.6
(2026-11-28) with one deliberate deviation in shape: the generated module
describes the document as `static` data and calls one runtime entry
(`Ui::apply_compiled`) instead of emitting a setter per attribute.

- **One model, two crates.** `bamboo-native-ui-layout-core` owns the value
  types the model needs (`geometry`, `color`, `skin`, `A11yRole`, `Cursor`,
  `WidgetId`, the `radius` tokens) plus the model, the expression evaluator,
  diagnostics, the canonical writer and — behind its own `parse` feature — the
  `.buixml` reader. `bamboo-native-ui` re-exports every historical path, so no
  application code moved. `layout-xml` turns on `layout-core/parse`;
  `layout-compiled` only turns on the runtime module. The default build has the
  core but no `quick-xml` (`cargo tree` is the gate); a compiled-only build has
  no parser at all.
- **The applier moved to a prepared plan.** `apply_layout` still plans
  (resolves ids, rects, expressions) and now hands a `Prepared` value to
  `Ui::install_prepared`: claims capture/restore, panels, the per-field claim
  write, the children → content → attributes order, the generation gate, the
  damage flush and `AppliedLayout` all live there. Both modes call it, so
  parity is structural, not a second implementation kept in sync by tests.
- **Model fields the applier walks are public.** `LayoutEntry`, `PropAttrs`,
  `StyleAttrs`, `ContentAttr` and friends cross the crate boundary now; the
  compiled descriptor reuses those exact types, which is what makes a generated
  static entry the same shape as a parsed one.
- **The descriptor is static.** `CompiledLayout` → variants → entries/scrolls;
  a rectangle is `fn(Size) -> Result<Rect, RectError>`, properties/surface are
  `&'static LazyLock<…>`, content is `&'static [CompiledContent]` and skin
  children are a static `CompiledElement` tree. `rustc` sees ordinary code, and
  an apply does not allocate for props/style. Geometry is emitted as `f64`
  arithmetic in the model's own evaluation order and converted through the same
  four `f32` edge casts, so rectangles are bit-identical; the golden output
  shows constants inlined.
- **Atomicity survives, though the walk does not disappear.** The preflight
  validates every target id and kind before mutation, and — because the widget
  vocabulary is a runtime property (a third-party widget declares its own
  `x-*` names) — the local skin declarations are validated at apply time in
  both modes. The compiled mode therefore removes parsing, expression
  interpretation and model allocation from startup, not the id/skin walk.
  Diagnostics carry the `.buixml` line and path that the generator embedded in
  the descriptor.
- **Inline Rust landed in the same round.** `rust:` geometry values become
  `Expr::Rust`; scalar values (properties, style, shared content) are recorded
  in `LayoutEntry::rust_attrs` under the schema's scalar names only, so a
  widget-local `x-*` value is never captured (it stays the widget's raw text).
  The writer keeps them in the canonical text, so the generation hash covers
  the code and a save round-trips. The dynamic applier refuses both forms
  atomically with `xml.rust.unsupported` — the file parses, the install does
  not. The codegen splices the text verbatim: geometry inside the `fn(Size)`
  with `cw`/`ch`/`cx`/`cy` as f64 locals (per apply, so it follows the client
  size), properties/style in `LazyLock` statics and content in the
  `CompiledContent` table (evaluated once), with the generated module glob
  importing its parent scope. `rustc` type-checks the values. A `context =
  Type` parameter (geometry would need a capturing form) and `<rust
  target=...>` data bindings stay open.
- **The proc macro landed too.** `crates/layout-macros` is a `proc-macro =
  true` crate with no `syn`/`quote` (the generated source is parsed straight
  into a token stream): `layout_xml!("path")` resolves the path against
  `CARGO_MANIFEST_DIR`, runs the same generator and expands the module in
  place; a document error becomes `compile_error!` with the source position. A
  stable macro cannot register a file dependency, so an edited document needs
  the CLI/build-script flow — documented, with the golden test as the CI
  guard. It deliberately cannot depend on `bamboo-native-ui` (cycle), which is
  why the pure model is in `layout-core`.
- **What is deliberately still open:** inline-Rust `context = Type` (geometry
  would need a capturing form) and `<rust target=...>` data bindings.
- **Evidence.** `crates/layout-codegen/tests/golden.rs` regenerates the
  checked-in module and fails when it is stale; `crates/native-ui/tests/
  layout_parity.rs` applies one fixture through both paths at two breakpoints
  and compares entries, warnings, the full a11y tree and typed content, then
  asserts a same-generation re-apply writes nothing.

## D146. Inline Rust gets a layout context and data bindings

M7.6 closed its two open inline-Rust items (2026-11-28) with one principle: a
value that must see runtime state is computed **per apply**, never stored in a
static, and the runtime API takes the computed values as a parameter instead of
growing a second descriptor.

- **`context = Type` is an `install` argument.** The codegen CLI
  (`--context Type`), the library (`Options { module, context }`) and the macro
  (`layout_xml!("...", context = Type)`) splice the type text into
  `install(.., context: &Type)`. `rustc` resolves it through the module's
  `use super::*`, like every other spliced name.
- **Geometry needed a capturing form.** A static `RectFn = fn(Size)` cannot see
  the context, so an entry whose geometry contains `Expr::Rust` now gets its
  rectangle from a `Box<dyn Fn(Size) -> Result<Rect, RectError>>` closure inside
  `CompiledEntryValues`, capturing `context` by reference. The static `fn`
  still exists (the field is required) and reports the new
  `RectError::Context` (`xml.rust.context`), so calling `apply_compiled` on a
  context module fails loudly instead of applying a wrong rectangle. The
  closure body and the static body are emitted by one function
  (`Emitter::geometry_body`), so the arithmetic and the four `f32` edge casts
  are literally the same code.
- **Scalars moved per apply, statics stayed static.** Properties, style and
  content with inline Rust are built inside
  `fn __values_<variant>(context: &Ctx) -> Vec<CompiledEntryValues<'_>>`
  instead of `LazyLock` statics; an entry with no inline Rust keeps its static
  (the no-context promise "an apply does not allocate for props/style" is
  unchanged). A scalar group with inline Rust replaces that group **whole** —
  literals included — so there is one source of truth per group.
  `install` builds one value list per variant, because claims are the union
  over every variant (D130): a `visible` read from the context must be claimed
  in a variant that is not currently displayed.
- **`Ui::apply_compiled_with` carries the values.** `CompiledValues { variants:
  Vec<Vec<CompiledEntryValues>> }` is indexed like
  `CompiledLayout::variants`; `apply_compiled` is the same call with
  `CompiledValues::default()`. `CompiledLayout::select_index` replaced the
  reference-returning `select` internals so the values and the variant always
  match. Claim merging reads a value override first and the static second.
- **A binding is a child list, not a second data path.**
  `<rust target="id" code="expr"/>` is a child of `<layout>` (the schema rejects
  text nodes, so the code lives in an attribute; `target` names the widget, as
  the placeholder spelling suggested). The expression must produce
  `Vec<SkinElement>` and is handed to the existing
  `validate_skin_children`/`apply_skin_children` seam — no new widget API, no
  registry, and the same children → content → attributes order and
  generation gate as `<item>`. Rules recorded in the parser: the target must be
  a widget of that variant exactly once (`xml.rust.target`,
  `xml.rust.duplicate`), a widget cannot have a static child list and a binding
  (`xml.rust.conflict`), and cross-variant disagreement is `xml.skin.conflict`
  because content is generation-scoped. Without a context the code is one more
  `LazyLock` static (`CompiledEntry::rust_children`); with a context it is part
  of the per-apply values.
- **Dynamic mode still refuses code.** The parser stores bindings (so the
  writer round-trips them and the generation hash covers the code), and
  `plan_layout` reports `xml.rust.unsupported` for every binding, beside the
  per-attribute rejections. A runtime file remains data.
- **Evidence.** Core parse/writer tests for the binding grammar and its
  rejections; codegen golden tests for the checked-in
  `crates/native-ui/tests/fixtures/context_generated.rs` (module
  `context_layout`, context `AppLayout`) and
  `crates/native-ui/tests/fixtures/binding_generated.rs` (a context-free
  binding as a static), for per-apply splicing and for the static shape;
  `crates/native-ui/tests/layout_context.rs` installs the context module at
  both breakpoints (rects, content, radius, a11y name, bound rows, a generation
  switch that does not rewrite content, the static-descriptor failure and the
  dynamic rejection), applies the context-free binding module through both its
  `install` and the bare descriptor; the macro crate expands the context file
  with `context = AppLayout` and installs it.

## D147. Skin images are numeric `ImageId`s, not name strings

The M7.3 follow-up left `Avatar image` undeclared because a page has no
image-name registry (2026-11-28). The close: use the application's own key.

- `Avatar` declares `image` as a `Number` and applies it through
  `Avatar::set_image_id(Option<ImageId>)`, so `image="7"` paints the page image
  registered as `ImageId(7)` and falls back to the initials when it is not
  registered (the existing paint rule). `validate_skin_attr` accepts only a
  whole, non-negative number in `u64` range; `image="-1"` and `image="1.5"` are
  `xml.skin.value`, rejected before anything mutates.
- This is the convention `<cover image>` already used (D140), so one skin
  speaks one image grammar. A name→`ImageId` registry was the alternative;
  it would not remove the application's job (the ids are assigned at runtime by
  `Ui::set_image`) and would add a document-level name table whose lifetime is
  unclear, so it stays out. If a future application wants names, it can keep
  the map and write the number.
- Evidence: `builtin_local_scalars_reach_the_widget` writes `image="7"` and
  asserts `image_id() == Some(ImageId(7))`;
  `a_builtin_rejects_an_unparsable_local_value_atomically` gains the two
  malformed cases. Docs: `docs/API.md` local-attribute table,
  `docs/M7-XML.md` §6.1/§6.3.

## D148. The document creates widgets through a public type registry

M7.7's first half (2026-11-28) closes the last structural gap: a document may
now create a widget, not only address one. The design sketch in §17 held; the
points below are the choices it did not pin down.

- **The registry lives on the `Ui`.** `WidgetRegistry<M>` (`crate::creation`,
  always compiled) maps `type` names to factories and optionally carries a
  `CommandMap<M>`; `Ui::set_widget_registry` installs it, so
  `apply_layout`/`apply_compiled` keep their signatures and the compiled
  `install` needs no extra parameter. `WidgetRegistry::standard()` registers
  twelve built-ins (`label`, `button`, `toggle`, `progress`, `slider`, `icon`,
  `badge`, `card`, `divider`, `kbd`, `avatar`, `spinner`); any crate registers
  its own names from its own code. `Ui::add_boxed`/`insert_boxed` exist because
  a factory produces `Box<dyn Widget<M>>`.
- **The schema gained two attributes.** `type="…"` (a name matching
  `[a-z][a-z0-9_.-]*`) and `command="…"` (a `u32`, only with `type`); both are
  written after `id`. An interactive built-in resolves `command` through the
  registry's `CommandMap` once at creation: `M` need not be `Clone` for a
  click, and a value-reporting control (`toggle`, `slider` — the only two
  standard factories that require `M: Clone`) reports the same message on each
  change and the application reads the state back through the typed accessors.
  A missing command or command table is `xml.command.missing`.
- **Creation is planned, not mutating.** The factories run during the planning
  phase (before any `Ui` mutation) and their widgets are staged; validation
  reads the staged widget's own `skin_attrs`/`content_caps`/`surface_style`
  through `Ui::widget_ref`, so a created widget gets exactly the same
  validate-then-mutate treatment as an addressed one, and a later error drops
  every staged widget. The staged widgets are inserted in
  `install_prepared` **before claims are captured**, so they get a normal claim
  record; insertion is **before the first backdrop widget** (page layer, the
  modal split keeps working), or at the end without one.
- **A created widget is a client-space widget.** v1 stays flat (D1): a created
  entry cannot join a `<scroll>` panel (there is no `<panel>` child), so its
  rectangle resolves in client space. `Ui::set_visible` on an id that does not
  exist yet now records the override instead of ignoring it, so an application
  page pass that hides a page before the layout runs also hides the widgets the
  document later creates on it.
- **Ownership is content-scoped and variant-invariant.** Every variant must
  declare the same typed ids with the same `type`/`command` (different geometry
  is the point); otherwise `xml.type.conflict`. The installed layout records
  `AppliedCreation { id, type_name, command }`; on a generation change a
  previously created id that the new document no longer declares is
  `Ui::remove`d (which already clears overlays, focus, Tab order, scroll child
  lists and a live edit session, D62), and a changed `type`/`command` is a
  remove-and-recreate. Same-generation applies (resize, theme) never remove or
  create. `Ui::clear_layout` removes every created widget with the claimed
  fields. An id that exists but is not document-owned is `xml.id.duplicate`;
  an absent id with no registry (or no such type) is `xml.type.unknown`; a
  factory that returns another id or refuses is `xml.type.create`.
- **The compiled mode shares the planner.** `CompiledEntry` gained
  `type_name`/`command`; both modes build `CreateRequest`s and call the one
  `Ui::plan_creations`. The compiled preflight no longer reports an unknown id
  for an entry the document itself creates. The generated `install` is bound
  `M: 'static` (factories produce boxed widgets).
- **Evidence.** Core parse/writer round trip and the rejection matrix
  (`xml.attr.value`, `xml.attr.conflict`, `xml.type.conflict`);
  `crates/native-ui/tests/layout_create.rs` (static-doc creation with a
  command message, claimed fields, page-layer insertion, in-place re-apply,
  `clear_layout`, reload removal, type replacement, unknown type, missing
  command, id collision, wrong-id factory, and the compiled module through the
  checked-in `fixtures/create_generated.rs`); codegen golden + descriptor
  assertions; 53 core tests, 14 codegen tests, 12 creation tests.

## D149. M7.7 follow-ups: scroll membership, document order, registry listing, value-reporting commands

The four items the M7.7 round left open, decided and implemented on 2026-11-28.
D1 still holds: there is no container tree, only membership in an existing
scroll panel.

- **`parent="…"` is membership, not a container.** A created `<widget>` may
  name a registered scroll panel as its `parent`; its rectangle is then in the
  panel's content space (`EntrySpace::Content(panel)`) and the applier appends
  it to the panel's child list (the panel cannot know an id that did not exist)
  before the panel is positioned. An *addressed* entry may declare `parent` as
  an assertion: it must already be that panel's child (`xml.parent.missing`).
  `parent` joins `type`/`command` in the content-scoped, variant-invariant
  tuple (`xml.type.conflict`), and a changed parent is a remove-and-recreate
  like a changed type. A declared `<scroll>` and an application-registered panel
  both work; a panel the document does not lay out still gets the child (the
  attach step re-registers it). Codes stay `xml.parent.missing` for "not a
  scroll panel", "both a widget and a panel" and "not that panel's child".
- **Document order is part of the model.** A kept created widget used to keep
  its old insertion slot, so a reorder or a middle insertion left the page
  layer in the previous order. On a generation change the planner computes the
  resulting order (kept ids in their old order, then new ids). When that differs
  from the document order, every kept created id is removed and re-created, so
  the block is rebuilt by declaration order. Same-generation applies never
  restack, and a document that only appends keeps its widgets in place.
- **The registry is visible to the debug form.** `DebugSnapshot` gained
  `widget_types: Vec<&'static str>` and `has_commands: bool` (empty/false with
  no registry). The snapshot stays data-only; the showcase's 布局快照 dialog
  prints the list. `WidgetRegistry::has_commands()` is the public query.
- **Value-reporting controls keep the fixed message.** Option (b), a
  payload-carrying callback, was rejected. A payload-specific command would
  need a second callback shape on `Toggle`/`Slider` plus a payload-aware
  `CommandMap` (two payload types, and `M` produced per change), for a benefit
  the library already provides: the message says *which control* changed and
  the application reads the new state through the typed accessors
  (`with_widget::<Toggle>`/`Slider`), exactly like an application-built control
  whose handler reads the widget. The fixed-message pattern also keeps `M` free
  of `Clone` for every click-only factory; only `WidgetRegistry::standard()`
  itself is bound by `M: Clone` because it registers the two value-reporting
  factories. A third-party factory that needs a payload registers its own type,
  closure and message shape.

## D150. Text colour in the layout schema and `.buitheme` theme documents

M7.8 closed the two colour gaps (2026-11-28). The layout file can set a text
colour, and a whole palette can be data. D1 and the flat-layout rule are
unaffected.

- **Text colour is a widget-declared local, not a new shared coordinate.**
  `color` (`SkinValueKind::Color`) is declared by the built-ins that already
  have a foreground API: `Label`, `SectionTitle`, `CopyableText`, `MarqueeText`,
  `RichText` (the normal/strong runs), `TextEdit` and `SearchBox` (the closed
  text and the editing session). It uses the one colour grammar (hex/decimal/
  `rgb()`/palette tokens) and, like every local, is validated before anything
  mutates. Each declaration has a silent setter (`set_color(Option<Color>)`,
  plus `set_text_style`/`text_style` for the `text-style` local) so the skin
  and the application write the same state; a widget without transparency drops
  the alpha. `Label`, `CopyableText` and `MarqueeText` also declare
  `text-style` (`body`/`small`/`title`); `RichText` already had it. `color`
  is content-scoped skin data, not a claim: it is written with the other locals
  (children → content → attributes) once per generation, and an application
  that wants the old colour back writes it itself — claims only cover
  `WidgetProps`/surface fields.
- **The theme document is `theme-xml`, an independent feature.** The model,
  reader (`ThemeDocument::parse`, behind `layout-core/parse`) and canonical
  writer live in `layout-core::theme`; `bamboo-native-ui`'s `theme-xml` feature
  re-exports them and adds the mapping (it does **not** enable the layout
  feature, and `layout-xml` does not enable it). Extension `.buitheme`, XML —
  one dialect family with `.buixml`, so the diagnostic style, canonical text and
  generation hash are shared.
- **The schema.** `<theme version="1" name="…" base="light|dark" accent="#…">`
  with optional `<color name value/>` children. `base` is required: the document
  does not depend on the application's current mode. `accent` is optional and
  feeds [`Palette::new`] *before* derivation, so the accent-derived tokens
  follow it; a `<color name="accent">` then sets that one field explicitly.
  Every `<color>` may use any palette token in kebab-case (the `background`
  token vocabulary), must be unique, and takes a literal colour (a palette
  token is not a literal: the document defines the palette). Alpha is accepted
  and dropped (palette colours are opaque). Duplicates, unknown names and bad
  values are errors before anything changes; codes: `theme.syntax`,
  `theme.version`, `theme.structure`, `theme.unknown-element`,
  `theme.unknown-attribute`, `theme.missing-attribute`, `theme.base.value`,
  `theme.color.value`, `theme.color.unknown`, `theme.color.duplicate`.
- **Applying it is the application's swap.** `ThemeDocumentExt::palette(
  &ThemeSettings) -> Palette` (in `theme_xml`, next to `Palette`) derives
  `Palette::new(settings, base)` and applies the overrides in declaration order
  through `set_palette_token`. The layout's colour tokens resolve against the
  palette the application passes to `apply_layout`, so a document and a theme
  compose. To make the painted page follow the whole table (not just base +
  accent), `WindowHandle::set_palette(Palette)` installs a palette on the
  window's painter and repaints once; `WindowHandle::palette()` reads it back.
  `Painter::set_palette` now keeps its dark flag in step with the palette. No
  new window state: the painter already owned the palette.
- **Showcase.** `crates/showcase/themes/night.buitheme` is checked in; the API
  page's 主题文档 section loads it (`api_theme_doc` → `api_theme_state`), the
  light/dark and accent presets still clear the document palette, and the
  `--self-test` probe checks a loaded token (background, accent, text) plus the
  window palette the page paints with, then restores the preset theme.
- **Read-only reaches every text field (user follow-up in the same round).**
  `TextEdit`/`TextArea`/`PasswordBox` already had `.read_only(bool)`; `SearchBox`
  and `NumberInput` ignored the container-pushed flag. Both now declare it
  (`.read_only(bool)`, `is_read_only()`, `Widget::read_only`/`set_read_only`):
  the session still selects and copies, while typing, paste, cut, delete,
  undo, IME, the search clear target and the number steppers are refused, and
  UIA reports read-only. `enabled(false)` stays the separate disabled state.
  The embedded `TextEdit::inline_insert`/`inline_set_composition` now honour
  read-only too, so a read-only composite cannot be typed into through the
  container session.
- **Evidence.** `layout-core` theme parse/write round trip, minimal document
  and the rejection matrix (58 core tests with `parse`); `theme_xml` tests
  (base, accent derivation, explicit override after derivation, every token);
  the built-in scalar test extended with `color`/`text-style` on the seven
  widgets and the malformed-value matrix; two read-only tests; the showcase
  self-test step.
