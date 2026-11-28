# 03 — First window

> Write a complete `Host` implementation: one label, one button, a paint pass
> and an event loop.

## What you will build

A 520×320 dark window titled "Bamboo Hello". It shows a label and a button;
each click updates the label to `Clicked N time(s)`. The program is the base
every later quickstart modifies.

## New terms on this page

- **`Host`** — the trait (interface) your application implements; the window
  calls it for paint, resize and input. Only `paint` is required.
- **trait** — Rust's interface: a named set of methods a type promises to
  provide.
- **`impl`** — a block that adds methods to a type (`impl App`) or implements a
  trait for it (`impl Host for App`).
- **`Ui<M>`** — the container that owns every widget of one page and routes
  input to them; `M` is your message type.
- **widget** — one element with a stable id, a rectangle, paint code and an
  input handler.
- **message (`M`)** — a value a widget returns to report what happened; `enum`
  means a fixed set of such values.
- **`Option<M>`** — zero or one message: `Some(Msg::Greet)` when something
  happened, `None` when nothing did.
- **event** — a normalized input or window occurrence (pointer move, click, key
  press) delivered to `Host`; an event describes input, a message describes
  intent.
- **DIP** — device-independent pixel: 1/96 inch. Rectangles are written in DIPs;
  the host applies the monitor's DPI scale.
- **paint** — the drawing pass. `Host::paint` draws the damaged pieces and asks
  `Ui` to draw the widgets on them.
- **dirty rectangle** — a rectangle whose pixels changed and must be repainted;
  the host merges them and copies only those pieces.
- **back buffer** — an off-screen surface that keeps the page between paints, so
  the next paint only needs to redraw what changed.
- **invalidate / damage** — mark a rectangle as needing a repaint. `Ui` setters
  record damage; `flush_damage` turns it into an invalidation.
- **`WindowBuilder`** — creates the native window, owns the back buffer and runs
  the message loop.
- **`WindowHandle`** — the per-callback handle used to invalidate the window and
  perform window operations.
- **`PaintCtx`** — the paint context: canvas, painter, dirty list, size and
  DPI.

## Prerequisites

- Pages [01](01-create-project.md) and [02](02-add-the-crate.md) are done.
- `bamboo-native-ui` is linked with its default features.

## Steps

### 1. Understand the pieces

- **`WindowBuilder`** creates the native window, runs the message loop and owns
  the back buffer. `WindowBuilder::new(host)` takes your `Host` by value; the
  chain ends in `run()`, which blocks until the window closes. Windows is
  required: the window host is compiled only on Windows.
- **`Host`** is the application side of the window. It is a trait, which in Rust
  means an interface: a set of methods with signatures. You write
  `impl Host for App` and fill in the callbacks. Only `paint` is required; the
  rest (`resize`, `event`, `started`, ...) have empty defaults. `resize` runs
  when the window is first shown and on every size change, and `event` runs for
  each input or window event.
- **`Ui<M>`** is the container that owns the widgets. `M` is your message type,
  defined here as an `enum` — a fixed set of values. A widget's input handler
  returns `Option<M>`: `Some(message)` to report one thing, or `None` to report
  nothing. The container queues the messages until your code drains them.
- **Events are not messages.** An event is what the user or Windows did
  (`PointerDown` at a position). A message is what a widget decided that means
  (`Msg::Greet`). `Host::event` receives events, forwards them to `Ui`, and then
  drains the messages the widgets produced.

The framework normalizes Win32 messages into `Event` values, so your code never
sees a raw `WM_*` message. Rectangles and pointer positions are in DIPs
(device-independent pixels), and the host applies the monitor's DPI scale when
it paints.

### 2. Replace `src/main.rs`

Open the crate's source, `bamboo-hello/bamboo-hello/src/main.rs` (not a file in
the outer workspace folder), and replace the generated `Hello, world!` program
with this:

```rust
// The two built-in widgets this page uses.
use bamboo_native_ui::widgets::{Button, Label};
// Window host, paint context, geometry, theme and widget identity.
use bamboo_native_ui::{
    Event, Host, PaintCtx, Rect, Size, ThemeSettings, Ui, WidgetId, WindowBuilder, WindowHandle,
};

// Every message this application can receive. Widgets produce these values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Msg {
    Greet, // Sent when the button is clicked.
}

struct App {
    ui: Ui<Msg>, // Owns the widgets; Msg is this app's message type.
    clicks: u32, // Application state, updated when a message arrives.
}

impl App {
    // Build the widget tree once; rectangles are assigned later in resize.
    fn new() -> Self {
        let mut ui = Ui::new();
        // add() takes ownership of the widget and indexes it by its id.
        ui.add(Label::new("status", "No clicks yet"));
        // The third argument is the message the button emits on click.
        ui.add(Button::new("greet", "Say hello", Msg::Greet));
        Self { ui, clicks: 0 }
    }

    // Turn one message into application state and a widget update.
    fn apply(&mut self, message: Msg) {
        match message {
            Msg::Greet => {
                self.clicks += 1;
                // The setter records damage; pixels change at the next paint.
                self.ui.set_label(
                    WidgetId::new("status"),
                    format!("Clicked {} time(s)", self.clicks),
                );
            }
        }
    }
}

// Implement the framework's interface: the window calls these methods.
impl Host for App {
    // Required: draw the damaged parts of the page.
    fn paint(&mut self, ctx: &mut PaintCtx<'_>) {
        // The back buffer keeps the page between paints: fill exactly the
        // dirty pieces, then let the container repaint the widgets on them.
        let background = ctx.painter.palette().background;
        for rect in ctx.dirty {
            // Each piece is a merged DIP rectangle that changed.
            ctx.canvas.fill_rect(*rect, background);
        }
        // Repaint the widgets that intersect the dirty pieces.
        self.ui.paint(ctx);
    }

    // Optional: called on the first show and on every size change.
    fn resize(&mut self, size: Size, _dpi: f32, window: &mut WindowHandle<'_>) {
        // The application owns layout: one rectangle per widget id.
        self.ui.set_rect(
            WidgetId::new("status"),
            // Full-width status line with 24 DIP margins.
            Rect::from_xywh(24.0, 24.0, size.width - 48.0, 28.0),
        );
        self.ui.set_rect(
            WidgetId::new("greet"),
            // Fixed-size button below the label.
            Rect::from_xywh(24.0, 64.0, 140.0, 36.0),
        );
        // The buffer still holds the old layout, so repaint everything once.
        window.invalidate_all();
    }

    // Optional: called for every normalized input or window event.
    fn event(&mut self, event: &Event, window: &mut WindowHandle<'_>) -> bool {
        // Forward what the widgets need; each call returns true if Ui used it.
        let handled = match *event {
            Event::PointerMove { pos, .. } => self.ui.pointer_move(pos, window),
            Event::PointerDown {
                pos,
                button,
                modifiers,
            } => self.ui.pointer_down_with(pos, button, modifiers, window),
            Event::PointerUp {
                pos,
                button,
                modifiers,
            } => self.ui.pointer_up_with(pos, button, modifiers, window),
            Event::Key(key) => self.ui.key(key, window),
            _ => false, // Events this page does not need.
        };
        // Widgets queue messages; take every one and apply it.
        for message in self.ui.drain_messages() {
            self.apply(message);
        }
        // A burst of setters becomes one repaint request.
        self.ui.flush_damage(window);
        handled
    }
}

// The process entry point; returning ExitCode reports success or failure.
fn main() -> std::process::ExitCode {
    // Build the native window, then run its message loop on this thread.
    let result = WindowBuilder::new(App::new())
        .title("Bamboo Hello")
        .size(520.0, 320.0) // Client area in DIPs.
        .min_size(320.0, 200.0)
        .theme(ThemeSettings::default(), true) // true selects dark.
        .run(); // Blocks until the window closes.
    match result {
        Ok(_) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            // Errors go to the terminal, not to a dialog.
            eprintln!("bamboo: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
```

**What just happened.** You replaced the generated program with an application
that builds two widgets, implements the `Host` callbacks and opens a window.
Nothing has run yet. The Rust compiler will check the whole file on the next
command; a typo in an id is not a compile error, so the ids (`"status"`,
`"greet"`) matter at runtime too.

### 3. What each part does

- **`paint`** receives a borrowed dirty list (`ctx.dirty`): the pieces that
  changed since the last paint, in DIPs. The back buffer still holds the rest of
  the page, so the method fills the background under each piece and calls
  `ui.paint(ctx)`, which paints every widget that intersects a dirty piece.
  Painting outside the pieces is wasted work: the host never copies those
  pixels. This is why the same code works after a one-pixel hover and after a
  full resize — the list, not the widget code, decides how much is drawn.
- **`resize`** is the only place layout is recomputed for this program. It runs
  when the window is first shown and on every size change. It assigns each
  widget a rectangle — the application owns layout; there is no measure pass —
  and invalidates the whole client once because the back buffer still contains
  the old layout. Sizes are in DIPs; the host applies the DPI scale.
- **`event`** translates the framework's normalized events into `Ui` calls.
  Each call returns whether `Ui` used the event. The pointer down/up pair on the
  button makes the button's input handler return `Some(Msg::Greet)`, and the
  container queues that message. `drain_messages` hands the queued messages to
  the application.
- **`apply`** updates application state and then calls `Ui::set_label`, which
  records the label's rectangle as pending damage; it does not paint. At the end
  of `event`, `flush_damage(window)` turns all pending damage into one
  invalidation, so a burst of setters still causes one repaint request.
- **`theme`** takes `ThemeSettings` plus the dark/light decision (`true` here).
- **`main`** returns `std::process::ExitCode`, so the shell can tell whether the
  program closed normally. The only error this page can report is a failure to
  create or run the window, printed to the terminal.

#### One click, end to end

1. Windows delivers a mouse message; the host normalizes it into
   `Event::PointerDown` and later `Event::PointerUp`.
2. `Host::event` forwards each event to `ui.pointer_down_with` /
   `pointer_up_with`.
3. `Ui` updates the button's pressed state and calls its input handler, which
   returns `Some(Msg::Greet)`.
4. `drain_messages` yields `Msg::Greet`; `apply` increments `clicks` and calls
   `set_label`, which records damage for the label rectangle.
5. `flush_damage` invalidates that rectangle. The host merges the damage and
   later calls `paint` with it in `ctx.dirty`.
6. `paint` fills the piece with the background and `ui.paint` draws the label
   over it.

### 4. Run it

Run this in the outer `bamboo-hello` folder, the one with the workspace
`Cargo.toml`:

```powershell
# Build if needed, then open the window. The command blocks until you close it.
cargo run
```

A window appears with the title "Bamboo Hello". Click the button: the label
changes to `Clicked 1 time(s)`, then `Clicked 2 time(s)`, and so on. Closing the
window ends the program and returns your prompt (together with any error text
the program printed).

The program owns the terminal while it runs. Window content is not printed
there; only diagnostics and errors from `eprintln!` appear in the terminal.

To watch the paint path, set the debug variable for this terminal session and
run from the same outer folder:

```powershell
# Turn on paint/invalidation diagnostics (PowerShell syntax).
$env:BAMBOO_UI_DEBUG = "1"
# No rebuild is needed for the variable; start the program again.
cargo run
```

The host logs paint and invalidation diagnostics to stderr. With the window
idle, no paint lines appear; hovering or clicking produces only the affected
rectangles. In `cmd.exe`, use `set BAMBOO_UI_DEBUG=1` instead; in either shell
the value lasts until you close that terminal.

## Expected result

- A 520×320 dark window with a label and a `Say hello` button.
- Clicking increments the counter in the label.
- The window does not repaint while idle; with `BAMBOO_UI_DEBUG=1`, the terminal
  stays quiet until you hover or click.
- Closing the window returns the prompt, normally with exit code 0.

## Troubleshooting

- **The window is blank or gray** — `resize` did not run, or the rectangles were
  never set. Check that `WindowBuilder::new(App::new())` receives your app and
  that `resize` calls `ui.set_rect` for every id. The ids passed to `set_rect`
  must match the ids given to `Label::new` and `Button::new` exactly.
- **The button does not react** — pointer events are not reaching `Ui`. Make
  sure `event` forwards `PointerDown`/`PointerUp` with
  `pointer_down_with`/`pointer_up_with` and that the button's rectangle is
  non-empty. Click inside the 140×36 rectangle at (24, 64); a click in the
  label area does nothing.
- **The label never changes after a click** — the message was drained but the
  change was not flushed. Call `ui.flush_damage(window)` after handling messages.
  Also check the `Msg::Greet` arm still calls `set_label` with the id
  `"status"`.
- **`run` returns an error immediately** — `WindowBuilder::run` must be called on
  the main thread; Win32 requires the message loop there. Do not spawn the
  window from a worker thread.
- **`error: could not find \`Cargo.toml\``** — you are in the wrong folder. Run
  `cargo run` from the outer `bamboo-hello` folder that contains the workspace
  manifest.
- **`cargo` is not recognized** — Rust is not installed or this terminal
  predates the installation; install from <https://rustup.rs> and open a new
  PowerShell window.
- **The link step fails with "link.exe not found"** — the C++ build tools or
  Windows SDK are missing. Install the **Desktop development with C++**
  workload and open a new terminal; page 01 has the details.
- **The window opens and closes immediately** — `run()` returned an error or the
  program panicked. Read the terminal that ran `cargo run`, including the
  `bamboo: ...` line. Starting the `.exe` from Explorer hides this output.
- **The old window still appears after an edit** — the editor has not saved
  `main.rs`, or you ran a different checkout. Save with Ctrl+S, stop the old
  program, and run `cargo run` again.
- **`unresolved import bamboo_native_ui`** — the framework dependency is missing
  or was added to the wrong manifest; finish [page 02](02-add-the-crate.md)
  first.
- **`$env:BAMBOO_UI_DEBUG` is not recognized** — you are in `cmd.exe`, not
  PowerShell. Use `set BAMBOO_UI_DEBUG=1` there. The variable must be set in the
  same terminal window before `cargo run`.
- **No paint diagnostics appear with the debug variable set** — confirm the
  variable is set in the terminal that runs the program, not in another window,
  and that the program was started from that terminal after setting it. Verify
  with `$env:BAMBOO_UI_DEBUG` in PowerShell.

## Next

[04 — Custom widgets](04-custom-widgets.md): paint your own component and emit a
message from it.
