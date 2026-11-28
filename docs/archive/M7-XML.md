# M7 — layout skin schema (`.buixml`)

> **Status: v1 implemented 2026-11-27 (`layout-xml`), D129/D130.**
> The document uses XML syntax with the dedicated `.buixml` extension so it is
> never confused with generic XML data. `crate::layout_xml` implements the
> parser, model, expression evaluator, geometry, canonical writer and
> `Ui::apply_layout`/`clear_layout`/`installed_layout`; the default build has
> no XML code. The widget-declared skin protocol for per-widget attributes and
> child lists (§6.1–6.4, D127) and the runtime showcase page (§13) are
> implemented; compile-time Rust generation and the compiled mode (§16, D128)
> remain design. The debug facility is a data API the application presents
> (M7.5, D143).
>
> It also records the two schema clarifications the implementation needed: a
> claim is per **declared field**, not per mentioned id (§8.2, D130), and
> constant names exclude `-` so `cw-32` is never a name (§3.5, D129).

The XML file is a **skin, not a constructor** (D7). It assigns rectangles,
runtime properties, style tokens and static initial content to widgets the
Rust side already created. A widget-declared skin protocol (§6.2) extends the
same file to per-widget state and child data lists without growing the parser,
so more of the component catalog becomes addressable over time; the same file
can be loaded at runtime or compiled into plain Rust (§16). None of it can
create, remove, reorder or reparent a widget, so hot reload keeps every piece
of live state — scroll offsets, typed text, playback, selection — that the
file does not explicitly own.

Design goals, in order:

1. **Readable by hand.** One widget per line in the common case; the 80 %
   layout (`x`/`y`/`w`/`h`) needs no expressions. Comments are for sections.
2. **Incremental adoption.** Any widget not mentioned by a variant keeps the
   application's rectangle. A page can start with three entries and grow.
3. **Safe hot reload.** Strict, atomic, versioned. A bad edit changes nothing
   and never crashes; the diagnostics say where.
4. **Predictable ownership.** While a document is installed, the fields it
   declares belong to the file; everything else belongs to the application.

## Contents

- [Quick tour](#quick-tour) — a complete file and the six idioms you will use
- [Glossary](#glossary)
- [1. Document structure](#1-document-structure)
- [2. Variants and selection](#2-variants-and-selection)
- [3. Spaces and geometry](#3-spaces-and-geometry)
- [4. Properties](#4-properties)
- [5. Style tokens](#5-style-tokens)
- [6. Content initial values](#6-content-initial-values)
  - [6.1 Standard component coverage](#61-standard-component-coverage)
  - [6.2 The widget-declared skin protocol](#62-the-widget-declared-skin-protocol)
  - [6.3 Child lists](#63-child-lists)
  - [6.4 Third-party widgets](#64-third-party-widgets)
- [7. Scroll panels](#7-scroll-panels)
- [8. Application model](#8-application-model)
- [9. Diagnostics](#9-diagnostics)
- [10. API contract](#10-api-contract)
- [11. Normalized writer](#11-normalized-writer)
- [12. Features and build matrix](#12-features-and-build-matrix)
- [13. Showcase demo page](#13-showcase-demo-page)
- [14. Test plan](#14-test-plan-implementation-round)
- [15. Non-goals and follow-ups](#15-non-goals-and-follow-ups)
- [16. Two consumption modes: dynamic and compiled](#16-two-consumption-modes-dynamic-and-compiled)
- [FAQ](#faq)

## Quick tour

A complete, valid file. Every rectangle is a DIP; `cw` and `ch` are the
container's width and height.

```xml
<?xml version="1.0" encoding="utf-8"?>
<layouts version="1">
  <layout id="base" min-width="0" min-height="0">

    <!-- 贴左：x 是距左边 16；w 是控件宽 -->
    <widget id="now_title" x="16" y="16" w="360" h="26"/>

    <!-- 满宽：cw 是容器宽，两侧各留 16 -->
    <widget id="seek_bar"  x="16" y="52" w="cw-32" h="28"/>

    <!-- 贴右：right 是距右边 16，w 仍是控件宽 -->
    <widget id="volume"    right="16" y="52" w="96" h="28"/>

    <!-- 居中：cx 是容器中心，44 是控件宽 -->
    <widget id="play"      x="cx-22" y="96" w="44" h="44"/>

  </layout>
</layouts>
```

The idioms above are the whole geometry language for day-to-day work:

| Intent | Write |
|---|---|
| 贴左 / 贴顶 | `x="16"` / `y="16"` |
| 贴右 / 贴底 | `right="16"` / `bottom="16"` |
| 满宽（左右留白） | `x="16" w="cw-32"` |
| 铺满剩余高度 | `y="48" h="bottom-48"` |
| 居中（固定尺寸） | `x="cx-22" w="44"` |
| 两等分 | `w="(cw-48)/2"`（左 `x="16"`，右 `right="16"`） |
| 百分比 | `w="50%"`、`x="25%+8"` |
| 重复的边距 | 先写 `<const name="pad" value="16"/>`，再 `x="pad" w="cw-2*pad"` |

Everything else — properties, style tokens, content — is optional and comes
later in this document. A file with only geometry is complete.

## Glossary

| Term | Meaning here |
|---|---|
| **Variant** | One `<layout>` block. Exactly one is active at a time, picked by client size. |
| **Entry** | One `<widget>` or `<scroll>` line inside a variant. |
| **Space** | The coordinate system an expression is evaluated in: the client area or a scroll panel's content area. |
| **Claim** | A field the installed document owns (properties, style). Restored by `clear_layout` or a reinstall. |
| **Generation** | A hash of the canonical model. Content is written only when the generation changes. |
| **Baseline** | The value captured before the first claim, used to give a widget back to the application. |

## 1. Document structure

```text
layouts            root, version="1"
├── const*         name + numeric expression (constants)
└── layout+        1..N breakpoint variants
    ├── scroll*     registered scroll panels: viewport + content height
    └── widget*     widget entries: geometry + properties + style + content
```

- UTF-8 only (a BOM is accepted). 4 MiB and 50 000 entries are hard limits.
- Element and attribute names are case-sensitive and kebab-case.
- Comments and whitespace are ignored. Text nodes must be whitespace only.
- No DTD, CDATA, namespaces, `xi:include`, or processing instructions beyond
  the XML declaration. `<!DOCTYPE>` is rejected (`xml.structure`).
- Element order inside `<layout>` does not matter; document order never changes
  paint, hit-test or focus order.

### 1.1 Entry anatomy and formatting

Any attribute order is accepted. This is the recommended grouping — it is
what a human reads fastest and what the writer emits when a line gets long:

```xml
<widget id="m7_action"                    <!-- 身份：Rust 里的 WidgetId -->
        x="pad" y="56" w="120" h="36"     <!-- 几何：见 §3 -->
        label="主操作"                     <!-- 内容初值：只在文档换代时写 -->
        tooltip="样式来自 XML"             <!-- 运行期属性：见 §4 -->
        radius="medium" background="accent"/>  <!-- 样式令牌：见 §5 -->
```

Formatting rules of thumb:

- Short entry → one line: `<widget id="play" x="640" y="52" w="44" h="44"/>`.
- Long entry → one group per line, continuation lines aligned under `id`.
- Section comments (`<!-- 传输栏 -->`, `<!-- 列表 -->`) replace the grouping
  element this schema deliberately does not have.
- Use constants for repeated margins (`pad`, `gap`) so the file reads like a
  layout, not arithmetic.
- Prefer plain numbers or one anchor over an expression: `x="16"` before
  `x="left+16"`; `right="16"` before `x="cw-16-w"`.
- Keep one blank line between logical blocks, as in the examples below.
- A `<widget>` that *creates* its id declares `type` and (for an interactive
  built-in) `command` before the geometry; see §17.

### 1.2 Reference example

The same file used by the showcase's `布局` page (§13). It exercises every
construct in this document: constants, two variants, a scroll panel, anchors,
percentages, properties, style tokens and content initial values.

```xml
<?xml version="1.0" encoding="utf-8"?>
<layouts version="1">
  <const name="pad" value="18"/>

  <!-- 宽版：>= 1000×700。滚动页在右侧，内容两栏 -->
  <layout id="wide" min-width="1000" min-height="700">

    <scroll id="m7_page"
            x="300" y="48" w="cw-300" h="ch-96"
            content-height="auto"/>

    <!-- 标题 -->
    <widget id="m7_title"
            x="pad" y="12" w="cw-2*pad" h="30"
            label="XML 布局演示" font-size="18"/>

    <!-- 一行三个操作：贴左、紧随、贴右 -->
    <widget id="m7_action"
            x="pad" y="56" w="120" h="36"
            label="主操作" tooltip="样式来自 XML"
            radius="medium" background="accent"/>
    <widget id="m7_outline"
            x="pad+132" y="56" w="120" h="36"
            label="描边"
            radius="pill" background="none"/>
    <widget id="m7_progress"
            right="pad" y="56" w="160" h="30"
            progress="0.6"
            radius="large"/>

    <!-- 卡片里放一个可编辑字段 -->
    <widget id="m7_card"
            x="pad" y="108" w="cw-2*pad" h="96"
            radius="large" background="card"/>
    <widget id="m7_edit"
            x="pad+16" y="140" w="cw-2*pad-32" h="32"
            text="重载不会打断正在输入的会话。"
            radius="medium"/>

    <!-- 滑杆 + 右侧读数 -->
    <widget id="m7_slider"
            x="pad" y="220" w="cw-2*pad-80" h="28"
            value="0.4"/>
    <widget id="m7_value"
            right="pad" y="220" w="64" h="28"
            label="40%" name="当前值"/>

    <!-- 列表 -->
    <widget id="m7_list"
            x="pad" y="260" w="cw-2*pad" h="180"
            selected="1"/>

  </layout>

  <!-- 窄版：其余尺寸，全部单列从上到下 -->
  <layout id="base" min-width="0" min-height="0">

    <scroll id="m7_page"
            x="16" y="48" w="cw-32" h="ch-96"
            content-height="auto"/>

    <widget id="m7_title"
            x="16" y="12" w="cw-32" h="30"
            label="XML 布局演示" font-size="16"/>

    <widget id="m7_action"
            x="16" y="56" w="cw-32" h="36"
            label="主操作"
            radius="medium" background="accent"/>
    <widget id="m7_outline"
            x="16" y="104" w="cw-32" h="36"
            label="描边"
            radius="pill" background="none"/>
    <widget id="m7_progress"
            x="16" y="152" w="cw-32" h="30"
            progress="0.6"
            radius="large"/>

    <widget id="m7_card"
            x="16" y="204" w="cw-32" h="96"
            radius="large" background="card"/>
    <widget id="m7_edit"
            x="16" y="312" w="cw-32" h="32"
            text="重载不会打断正在输入的会话。"
            radius="medium"/>

    <widget id="m7_slider"
            x="16" y="360" w="cw-96" h="28"
            value="0.4"/>
    <widget id="m7_value"
            right="16" y="360" w="64" h="28"
            label="40%" name="当前值"/>

    <widget id="m7_list"
            x="16" y="400" w="cw-32" h="160"
            selected="1"/>

  </layout>
</layouts>
```

## 2. Variants and selection

Each `<layout>` is a complete arrangement selected by client size:

| Attribute | Required | Meaning |
|---|---|---|
| `id` | yes | unique within the file; shown by the debug form |
| `min-width` | no (0) | applies only when the client width is at least this |
| `min-height` | no (0) | applies only when the client height is at least this |

Rules:

- Exactly one variant must have `min-width="0"` **and** `min-height="0"`. It is
  the fallback; call it the base variant.
- `(min-width, min-height)` pairs must be unique. There is no `max-*`; a
  variant reads as "at least this large". Ordering variants from small to
  large in the file is a convention, not a requirement.
- Selection is a pure function: among variants whose both minimums are ≤ the
  client size (DIPs), pick the greatest `min-width`, then the greatest
  `min-height`; fall back to the base variant. Ties cannot happen (unique
  pairs).
- The selected variant alone is applied. An id declared in another variant
  but not this one is restored to the application's value (§8.2).
- Only geometry, properties and style differ between variants. **Content
  attributes must be identical in every variant that declares them**
  (`xml.content.conflict` otherwise), because content is written once per
  document generation, independent of the active breakpoint.

## 3. Spaces and geometry

### 3.1 Two coordinate spaces

| Space | Used by | `cw` / `ch` are |
|---|---|---|
| **client** | ordinary widgets; the viewport of a root scroll panel | client width / height |
| **content** | widgets registered as children of a scroll panel; the viewport of a nested panel | that panel's content width / height |

A widget's space is decided by ownership, not by a flag:
`Ui::scroll_owner_of(id)` returns the enclosing panel (nested panels chain to
the root). A widget with no owner uses client space. `<scroll>` entries use
the client space for a root panel and the parent's content space for a nested
panel.

### 3.2 Space variables

| Name | Value |
|---|---|
| `left`, `top` | `0` (readability only; `x="0"` is the same) |
| `right`, `bottom` | the space's right/bottom edge (`cw`, `ch`) |
| `cw`, `ch` | space width/height — **c**ontainer **w**idth / **h**eight |
| `cx`, `cy` | space centre (`cw/2`, `ch/2`) |

### 3.3 Expression grammar

```text
expr    := term (('+' | '-') term)*
term    := factor (('*' | '/') factor)*
factor  := number | number '%' | variable | '(' expr ')' | '-' factor
number  := digit+ ('.' digit+)?          // DIPs, no exponent, no leading '.'
variable:= left | top | right | bottom | cw | ch | cx | cy | const-name
```

- Standard precedence, left associative. Whitespace is allowed anywhere.
- `N%` is `N/100` of the expression's axis: `x`, `w`, `right` use `cw`;
  `y`, `h`, `bottom` use `ch`.
- Constants (§3.5) resolve before evaluation.
- Division by zero, a non-finite result, and an expression deeper than 32
  nodes are errors (`xml.expr.*`).

### 3.4 Rectangle determination

Every entry must determine a rectangle; there is no implicit sizing (the
framework has no measure pass, D1).

Horizontal, exactly one of:

| Form | Result |
|---|---|
| `x` + `w` | fixed left edge and width |
| `right` + `w` | `x = cw - right - w` |
| `x` + `right` | stretched: `w = cw - x - right` |

Vertical, exactly one of:

| Form | Result |
|---|---|
| `y` + `h` | fixed top edge and height |
| `bottom` + `h` | `y = ch - bottom - h` |
| `y` + `bottom` | stretched: `h = ch - y - bottom` |

- Giving all three (`x`, `right`, `w`) is over-determined
  (`xml.geom.overdetermined`); giving neither a size nor both edges is
  under-determined (`xml.geom.underdetermined`).
- `w` and `h` must evaluate to `>= 0`; a zero size is allowed but emits
  `xml.value.zero-size` (warning). `x` and `y` may be negative (off-screen
  staging); `right` and `bottom` must be `>= 0`.
- Fractional DIPs are kept; nothing is rounded. The platform scales by DPI.
- A widget cannot reference its own size; centre a fixed-size widget with
  `x="cx-22"` (for a 44 DIP button).

### 3.5 Constants

```xml
<const name="pad" value="18"/>
<const name="gap" value="12"/>
<const name="double" value="2*pad"/>
```

- Names match `[a-z][a-z0-9_]*` (no `-`, so `cw-32` is always a subtraction),
  must be unique, and may not shadow a space variable (`xml.const.reserved`).
  A name outside the grammar is `xml.const.invalid`.
- Values use the same expression grammar but may not reference space variables
  or `%` (`xml.const.vars`); they may reference constants declared **above**
  them. Forward references and cycles are therefore impossible; an unknown
  name is `xml.const.undefined`.
- Constants are document-scope. The writer preserves them; expressions are
  never inlined.

### 3.6 Authoring recipes

| Goal | Write |
|---|---|
| full width, 16 DIP margins | `x="16" w="cw-32"` |
| right-aligned fixed width | `right="16" w="120"` |
| two equal columns with a gap | left `x="16" w="(cw-48)/2"`, right `right="16" w="(cw-48)/2"` |
| centred 44 DIP button | `x="cx-22" w="44"` |
| fill remaining height | `y="48" h="bottom-48"` (i.e. `y` + `bottom`) |
| percentage | `w="50%"`, `x="25%+8"` |
| repeated margins | `<const name="pad" value="16"/>` then `x="pad" w="cw-2*pad"` |

Percent and edge forms both work; prefer the one that states the intent.

## 4. Properties

All properties are runtime-safe: they land through existing container seams
and never rebuild a widget. Every attribute is optional; omitted means "leave
whatever the application set".

| Attribute | Values | Applied through |
|---|---|---|
| `visible` | `true`/`false`/`1`/`0` | `Ui::set_visible` |
| `enabled` | bool | `Ui::set_enabled` |
| `interactive` | bool | `WidgetProps::mouse` (display widgets opt into hit-testing) |
| `focusable` | bool | `Ui::set_focusable` |
| `focus-order` | integer (smaller first) | `Ui::set_focus_order` |
| `read-only` | bool | `WidgetProps::read_only` |
| `tooltip` | string | `WidgetProps::tooltip` |
| `name` | string | `WidgetProps::name` (accessible name) |
| `description` | string | `WidgetProps::describe` |
| `automation-id` | string | `WidgetProps::automation_id` |
| `role` | role token (list below) | `WidgetProps::role` |
| `expose` | bool | `WidgetProps::expose` |
| `cursor` | `arrow`/`hand`/`i-beam`/`size-we`/`size-ns` | `Ui::set_cursor` |

Role tokens are the kebab-case form of `A11yRole`: `button`, `toggle`,
`slider`, `text-field`, `number-field`, `radio-group`, `tab-list`, `list`,
`multi-select-list`, `radio-item`, `tab-item`, `list-item`, `tree`,
`tree-item`, `grid`, `grid-cell`, `column-header`, `heading`, `group`,
`separator`, `text`, `combo-box`, `progress-bar`, `menu-item`, `menu`,
`menu-bar`, `tool-bar`, `status-bar`.

Notes:

- `visible` is structural (input, Tab order, a11y, scroll bars). The
  application's page-visibility pass must run **before** `apply_layout`; a
  declared `visible` then belongs to the file until `clear_layout`.
- `name`/`description` replace narration only; they never change the widget's
  own painted text (that is content, §6).
- `tooltip=""` suppresses the tip for that widget.
- `type`/`command` are not properties: they are creation-time schema attributes
  (§17) and never reach `WidgetProps`.

## 5. Style tokens

| Attribute | Values | Landed through |
|---|---|---|
| `radius` | `0`..`N`, `small` (4), `medium` (6), `large` (10), `pill` (height/2) | `SurfaceStyle::with_radius` |
| `corners` | four numbers `"tl tr br bl"` (spaces or commas) | `SurfaceStyle::with_corners` |
| `background` | palette token, `#rgb`/`#rrggbb`, or `none` | `with_background` / `with_transparent` |
| `font-size` | number > 0 (DIPs) | `WidgetProps::font` |
| `font-family` | non-empty string | `WidgetProps::font` |

- `radius` and `corners` are mutually exclusive (`xml.attr.conflict`).
- `pill` is resolved against the entry's final height on every apply; a resize
  re-computes it.
- `background="none"` suppresses the widget's fill but keeps its border; it is
  the readable spelling of D110's `transparent_background` and replaces a
  separate `transparent` flag. It cannot be combined with a colour (one
  attribute, one value).
- Text colour is **not** in v1. The painted widgets only expose it at
  construction time, and adding a generic runtime seam is a separate decision.
- Background palette tokens (the `Palette` field names, kebab-case):
  `background`, `sidebar`, `player`, `cover`, `nav-active`, `accent`, `text`,
  `muted`, `divider`, `row-selected`, `scroll-track`, `scroll-thumb`,
  `accent-text`, `search-bg`, `button-bg`, `button-hover`, `button-pressed`,
  `button-active`, `button-active-hover`, `explicit-border`, `nav-hover`,
  `row-hover`, `nav-pressed`, `row-pressed`, `scroll-thumb-hover`, `card`,
  `panel`, `button-disabled`, `text-disabled`, `play-button`, `play-hover`.
  Literal colours are an escape hatch for one-off surfaces; tokens follow the
  theme. The host re-applies the layout after a theme change (§8.6).
- The same table also backs a widget's **local** `Color` attributes (M7.3:
  `Icon` `tint`, `Avatar` `tint`, `ColorSwatch` `color`, `ColorPicker` `color`,
  a user `x-tint`, …). There is one value type, `SkinValue::Color(Rgba)`, and a
  `Color` attribute accepts one grammar (whitespace ignored anywhere):
  `#rgb`/`#rgba`/`#rrggbb`/`#rrggbbaa`, decimal channels `r,g,b`/`r,g,b,a`
  (commas or spaces), `rgb(r,g,b)`/`rgba(r,g,b,a)`, or a palette token name.
  The applier resolves
  the token against the active palette before calling `apply_skin_attr`, so the
  widget receives the value in theme coordinates and knows nothing about
  tokens. A widget without transparency support calls `as_color()` and drops
  the alpha silently; the in-window `ColorPicker` calls `as_rgba()` and seeds
  its default color plus alpha. An unknown name or malformed literal is
  `xml.skin.value`.
- Alpha conventions: a functional `rgba(r,g,b,a)` follows CSS (`a` in 0..1, or
  0..255 when it exceeds 1); a bare list `r,g,b,a` uses 0..255, where a
  fractional `a` below 1 (`0.5`) is a fraction. Both `rgb`/`rgba` names are
  case-insensitive.
- The surface `background` literal shares this parser: the decimal and
  functional forms work too, and the alpha is dropped (a fill is opaque);
  `background="none"` remains the transparent-fill spelling. Local colors have
  no `none` state (the widget's own `Option` models "no color").
- Capability: a widget that does not adopt `SurfaceStyle` gets
  `xml.style.unsupported` (warning) and the field is skipped. `font-*` is
  paint-level and works on every widget.
- The XML layer needs one new widget seam:
  `Widget::surface_style_mut() -> Option<&mut SurfaceStyle>` (provided default
  `None`; the existing `surface_builders!` macros implement it). This is how
  the applier reads the baseline and writes the tokens without downcasting.

## 6. Content initial values

Content is deliberately limited: it is an **initial value**, not a binding.

| Attribute | Value | Applied through |
|---|---|---|
| `text` | string | `Ui::set_text` |
| `label` | string | `Ui::set_label` |
| `value` | number | `Ui::set_value` |
| `progress` | number, clamped to 0..1 | `Ui::set_progress` |
| `number` | integer | `Ui::set_number` |
| `checked` | bool | `Ui::set_toggle` |
| `selected` | non-negative integer | `Ui::set_selected` |

Rules:

- **Write timing.** Content is written only when the *document generation*
  changes: the first install and each successful reload whose canonical model
  differs. Window resize, breakpoint switching, theme changes and re-applying
  the same document never write content, so a user's typing is never lost to
  a resize. A reload does overwrite (that is the point of an initial value);
  document it in the debug form.
- **Variant invariant.** If two variants declare content for the same id, the
  parsed values must be equal (`xml.content.conflict`). This is what makes
  "written once" independent of the active breakpoint.
- **Open sessions.** If an editing session (painted or native) is open on the
  target id, that entry's content write is skipped and reported as
  `xml.content.session-open` (warning); nothing is committed or discarded.
- **Silent.** Writes go through the D119 setters, so no user callback fires.
- `text` and `label` are mutually exclusive on one entry, as are
  `value`/`progress`/`number` (`xml.attr.conflict`).
- Capability is published by a second additive seam:
  `Widget::content_caps() -> ContentCaps` (bit flags `TEXT`, `LABEL`, `VALUE`,
  `PROGRESS`, `NUMBER`, `TOGGLED`, `SELECTION`; default `NONE`). A mismatch is
  `xml.content.unsupported` (warning), the attribute is skipped.
  Indicative support:

  | Attribute | Widgets (the trait is the source of truth) |
  |---|---|
  | `text` | `TextEdit` and aliases (`SearchBox`, `TextArea`, `PasswordBox`), `ComboBox` |
  | `label` | `Label`, `Button`, `Banner`, `StatCard`, `EmptyState`, chrome labels |
  | `value` | `Slider`, `ProgressBar`, `WaveformSeekBar` |
  | `progress` | `ProgressBar`, `StatusBar` progress segment |
  | `number` | `NumberInput` |
  | `checked` | `Toggle`, `Checkbox`, `Switch` |
  | `selected` | `Tabs`, `Segmented`, `ListView`, `MultiSelectList`, `Table`, `TreeView`, `ComboBox`, `PlaylistView`, `CoverWall` |

- Row/item arrays (`ListView` rows, tab labels, table columns) are not
  expressible in v1. They are data, not layout; the designed extension is
  §6.3.

### 6.1 Standard component coverage

v1 ships the shared vocabulary above. Everything below it is a **target**:
the parser stays the same, and each widget publishes the extra names it
accepts, so supporting a new component is a local implementation, not a
parser change.

| Tier | What | Status |
|---|---|---|
| v1 | geometry, generic properties, surface/font, shared content | implemented (`layout-xml`) |
| protocol | the six `Widget` methods, validation, apply order, writer | implemented (D137) |
| v1.1 | per-widget scalar state through `skin_attrs` | implemented (M7.3, D139), extended with the data/list appearance/interaction locals (D144) and the text colour/role locals (M7.8, D150) |
| v2 | child lists (rows, options, nodes, tracks, runs) through `skin_children` | implemented for every data/label-owning widget (D140–D142); construction-only chrome stays Rust-owned |
| extension | third-party `x-*` attributes and child elements | implemented (showcase `m7_meter`) |

Representative coverage — the widget's own declaration is authoritative:

| Family | Widgets | Shared (v1 name) | Local scalars (v1.1 target) | Child lists (v2 target) |
|---|---|---|---|---|
| Text | Label, SectionTitle, CopyableText, MarqueeText, RichText | `text`/`label` | `color` (all five) ✓, `text-style` (Label, CopyableText, MarqueeText, RichText) ✓, `offset` (Marquee) ✓, `wrap`/`align` (RichText) ✓ | `<run>` (RichText) |
| Buttons | Button, MenuButton, SplitButton, Toggle, Checkbox, RadioButton, Switch | `label`, `checked` | `glyph` | — |
| Inputs | TextEdit, SearchBox, TextArea, PasswordBox, NumberInput, ComboBox, DatePicker, TimePicker, ColorPicker | `text`, `number`, `value` | `cue` ✓, `color` (TextEdit, SearchBox) ✓, `prefix`/`suffix` ✓, `date`/`time`/`color`/`open` ✓ | `<option>` (ComboBox) |
| Navigation | Tabs, Segmented, ListView, MultiSelectList, Pager, NavRail, Breadcrumb, Accordion | `selected` | `open` (Accordion) ✓, `label-align` (Tabs) ✓, `row-height`/`alternating`/`empty-text` (ListView, NavRail row-height) ✓, `separator` (Breadcrumb) ✓ | `<option>`, `<step>`, `<item>`, `<page>`, `<section>` |
| Data | Table, TreeView, PropertyGrid, CoverWall, PlaylistView | `selected` | row/header/cell/key sizes, `striped`, `grid-lines`, `editable`, `alternating`, `indent`, `renameable`, `draggable`, `gap`, `corner-radius`, `caption-height`, `empty-text` ✓ | `<column>`, `<row>`, `<cell>`, `<node>`, `<section>`, `<cover>`, `<track>` |
| Feedback | ProgressBar, ProgressRing, Slider, Spinner, RatingStars, Gauge, WaveformSeekBar, Sparkline | `value`, `progress` | `phase`/`animating` ✓, `rating` via `value`, `color`/`gap` (Sparkline) ✓ | `<point>`, `<threshold>` |
| Display | Card, StatCard, Badge, Kbd, Avatar, Icon, Banner, EmptyState, Toast, Snackbar, ColorSwatch, Divider | `label`, `text` | `title`/`body` ✓, `shortcut` ✓, `initials`/`image` ✓, `glyph` ✓, `color` ✓, `message`/`error` ✓, `tone` ✓ | `<action>` |
| Chrome | StatusBar, Toolbar, Ribbon, MenuBar | — | `progress` (StatusBar) | `<segment>`, `<cell>`, `<tab>`, `<command>` |
| Player | PlaybackTransport, PlaylistView, WaveformSeekBar | `progress`, `value` | `playing`, `shuffle`, `repeat` ✓, PlaylistView's row locals ✓ | `<track>` |

- "Shared" is the standardized name; `content_caps` decides whether a given
  widget supports it. A widget that supports one implements the matching setter
  (Badge `set_label`, RatingStars `set_value`, …) — the shared attribute name
  does not change; state without a shared name goes through the local
  vocabulary below.
- "Local" names are illustrative. `skin_attrs()` is the source of truth; the
  debug form prints it and the writer accepts exactly that set. M7.3/D139 and
  M7.4/D144 implemented the rows above (✓). Event handlers are never local
  attributes: `editable`/`renameable`/`draggable` only enable the interaction.
- Construction-only chrome (`Toolbar`, `Ribbon`, `MenuBar`) stays Rust-owned
  and declares no child lists.
- Widgets not listed still get geometry, properties and style; they simply
  declare no local slots yet.

### 6.2 The widget-declared skin protocol

**Implemented (D137).** A central `match` on widget types would grow with
every component and would be closed to third-party widgets. Instead a widget
publishes the attributes and child elements it accepts, and the applier
validates against that declaration. The types live in `crate::skin` and are
compiled unconditionally, so a widget can declare support without enabling
`layout-xml`.

```rust
pub enum SkinValueKind { Text, Number, Bool, Color }

pub struct SkinAttrDecl {
    pub name: &'static str,
    pub kind: SkinValueKind,
}

pub enum SkinValue<'a> {
    Text(&'a str),
    Number(f64),
    Bool(bool),
    /// One value type for every color: alpha included (opaque hex/tokens are
    /// `Rgba::opaque`). `as_color()` drops alpha for widgets without it.
    Color(Rgba),
}

// Implemented as an owned tree: a parsed document is self-contained, and the
// source buffer is dropped after parsing (D137).
pub struct SkinElement {
    pub name: String,
    pub attrs: Vec<(String, String)>,   // raw, source order; the widget parses
    pub children: Vec<SkinElement>,
    pub line: usize,                    // 1-based opening-tag position
    pub column: usize,
}

// Provided on Widget<M>, all defaulted (full trait in §10):
fn skin_attrs(&self) -> &'static [SkinAttrDecl];
fn validate_skin_attr(&self, name: &str, value: &SkinValue<'_>) -> bool;
fn apply_skin_attr(&mut self, name: &str, value: &SkinValue<'_>);
fn skin_children(&self) -> &'static [&'static str];
fn validate_skin_children(&self, children: &[SkinElement]) -> bool;
fn apply_skin_children(&mut self, children: &[SkinElement]);
```

Rules:

1. **Names.** The shared vocabulary (§6) is reserved; a declaration that
   reuses one is `xml.skin.reserved`. Everything else is the widget's own
   namespace. Names beginning with `x-` are reserved for user code and are
   guaranteed never to be claimed by the library.
2. **Types.** The applier parses the raw value according to `kind`; a parse
   failure is an error before any mutation. Date/time/enum values are `Text`
   and interpreted by the widget.
3. **Two phases.** Every declared attribute and child list is validated
   first (`validate_*`), then applied (`apply_*`). This keeps the strict
   atomic promise of §8.3: a handler cannot half-apply and then fail.
   `validate_*` must be side-effect-free; `apply_*` must succeed for a value
   that validated.
4. **Display-only and silent.** Handlers never fire callbacks and never
   create/remove widgets, move a rectangle, change focus scope or Tab order.
   Visibility, size and structure belong to the generic schema and the
   application.
5. **Scope.** Local attributes and child lists are **content**: written only
   when the document generation changes, and variant-invariant (§6). They are
   not restored by `clear_layout`.
6. **Order.** Child lists first, then shared content (including `selected`),
   then local attributes. Data is in place before selection and state.
7. **One damage mark.** The applier marks the target widget once after all of
   its attributes/children are applied.
8. **Introspection.** The debug form lists `skin_attrs()`/`skin_children()` for
   the selected widget, so the vocabulary is discoverable at runtime.

A built-in widget's implementation is one small block; it calls the setter
the widget already has and validates anything the type system cannot:

```rust
static ATTRS: &[SkinAttrDecl] = &[
    SkinAttrDecl { name: "playing", kind: SkinValueKind::Bool },
];

impl<M> Widget<M> for PlaybackTransport<M> {
    fn skin_attrs(&self) -> &'static [SkinAttrDecl] { ATTRS }

    fn validate_skin_attr(&self, _name: &str, _value: &SkinValue<'_>) -> bool {
        true
    }

    fn apply_skin_attr(&mut self, name: &str, value: &SkinValue<'_>) {
        if let ("playing", SkinValue::Bool(on)) = (name, value) {
            self.set_playing(*on);
        }
    }
}
```

### 6.3 Child lists

The same protocol covers data lists (the protocol is implemented; the
built-in widgets declare their lists in M7.4):

```xml
<widget id="playlist" selected="1">
  <track title="夜曲" subtitle="周杰伦" current="true"/>
  <track title="晴天" subtitle="周杰伦"/>
</widget>
```

- Built-in child names: `item`, `option`, `row`, `cell`, `column`, `node`,
  `section`, `cover`, `track`, `segment`, `step`, `page`, `run`, `point`.
  User-code child names start with `x-`. A widget declares the names it
  accepts in `skin_children()`.
- Attributes inside a child are raw strings; the widget validates and parses
  them. Nesting is allowed (`<node>` in `<node>`, `<cell>` in `<row>`), and the
  widget validates the nesting rule.
- Child attributes are never expressions: data is data, not layout. Image
  values are the **numeric** `ImageId` the application assigned through
  `Ui::set_image` (`image="7"`), never paths or bytes; a page has no
  image-name registry. `Avatar image` uses the same convention (D147).
- Replacement, not reconstruction: `apply_skin_children` replaces the widget's
  data model (rows, options, nodes) without creating widgets. `selected` and
  local state apply afterwards, so a list plus a selection is deterministic.
- Scope and validation are those of §6.2: generation-scoped, variant-invariant,
  validated before mutation.
- A child list that would change the widget's structure must be refused in
  `validate_skin_children` until the widget owns that data at runtime. Every
  widget below now owns its data (`Table` columns, `Tabs`/`Segmented` labels,
  `NavRail` rows); only construction-only chrome (`Toolbar`, `Ribbon`,
  `MenuBar`) stays Rust-owned and declares nothing.
- Writer: child elements are emitted after the attributes in source order;
  the model preserves them for a byte-stable round trip.

Implementation order (TODO):

1. Existing runtime data setters: `ListView` `<item>`, `ComboBox` `<option>`,
   `PlaylistView` `<track>`, `CoverWall` `<cover>`, `StatusBar` `<segment>`.
   **Done (M7.4, D140).** Row-like children share
   `title`/`subtitle`/`trailing`/`current`/`disabled`; `<option label>`,
   `<cover title subtitle image corner-radius>` (image is the raw `ImageId`
   number) and `<segment text icon width align>`. Unknown child attributes are
   rejected; the feature test applies all five lists and rejects malformed ones
   atomically.
2. Data with a small widget-side change: `Table` `<column>`/`<row>`/`<cell>`,
   `TreeView` `<node>`, `PropertyGrid` `<section>`/`<row>`, `Pager` `<page>`.
   **Done (M7.4, D141).** A table child list is the complete table data
   (columns and rows replace together); `<row>` owns `<cell text>` children.
   Tree nodes nest recursively. Property rows select an editor kind and the
   kind's own parameters:

   | Widget | Child | Attributes |
   |---|---|---|
   | `Table` | `<column>` | `title` (required), `width`, `flex`, `align` |
   | `Table` | `<row>` | `disabled`; children `<cell text>` (a present, possibly empty `text`) |
   | `TreeView` | `<node>` | `label` (required), `glyph` (one character), `secondary`, `expanded`, `disabled`; nested `<node>` |
   | `PropertyGrid` | `<section>` | `title`; children `<row>` |
   | `PropertyGrid` | `<row>` | `key` (required), `value`, `kind`, `read-only`, and the kind's parameters: `min`/`max`/`step`/`decimals` (number), `options` (`|`-separated, choice), `on`/`off` (check), `stars` (rating) |
   | `Pager` | `<page>` | `label` (required) — the page count becomes the list length |

   Row `kind` is one of `read-only` (default), `text`, `number`, `choice`,
   `check`, `rating`; an attribute that does not belong to the selected kind is
   a typo (`xml.skin.value`), as is a non-positive step, an empty choice
   option, a check without both value texts and a star count outside 1..=10.
3. Need owned labels/data first: `Tabs`/`Segmented` `<option>`,
   `NavRail`/`Breadcrumb` `<step>`, `RichText` `<run>`, `Sparkline` `<point>`.
   **Done (M7.4, D142).** `Tabs`/`Segmented` now own their labels
   (`set_labels`/`set_options`); `NavRail` gained the public `NavItem` plus
   `set_items`; `Breadcrumb`, `RichText` and `Sparkline` already owned their
   models. The vocabularies:

   | Widget | Child | Attributes |
   |---|---|---|
   | `Tabs`, `Segmented` | `<option>` | `label` (required) |
   | `Breadcrumb` | `<step>` | `label` (required) |
   | `NavRail` | `<step>` | `label` (required), `icon` (one character) |
   | `RichText` | `<run>` | `text` (required), `style` (`normal`/`strong`/`muted`/`link`), `link` (`u32`, link runs only) |
   | `Sparkline` | `<point>` | `value` (required, finite number) |

   Per-option disabled flags and badges stay application-owned: the file
   supplies labels, not editor state.
4. Construction-only chrome (`Toolbar`, `Ribbon`, `MenuBar`) may stay
   Rust-owned; do not block on it.

### 6.4 Third-party widgets

A widget outside this crate implements only the methods it needs and is fully
supported by the same file. No registry, no `TypeId` table, no feature
dependency: `crate::skin` is always compiled. A third-party scalar uses the
`x-` namespace:

```xml
<widget id="meter" x="16" y="16" w="200" h="24" x-level="72" x-units="dB"/>
```

The showcase carries one such test widget (`crates/showcase/src/meter.rs`:
`x-level`/`x-units` scalars and an `x-item` child list, file-driven on the
`布局` page); the self-test checks its applied data and write count, and the
a11y dump finds its named ProgressBar. The debug form (M7.5) will print its
declared vocabulary.

## 7. Scroll panels

`<scroll>` addresses a panel already registered with `Ui::set_scroll_view`:

| Attribute | Meaning |
|---|---|
| `id` | required; must be a registered scroll panel |
| `x`/`right`, `y`/`bottom`, `w`/`h` | the panel's **viewport**, same geometry rules |
| `content-height` | omitted = keep; number/expression = set exactly; `auto` = grow to fit |

- A root panel's viewport is in client space; a nested panel's viewport is in
  its parent's content space (matching `ScrollView::nested_in`).
- A `<widget>` whose id is a child of a panel is positioned in that panel's
  content space. The applier updates the panel's `ScrollChild` rect and re-runs
  the panel's layout (the same path as `Ui::scroll_layout`), so offsets,
  clamping, clipping and hit testing stay consistent. It never calls
  `Widget::set_rect` directly on a scroll child.
- `content-height="auto"` sets
  `max(current content height, viewport height, every registered child's
  bottom + bottom padding)`. It never shrinks: reloading cannot yank a scroll
  position. Write an explicit number to shrink.
- While `auto` resolves `ch`, the entries of that panel see the **previous**
  content height (one apply of lag). A widget anchored with `bottom`/`ch`
  inside an `auto` panel should therefore use an explicit `content-height`.
  The reference example only uses top-anchored entries.
- Content width is not settable: it is the panel's content width (viewport
  minus padding and bars). Content height expressions may use the space
  variables; `ch` is the pre-apply height.
- Updating a panel keeps its scroll offset, bar style, padding, nesting parent
  and name; only viewport, content size, offset clamping and the declared
  child rects change.

## 8. Application model

### 8.1 Generation

`LayoutDocument::generation()` is the FNV-1a 64 hash of `to_xml()` (the
canonical model, §11). Formatting, comments and attribute order do not change
it; a real edit does. `Ui` compares it to decide between a *load* (write
content, capture baselines) and a *re-apply* (geometry/properties/style only).

### 8.2 Claims and baselines

When a document is installed (generation change), the applier captures, for
every id mentioned anywhere in the document:

- the container-side `WidgetProps` entry (or "none"),
- the widget-level `visible` / `enabled` / `read_only` state,
- the focus policy (order + focusable override) and the cursor override,
- its `SurfaceStyle` (through `surface_style`/`surface_style_mut`),
- its rectangle (recorded for the debug form only; see below).

A **claim is per declared field**, not per mentioned id (D130): `visible`,
`enabled`, `focusable`, `focus-order`, `cursor`, each `WidgetProps` field, the
font, the corner shape and the background are separate claims, and a field is
claimed only when at least one variant declares it for that id. On every apply
(including a breakpoint switch), each claimed field is restored to its baseline
and then overwritten by the active variant's value — or left at the baseline
when this variant does not declare it. Unclaimed fields keep whatever the
application last set, so a file that only positions widgets never fights the
application's page visibility or props. An id declared in the wide variant but
not in base therefore returns to the application's own properties when the
window crosses the breakpoint.

Restoration limits, stated so nobody is surprised:

- **Rectangles are not restored.** The application recomputes every rectangle
  before calling `apply_layout` (D1: layout runs on resize/rebuild); an
  undeclared id simply keeps the rectangle the application just assigned. The
  baseline rect is kept only for debug display.
- **Scroll viewports are not restored.** They are the application's layout; a
  panel the reloaded file no longer mentions keeps its current viewport.
- **Content is not restored.** Content is an initial value; `clear_layout`
  does not put typed text back.
- An application change to a claimed property while XML owns it is
  overwritten on the next apply. Use `clear_layout` (or re-install the
  document) to hand the id back.

`Ui::clear_layout(sink)` restores every claimed baseline, drops the document
and the generation, and damages the union once.

### 8.3 Atomic validation

Parsing and application are separate:

- `LayoutDocument::parse` is pure and produces the full document model or a
  list of diagnostics. Structural errors reject the whole file.
- `apply_layout` first resolves every id (widget, scroll panel, scroll child),
  every range and every expression against the current client size. Any error
  returns `Err(LayoutFailure)` and **mutates nothing**: the previous layout
  stays active. Warnings (capability, open session, clamped values) do not
  abort.

This is the hot-reload contract from `TODO.md`: success → one damage union
repainted; failure → last layout kept, diagnostics logged, no crash.

### 8.4 Damage

Every change goes through the existing D119/D120 damage batch: setters mark
their widget, `set_visible` and scroll re-registration do their own structural
damage, and `apply_layout` ends with one `Ui::flush_damage(sink)`. A layout
pass therefore costs one repaint request, not one per entry. Scroll child
moves mark the old ∪ new client rectangles and the panel viewport when the
content size changes.

### 8.5 Hot reload

The library does no file IO and starts no watcher. The application:

1. reads the file (it may be embedded with `include_str!` as a fallback),
2. calls `LayoutDocument::parse`,
3. on `Ok`, compares `generation()` and calls `apply_layout`,
4. on `Err`, keeps the installed document, repaints nothing and hands the
   diagnostics to its log/debug form.

The showcase polls the file's modification time on its existing timer
(250 ms); no new dependency and no `ReadDirectoryChangesW` complexity.

### 8.6 Theme changes, DPI

- Resolve `background` tokens at apply time with the palette passed to
  `apply_layout`. After a theme or accent change the host calls it again (same
  generation; content is not rewritten).
- Everything is DIPs. DPI changes go through the normal resize path; `pill`
  and percent values re-resolve.

## 9. Diagnostics

```rust
pub struct LayoutDiagnostic {
    pub severity: LayoutSeverity, // Error | Warning
    pub code: &'static str,       // stable, testable
    pub line: usize,              // 1-based, points at the element
    pub column: usize,
    pub path: String,             // "layout/wide/m7_action", best effort
    pub message: String,
}
```

Errors reject the whole document; warnings are reported and the rest applies.
At most 100 diagnostics are collected per parse.

| Code | Severity | Cause |
|---|---|---|
| `xml.syntax` | error | malformed XML |
| `xml.version` | error | missing/unsupported `version` |
| `xml.structure` | error | wrong nesting, stray text, DTD, too large |
| `xml.unknown-element` | error | element outside the schema |
| `xml.unknown-attribute` | error | attribute outside the schema |
| `xml.missing-attribute` | error | required attribute absent |
| `xml.attr.value` | error | malformed attribute value (bad number/bool/token/colour) |
| `xml.duplicate-layout-id` | error | two variants share `id` |
| `xml.duplicate-widget-id` | error | two entries in one variant share `id` |
| `xml.layout.base` | error | no `(0,0)` variant, or duplicate minimums |
| `xml.const.reserved` / `.duplicate` / `.undefined` / `.vars` / `.invalid` | error | constant misuse |
| `xml.expr.syntax` / `.unknown-var` / `.div-zero` / `.non-finite` / `.depth` | error | expression misuse |
| `xml.geom.underdetermined` / `.overdetermined` / `.negative` | error | rectangle not determined |
| `xml.attr.conflict` | error | `radius`+`corners`, `x`+`right`+`w`, `text`+`label`, … |
| `xml.id.unknown-widget` / `.unknown-scroll` / `.ambiguous` | error | id does not resolve (apply time) |
| `xml.content.conflict` | error | variants disagree on an id's content |
| `xml.skin.conflict` | error | variants disagree on an id's local skin data |
| `xml.skin.reserved` | error | a widget declaration reuses a shared name |
| `xml.skin.unknown` | error | local attribute/child the widget does not declare |
| `xml.skin.value` | error | a declared value fails its `SkinValueKind` parse, or the widget's `validate_*` rejects it |
| `xml.rust.unsupported` | error | `rust:` value in a dynamic document (§16.2) |
| `xml.type.conflict` | error | variants disagree on an id's `type`/`command`, or one variant omits a created id |
| `xml.type.unknown` | error | a `type` entry with no registry installed or no such registered type |
| `xml.type.create` | error | a factory refused the entry or returned another id |
| `xml.command.missing` | error | an interactive type needs a `command` (or the registry has no command map) |
| `xml.id.duplicate` | error | a `type` entry's id exists and the document does not own it |
| `xml.rust.context` | error | a context rectangle applied through the static descriptor |
| `xml.style.unsupported` | warning | style token on a widget without a surface |
| `xml.content.unsupported` | warning | content attribute the widget cannot take |
| `xml.content.session-open` | warning | target is being edited; write skipped |
| `xml.value.clamped` | warning | value outside the widget's range; clamped |
| `xml.value.zero-size` | warning | `w`/`h` evaluates to 0 |

Warnings never prevent the file from loading; they are what the debug form
shows in its log ring.

## 10. API contract

Behind `feature = "layout-xml"`:

```rust
impl LayoutDocument {
    pub fn parse(text: &str) -> Result<Self, Vec<LayoutDiagnostic>>;
    pub fn generation(&self) -> u64;
    pub fn variants(&self) -> &[LayoutVariant];
    pub fn select(&self, client: Size) -> &LayoutVariant;
    pub fn to_xml(&self) -> String;       // normalized, §11
}

impl LayoutVariant {
    pub fn id(&self) -> &str;
    pub fn minimums(&self) -> (f32, f32);
    pub fn entries(&self) -> &[LayoutEntry];
    pub fn scrolls(&self) -> &[ScrollEntry];
}

impl LayoutEntry {
    pub fn id(&self) -> WidgetId;
    pub fn space(&self) -> EntrySpace;   // Client | Content(WidgetId)
    pub fn resolve(&self, space: LayoutSpace) -> Result<Rect, LayoutDiagnostic>;
}

impl<M> Ui<M> {
    pub fn apply_layout(
        &mut self,
        doc: &LayoutDocument,
        palette: &Palette,
        client: Size,
        sink: &mut impl UiHost,
    ) -> Result<LayoutReport, LayoutFailure>;

    pub fn clear_layout(&mut self, sink: &mut impl UiHost);

    /// Read-only view for the debug form: generation, active variant,
    /// resolved rectangles, claimed ids, last warnings.
    pub fn installed_layout(&self) -> Option<&AppliedLayout>;
}
```

Supporting additions (additive trait methods with defaults; the skin types
live in `crate::skin`, §6.2):

```rust
pub trait Widget<M> {
    /// Runtime surface seam for the XML skin (and future inspectors).
    fn surface_style_mut(&mut self) -> Option<&mut SurfaceStyle> { None }
    /// Which content setters mean something for this widget.
    fn content_caps(&self) -> ContentCaps { ContentCaps::NONE }

    // The widget-declared skin protocol (§6.2/§6.3, D137).
    fn skin_attrs(&self) -> &'static [SkinAttrDecl] { &[] }
    fn validate_skin_attr(&self, _name: &str, _value: &SkinValue<'_>) -> bool { true }
    fn apply_skin_attr(&mut self, _name: &str, _value: &SkinValue<'_>) {}
    fn skin_children(&self) -> &'static [&'static str] { &[] }
    fn validate_skin_children(&self, _children: &[SkinElement]) -> bool { true }
    fn apply_skin_children(&mut self, _children: &[SkinElement]) {}
}
```

The six skin methods are implemented (D137) and shared by the runtime
applier and the compiled code generator (§16). Defaults cost nothing for
widgets that ignore them, and `crate::skin` is compiled in every build (no
feature gate), so a third-party widget can declare support without pulling in
`quick-xml`.

`LayoutReport { generation, variant_id, applied_entries, content_writes,
warnings }`; `LayoutFailure { diagnostics }`; both implement `Display` for the
showcase/status line and the debug form.

`LayoutSpace` is the resolver input: `LayoutSpace::client(Size)` or
`LayoutSpace::content(width, height)`. It is public so a test or a debug form
can resolve an entry without a `Ui`.

## 11. Normalized writer

`to_xml()` exists so the debug form can save and so the generation hash has a
canonical source. It emits:

- the XML declaration and `<layouts version="1">`;
- constants first, in document order;
- variants in document order, entries in document order;
- a fixed attribute order: `id`, geometry (`x`/`right`, `y`/`bottom`,
  `w`/`h`), properties in the §4 table order, style, content;
- widget-declared attributes (`skin_attrs`) after the shared ones, sorted by
  name; child elements (`skin_children`) after the attributes, in source
  order, with their raw values preserved verbatim so an untouched extension
  round-trips;
- two-space indentation, one entry per line when it fits in 100 columns;
  otherwise the grouped layout from §1.1 (deterministic, so two saves of the
  same model are byte-identical);
- self-closing elements, the shortest representation of a number (`18`,
  `18.5`), XML-escaped strings;
- no comments (they are not part of the model).

Round-trip rule: `parse(to_xml(parse(text)))` equals `parse(text)`. Saving
writes `layout.buixml.bak` before replacing the file; that is the debug form's
contract, not the library's.

## 12. Features and build matrix

| Feature | Effect |
|---|---|
| `layout-xml` | runtime parse/apply/write; optional `quick-xml` (`default-features = false`) — **implemented**, parser and applier live in `crate::layout_xml` until the `layout-core` split (§16) |
| `layout-compiled` | runtime helpers for generated code (claims, variant selection, apply); no parser, no `quick-xml` (§16) — TODO |
| `theme-xml` | `.buitheme` reader + canonical writer (enables `layout-core/parse`) and the `Palette` mapping; independent of `layout-xml` (M7.8, D150) |
| `debug-form` | implies `layout-xml`; `crate::debug` read-only snapshots (layout, widgets, damage, paint stats, diagnostics ring) — the application presents them (M7.5, D143) |
| default | no XML code, no `quick-xml` in `cargo tree -p bamboo-native-ui -e normal` — verified |
| showcase `m7` | `["bamboo-native-ui/layout-xml", "bamboo-native-ui/layout-compiled", "bamboo-native-ui/debug-form", "bamboo-native-ui/theme-xml"]`; on by default (the default catalog has the `布局` page, the `调试` menu and the API page's 主题文档 demo); `--no-default-features` builds the eight-page catalog |

Verification for the implementation round:

```powershell
cargo build                                  # default: no quick-xml
cargo build -p bamboo-native-ui --features layout-xml
cargo test -p bamboo-native-ui --features layout-xml
cargo build -p bamboo-native-ui --features theme-xml
cargo test -p bamboo-native-ui --features theme-xml
cargo test -p bamboo-native-ui-layout-core --features parse
cargo run -p bamboo-native-ui-showcase -- --self-test
cargo run -p bamboo-native-ui-showcase -- --a11y-dump
cargo run -p bamboo-native-ui-showcase --no-default-features -- --self-test
cargo run -p bamboo-native-ui-showcase --no-default-features -- --a11y-dump
```

## 13. Showcase demo page

The first real consumer is a new ninth page, `布局`, built whenever the
showcase's `m7` feature is on — which is the showcase default; `--no-default-
features` builds the eight-page catalog without XML code. The page proves the
whole schema without touching the existing eight pages:

- Rust creates a small set of widgets whose ids all start with `m7_`
  (`m7_title`, `m7_action`, `m7_outline`, `m7_progress`, `m7_card`,
  `m7_edit`, `m7_slider`, `m7_value`, `m7_list`) and registers the `m7_page`
  scroll panel. It also computes a plain vertical fallback layout, so the page
  is valid before any XML is read.
- `crates/showcase/layouts/layout-demo.buixml` is the file in §1.2. It supplies
  both breakpoints, so the page reflows from a wide two-column arrangement to
  the narrow stack when the window crosses 1000×700.
- A status line under the page shows the active variant, entry count and the
  last warning; the app writes it from `LayoutReport`.
- Hot reload polls the file's mtime on the existing 250 ms timer. A malformed
  edit leaves the last layout on screen and shows the diagnostics; a missing
  file falls back to `include_str!`.
- The page's widget ids join `PAGE_NAMES`/tab handling only with `m7`;
  `--no-default-features` keeps the eight-page catalog, self-test and a11y
  rounds unchanged.

The debug API is designed as data, not a painted inspector (M7.5, D143). The
XML layer promises `installed_layout()` and the diagnostics; `crate::debug`
adds `Ui::debug_snapshot()` (layout entries/claims/scroll panels/warnings,
per-widget effective state, accessibility with narration resolved, the
declared skin vocabulary), pending/last-flushed damage pieces, per-widget paint
measurements and a bounded diagnostics ring (`Ui::debug_event`). The showcase's
`调试` menu formats those values into information dialogs and offers reload
(`m7_load` + apply), save (`to_xml` with a `.buixml.bak` backup) and reset
(`clear_layout`).

## 14. Test plan (implementation round)

Parser and expressions:

- precedence, parentheses, unary minus, whitespace, percent per axis;
- constants: forward reference, duplicate, reserved name, variable use;
- `xml.expr.div-zero`, non-finite, depth, unknown variable;
- UTF-8 BOM, astral characters in strings, entity decoding, size cap,
  DOCTYPE rejection.

Geometry and selection:

- every legal form in §3.4, every illegal combination;
- percent + anchors + `cx`/`cy` at fractional sizes;
- variant selection at each minimum boundary, base fallback, duplicate
  minimums rejected;
- `pill` re-resolves after a height change.

Application:

- atomicity: one error per code → nothing moved, previous layout intact;
- idempotence: two applies of the same generation produce identical rects
  and one damage union;
- breakpoint switch restores claimed properties/styles and leaves undeclared
  ids alone;
- `clear_layout` restores baselines and is safe twice;
- warnings do not abort.

Scroll:

- content coordinates, `content-height` keep/explicit/`auto` (grow-only),
  nested panels, offset clamping, old ∪ new damage;
- a scroll child is never written through `set_rect`.

Content:

- written once per generation, never on resize or a breakpoint switch;
- variant equality enforced; a changed reload overwrites;
- open painted and native sessions are skipped with a warning;
- silent (no callbacks) and coalesced.

Skin protocol (designed; tests land with the implementation):

- declared vs undeclared attributes, reserved shared names, `x-` namespace;
- typed value parse errors and validate-before-apply (no mutation on failure);
- handler silence (no callbacks) and display-only changes;
- child names, nesting and attribute validation; data replaced before
  `selected`;
- writer round-trip of `x-*` attributes and child lists;
- a third-party test widget implementing the protocol without the feature.

Writer and features:

- `parse(to_xml(parse(x))) == parse(x)`;
- default build has no `quick-xml`; `layout-xml` builds and tests;
- showcase `m7` builds, lays out the page, and survives a malformed reload.

## 15. Non-goals and follow-ups

v1 does not:

- create, remove, reorder or reparent widgets; change paint order;
- set context menus or narration row formatters; install a whole palette
  (text colour is a local since M7.8; the palette is a `.buitheme` document,
  §18);
- express row/item arrays **in v1** (the v2 design is §6.3; not implemented);
- position composite internals (tab headers, table columns, picker grids);
- use expressions in style or content values (content is literal);
- offer `max-*`, orientation breakpoints, stacks, grids or group offsets —
  anchors cover the layout arithmetic; readability comes from comments and
  the one-line entry;
- include files or bind content live (initial values only);
- use a file watcher or touch the file system from the library.

Likely follow-ups, in order of value: v1.1 local scalars (§6.1), v2 child
lists (§6.3), the compile-time mode (§16), a text-colour seam (needs a widget
API), per-entry comments preserved by the writer, `max-*` bands, and a
"layout owns rectangles" mode that snapshots/restores rects as well (needs an
explicit app-layout hand-off).

## 16. Two consumption modes: dynamic and compiled

**Implemented (M7.6, 2026-11-28): dynamic and compiled both run against one
schema.** The schema is one language; it can be consumed in two ways. Both
produce the same result; the choice is a build-time trade-off.

| | Dynamic (`layout-xml`) | Compiled (`layout-codegen`) |
|---|---|---|
| When XML is read | runtime, on first install and on reload | build time (proc macro, build script or CLI) |
| Runtime parser | `quick-xml`, `LayoutDocument::parse` | none; no `quick-xml` in the binary |
| Errors | diagnostics, previous layout kept | `compile_error!` with file:line:column |
| Hot reload / debug form | yes | no; requires a rebuild |
| Geometry | expressions evaluated per apply | emitted Rust expressions, folded by `rustc` |
| Inline Rust | rejected | allowed (`rust:` values, `<rust>` data bindings, optional `context = Type`) |
| Best for | development, theming, the debug form, slow-changing pages | release builds, large static pages, startup speed |

Both modes use the same `crate::skin` protocol (§6.2–6.4), the same
claims/baseline rules (§8.2), the same content timing (§6) and the same
diagnostic codes. A page can switch from one to the other without editing the
XML.

### 16.1 Compiled output

The generator emits ordinary Rust, not an opaque blob: a small module of
`static` data plus one `pub fn install` that can be `include!`d or expanded by a
proc macro. Constants are folded into the expressions and the runtime resolves
the descriptor through the *same* claim/content/skin code the dynamic applier
uses (`Ui::install_prepared`), so behaviour is identical by construction.

```rust
// Actual shape generated from a fixture (abridged); names are the codegen's
// business, the `compiled::*` types are the contract.
pub mod parity {
    static __rect_0_1: fn(Size) -> Result<Rect, RectError> = |s| {
        let x = (10.0); let w = (120.0); let y = (44.0); let h = (36.0);
        let left = x; let right = x + w; let top = y; let bottom = y + h;
        edges(left, top, right, bottom) // `edges` casts f64 → f32 like the applier
    };
    static __props_0_1: LazyLock<PropAttrs> =
        LazyLock::new(|| PropAttrs { tooltip: Some("样式来自文件".to_string()), ..Default::default() });
    static __entries_0: &[CompiledEntry] = &[ /* id, line, path, rect, props, style, content, skin */ ];
    pub static LAYOUT: CompiledLayout = CompiledLayout { /* name, generation, variants, targets */ };
    pub fn install<M, H: UiHost>(ui: &mut Ui<M>, sink: &mut H, palette: &Palette, client: Size)
        -> Result<LayoutReport, LayoutFailure>
    { ui.apply_compiled(&LAYOUT, palette, client, sink) }
}
```

The exact shape is the codegen's business, but the contract is:

- validate every id/scroll/child target before the first mutation (the
  generated function starts with a target check, so the atomic promise
  survives);
- restore claims, then apply only the selected variant;
- write content on the **first install only**; a later re-apply (resize or a
  breakpoint switch) must not rewrite it — the generated install keeps the
  same generation gate as the dynamic path, just stored statically;
- one damage flush at the end;
- a `context = Type` module computes the inline Rust that reads the context per
  apply (`CompiledValues`/`CompiledEntryValues`) and is applied through its
  `install` or `Ui::apply_compiled_with`; entries without context-dependent
  Rust keep their statics;
- no allocation in the paint path; generated data is static where the types
  allow it.

### 16.2 Inline Rust (implemented 2026-11-28)

Compiled mode can splice Rust into the document. This is the escape hatch for
values the XML cannot know: app constants, `cfg!`, theme accessors, computed
data. The file parses in both modes, but the dynamic applier refuses it
atomically — a runtime file must stay portable data — with
`xml.rust.unsupported`.

```xml
<widget id="m7_card"
        x="rust:PAGE_PAD"
        y="12"
        w="rust:cw - 2.0 * PAGE_PAD"
        h="30"
        visible="rust:!cfg!(test)"
        radius="medium"/>
```

- `rust:` is allowed on the schema's scalar attributes: geometry
  (`x`/`right`/`w`/`y`/`bottom`/`h`), the generic properties, the surface/font
  tokens and the shared content values. A widget-local `x-*` value is **not**
  spliced (it stays the raw string the widget sees), and `id` is never one.
- Geometry code is emitted inside the generated `fn(Size)`, where `cw`, `ch`,
  `cx`, `cy` exist as `f64` locals (equal to the space's width/height and their
  halves); it is evaluated on every apply, so it can follow the client size.
- Property, style and content code is emitted into the entry's `LazyLock`
  static or its content table: it is evaluated once, in the generated module's
  scope. The module starts with `use super::*;`, so the application's
  constants/types in the including module resolve by name (an explicit import
  wins on a clash). Types must match the field (`bool` for `visible`, `f32` for
  `font-size`, `RadiusAttr` for `radius`, `&'static str` for `text`/`label`,
  and so on); `rustc` checks them at the include site.
- The writer keeps the values (`rust:` + text) in the canonical document, so
  the generation hash changes with the code and a save round-trips.
- A layout **context** (`context = Type`, D146) makes the generated `install`
  take `context: &Type` and binds the name `context` in every inline-Rust
  value of that module:

  ```rust
  layout_xml!("layouts/main.buixml", context = AppLayout);
  layout_main::install(&mut ui, sink, &palette, client, &app)?;
  ```

  Geometry that contains inline Rust becomes a closure capturing the context
  (per entry, evaluated on every apply); its static `RectFn` reports
  `RectError::Context`, so a context module is applied through `install` (or
  `Ui::apply_compiled_with`). Properties, style and content that contain inline
  Rust are built per apply instead of in a `LazyLock`; entries without inline
  Rust keep their statics. The generated function is
  `fn __values_<variant>(context: &Type) -> Vec<CompiledEntryValues<'_>>`, one
  per variant, and `install` passes them all — claims are the union over every
  variant, so a context-dependent `visible` is claimed like a literal one.

- A **data binding** replaces one widget's declared child list with Rust:

  ```xml
  <layout id="base">
    <widget id="queue" x="0" y="0" w="320" h="240"/>
    <rust target="queue" code="context.rows()"/>
  </layout>
  ```

  `target` names a `<widget>` of the same variant (exactly once); `code` is a
  Rust expression that must produce `Vec<SkinElement>`. The list is the
  widget's child list: it goes through `validate_skin_children` /
  `apply_skin_children` exactly like `<item>`/`<row>` elements, is written in
  the children → content → attributes order, and only when the generation
  changes. A widget has either a static child list or a binding
  (`xml.rust.conflict`), and every variant that declares one must agree on the
  code (`xml.skin.conflict`), because content is generation-scoped. Without a
  context the expression is evaluated once into a
  `LazyLock<Vec<SkinElement>>` static (`CompiledEntry::rust_children`); with a
  context it is evaluated per apply. Element codes are attribute values, so
  `<`, `&` and quotes are XML-escaped (`&lt;`, `&amp;`, `&quot;`).

- Inline Rust makes the file source code. Review it like code; do not load such
  a file from an untrusted location. Dynamic mode never evaluates it.

### 16.3 Crates and build

The parser/model is factored so both modes share it and the runtime build
stays free of the parser:

| Crate / feature | Role | State |
|---|---|---|
| `bamboo-native-ui-layout-core` | value types + model + writer, `.buixml` reader behind its `parse` feature; no Win32, no `Ui` | landed |
| `bamboo-native-ui` `layout-xml` | enables core/`parse`; runtime `apply_layout`/`to_xml` | landed |
| `bamboo-native-ui` `layout-compiled` | `layout_xml::compiled` vocabulary + `Ui::apply_compiled`; no parser | landed |
| `bamboo-native-ui-layout-codegen` | XML → Rust source (library + small CLI) | landed |
| `bamboo-native-ui-layout-macros` | `layout_xml!("...buixml"[, module = name][, context = Type])` | landed |

Both consumption shapes are implemented: the CLI (check in the generated file,
`include!` it) and `layout_xml!` (expand in place, resolving the path against
`CARGO_MANIFEST_DIR`). A proc-macro crate must not depend on
`bamboo-native-ui` itself (cycle), which is why the pure model lives in
`layout-core`.

```powershell
# dynamic (development)
cargo run -p bamboo-native-ui-showcase --features m7

# compiled (release): an explicit step or a build script
cargo run -p bamboo-native-ui-layout-codegen -- layouts/layout-demo.buixml -o src/layout_demo.rs

# parity: the same file through both paths must produce the same rects,
# properties and content
cargo test -p bamboo-native-ui --features layout-xml,layout-compiled layout_parity
```

### 16.4 Efficiency expectations

- No XML parse, no validation walk and no model allocation at startup;
  constants and arithmetic fold in `rustc`.
- The generated function is straight-line code; LLVM can inline setters and
  eliminate branches that depend only on `cfg!`/constants.
- What remains is the container's id lookup and the widget setters, which the
  dynamic path also pays, plus the preflight/skin-validation walk that keeps
  the atomic promise (it cannot move to compile time because the widget
  vocabulary is a runtime property). A batched "resolve ids once" seam is a
  measured follow-up for both modes, not a prerequisite.
- Measured/implemented: property and surface statics are `LazyLock` (one
  allocation, then borrowed); content strings and `SkinElement` trees are
  materialized only when a generation writes content; geometry functions are
  native `f64` code, so `rustc` folds the constants visible in the golden
  output.
- Binary size grows with the generated code and inline Rust; that is the
  trade-off for the startup win. The dynamic path keeps the schema in a data
  file and the binary smaller.

## 17. Dynamic creation (M7.7, implemented)

**Implemented 2026-11-28 (D148).** The skin used to only address widgets the
application already created. Now a document can create them, with a **public
widget registry** so a third-party crate adds its own type without touching the
library.

```xml
<widget id="save" type="button" command="101" x="pad" y="8" w="96" h="32"/>
```

- `type` absent keeps the address-only behaviour. `type` present creates an id
  that does not exist yet, updates one this document already created (same
  `type`/`command`) or removes and re-creates it (different `type`/`command`,
  generation change only). A name matches `[a-z][a-z0-9_.-]*`; `command` is a
  `u32` and only legal with `type`.
- `WidgetRegistry<M>` (`crate::creation`, always compiled) maps a type name to
  a factory
  `Fn(&WidgetSpec, &FactoryCtx<'_, M>) -> Result<Box<dyn Widget<M>>, FactoryError>`
  and optionally carries a `CommandMap<M>` (`command id -> M`).
  `WidgetRegistry::standard()` registers `label`, `button`, `toggle`,
  `progress`, `slider`, `icon`, `badge`, `card`, `divider`, `kbd`, `avatar`
  and `spinner`; `Ui::set_widget_registry` installs the table before applying.
  A third-party crate registers `x-*`/`vendor.type` names from its own code.
- `WidgetSpec` exposes the id, the resolved rectangle, the shared content, the
  raw local attributes/children, the command, and the source line/path.
  `FactoryCtx` exposes the palette, the client size and the command table
  (`ctx.message(command)`).
- Local attributes, child lists, `SurfaceStyle`/font and `selected` reuse the
  existing §6.2/§6.3 applier, so a created widget has the same contract as an
  addressed one; a factory may construct with empty text because the file's
  content is written right after insertion. The factory only supplies what the
  file cannot: the application message behind `command` (`M` need not be
  `Clone` for a click; `toggle`/`slider` require it and report the command on
  every change — read the state through typed access, see D149).
- v1 stays flat: widgets plus `<scroll>` panels, no implicit container tree
  (D1). A created widget is client-space unless it declares `parent` (below);
  a general container is a later, explicit addition.

**Scroll membership (`parent="panel-id"`, implemented 2026-11-28).** A created
entry may name a registered scroll panel:

```xml
<widget id="row" type="label" parent="list" x="0" y="250" w="80" h="20"
        label="行"/>
```

Its rectangle resolves in the panel's content space and the applier appends the
id to the panel's child list (the panel cannot know an id that did not exist);
the container positions it at `content origin + rect - offset` like every other
child, and scrolling moves it. An *addressed* entry may declare `parent` as an
assertion: it must already be that panel's child. A declared `<scroll>` and an
application-registered panel both work. `parent` joins `type`/`command` in the
content-scoped, variant-invariant tuple, and a changed parent is a
remove-and-recreate. `xml.parent.missing` covers "not a scroll panel", "both a
widget and a panel" and "not that panel's child".

**Creation is planned before anything mutates.** The factories run in the
planning phase and their widgets are staged; the staged widget supplies its own
skin/content declarations to validation (`Ui::widget_ref` reads the tree, the
stager reads the box), so a later error drops every staged widget and the
previous layout stays. `install_prepared` inserts them **before the first
backdrop widget** (a modal `Scrim`; page layer) or at the end, in declaration
order, and only then captures claims — a created widget gets a normal claim
record. `Ui::set_visible` on an id that does not exist yet is remembered, so a
page model that hides the page before the layout runs keeps the document's
future widgets hidden too.

**Document order (implemented 2026-11-28).** A kept created widget keeps its
old insertion slot, so a reorder or a middle insertion would leave the page
layer in the previous order. On a generation change the planner computes the
resulting order (kept ids in their old order, then new ids); when that differs
from the document order, every kept id is removed and re-created so the block
is rebuilt by declaration order. A same-generation apply never restacks.

**Ownership, reload and removal (the user-called-out part).** A successful
reload that changes the generation is a **diff**, not an additive merge:

1. The document owns only the widgets it created (entries with `type`).
   Application-created widgets (no `type`) are never removed by a reload.
2. Every variant must declare every created id with the same
   `type`/`command`/`parent` (geometry may differ — that is the breakpoint);
   otherwise `xml.type.conflict`. The widget set therefore does not depend on
   the active variant.
3. Ids the new document no longer declares (or whose `type`/`command`/`parent`
   changed) are removed before insertion and re-created; `Ui::remove` already
   closes their overlays/menus/tooltips/drags, relocates focus by Tab position,
   commits a live edit session, forgets scroll children and clears
   Tab/a11y/drag registrations (D62).
4. Insertion is page-layer and happens before the rect/content/skin passes, so
   one frame covers old ∪ new rectangles (the created rects are invalidated
   explicitly; the host coalesces).
5. A **failed** reload removes nothing (atomic). A same-generation apply
   (resize, theme) removes nothing and creates nothing for ids already present.
6. An id that exists but is not document-owned is `xml.id.duplicate`; an
   absent registry or unknown type is `xml.type.unknown`; a factory that
   returns another id or refuses is `xml.type.create`; an interactive type
   without a resolvable command is `xml.command.missing`.
7. `Ui::clear_layout` removes every created widget along with handing the
   claimed fields back.
8. The debug form lists the installed registry: `DebugSnapshot` carries
   `widget_types` (registration order, empty without a registry) and
   `has_commands`.
9. Tests (`crates/native-ui/tests/layout_create.rs`) assert `ui.ids()` content
   after creation, `clear_layout`, a reload and a document reorder/middle
   insertion, the page-layer position, the panel child rect of a created
   scrolled widget and its movement under `set_scroll_offset`, the parent
   rejections, that a removed id is gone from the a11y tree, and the compiled
   module's parity through the checked-in fixture.

## 18. Theme documents (`.buitheme`, M7.8, implemented)

**Implemented 2026-11-28 (D150).** A second skin schema declares a whole
palette as data. It is a sibling grammar of `.buixml` (same XML dialect, same
diagnostic style, canonical text and generation hash), so an application that
already loads a layout file loads a theme file the same way.

```xml
<?xml version="1.0" encoding="utf-8"?>
<theme version="1" name="夜蓝" base="dark" accent="#7aa2f7">
  <color name="background" value="#16181d"/>
  <color name="text" value="#e8ebee"/>
</theme>
```

- `version` is required and must be `1` (`theme.version`).
- `name` is optional and human-readable; the canonical text keeps it trimmed.
- `base` is required (`light`/`dark`, `theme.base.value`). The document does
  not depend on the application's current mode.
- `accent` is optional and feeds `Palette::new` *before* derivation, so the
  accent-derived tokens (`accent-text`, `row-selected`, …) follow it. A
  `<color name="accent" …>` then sets that one field explicitly.
- `<color name value>` applies one palette field after derivation. `name` is a
  palette token in kebab-case (the `background` token vocabulary); `value` is a
  literal colour from the one grammar (alpha accepted and dropped — a palette
  colour is opaque). A palette token is not a literal here: the document
  *defines* the palette. Duplicates and unknown names are errors.
- Codes: `theme.syntax`, `theme.version`, `theme.structure`,
  `theme.unknown-element`, `theme.unknown-attribute`,
  `theme.missing-attribute`, `theme.base.value`, `theme.color.value`,
  `theme.color.unknown`, `theme.color.duplicate`. A file with an error changes
  nothing (the apply/swap is the application's step, as below).

**Where it lives.** The model (`ThemeDocument`/`ThemeBase`/`ThemeColor`), the
reader (`ThemeDocument::parse`, behind `layout-core/parse`) and the canonical
writer (`to_xml`, `generation`) live in `bamboo-native-ui-layout-core::theme`;
they need no Win32 and no `Palette`. `bamboo-native-ui`'s **`theme-xml`**
feature re-exports them and adds the mapping next to `Palette`:

```rust
use bamboo_native_ui::theme_xml::{ThemeDocument, ThemeDocumentExt};

let document = ThemeDocument::parse(text).expect("a valid .buitheme");
let palette = document.palette(&settings);   // Palette::new(settings, base) + overrides
window.set_theme(settings.clone(), palette.dark);
window.set_palette(palette);                 // whole-table install + one repaint
ui.apply_layout(&layout, &palette, client, sink)?;  // colour tokens resolve here
```

`theme-xml` enables only `layout-core/parse`; the layout feature and the theme
feature are independent. `set_palette_token(&mut Palette, name, colour)` is the
public write side of the token vocabulary. `WindowHandle::set_palette` installs
the table on the window's painter (page/chrome fills and the native chrome) and
`WindowHandle::palette()` reads it back; no new window state, the painter
already owned the palette.

**Showcase.** `crates/showcase/themes/night.buitheme` is checked in; the API
page's 主题文档 section loads it (`api_theme_doc`) and reports the name/base
(`api_theme_state`). The preset light/dark and accent controls clear the
document palette, so they keep working. The `--self-test` probe loads the file,
checks the derived palette (background/accent/text tokens and the palette the
window paints with) and restores the preset theme.

## FAQ

**Why is there no `<group>`?** `Ui` is a flat widget list; a group would be a
second hierarchy that the container would have to undo. Comments mark
sections, and constants plus anchors move a whole block (`pad`, `gap`,
`x="pad"`, `w="cw-2*pad"`). If grouping ever becomes necessary, it can be a
non-semantic element that only adds a coordinate offset; it is not in v1.

**Why `cw`/`ch` instead of `width`/`height`?** So the container's extent never
collides with the widget's own `w`/`h` attributes. Read them as
"container width/height"; they are on the quick-tour card.

**Why is content written only on reload?** So that resizing a window or
crossing a breakpoint never overwrites what the user just typed. The file is
an initial value; the application owns the live value afterwards.

**Why does a bad line reject the whole file?** Hot reload must not leave a
half-applied UI. One typo changes nothing, and the diagnostic says where. A
style token the widget cannot use is only a warning, so a generic layout can
target heterogeneous widgets.

**Why is text colour a local and not a shared attribute?** Because it is not
content: a widget paints its text, and the widgets that support an explicit
foreground declare `color` themselves (M7.8/D150), with the same grammar as
every other colour. They also expose silent setters, so the skin and the
application write one state. `text` stays the content attribute; `color` is the
foreground. A widget without a foreground API simply does not declare it.

**Why a separate `.buitheme` file instead of putting the palette in the layout
document?** Content is written once per generation and colour tokens resolve
against the palette the application passes; a palette is a different lifecycle
(the whole application chrome follows it, not just the layout) and a different
owner. A sibling schema keeps the layout grammar focused and lets the
application load, edit and persist a theme independently. See §18/D150.

**Why `background="none"` instead of `transparent="true"`?** One attribute,
one value: `background` names the fill, and `none` says there is no fill
(border and text stay). It reads like the thing it does and cannot conflict
with a colour.

**How do I add XML support for my own widget?** Implement only the skin
methods you need (§6.2): `skin_attrs` + `validate_skin_attr` +
`apply_skin_attr` for scalar state, and the children trio for data lists. Use
`x-` names. No registry, no library change, no `layout-xml` feature
dependency: the types live in `crate::skin` and are always compiled.

**Why two consumption modes?** Dynamic loading keeps hot reload and the debug
form; compile-time codegen removes the parser and lets `rustc` fold
expressions. Same schema, same semantics; pick per build or per page (§16).
A dev build can use dynamic and a release build compiled, without editing the
XML.

**Why is inline Rust only in compiled mode?** A runtime file has to stay
portable data and must never be executed from disk; the dynamic parser rejects
`rust:`. Compiled mode already runs through `rustc`, so splicing an expression
costs nothing extra and keeps the file type-checked.

**Can the generated code be inlined into my crate?** Yes. The generator emits
ordinary Rust; `include!` it from a build script or let the optional
`layout_xml!` proc macro expand it in place. That is also what makes inline
Rust practical — it is spliced into the same function body.
