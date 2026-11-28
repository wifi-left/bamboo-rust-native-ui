# Contributing to Bamboo Native UI

Thanks for considering a contribution. This project is a small, opinionated
native Windows UI framework, and it grows through careful, well-tested changes.
Bug fixes, new features, documentation, translations, accessibility work and
cross-platform ports are all welcome.

中文版：[CONTRIBUTING.zh-CN.md](CONTRIBUTING.zh-CN.md)

## Before you start

- Search the [existing issues](https://github.com/wifi-left/bamboo-rust-native-ui/issues)
  before opening a new one.
- Open an issue for a large change, a new public API, a new dependency, or a
  cross-platform port. It is cheaper to agree on the shape first.
- Read the design contract in
  [`docs/en/overview/design-principles.md`](docs/en/overview/design-principles.md)
  and the widget/API reference in [`docs/en/`](docs/en/index.md).
- Be respectful and specific. Critique code, not people.

## Ways to contribute

### Bug fixes

A good bug report includes a minimal reproduction, the exact commands, the
operating system and Windows build, the Rust version, the enabled features, the
renderer (`gdi`, `d2d-software`, `d2d-hardware`), and the observed vs expected
behaviour. If the bug is visual, a screenshot or a short recording helps.

For the fix itself:

- Add a regression test where the behaviour is testable without a window
  (most of `layout-core`, layout application, widget input/state, geometry).
- Keep the change focused; do not mix refactors and fixes.
- Run the checks listed below.

### New features

Open a feature issue first. Describe the problem, the proposed API or XML
surface, alternatives, and what the change costs in complexity. The framework
deliberately avoids a relayout/repaint-every-frame model, runtime dependencies,
and paint-path allocations; a feature that needs one of those needs a design
discussion, not just a patch.

Useful feature areas include widgets, layout-document vocabulary, theming,
accessibility, performance, examples and tooling.

### Documentation

The documentation lives in `docs/en/` (source of truth) and `docs/zh/`
(Simplified Chinese mirror). Every user-facing change should update both.

- Follow [`docs/DOC-GUIDE.md`](docs/DOC-GUIDE.md): page templates, terminology,
  link rules.
- Widget pages must include Introduction, Usage (Rust), Usage (XML),
  Properties, Events, XML definition and See also.
- Build with Zensical and keep the site warning-free:
  `zensical build -s -f mkdocs.yml` and `zensical build -s -f mkdocs.zh.yml`.

### Translations

Translations are first-class contributions. `docs/en/` is the source; a
translation mirrors the same relative paths under `docs/<lang>/`.

- Keep code, identifiers, XML/TOML, commands, paths and link targets unchanged;
  translate prose, headings, tables and code comments.
- Use the terminology already established in `docs/zh/` (控件, 宿主, 消息,
  事件, 触发器, 注册表, 布局文档 …).
- Add or update `mkdocs.<lang>.yml` and the language switch in the other
  configs.
- Run the link check described below. A translation PR should not silently
  change technical content; if the English page is wrong, fix it there first.

New languages are welcome. Propose the language code and scope in a
translation issue; the Pages workflow currently builds English and Chinese, so a
new language also needs a workflow entry.

### Cross-platform support

The public types are platform-neutral, but the window host and the platform
layer are currently Windows-only (`#[cfg(windows)]`). A port is a substantial
but well-scoped project:

- **Backend**: implement the `Canvas` trait (fill, rounded rectangles, text,
  clipping, images, gradients) and a `Renderer` selection for the target.
- **Window host**: the message loop, `Host` callbacks, DPI, timers, damage
  tracking, one persistent back buffer and dirty-box blitting.
- **Platform modules**: window creation, keyboard, menus, tray, notifications,
  dialogs, drag and drop, accessibility, soft keyboard and IME.
- **Keep the contract**: app-owned rectangles, dirty-only paints, zero idle
  paints, no paint-path allocations, platform-neutral public API.

Open a cross-platform issue with the target OS, the layer you want to start
with, and a build/test plan. Keep the widget layer and layout model shared; do
not fork them per platform. CI for the new target and a small smoke test
(equivalent of `showcase --self-test`) are expected.

### Accessibility, performance, examples

Accessibility fixes (UI Automation roles, names, keyboard paths, Narrator),
measured performance improvements and new showcase examples are welcome. For
performance, include before/after numbers from `--bench` or the paint log; for
accessibility, include the `--a11y-dump` output or a Narrator reproduction.

## Development setup

Prerequisites: Windows 10/11 x64, Rust 1.85+ with the MSVC target, Visual Studio
2022 Build Tools with the Desktop C++ workload, and the Windows SDK.

```powershell
git clone https://github.com/wifi-left/bamboo-rust-native-ui.git native-ui
cd native-ui
cargo build
cargo test
cargo run -p bamboo-native-ui-showcase -- --self-test
cargo run -p bamboo-native-ui-showcase -- --a11y-dump
```

Documentation:

```powershell
pip install -r requirements-docs.txt
zensical build -s -f mkdocs.yml
zensical build -s -f mkdocs.zh.yml
```

The repository layout and feature flags are described in
[`README.md`](README.md) and [`docs/en/guides/feature-flags.md`](docs/en/guides/feature-flags.md).

## Design rules to keep

These are not style preferences; they are the contract the framework is built
on. A change that breaks one needs an explicit design decision.

- The application computes widget rectangles. The framework paints the dirty
  rectangles and routes normalized events; state changes never trigger a
  relayout or a full-window repaint.
- One persistent back buffer per window; only the dirty bounding box is
  blitted. Idle means zero paints; no periodic repaint without an explicit
  timer.
- No allocations on the paint path. Borrow text and dirty rectangles; cache
  fonts, brushes and converted images for the window lifetime.
- Public types stay platform-neutral; Win32/WinUI/Direct2D types stay behind
  the platform layer.
- New dependencies need a reason and a discussion; the default build links no
  XML parser.
- Layout application is atomic: a document that fails validation changes
  nothing.
- Framework setters are silent and record damage; the host flushes it.

## Pull requests

1. Fork the repository and create a focused branch.
2. Make the change, add tests, and update `docs/en/` and `docs/zh/`.
3. Run the checks below.
4. Open a pull request using the template; link the issue it closes.
5. Keep the PR small enough to review. Separate mechanical refactors from
   behaviour changes.

Commit and PR titles follow `type: summary` — `feat:`, `fix:`, `docs:`,
`i18n:`, `refactor:`, `test:`, `perf:`, `chore:`. The body explains what changed
and why.

### Required checks

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets
cargo build                     # 0 warnings
cargo test
cargo run -p bamboo-native-ui-showcase -- --self-test
cargo run -p bamboo-native-ui-showcase -- --a11y-dump
zensical build -s -f mkdocs.yml
zensical build -s -f mkdocs.zh.yml
```

For documentation-only changes, the Rust checks may be skipped if no code or
feature file changed; say so in the PR. The docs builds and link check are
always required.

### AI-assisted contributions

AI-assisted changes are welcome. Say so in the pull request, keep the diff
reviewable, and verify the result yourself — generated code and documentation
can look plausible and still be wrong. You are responsible for the change you
submit.

## Future considerations

These are directions the project may take; they are **not commitments** and do
not imply a timeline.

- Cross-platform backends (additional `Canvas` implementations and window
  hosts).
- More translations and a smoother language workflow.
- Deeper accessibility coverage and automated UI Automation checks.
- Performance work: paint statistics, damage merging, text caching.
- More widgets, layout-document vocabulary and theming flexibility.
- Better examples and a component gallery that doubles as visual tests.
- An extension story for third-party widgets and XML type registries.

If you want to work on one of these, open an issue and describe the scope. A
small, tested first step is more useful than a large unfinished branch.

## License

The project is licensed under the Apache License 2.0. By contributing you agree
that your contribution is licensed under the same terms. There is no CLA.
Keep the existing copyright and `NOTICE` requirements in mind when you modify
or redistribute the source.
