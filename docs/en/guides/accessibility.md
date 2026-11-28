# Accessibility

> Describe each widget once, override names and descriptions from the
> application, wire the UIA provider, and verify the tree with `--a11y-dump`.

## How the model works

1. A widget returns `A11yInfo` from `Widget::a11y`: role, name, value, range,
   checked state, published children and optional scroll/grid state.
2. `Ui::a11y_nodes()` flattens every visible widget into `Vec<A11yNode>` and
   applies the application's props (role override, narration, exposure,
   visibility, focus).
3. The Windows host returns that vector from `Host::a11y_nodes`; the platform
   layer exposes it through UI Automation.
4. A screen-reader request comes back as an `A11yAction`, which
   `Ui::apply_a11y` turns into the same input a user would produce.

The model is platform-neutral; only the UIA provider and
`WindowHandle::a11y_*` are Windows-only.

## Steps

### 1. Give every widget a role and a name

A built-in already does this. A custom widget returns an `A11yInfo`:

```rust
use bamboo_native_ui::{A11yInfo, A11yRole, Widget};

impl<M> Widget<M> for Rating {
    fn a11y(&self) -> Option<A11yInfo> {
        Some(
            A11yInfo::new(A11yRole::Slider, "Rating")
                .value(format!("{} of 5", self.value))
                .range(0.0, 5.0, f64::from(self.value)),
        )
    }
}
```

Choose the role from the vocabulary (`Button`, `Toggle`, `Slider`,
`TextField`, `NumberField`, `RadioGroup`, `TabList`, `List`,
`MultiSelectList`, `RadioItem`, `TabItem`, `ListItem`, `Tree`, `TreeItem`,
`Grid`, `GridCell`, `ColumnHeader`, `Heading`, `Group`, `Separator`, `Text`,
`ComboBox`, `ProgressBar`, `MenuItem`, `Menu`, `MenuBar`, `ToolBar`,
`StatusBar`). The role selects the UIA control type and the patterns offered
(for example `Button` → Invoke, `Toggle` → Toggle, `Slider` → RangeValue,
`TreeItem` → SelectionItem + ExpandCollapse).

The full `A11yInfo` builder set:

| Builder | Meaning |
|---|---|
| `new(role, name)` | Role and accessible name. |
| `.value(text)` | Current value (editors, sliders, progress). |
| `.description(text)` | UIA HelpText / FullDescription. |
| `.checked(bool)` | Checkbox/toggle state. |
| `.range(min, max, value)` | RangeValue range and position. |
| `.text_selection(start, end)` | Byte offsets into `value` for a closed text field. |
| `.read_only(bool)` | Value pattern reports read-only. |
| `.children(Vec<A11yChild>)` | Options, tabs or visible rows. |
| `.display_only()` | Decoration: out of the tree and Tab unless opted in. |
| `.expanded(bool)` | ExpandCollapse state (menu, tree row, accordion header). |
| `.scroll(A11yScroll)` | Scroll pattern state. |
| `.grid(rows, columns)` | Grid extent; cells are the children. |

`A11yInfo::new` defaults `read_only` to `false`, so an editor that accepts
typing does not need an extra call; a read-only text block sets
`.read_only(true)`.

### 2. Publish child elements for composite widgets

One `A11yChild` is one option, tab or visible row. The index is the widget-level
index, so a scrolled list keeps absolute row numbers.

```rust
use bamboo_native_ui::{A11yChild, A11yInfo, A11yRole, Rect};

fn tabs_info(labels: &[&str], selected: usize, rect: Rect) -> A11yInfo {
    let children = labels
        .iter()
        .enumerate()
        .map(|(index, label)| {
            let cell = Rect::from_xywh(
                rect.left + index as f32 * 80.0,
                rect.top,
                80.0,
                rect.height(),
            );
            A11yChild::new(index, *label, cell).selected(index == selected)
        })
        .collect();
    A11yInfo::new(A11yRole::TabList, "Views").children(children)
}
```

`A11yChild` builders: `.selected`, `.enabled`, `.offscreen`, `.checked`,
`.expanded`, `.role` (override the container's child role), `.grid_cell(row,
column)`, `.value`, `.editable`. Publish only realized rows and mark a
look-ahead row `.offscreen(true)`; a 600-row queue stays cheap.

For scrolling containers, publish `A11yScroll`:

```rust
use bamboo_native_ui::{A11yInfo, A11yRole, A11yScroll};

fn list_info(offset: f32, viewport: f32, content: f32) -> A11yInfo {
    A11yInfo::new(A11yRole::List, "Library")
        .scroll(A11yScroll::new().vertical(offset, viewport, content))
}
```

UIA advertises the Scroll pattern only while an axis can actually move.

### 3. Override from the application with `WidgetProps`

Narration changes what a screen reader says without changing the widget's
painted text.

```rust
use bamboo_native_ui::{Narration, Ui, UiHost, WidgetId, WidgetProps};

fn describe_play(ui: &mut Ui<()>, sink: &mut impl UiHost) {
    let props = WidgetProps::new().narrate(
        Narration::new()
            .name("Play")
            .description("Starts playback of the selected track"),
    );
    ui.set_props(WidgetId::new("play"), props, sink);
}
```

| Builder | Effect |
|---|---|
| `Narration::name(text)` | Fixed accessible name. |
| `Narration::name_with(f)` | Name built from the current `A11yInfo` (a live counter). |
| `Narration::description(text)` | Fixed description. |
| `Narration::description_with(f)` | Description built from the current info. |
| `Narration::row(f)` | Rename the children of a composite (`Fn(usize, &A11yChild) -> String`). |
| `WidgetProps::role(role)` | Override the widget's own role. |
| `WidgetProps::automation_id(id)` | Stable UIA `AutomationId`. |
| `WidgetProps::expose(bool)` | Force a display-only widget into or out of the tree. |

A `display_only()` widget stays out of the tree and out of Tab unless it is
focusable, interactive, narrated, role-overridden or `expose(true)`. That is why
titles, cards and progress bars do not read on every Tab.

### 4. Wire the host

```rust
use bamboo_native_ui::{A11yAction, A11yNode, Host, WindowHandle};

impl Host for App {
    fn a11y_nodes(&mut self) -> Vec<A11yNode> {
        self.ui.a11y_nodes()
    }

    fn a11y_action(&mut self, action: A11yAction, window: &mut WindowHandle<'_>) {
        if let Some(message) = self.ui.apply_a11y(&action, window) {
            self.apply(message, window);
        }
        window.a11y_focus(self.ui.focus_id());
    }
}
```

Both hooks are required for a useful tree. The default `a11y_nodes` returns an
empty vector, and the default `a11y_action` does nothing: read-only inspection
works, but Invoke/Toggle/Select/SetValue do nothing.

After a page rebuild or a layout apply, call `WindowHandle::a11y_refresh()` so
the next provider request rebuilds the snapshot. `a11y_focus(Some(id))`
announces a focus change; `a11y_focus(None)` clears it.

`Ui::apply_a11y` handles `Invoke`, `Toggle`, `Select`, `SetValue`, `Focus`,
`Expand`, `Collapse` and `Scroll { horizontal, vertical }`. It routes through
the widget's normal input path, so application messages and observers fire
exactly as they do for a mouse or keyboard.

### 5. Focus and shortcut patterns

- Preferred: make the control focusable and let Tab reach it. Use
  `Ui::set_focusable(id, true, sink)` or `WidgetProps::focusable(true)`;
  `Ui::set_focus_order(id, weight)` sets the order (smaller first, ties keep
  insertion order).
- Enter/Space on a focused control becomes `WidgetInput::Activate`. A UIA
  Invoke/Toggle arrives as the same input, so no second code path is needed.
- Composite widgets (lists, tabs, trees) own arrow-key navigation and publish
  selection changes; do not add a parallel keyboard handler.
- Chrome labels (menu bar, menu button, toolbar, split button) parse `&` as an
  access key: `"&File"` paints `File` and answers `Alt+F`; `"&&"` is a literal
  ampersand. Ordinary controls use Tab plus Space/Enter, not access keys.
- Window shortcuts (`register_window_hotkey`) fire only while the window has
  focus; global hotkeys (`register_hotkey`) are system-wide. Both are covered
  in [Key codes and hotkeys](../reference/key-codes-and-hotkeys.md).
- `Ui::bind(id, Trigger::Activate, handler)` observes activation in addition to
  the widget's own behavior; return `Some(message)` to queue one, or call
  `ctx.run_default()` to run the built-in action yourself.

### 6. Announce status and errors

```rust
use bamboo_native_ui::{AnnouncementPriority, WindowHandle};

fn imported(count: usize, window: &mut WindowHandle<'_>) {
    window.a11y_announce(format!("{count} files imported"));
}

fn failed(reason: &str, window: &mut WindowHandle<'_>) {
    window.a11y_announce_with(reason, AnnouncementPriority::Assertive);
}
```

`Polite` (the default) keeps only the newest message, which suits status ticks
and repeated toasts; `Assertive` interrupts the current utterance for errors
and destructive actions. Announcements are transient and not tied to an
element.

### 7. Check the tree

```powershell
cargo run -p bamboo-native-ui-showcase -- --a11y-dump
```

`--a11y-dump` runs a real UIA **client** on a worker thread: it asks the window
for its provider, walks the control view and prints names, roles, patterns,
values and selection. It exits non-zero if the probe fails, so it is a CI check
as well as a debugging tool.

For interactive inspection, use the Windows SDK tools (Inspect, Accessibility
Insights for Windows) against a running application. Verify the same things the
dump checks:

- each control has a non-empty name and the expected control type;
- a button exposes Invoke, a toggle Toggle, a slider RangeValue, a list
  Selection + Scroll, a table Grid + GridItem;
- the focused element is the one the application thinks is focused;
- a selection change raises `SelectionItem_ElementSelected` and a toggle state
  change raises `ToggleState` (the dump listens for both);
- rows scrolled out of view report `IsOffscreen` rather than disappearing.

## Common mistakes

- **Empty tree** — `Host::a11y_nodes` is not implemented.
- **Readable but not operable** — `Host::a11y_action` is not implemented, or the
  widget's role offers no pattern for the action.
- **A decoration reads on every Tab** — the widget is `display_only` but the
  application made it focusable or narrated; drop the opt-in or set
  `expose(false)`.
- **Stale names after a rebuild** — call `a11y_refresh()` after the rebuild.
- **Wrong row announced after scrolling** — the child index must be the
  widget-level index, not the viewport index.
- **A list is slow to inspect** — publish only realized rows plus one
  look-ahead; mark it `offscreen`.
- **`--a11y-dump` fails only in CI** — the desktop session must allow a window;
  run it interactively or in a session with an interactive desktop.

## See also

- [Accessibility API](../api/accessibility.md)
- [The widget trait](../api/widget-trait.md)
- [Host and window](../api/host-and-window.md)
- [Scrolling](../api/scrolling.md)
- [XML elements](../reference/xml-elements.md) (`role`, `name`, `description`, `automation-id`, `expose`)
- [Key codes and hotkeys](../reference/key-codes-and-hotkeys.md)
- [Debugging](debugging.md)
