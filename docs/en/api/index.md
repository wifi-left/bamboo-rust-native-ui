# API reference

The public API is grouped by topic. Each page is a lookup reference: the real
signatures, a short example, and the limits that matter. Start with the
[Ui container](ui-container.md) and the [widget trait](widget-trait.md) if you
are new; the rest are capabilities you reach for as needed.

Feature gates are noted on each page. The default build enables `gdi` only;
`layout-xml`, `layout-compiled`, `theme-xml` and `debug-form` are opt-in.

## Foundations

| Page | Summary |
|---|---|
| [Geometry and color](geometry-and-color.md) | DIP rectangles, points, sizes, insets, corners, damage merging, and platform-neutral colors. |
| [Theme and palette](theme-and-palette.md) | Theme settings, the resolved palette, accent presets, list density, and named radii. |
| [The Ui container](ui-container.md) | The retained widget tree: construction, routing, bindings, scrolling, damage, menus and accessibility. |
| [The widget trait](widget-trait.md) | The `Widget<M>` contract, input/paint rules, state types and the optional skin protocol. |
| [Events and input](events-and-input.md) | Normalized events, key codes, buttons and modifiers, and how a host routes them. |
| [Painting and Canvas](painting-and-canvas.md) | Theme-aware painting, the drawing backend trait, text styles and renderer selection types. |
| [Host and window](host-and-window.md) | Windows host callbacks, the paint context, window builder/handle and renderer selection. |
| [Layout helpers](layout-helpers.md) | Stack cursors, padding, weighted splits and grids for `Host::resize`. |

## Capabilities

| Page | Summary |
|---|---|
| [Images](images.md) | Register application-owned RGBA pixels and reference them by `ImageId`. |
| [Scrolling](scrolling.md) | Scrollable viewports over ordinary widgets and the shared scrollbar model. |
| [Accessibility](accessibility.md) | Author `A11yInfo` and expose the widget tree through UI Automation. |
| [Dialogs](dialogs.md) | Native message, file, font and color dialogs with asynchronous results. |
| [Menus](menus.md) | Declarative popup menus, the native menu bar and the menu event hook. |
| [Tray icons and notifications](tray-and-notifications.md) | Notification-area icons and system toasts with balloon fallback. |
| [Hotkeys](hotkeys.md) | Parse, register and capture global or per-window keyboard chords. |
| [Drag and drop](drag-and-drop.md) | System drag and drop of text and file lists. |
| [Gamepad](gamepad.md) | Map an XInput controller to keyboard navigation and the soft keyboard. |

## Layout documents, registry and diagnostics

| Page | Summary |
|---|---|
| [Layout documents (`.buixml`)](layout-documents.md) | Parsed model, variants/selection, geometry forms, expression grammar, constants and diagnostics. |
| [Layout codegen and compiled layouts](layout-codegen.md) | Codegen CLI/library, the generated `LAYOUT`/`install` module, `layout_xml!`, and `apply_compiled`. |
| [Theme documents (`.buitheme`)](theme-documents.md) | `ThemeDocument`, palette derivation, `set_palette_token` and the theme-document syntax. |
| [Widget registry and creation](widget-registry-and-creation.md) | `WidgetRegistry`/factory types, `type`/`command`/`parent`, `STANDARD_TYPES`, and registering `x-*` types. |
| [Properties and narration](props-and-narration.md) | `WidgetProps` builders, `FontSpec`, `Narration`, `ContentCaps`, and `Ui::add_with`. |
| [Diagnostics and debug](diagnostics-and-debug.md) | Diagnostic types/codes, `LayoutReport`/`LayoutFailure`, `debug-form` snapshots and `BAMBOO_UI_DEBUG`. |

## See also

- [Widget catalog](../widgets/index.md)
- [Feature flags](../guides/feature-flags.md)
- [Architecture](../overview/architecture.md)
