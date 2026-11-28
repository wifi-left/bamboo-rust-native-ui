# Guides

> Task-oriented walkthroughs for theming, layout documents, performance,
> debugging, testing, feature selection and accessibility.

Every page starts from a working window (see the
[quickstart](../quickstart/index.md)) and ends with a result you can see or a
command you can run. Reference tables live in [reference](../reference/index.md);
API contracts live in [api](../api/theme-and-palette.md).

## Choose a guide

| Guide | Use it when | You will end with |
|---|---|---|
| [Theming](theming.md) | You need accent colors, light/dark mode, font settings, list density, corner radii, or a `.buitheme` document. | A settings struct installed at startup and switchable at runtime, plus a reloadable theme file. |
| [XML layout authoring](xml-layout-authoring.md) | You want rectangles, properties, styles and initial content in `.buixml` instead of Rust. | A document applied in `resize`, with variants, constants and a hot-reload loop. |
| [Performance](performance.md) | You want to keep idle paints at zero, invalidate the right rectangles, and understand the caches. | A page that paints only changed regions and an animation timer that stops. |
| [Debugging](debugging.md) | A layout is rejected, a widget does not paint, UIA looks wrong, or you want the debug snapshot. | A diagnosed failure with a file/line/code, or a working `--a11y-dump`. |
| [Testing](testing.md) | You are adding a widget, changing the parser, or wiring CI around the workspace test matrix. | A passing feature matrix and a unit test that needs no window. |
| [Feature flags](feature-flags.md) | You are choosing between the default build, `layout-xml`, `layout-compiled`, `theme-xml` and `debug-form`. | A dependency line and a build command for each mode. |
| [Accessibility](accessibility.md) | You are authoring `A11yInfo`/`Narration`, wiring UIA, or checking the tree with `--a11y-dump`. | Widgets with roles and names, a host that answers UIA, and a verified tree. |
| [FAQ](faq.md) | You want the short answer on egui, DPI, fonts, Chinese text, images, hot reload or licensing. | The decision, with a link to the detailed page. |

## Reading paths

### Path 1: first application

1. [01 — Create project](../quickstart/01-create-project.md)
2. [02 — Add the crate](../quickstart/02-add-the-crate.md)
3. [03 — First window](../quickstart/03-first-window.md)
4. [Theming](theming.md) — pick an accent and dark mode
5. [XML layout authoring](xml-layout-authoring.md) — move rectangles out of Rust
6. [Performance](performance.md) — keep the idle window quiet

### Path 2: layout-heavy application

1. [05 — Dynamic XML](../quickstart/05-xml-dynamic.md)
2. [XML layout authoring](xml-layout-authoring.md)
3. [XML elements](../reference/xml-elements.md) — look up an attribute
4. [Palette tokens](../reference/palette-tokens.md) — look up a color name
5. [06 — Compiled XML](../quickstart/06-xml-compiled.md) — ship without a parser
6. [Feature flags](feature-flags.md) — choose the release combination

### Path 3: framework contributor

1. [Architecture](../overview/architecture.md)
2. [Design principles](../overview/design-principles.md)
3. [The widget trait](../api/widget-trait.md)
4. [04 — Custom widgets](../quickstart/04-custom-widgets.md)
5. [Testing](testing.md)
6. [Debugging](debugging.md)

## Common tasks

| Task | Start here |
|---|---|
| Open a window and paint one label | [03 — First window](../quickstart/03-first-window.md) |
| Implement a custom control | [04 — Custom widgets](../quickstart/04-custom-widgets.md) |
| Set an accent, font size or dark mode | [Theming](theming.md) |
| Follow the Windows light/dark setting | [Theming](theming.md) — resolve the mode yourself, then `set_theme` |
| Move layout into a file | [XML layout authoring](xml-layout-authoring.md) |
| Reload the layout while the app runs | [XML layout authoring](xml-layout-authoring.md) |
| Ship a release without an XML parser | [06 — Compiled XML](../quickstart/06-xml-compiled.md) |
| Look up a `background=` color name | [Palette tokens](../reference/palette-tokens.md) |
| Look up a `<widget>` attribute | [XML elements](../reference/xml-elements.md) |
| Put a long page in a scroll view | [Scrolling](../api/scrolling.md) |
| Stop repainting the whole window | [Performance](performance.md) |
| Find out why a layout was rejected | [Debugging](debugging.md) |
| Check what a screen reader sees | [Accessibility](accessibility.md) and `--a11y-dump` |
| Add a widget test that needs no window | [Testing](testing.md) |
| Choose features for development vs release | [Feature flags](feature-flags.md) |

## Conventions used here

- **DIPs** means device-independent pixels. Every rectangle, radius and font
  size in the public API is in DIPs; the platform multiplies by `dpi / 96`.
- A **widget id** is a `WidgetId` built from a `&'static str`. A document may
  only address ids the Rust side created (unless it creates them through a
  `WidgetRegistry`; see
  [XML layout authoring](xml-layout-authoring.md)).
- A **layout document** is a `.buixml` file. The runtime reader is
  `feature = "layout-xml"`; the build-time form is
  `feature = "layout-compiled"`. A **theme document** is a `.buitheme` file
  behind `feature = "theme-xml"`.
- Feature gates are written `feature = "layout-xml"` on first use, then only by
  name.
- The framework crate is `bamboo-native-ui`; the container is `Ui<M>`; the
  application message type is `M`.

## Features at a glance

| Feature | Default | Adds |
|---|---|---|
| `gdi` | yes | The Windows GDI backend and Direct2D renderer options. |
| `layout-xml` | no | Runtime `.buixml` parsing and `Ui::apply_layout`. |
| `theme-xml` | no | Runtime `.buitheme` parsing and `ThemeDocumentExt`. |
| `layout-compiled` | no | `Ui::apply_compiled` for generated Rust; no parser. |
| `debug-form` | no | `Ui::debug_snapshot` and paint statistics; implies `layout-xml`. |

The trade-offs and build commands are in [Feature flags](feature-flags.md).

## Where each kind of page lives

| Folder | Answers |
|---|---|
| [quickstart](../quickstart/index.md) | "Show me a complete program, step by step." |
| [guides](index.md) | "How do I accomplish this task, and what does it cost?" |
| [api](../api/theme-and-palette.md) | "What is the exact signature and behaviour of this module?" |
| [widgets](../widgets/custom/custom-widget.md) | "What does this control do, and how do I configure it?" |
| [reference](../reference/index.md) | "What is the exact spelling of this token, attribute or key?" |
| [overview](../overview/introduction.md) | "Should I use this framework, and how is it built?" |

## See also

- [Introduction](../overview/introduction.md)
- [Architecture](../overview/architecture.md)
- [Design principles](../overview/design-principles.md)
- [Quickstart](../quickstart/index.md)
- [Reference](../reference/index.md)
- [FAQ](faq.md)
