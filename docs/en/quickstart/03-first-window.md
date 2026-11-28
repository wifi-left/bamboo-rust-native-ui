# 03 — First window

> Write a complete `Host` implementation: one label, one button, a paint pass
> and an event loop.

## What you will build

A 520×320 dark window titled "Bamboo Hello". It shows a label and a button;
each click updates the label to `Clicked N time(s)`. The program is the base
every later quickstart modifies.

## Prerequisites

- Pages [01](01-create-project.md) and [02](02-add-the-crate.md) are done.
- `bamboo-native-ui` is linked with its default features.

## Steps

### 1. Understand the two types

- `WindowBuilder` creates the native window, runs the message loop and owns the
  back buffer. `WindowBuilder::new(host)` takes your `Host` by value.
- `Host` is the application side of the window. Only `paint` is required; the
  rest (`resize`, `event`, `started`, ...) have empty defaults.
- `Ui<M>` is the container that owns the widgets. `M` is your message type; a
  widget returns `Option<M>` from its input handler.

The framework normalizes Win32 messages into `Event` values. Your `event`
method forwards them to `Ui` and drains the messages the widgets produced.

### 2. Replace `src/main.rs`

```rust
use bamboo_native_ui::widgets::{Button, Label};
use bamboo_native_ui::{
    Event, Host, PaintCtx, Rect, Size, ThemeSettings, Ui, WidgetId, WindowBuilder, WindowHandle,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Msg {
    Greet,
}

struct App {
    ui: Ui<Msg>,
    clicks: u32,
}

impl App {
    fn new() -> Self {
        let mut ui = Ui::new();
        ui.add(Label::new("status", "No clicks yet"));
        ui.add(Button::new("greet", "Say hello", Msg::Greet));
        Self { ui, clicks: 0 }
    }

    fn apply(&mut self, message: Msg) {
        match message {
            Msg::Greet => {
                self.clicks += 1;
                self.ui.set_label(
                    WidgetId::new("status"),
                    format!("Clicked {} time(s)", self.clicks),
                );
            }
        }
    }
}

impl Host for App {
    fn paint(&mut self, ctx: &mut PaintCtx<'_>) {
        // The back buffer keeps the page between paints: fill exactly the
        // dirty pieces, then let the container repaint the widgets on them.
        let background = ctx.painter.palette().background;
        for rect in ctx.dirty {
            ctx.canvas.fill_rect(*rect, background);
        }
        self.ui.paint(ctx);
    }

    fn resize(&mut self, size: Size, _dpi: f32, window: &mut WindowHandle<'_>) {
        self.ui.set_rect(
            WidgetId::new("status"),
            Rect::from_xywh(24.0, 24.0, size.width - 48.0, 28.0),
        );
        self.ui.set_rect(
            WidgetId::new("greet"),
            Rect::from_xywh(24.0, 64.0, 140.0, 36.0),
        );
        window.invalidate_all();
    }

    fn event(&mut self, event: &Event, window: &mut WindowHandle<'_>) -> bool {
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
            _ => false,
        };
        for message in self.ui.drain_messages() {
            self.apply(message);
        }
        // A burst of setters becomes one repaint request.
        self.ui.flush_damage(window);
        handled
    }
}

fn main() -> std::process::ExitCode {
    let result = WindowBuilder::new(App::new())
        .title("Bamboo Hello")
        .size(520.0, 320.0)
        .min_size(320.0, 200.0)
        .theme(ThemeSettings::default(), true)
        .run();
    match result {
        Ok(_) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("bamboo: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
```

### 3. What each part does

- **`paint`** receives a borrowed dirty list (`ctx.dirty`). It fills the page
  background under each piece, then calls `ui.paint(ctx)`, which paints every
  widget that intersects a dirty piece. Painting outside the pieces is wasted
  work: the host never copies those pixels.
- **`resize`** is the only place layout is recomputed for this program. It sets
  each widget's rectangle and invalidates the whole client once. Sizes are in
  DIPs; the host applies the DPI scale.
- **`event`** translates the framework's normalized events into `Ui` calls. The
  pointer down/up pair on the button produces `Msg::Greet`; `drain_messages`
  hands it to the application.
- **`apply`** updates application state and then calls `Ui::set_label`, which
  records the label's rectangle as pending damage. `flush_damage(window)` turns
  that into one invalidation.
- **`theme`** takes `ThemeSettings` plus the dark/light decision (`true` here).

### 4. Run it

```powershell
cargo run
```

A window appears with the title "Bamboo Hello". Click the button: the label
changes to `Clicked 1 time(s)`, then `Clicked 2 time(s)`, and so on.

To watch the paint path, set the debug variable:

```powershell
$env:BAMBOO_UI_DEBUG = "1"
cargo run
```

The host logs paint and invalidation diagnostics to stderr. With the window
idle, no paint lines appear; hovering or clicking produces only the affected
rectangles.

## Expected result

- A 520×320 dark window with a label and a `Say hello` button.
- Clicking increments the counter in the label.
- The window does not repaint while idle.

## Troubleshooting

- **The window is blank or gray** — `resize` did not run, or the rectangles were
  never set. Check that `WindowBuilder::new(App::new())` receives your app and
  that `resize` calls `ui.set_rect` for every id.
- **The button does not react** — pointer events are not reaching `Ui`. Make
  sure `event` forwards `PointerDown`/`PointerUp` with
  `pointer_down_with`/`pointer_up_with` and that the button's rectangle is
  non-empty.
- **The label never changes after a click** — the message was drained but the
  change was not flushed. Call `ui.flush_damage(window)` after handling messages.
- **`run` returns an error immediately** — `WindowBuilder::run` must be called on
  the main thread; Win32 requires the message loop there. Do not spawn the
  window from a worker thread.

## Next

[04 — Custom widgets](04-custom-widgets.md): paint your own component and emit a
message from it.
