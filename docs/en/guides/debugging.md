# Debugging

> Turn on the framework's stderr diagnostics, run the showcase probes, inspect a
> rejected layout, and read the opt-in debug snapshot.

## What is available

| Tool | What it answers |
|---|---|
| `--self-test` | Does the window paint and the host path work? |
| `--a11y-dump` | Does a real UI Automation client see the expected tree? |
| `BAMBOO_UI_DEBUG` | What is the window doing at the platform level? |
| `BAMBOO_UI_PAINT_LOG` | Which widgets painted, and which were slow? |
| `debug-form` snapshot | What is the effective state of every widget and layout entry? |
| `LayoutFailure` | Why was a layout document rejected? |

The showcase probes are the fastest starting point because they need no code
changes. The env variables work for any application built on the framework.

## Steps

### 1. Run the showcase probes

```powershell
cargo run -p bamboo-native-ui-showcase -- --self-test
cargo run -p bamboo-native-ui-showcase -- --a11y-dump
cargo run -p bamboo-native-ui-showcase -- --light
cargo run -p bamboo-native-ui-showcase -- --renderer d2d-software
cargo run -p bamboo-native-ui-showcase -- --bench 2
```

- `--self-test` creates the window, paints it, drives a native edit session so
  the host text path is exercised, and exits with status 0. A non-zero exit
  means a probe failed; the reason is printed.
- `--a11y-dump` starts the window and runs a real UIA **client** on a worker
  thread. It asks the window for its provider, walks the control view and
  prints names, roles, patterns, values and selection. It exits 0 only when the
  probe succeeded.
- `--light` starts in light mode (otherwise the showcase starts dark).
- `--renderer gdi|d2d-software|d2d-hardware` sets `BAMBOO_UI_RENDERER` before
  the window is created.
- `--bench N` runs for N seconds and prints frames, ticks, invalidations,
  average paint time, private bytes and the renderer that was actually active.

The showcase menu bar also has a debug menu built on the `debug-form` API
(snapshot, widget tree, frame stats, diagnostics, reload/save/reset). That is
showcase code, not a library inspector.

### 2. Turn on `BAMBOO_UI_DEBUG`

`BAMBOO_UI_DEBUG` is read with `std::env::var_os`, so any non-empty value
enables it. Diagnostics go to stderr:

```powershell
$env:BAMBOO_UI_DEBUG=1
cargo run -p bamboo-native-ui-showcase
```

Typical messages:

- whether the uxtheme dark-mode exports are available (menus follow the page);
- `WM_SIZE` with client size and DPI;
- exposure and occlusion decisions (client covered by another window, exposed
  area grew, full repaint);
- `WM_GETOBJECT` with the node count when a screen reader asks for the tree;
- native editor and Direct2D fallback failures.

A Direct2D mode that cannot start logs here and switches to GDI permanently;
`WindowHandle::renderer()` reports the active one afterwards.

### 3. Turn on the paint log

```powershell
$env:BAMBOO_UI_PAINT_LOG=1
cargo run -p bamboo-native-ui-showcase
```

`Ui::paint` prints any widget whose paint took longer than 1.5 ms, with its id
and rectangle, when `debug-form` is enabled. The showcase adds a per-frame line
(dirty count, union rectangle, widget count, microseconds). This is a
diagnostic only; it does not change what is painted.

### 4. Enable the debug snapshot

```toml
[dependencies]
bamboo-native-ui = { path = "../native-ui/crates/native-ui", features = ["debug-form"] }
```

`debug-form` implies `layout-xml`; the default build and its binaries stay free
of debug code. The snapshot is a value: `Ui::debug_snapshot()` walks the widget
tree once, resolves effective state and returns owned data. Nothing is mutated.

```rust
#[cfg(feature = "debug-form")]
fn report(ui: &bamboo_native_ui::Ui<()>) {
    let snapshot = ui.debug_snapshot();
    if let Some(layout) = &snapshot.layout {
        eprintln!(
            "variant={} client={:?} entries={} warnings={}",
            layout.variant,
            layout.client,
            layout.entries.len(),
            layout.warnings.len()
        );
    }
    for widget in &snapshot.widgets {
        eprintln!(
            "{} visible={} enabled={} focused={} a11y={:?}",
            widget.id.name(),
            widget.visible,
            widget.enabled,
            widget.focused,
            widget.a11y.as_ref().map(|info| info.name.as_str())
        );
    }
    eprintln!("pending damage: {:?}", snapshot.pending_damage);
    eprintln!("frames={} last={}us", snapshot.paint.frames, snapshot.paint.last_frame_micros);
}
```

`DebugSnapshot` carries the installed layout (entries, claims, scrolls,
warnings), every widget's effective visibility/enabled/focus/read-only state,
its narration-resolved `A11yInfo` and skin vocabulary, interaction ids, pending
and last damage, paint statistics and a 128-entry diagnostics ring.

Related calls:

| Call | Use |
|---|---|
| `Ui::debug_event("note", text)` | Append an application note to the ring. |
| `Ui::clear_debug_log()` | Empty the ring. |
| `Ui::hover_id()` | The widget under the pointer. |
| `Ui::installed_layout()` | The applied layout record (`AppliedLayout`). |
| `snapshot.widget_types` | Registered creation types, in registration order. |
| `snapshot.has_commands` | Whether the registry carries a command table. |

`PaintStats::per_widget` is sorted by cumulative microseconds, longest first, so
the expensive widget is the first row.

### 5. Read a rejected layout

`apply_layout` and `apply_compiled` return `Result<LayoutReport, LayoutFailure>`.
An error means **nothing was applied**; the previous layout is still active.

```rust
use bamboo_native_ui::{Host, Size, WindowHandle};

impl Host for App {
    fn resize(&mut self, size: Size, _dpi: f32, window: &mut WindowHandle<'_>) {
        let palette = *window.palette();
        match self.ui.apply_layout(&self.layout, &palette, size, window) {
            Ok(report) => {
                if !report.warnings.is_empty() {
                    eprintln!("layout warnings: {:?}", report.warnings);
                }
            }
            Err(failure) => {
                eprintln!("layout failed: {failure}");
                for diagnostic in &failure.diagnostics {
                    eprintln!(
                        "{}:{} {} {}",
                        diagnostic.line, diagnostic.column, diagnostic.code, diagnostic.message
                    );
                }
            }
        }
        window.invalidate_all();
    }
}
```

Every `LayoutDiagnostic` has `severity` (`Error`/`Warning`), a stable `code`, a
1-based `line` and `column`, a best-effort `path` (`layout/wide/save`) and a
`message`. `LayoutFailure::first()` returns the first error (or the first
diagnostic when only warnings exist). The common codes are listed in
[XML elements](../reference/xml-elements.md); the fixes are usually:

| Code | Fix |
|---|---|
| `xml.geom.underdetermined` | Give exactly two horizontal and two vertical values. |
| `xml.geom.overdetermined` | Remove one of `x`/`right`/`w` (or the vertical equivalent). |
| `xml.layout.base` | Keep one variant with `min-width="0" min-height="0"`. |
| `xml.id.unknown-widget` | Create the widget in Rust first, or add `type` and a registry. |
| `xml.attr.value` | Check bool/number/role/cursor/color spelling at the reported line. |
| `xml.content.unsupported` | The widget does not publish that content capability (warning). |
| `xml.skin.unknown` | The widget did not declare that local attribute or child. |

### 6. Inspect UI Automation

`--a11y-dump` is the built-in check. It validates that the provider answers a
real client, so it catches a tree that is empty, a missing pattern, a wrong
role or a name that never reaches UIA.

For interactive inspection, use the Windows SDK tools (Inspect,
Accessibility Insights for Windows) against a running showcase or your own
application. The framework's own provider is `#[cfg(windows)]`; the portable
model is `bamboo_native_ui::a11y`.

Two host-side mistakes make the tree empty:

- `Host::a11y_nodes` is not implemented (the default returns an empty vector);
- `Host::a11y_action` is not implemented, so a screen reader can read the tree
  but cannot invoke or toggle anything.

`WindowHandle::a11y_refresh()` rebuilds the snapshot after a page rebuild;
`a11y_focus(Some(id))` announces the focus change.

### 7. Use the application log

The crate depends on `tracing` and emits a few `tracing::warn!` events (for
example a native tooltip window failure). The framework installs no subscriber,
so install one in your application if you want those events. The library's own
debug output (`BAMBOO_UI_DEBUG`, paint log) uses stderr and works without a
subscriber.

For application-level diagnostics, prefer `Ui::debug_event` when `debug-form`
is on: the entries land in the same ring as layout applies and warnings, and
the debug menu can show them together.

## Common mistakes

- **`--a11y-dump` exits non-zero** — the probe failed, not the window. Read the
  printed error; a missing `Host::a11y_nodes` is the usual cause.
- **No `BAMBOO_UI_DEBUG` output** — the variable must be set in the process
  that creates the window; setting it in the IDE's test runner may not reach
  the built binary.
- **The snapshot is empty** — `debug-form` is not enabled, or the application
  calls `debug_snapshot` on an empty `Ui`.
- **A layout edit is ignored** — `LayoutDocument::parse` succeeded but the
  generation did not change (comment/formatting only), or the apply failed and
  the old document is still installed.
- **The paint log names the same widget every frame** — it is dirty every
  frame; find the setter or invalidation that runs in `timer`.
- **Renderer falls back to GDI** — read the `BAMBOO_UI_DEBUG` line at startup;
  the driver or the Direct2D target could not be created.

## See also

- [Performance](performance.md)
- [Testing](testing.md)
- [Accessibility](accessibility.md)
- [Host and window](../api/host-and-window.md)
- [XML layout authoring](xml-layout-authoring.md)
- [XML elements](../reference/xml-elements.md)
