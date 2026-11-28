# Diagnostics and debug

> Stable diagnostic codes, the apply report/failure pair, and the read-only `debug-form` snapshots.

## Overview

Diagnostics come in two layers. The parser and applier produce
`LayoutDiagnostic` values with stable string codes and a source position; an
apply either returns a `LayoutReport` (applied, maybe with warnings) or a
`LayoutFailure` (rejected, nothing changed). The `debug-form` feature adds
read-only snapshots of the live container — layout, widgets, damage, paint
timings and a bounded log ring — and the platform layer logs internals to
stderr when `BAMBOO_UI_DEBUG` is set.

Errors reject the whole document; warnings are reported and the rest still
applies. That split is the atomicity promise: a bad hot reload leaves the
previous layout on screen and hands back the reasons.

| Layer | Types | Feature |
|---|---|---|
| Parse / apply | `LayoutSeverity`, `LayoutDiagnostic`, `Diagnostics` | `layout-xml` (model types also in core) |
| Apply result | `LayoutReport`, `LayoutFailure`, `AppliedLayout` | `layout-xml` or `layout-compiled` |
| Debug snapshots | `DebugSnapshot` family, `Ui::debug_snapshot` | `debug-form` (implies `layout-xml`) |
| Process metric | `private_bytes` | Windows-only, `#[cfg(windows)]` |

## Key types

### `LayoutDiagnostic` and `LayoutSeverity`

```rust
pub enum LayoutSeverity { Error, Warning }

pub struct LayoutDiagnostic {
    pub severity: LayoutSeverity,
    pub code: &'static str,   // stable, testable (`xml.geom.negative`, …)
    pub line: usize,          // 1-based, points at the element
    pub column: usize,
    pub path: String,         // `layout/wide/action`, best effort
    pub message: String,
}

impl LayoutDiagnostic {
    pub fn error(code: &'static str, line: usize, column: usize,
                 path: impl Into<String>, message: impl Into<String>) -> Self;
    pub fn warning(code: &'static str, line: usize, column: usize,
                   path: impl Into<String>, message: impl Into<String>) -> Self;
    pub fn is_error(&self) -> bool;
}
```

`Display` prints `line:column: severity[code] (path): message`, so a diagnostic
can be logged or shown directly.

### `Diagnostics` sink

```rust
pub struct Diagnostics { /* private */ }

impl Diagnostics {
    pub fn new(limit: usize) -> Self;
    pub fn push(&mut self, diagnostic: LayoutDiagnostic);
    pub fn error(&mut self, code: &'static str, line: usize, column: usize,
                 path: impl Into<String>, message: impl Into<String>);
    pub fn warning(&mut self, code: &'static str, line: usize, column: usize,
                   path: impl Into<String>, message: impl Into<String>);
    pub fn has_errors(&self) -> bool;
    pub fn len(&self) -> usize;
    pub fn into_vec(self) -> Vec<LayoutDiagnostic>;
}
```

The parser and applier collect at most 100 diagnostics each; further ones are
dropped rather than growing without bound.

### `LayoutReport` and `LayoutFailure`

```rust
pub struct LayoutReport {
    pub generation: u64,
    pub variant_id: String,
    pub applied_entries: usize,
    pub content_writes: usize,
    pub skin_writes: usize,
    pub created: usize,
    pub warnings: Vec<LayoutDiagnostic>,
}

impl LayoutReport {
    pub fn summary(&self) -> String;   // one line, suitable for a status bar
}

pub struct LayoutFailure {
    pub diagnostics: Vec<LayoutDiagnostic>,
}

impl LayoutFailure {
    pub fn first(&self) -> Option<&LayoutDiagnostic>;
}
```

`summary()` names the variant, entry/content/skin counts and the first warning
(or a no-warning marker). `LayoutFailure` implements `Display` and
`std::error::Error`; `first()` returns the first error, falling back to the
first diagnostic when only warnings exist. `diagnostics` is sorted errors
first.

`Ui::installed_layout()` returns an `AppliedLayout` for inspection: generation,
client, variant id, resolved entries, claim ids, created widgets and warnings.

### Debug snapshots (`debug-form`)

```rust
impl<M> Ui<M> {
    #[cfg(feature = "debug-form")]
    pub fn debug_snapshot(&self) -> DebugSnapshot;
    #[cfg(feature = "debug-form")]
    pub fn debug_event(&mut self, kind: &'static str, text: impl Into<String>);
    #[cfg(feature = "debug-form")]
    pub fn clear_debug_log(&mut self);
}

pub struct DebugSnapshot {
    pub layout: Option<DebugLayout>,
    pub widgets: Vec<DebugWidget>,
    pub widget_types: Vec<&'static str>,
    pub has_commands: bool,
    pub focus: Option<WidgetId>,
    pub hover: Option<WidgetId>,
    pub press: Option<WidgetId>,
    pub pending_damage: Vec<Rect>,
    pub last_damage: Vec<Rect>,
    pub paint: PaintStats,
    pub log: Vec<DebugEvent>,
}
```

The family is owned data, safe to keep while the tree changes:

- `DebugLayout` — `generation`, `variant`, `client`, `content_writes`,
  `skin_writes`, `entries: Vec<DebugEntry>`, `claims: Vec<DebugClaim>`,
  `warnings: Vec<String>`, `scrolls: Vec<DebugScroll>`.
- `DebugEntry` — `id`, `rect`, `space` (`EntrySpace`).
- `DebugClaim` — `id`, `baseline` rectangle and the claimed field names
  (`visible`, `enabled`, `focusable`, `focus-order`, `cursor`, `props`,
  `corners`, `background`).
- `DebugWidget` — effective `visible`/`enabled`/`focusable`/`read_only`,
  interaction flags, `a11y: Option<A11yInfo>` with narration resolved,
  `skin_attrs` and `skin_children` vocabulary.
- `PaintStats` — cumulative frames/`widget_paints`, the last frame's
  `last_frame_micros`, and `per_widget` sorted by cost.
- `DebugEvent` — `sequence`, `kind` (`layout`, `warning`, `error`, `note`) and
  `text`; the ring keeps the most recent 128 entries.

The library paints no inspector; the application presents these values however
it likes (the showcase writes summaries into a dialog).

### Environment and process metrics

- `BAMBOO_UI_DEBUG` (any value) makes the platform layer log internal
  decisions to stderr: repaint/visibility changes, renderer start failures and
  GDI fallback, chrome resolution.
- `BAMBOO_UI_PAINT_LOG` makes `Ui::paint` print per-paint diagnostics (dirty
  count, widget count, microseconds; widgets slower than 1.5 ms with their
  rectangle).
- `bamboo_native_ui::diagnostics::private_bytes() -> Option<u64>` returns the
  process's private commit in bytes (`PagefileUsage`) on Windows and `None`
  elsewhere. `diagnostics` is a `#[cfg(windows)]` module.

## Example

```rust
use bamboo_native_ui::layout_xml::LayoutDocument;
use bamboo_native_ui::{Palette, Size, Ui, UiHost};

fn apply<M: 'static>(
    ui: &mut Ui<M>, text: &str, palette: &Palette,
    client: Size, sink: &mut impl UiHost,
) -> String {
    let document = LayoutDocument::parse(text).expect("parse before applying");
    match ui.apply_layout(&document, palette, client, sink) {
        Ok(report) => {
            if let Some(warning) = report.warnings.first() {
                eprintln!("layout warning: {warning}");
            }
            report.summary()
        }
        Err(failure) => {
            // Nothing changed: the previous layout is still installed.
            let first = failure.first().map(ToString::to_string).unwrap_or_default();
            format!("layout rejected: {first}")
        }
    }
}
```

Reading a snapshot:

```rust
#[cfg(feature = "debug-form")]
fn dump<M>(ui: &bamboo_native_ui::Ui<M>) {
    let snapshot = ui.debug_snapshot();
    if let Some(layout) = &snapshot.layout {
        for entry in &layout.entries {
            let _ = (entry.id, entry.rect, entry.space);
        }
        for claim in &layout.claims {
            let _ = (claim.id, claim.baseline, &claim.fields);
        }
    }
    for widget in &snapshot.widgets {
        let _ = (widget.id, widget.visible, widget.a11y.as_ref());
    }
    if let Some(event) = snapshot.log.last() {
        let _ = (event.sequence, event.kind, &event.text);
    }
}
```

## Notes and limits

- Feature gates: diagnostic types need `layout-xml` (the model/diag types also
  exist in `bamboo-native-ui-layout-core`); `LayoutReport`/`LayoutFailure`
  exist with either layout feature; `debug` and `Ui::debug_snapshot` need
  `debug-form`, which implies `layout-xml`.
- A failed apply mutates nothing, including created widgets and claims.
- Representative codes: `xml.syntax`, `xml.structure`, `xml.unknown-attribute`,
  `xml.attr.value`, `xml.const.*`, `xml.expr.*`, `xml.geom.negative`,
  `xml.geom.overdetermined`, `xml.geom.underdetermined`, `xml.id.unknown-widget`,
  `xml.content.conflict`, `xml.skin.unknown`, `xml.skin.value`,
  `xml.rust.unsupported`, `xml.type.unknown`, `xml.command.missing`. Warnings:
  `xml.style.unsupported`, `xml.content.unsupported`, `xml.content.session-open`,
  `xml.value.clamped`, `xml.value.zero-size`.
- Theme documents use their own `theme.*` codes; see
  [theme-documents.md](theme-documents.md).
- The debug ring records a `layout` event per apply (`variant=…`), the first
  error of a rejected apply, and application notes passed to `debug_event`.
- `private_bytes` is a Windows process metric; it does not include shared DLLs.

## See also

- [layout-documents.md](layout-documents.md) — model and parse errors
- [layout-codegen.md](layout-codegen.md) — the same report/failure types
- [host-and-window.md](host-and-window.md) — host callbacks and painting
- [Debugging guide](../guides/debugging.md)
- [05-xml-dynamic.md](../quickstart/05-xml-dynamic.md) — loading and reloading
