# Bamboo Native UI

A small, declarative native Windows UI framework: GDI + dirty-rectangle
rendering, a backend-abstracted painter, and no egui, wgpu or WebView. You
compute widget rectangles; the framework paints only the dirty rectangles and
routes normalized input. Idle means zero paints.

This documentation covers the `bamboo-native-ui` workspace: the framework
crate, the layout model/codegen, and the all-controls showcase.

## Start here

| If you want to… | Read |
|---|---|
| Understand what this is and why it exists | [Introduction](overview/introduction.md) |
| See how the pieces fit together | [Architecture](overview/architecture.md) |
| Learn the rendering and ownership contract | [Design principles](overview/design-principles.md) |
| Build something in ten minutes | [Quick start](quickstart/index.md) |
| Look up a widget | [Widget catalog](widgets/index.md) |
| Look up a type or function | [API reference](api/index.md) |
| Solve a specific problem | [Guides](guides/index.md) |

## The two-minute version

```rust
use bamboo_native_ui::{
    Color, Host, PaintCtx, Rect, Size, ThemeSettings, Ui, WidgetId, WindowBuilder, WindowHandle,
};

enum Msg { Play }

struct App { ui: Ui<Msg> }

impl App {
    fn new() -> Self {
        let mut ui = Ui::new();
        ui.add(bamboo_native_ui::widgets::Button::new("play", "Play", Msg::Play));
        Self { ui }
    }
}

impl Host for App {
    fn paint(&mut self, ctx: &mut PaintCtx<'_>) {
        for rect in ctx.dirty {
            ctx.canvas.fill_rect(*rect, ctx.painter.palette().background);
        }
        self.ui.paint(ctx);
    }

    fn resize(&mut self, size: Size, _dpi: f32, window: &mut WindowHandle<'_>) {
        self.ui.set_rect(
            WidgetId::new("play"),
            Rect::from_xywh((size.width - 120.0) / 2.0, (size.height - 36.0) / 2.0, 120.0, 36.0),
        );
        window.invalidate_all();
    }
}

fn main() -> windows::core::Result<()> {
    WindowBuilder::new(App::new())
        .title("Hello")
        .size(640.0, 400.0)
        .theme(ThemeSettings::default(), true)
        .run()
}
```

See [Your first window](quickstart/03-first-window.md) for the full walkthrough.

## Documentation map

### Overview

- [Introduction](overview/introduction.md) — what it is, status, audience.
- [Architecture](overview/architecture.md) — crates, layers, message and paint flow.
- [Design principles](overview/design-principles.md) — the dirty-rectangle contract and its trade-offs.

### Quick start

Core tutorials:

1. [Create a project](quickstart/01-create-project.md)
2. [Add the crate](quickstart/02-add-the-crate.md)
3. [Your first window](quickstart/03-first-window.md)
4. [Custom widgets](quickstart/04-custom-widgets.md)
5. [XML layouts at runtime](quickstart/05-xml-dynamic.md)
6. [Compiled XML layouts](quickstart/06-xml-compiled.md)

Practical tutorials:

7. [Layout and breakpoints](quickstart/07-layout-and-breakpoints.md)
8. [Lists, tables and trees](quickstart/08-lists-tables-trees.md)
9. [Menus, toolbars and status](quickstart/09-menus-toolbars-status.md)
10. [Dialogs, tray and hotkeys](quickstart/10-dialogs-tray-hotkeys.md)
11. [Theming and images](quickstart/11-theming-and-images.md)
12. [Text editing and IME](quickstart/12-text-editing-and-ime.md)
13. [Player UI](quickstart/13-player-ui.md)

### API reference

Foundations: [geometry and color](api/geometry-and-color.md),
[theme](api/theme-and-palette.md), [Ui](api/ui-container.md),
[widget trait](api/widget-trait.md), [events](api/events-and-input.md),
[painting](api/painting-and-canvas.md), [host/window](api/host-and-window.md),
[layout helpers](api/layout-helpers.md).

Capabilities: [images](api/images.md), [scrolling](api/scrolling.md),
[accessibility](api/accessibility.md), [dialogs](api/dialogs.md),
[menus](api/menus.md), [tray/notifications](api/tray-and-notifications.md),
[hotkeys](api/hotkeys.md), [drag and drop](api/drag-and-drop.md),
[gamepad](api/gamepad.md).

XML and tooling: [layout documents](api/layout-documents.md),
[codegen](api/layout-codegen.md), [theme documents](api/theme-documents.md),
[registry](api/widget-registry-and-creation.md),
[props/narration](api/props-and-narration.md),
[diagnostics](api/diagnostics-and-debug.md).

### Widgets

The [catalog](widgets/index.md) lists all 62 widgets by category. Every widget
page includes a Rust example, an XML example, a property table, an Events
section (messages, shared observers, widget-specific callbacks, `Ui` triggers),
and the exact XML definition rules.

### Guides and reference

- [Theming](guides/theming.md)
- [XML layout authoring](guides/xml-layout-authoring.md)
- [Performance](guides/performance.md)
- [Debugging](guides/debugging.md)
- [Testing](guides/testing.md)
- [Feature flags](guides/feature-flags.md)
- [Accessibility](guides/accessibility.md)
- [FAQ](guides/faq.md)
- [Palette tokens](reference/palette-tokens.md)
- [XML elements](reference/xml-elements.md)
- [Key codes and hotkeys](reference/key-codes-and-hotkeys.md)

## For AI assistants

A compact, self-contained context file lives at
[`ai/reference.md`](ai/reference.md), with a one-page
[widget lookup table](ai/widgets.md). The repository-level
[`llms.txt`](../llms.txt) is the entry point for tools that follow the
llmstxt.org convention.

## Status

The framework is under active development. Widgets, per-instance properties,
triggers, UI Automation, tooltips, menus, dialogs, tray and hotkeys are in;
the M6.5 component groups are complete; the M7 layout skin (`.buixml`) and
M7.8 theme documents (`.buitheme`) are implemented. The showcase exercises
every area. See the [introduction](overview/introduction.md) for details.

## License

Apache License 2.0. See [`LICENSE`](../../LICENSE) and [`NOTICE`](../../NOTICE).
