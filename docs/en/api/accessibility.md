# Accessibility

> The platform-neutral accessibility model, how a widget describes itself, and how the Windows host exposes it through UI Automation.

## Overview

Widgets describe themselves once per snapshot (`Widget::a11y`), `Ui` flattens
that into a tree of `A11yNode`s, and the Windows platform layer exposes the tree
through UI Automation (UIA). Strings are owned, so a snapshot can outlive the
paint pass and cross into a screen reader.

Authoring happens in two places: a widget returns an `A11yInfo` describing its
role, name, value and published children; the application wires the host by
returning the snapshot from `Host::a11y_nodes` and applying incoming
`A11yAction`s through `Ui::apply_a11y`. The UIA provider is Windows-only
(`platform::uia`), but the model in `a11y` and the `A11yRole` vocabulary are
cross-platform.

## Key types

### `A11yRole`

```rust
pub enum A11yRole {
    Button, Toggle, Slider, TextField, NumberField,
    RadioGroup, TabList, List, MultiSelectList,
    RadioItem, TabItem, ListItem,
    Tree, TreeItem, Grid, GridCell, ColumnHeader,
    Heading, Group, Separator, Text, ComboBox, ProgressBar,
    MenuItem, Menu, MenuBar, ToolBar, StatusBar,
}
```

Each role maps to a UIA control type and a fixed set of patterns (for example
`Button` → Invoke, `Toggle` → Toggle, `Slider`/`ProgressBar` → RangeValue,
`TreeItem` → SelectionItem + ExpandCollapse). `A11yRole::child_role()` gives the
role a container publishes for its children.

### `A11yInfo`

```rust
pub struct A11yInfo {
    pub role: A11yRole,
    pub name: String,
    pub description: Option<String>,
    pub value: Option<String>,
    pub checked: Option<bool>,
    pub selected: Option<bool>,
    pub range: Option<(f64, f64, f64)>,
    pub read_only: bool,
    pub text_selection: Option<(usize, usize)>,
    pub children: Vec<A11yChild>,
    pub display_only: bool,
    pub expanded: Option<bool>,
    pub scroll: Option<A11yScroll>,
    pub grid: Option<(usize, usize)>,
}
```

Built with `A11yInfo::new(role, name)` plus:
`value`, `description`, `checked`, `range(min, max, value)`,
`text_selection(start, end)`, `read_only`, `children(Vec<A11yChild>)`,
`display_only()`, `expanded`, `scroll(A11yScroll)`, `grid(rows, columns)`.

`display_only()` keeps a decoration out of the tree and out of Tab unless the
instance opts in — titles, cards and progress bars use it.

### `A11yChild`

```rust
pub struct A11yChild {
    pub index: usize,
    pub name: String,
    pub selected: bool,
    pub enabled: bool,
    pub rect: Rect,
    pub offscreen: bool,
    pub checked: Option<bool>,
    pub expanded: Option<bool>,
    pub role: Option<A11yRole>,
    pub value: Option<String>,
    pub read_only: bool,
    pub row: Option<usize>,
    pub column: Option<usize>,
}
```

`A11yChild::new(index, name, rect)` plus `selected`, `enabled`, `offscreen`,
`checked`, `expanded`, `role`, `grid_cell(row, column)`, `value`, `editable`.
One child is one option, tab or visible row.

### `A11yNode` and `A11yScroll`

`A11yNode` is the flattened snapshot element: `id: WidgetId`, `role`,
`child: Option<usize>`, `name`, `automation_id`, `description`, `value`,
`enabled`, `focusable`, `focused`, `checked`, `selected`, `range`, `read_only`,
`text_selection`, `rect`, `offscreen`, `expanded`, `scroll`, `grid_row`,
`grid_column`, `grid`, `parent`, `children`. `parent`/`children` are indices into
the vector returned by `Ui::a11y_nodes`.

`A11yScroll { horizontal, vertical }` holds `A11yScrollAxis { offset, viewport,
content }` per axis. Build with `A11yScroll::new().horizontal(o, v, c)` and
`.vertical(o, v, c)`; `scrollable()` gates whether UIA advertises the Scroll
pattern.

### `A11yAction` and priorities

```rust
pub struct A11yAction {
    pub kind: A11yActionKind,
    pub id: WidgetId,
    pub child: Option<usize>,
    pub value: Option<String>,
    pub number: Option<f64>,
}

pub enum A11yActionKind {
    Invoke, Toggle, Select, SetValue, Focus, Expand, Collapse,
    Scroll { horizontal: Option<f32>, vertical: Option<f32> },
}
```

`AnnouncementPriority` is `Polite` (default; keeps only the newest message) or
`Assertive` (interrupts the current utterance).

### Wiring

| API | Signature |
|---|---|
| `Widget::a11y` | `fn a11y(&self) -> Option<A11yInfo>` |
| `Ui::a11y_nodes` | `a11y_nodes(&self) -> Vec<A11yNode>` |
| `Ui::apply_a11y` | `apply_a11y(&mut self, action: &A11yAction, sink) -> Option<M>` |
| `Host::a11y_nodes` | `fn a11y_nodes(&mut self) -> Vec<A11yNode>` |
| `Host::a11y_action` | `fn a11y_action(&mut self, action: A11yAction, window: &mut WindowHandle<'_>)` |
| `WindowHandle::a11y_refresh` | `a11y_refresh(&mut self)` |
| `WindowHandle::a11y_focus` | `a11y_focus(&mut self, id: Option<WidgetId>)` |
| `WindowHandle::a11y_announce` | `a11y_announce(&mut self, text: impl Into<String>)` |
| `WindowHandle::a11y_announce_with` | `a11y_announce_with(&mut self, text, priority: AnnouncementPriority)` |

## Example

Authoring a custom widget and narrating a built-in one:

```rust
use bamboo_native_ui::{A11yInfo, A11yRole, Narration, Widget, WidgetProps};

impl<M> Widget<M> for Rating {
    fn a11y(&self) -> Option<A11yInfo> {
        Some(
            A11yInfo::new(A11yRole::Slider, "Rating")
                .value(format!("{} of 5", self.value))
                .range(0.0, 5.0, self.value as f64),
        )
    }
}

fn annotate(ui: &mut bamboo_native_ui::Ui<Msg>, sink: &mut impl bamboo_native_ui::UiHost) {
    let props = WidgetProps::new().narrate(
        Narration::new()
            .name("Play")
            .description("Starts playback of the selected track"),
    );
    ui.set_props(bamboo_native_ui::WidgetId::new("play"), props, sink);
}
```

Host wiring:

```rust
use bamboo_native_ui::{A11yAction, A11yNode, WindowHandle};

fn a11y_nodes(&mut self) -> Vec<bamboo_native_ui::A11yNode> {
    self.ui.a11y_nodes()
}

fn a11y_action(&mut self, action: A11yAction, window: &mut WindowHandle<'_>) {
    if let Some(message) = self.ui.apply_a11y(&action, window) {
        self.apply(message, window);
    }
    window.a11y_focus(self.ui.focus_id());
}
```

`apply_a11y` turns a screen-reader request into the same widget input a user
would produce, so there is no separate code path.

## Notes and limits

- The UIA provider is Windows-only (`#[cfg(windows)]`, `platform::uia`). The
  model and role vocabulary compile everywhere; only the provider and
  `WindowHandle::a11y_*` are Windows-gated.
- `Host::a11y_nodes` defaults to an empty tree; a host that does not implement it
  exposes nothing. `Ui::a11y_nodes` is the usual body.
- Call `WindowHandle::a11y_refresh` after a page rebuild or layout change so the
  next screen-reader request rebuilds the snapshot. `a11y_focus(Some(id))`
  announces focus after the refresh; `a11y_focus(None)` clears it.
- Announcements (`a11y_announce`) are transient and not tied to an element;
  `Assertive` interrupts, `Polite` replaces older status text.
- Only visible list rows are published, so a 600-row queue is cheap. A
  scrolled-out row keeps its rectangle and sets `offscreen`.
- Pattern methods never touch the application directly: they post an
  `A11yRequest` back to the window and actions run on the UI thread through the
  widget input path.

## See also

- `scrolling.md` (Scroll pattern state on a panel)
- `menus.md` (Menu/MenuItem roles)
- `../widgets/controls/slider.md`
- `../widgets/data/list-view.md`
- `../overview/architecture.md`
