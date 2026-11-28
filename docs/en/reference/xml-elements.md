# XML elements

> Every element, attribute and accepted value in a `.buixml` document, plus the
> per-widget skin vocabulary and the stable diagnostic codes.

## Document skeleton

```xml
<?xml version="1.0" encoding="utf-8"?>
<layouts version="1">
  <const name="pad" value="16"/>
  <layout id="base" min-width="0" min-height="0">
    <scroll id="page" x="0" y="0" w="cw" h="ch" content-height="auto"/>
    <widget id="title" x="pad" y="pad" w="cw-2*pad" h="28" label="Title"/>
  </layout>
</layouts>
```

| Element | Attributes | Required | Notes |
|---|---|---|---|
| `<layouts>` | `version` | `version` = `1` | Root; no DTD, CDATA, namespaces or processing instructions. |
| `<layout>` | `id`, `min-width`, `min-height` | `id` | One variant must be `(0, 0)`; minimum pairs unique. |
| `<const>` | `name`, `value` | both | `name` matches `[a-z][a-z0-9_]*`; `value` is a constant expression. |
| `<widget>` | See below | `id` | One entry per id per variant. |
| `<scroll>` | `id`, `x`/`right`/`w`, `y`/`bottom`/`h`, `content-height` | `id` | Addresses a panel registered with `Ui::set_scroll_view`. |
| `<rust>` | `target`, `code` | both | Compiled mode only; `<rust>` must be empty. |

`<scroll content-height>` accepts a number/expression (exact height), `auto`
(grow to fit; never shrinks), or omission (keep).

### Variants

Selection: among `<layout>` elements whose `min-width` and `min-height` are both
`<=` the client size (DIPs), the greatest `min-width`, then the greatest
`min-height` wins; otherwise the `(0, 0)` variant. Geometry, properties and
style may differ between variants. Content and local skin data must be equal in
every variant that declares them.

## `<widget>` attributes

### Identity and creation

| Attribute | Accepted values | Notes |
|---|---|---|
| `id` | non-empty string | The `WidgetId` name. Required. |
| `type` | `[a-z][a-z0-9_.-]*` | Creates the id when it does not exist; needs a `WidgetRegistry`. |
| `command` | non-negative integer (`u32`) | Only with `type`; resolved by the registry's `CommandMap`. |
| `parent` | a scroll panel id | Membership for a created widget; an assertion for an addressed one. |

### Geometry

Give exactly two of `x`, `right`, `w` and exactly two of `y`, `bottom`, `h`:

| Axis | Accepted pairs | Result |
|---|---|---|
| Horizontal | `x` + `w` | left edge and width |
| Horizontal | `right` + `w` | `x = cw - right - w` |
| Horizontal | `x` + `right` | stretched width |
| Vertical | `y` + `h` | top edge and height |
| Vertical | `bottom` + `h` | `y = ch - bottom - h` |
| Vertical | `y` + `bottom` | stretched height |

Values are expressions. `w`/`h` must be `>= 0`; `right`/`bottom` must be
`>= 0`; `x`/`y` may be negative. Zero size is a warning.

```text
expr   := term (('+' | '-') term)*
term   := factor (('*' | '/') factor)*
factor := number | number '%' | variable | '(' expr ')' | '-' factor
```

| Variable | Value |
|---|---|
| `left`, `top` | `0` |
| `right`, `bottom` | space width / height |
| `cw`, `ch` | space width / height |
| `cx`, `cy` | space centre |
| a `<const>` name | its value |

`N%` is a percentage of the attribute's axis (`x`/`w`/`right` use `cw`;
`y`/`h`/`bottom` use `ch`). `rust:` prefixes an inline-Rust expression for the
compiled mode only.

### Properties

| Attribute | Accepted values |
|---|---|
| `visible`, `enabled`, `interactive`, `focusable`, `read-only`, `expose` | `true`, `false`, `1`, `0` |
| `focus-order` | integer (smaller first) |
| `tooltip`, `name`, `description`, `automation-id` | string |
| `role` | a role token below |
| `cursor` | `arrow`, `hand`, `i-beam`, `size-we`, `size-ns` |

Role tokens: `button`, `toggle`, `slider`, `text-field`, `number-field`,
`radio-group`, `tab-list`, `list`, `multi-select-list`, `radio-item`,
`tab-item`, `list-item`, `tree`, `tree-item`, `grid`, `grid-cell`,
`column-header`, `heading`, `group`, `separator`, `text`, `combo-box`,
`progress-bar`, `menu-item`, `menu`, `menu-bar`, `tool-bar`, `status-bar`.

### Style tokens

| Attribute | Accepted values |
|---|---|
| `radius` | number `>= 0`, `small`, `medium`, `large`, `pill` |
| `corners` | four non-negative numbers `tl tr br bl` (spaces or commas) |
| `background` | palette token, color literal, `none` |
| `font-size` | number `> 0` (DIPs) |
| `font-family` | non-empty string |

`radius` and `corners` are mutually exclusive. A widget without a surface
reports `xml.style.unsupported` (warning) and skips the surface token.

### Content

| Attribute | Value | Capability |
|---|---|---|
| `text` | string | `TEXT` |
| `label` | string | `LABEL` |
| `value` | finite number | `VALUE` |
| `progress` | number, clamped to `0..=1` | `PROGRESS` |
| `number` | integer | `NUMBER` |
| `checked` | bool | `TOGGLED` |
| `selected` | non-negative integer | `SELECTION` |

`text` and `label` conflict on one entry; `value`, `progress` and `number`
conflict with each other. Content is written only on a generation change.

### Inline Rust (`rust:`)

The compiled mode accepts `rust:` on every scalar attribute named in
`scalar_attr_name` (properties, style, content) and on geometry values. The
dynamic applier refuses it with `xml.rust.unsupported`. `<rust target="..."
code="..."/>` replaces one widget's child list with a `Vec<SkinElement>` in the
compiled mode only.

## Content capabilities by widget

| Capability | Widgets |
|---|---|
| `TEXT` | `TextEdit`, `SearchBox`, `CopyableText`, `MarqueeText` |
| `LABEL` | `Label`, `Button`, `Badge`, `Divider`, `Avatar`, `Kbd`, `SectionTitle`, `GroupBox`, `StatCard`, `Banner`, `Checkbox`, `RadioButton`, `ColorSwatch`, `RichText`, `CopyableText`, `MarqueeText` |
| `VALUE` | `Slider`, `RatingStars`, `WaveformSeekBar` |
| `PROGRESS` | `ProgressBar`, `StatusBar` |
| `NUMBER` | `NumberInput` |
| `TOGGLED` | `Toggle`, `Checkbox` |
| `SELECTION` | `ComboBox`, `Segmented`, `Tabs`, `Pager`, `CoverWall`, `RadioGroup`, `ListView`, `MultiSelectList`, `Accordion`, `Breadcrumb`, `NavRail`, `PlaylistView`, `Table`, `TreeView` |

A widget not listed for a capability reports `xml.content.unsupported`
(warning) and the attribute is skipped. `Checkbox` accepts both `label` and
`checked`; `StatusBar` accepts `text` and `progress`. Every capability is
published by the widget (`Widget::content_caps`), so aliases inherit the same
set.

## Per-widget local attributes

A widget declares these through the skin protocol. Any other non-schema name is
`xml.skin.unknown` at apply time. Names starting with `x-` belong to
application widgets.

| Widget | Attribute | Kind / accepted values |
|---|---|---|
| `Accordion` | `open` | number `>= 0` (section index) |
| `Avatar` | `initials` | text |
| `Avatar` | `tint` | color |
| `Avatar` | `ring` | bool |
| `Avatar` | `image` | non-negative integer `ImageId` |
| `Badge` | `tone` | `neutral`, `accent`, `success`, `warning`, `danger` (case-insensitive) |
| `Banner` | `body` | text |
| `Breadcrumb` | `separator` | one character |
| `Card` | `title`, `body` | text |
| `Card` | `emphasis` | bool |
| `ColorPicker` | `color` | color (alpha kept) |
| `ColorPicker` | `open` | bool |
| `ColorSwatch` | `color` | color |
| `CopyableText` | `color` | color |
| `CopyableText` | `text-style` | `body`, `small`, `title` |
| `CoverWall` | `gap`, `corner-radius`, `caption-height` | number `>= 0` |
| `CoverWall` | `empty-text` | text |
| `DatePicker` | `date` | text `YYYY-MM-DD`, month `1..=12`, day `>= 1` |
| `DatePicker` | `open` | bool |
| `Icon` | `glyph` | exactly one character |
| `Icon` | `tint` | color |
| `Kbd` | `shortcut` | text |
| `Label` | `color` | color |
| `Label` | `text-style` | `body`, `small`, `title` |
| `ListView` | `row-height` | number `>= 24` |
| `ListView` | `alternating` | bool |
| `ListView` | `empty-text` | text |
| `MarqueeText` | `offset` | number `>= 0` |
| `MarqueeText` | `color` | color |
| `MarqueeText` | `text-style` | `body`, `small`, `title` |
| `NavRail` | `row-height` | number `>= 18` |
| `NumberInput` | `prefix`, `suffix` | text |
| `PlaybackTransport` | `playing`, `shuffle` | bool |
| `PlaybackTransport` | `repeat` | `off`/`none`, `all`/`list`, `one`/`single` |
| `PlaylistView` | `row-height` | number `>= 24` |
| `PlaylistView` | `alternating` | bool |
| `PlaylistView` | `empty-text` | text |
| `PropertyGrid` | `row-height` | number `>= 16` |
| `PropertyGrid` | `header-height` | number `>= 0` |
| `PropertyGrid` | `key-width` | number `>= 40` |
| `PropertyGrid` | `grid-lines` | bool |
| `PropertyGrid` | `empty-text` | text |
| `RichText` | `wrap` | bool |
| `RichText` | `align` | `left`, `center`, `right` |
| `RichText` | `text-style` | `body`, `small`, `title` |
| `RichText` | `color` | color |
| `SearchBox` | `cue` | text |
| `SearchBox` | `color` | color |
| `SectionTitle` | `color` | color |
| `Sparkline` | `color` | color |
| `Sparkline` | `gap` | number `>= 0` |
| `Spinner` | `phase` | number |
| `Spinner` | `animating` | bool |
| `Table` | `row-height` | number `>= 16` |
| `Table` | `header-height`, `cell-padding` | number `>= 0` |
| `Table` | `striped`, `grid-lines`, `editable` | bool |
| `Table` | `empty-text` | text |
| `Tabs` | `label-align` | `left`, `center`, `right` |
| `TextEdit` | `cue` | text |
| `TextEdit` | `color` | color |
| `TimePicker` | `time` | text `HH:MM`, hour `<= 23`, minute `<= 59` |
| `TimePicker` | `open` | bool |
| `Toast` | `message` | text |
| `Toast` | `error` | bool |
| `TreeView` | `row-height` | number `>= 16` |
| `TreeView` | `indent` | number `>= 8` |
| `TreeView` | `renameable`, `draggable` | bool |
| `TreeView` | `empty-text` | text |

These widgets declare no local attributes and no child elements: `Button`,
`Toggle`, `Slider`, `ProgressBar`, `Divider`, `Checkbox`, `RadioButton`,
`RadioGroup`, `MultiSelectList`, `GroupBox`, `StatCard`, `EmptyState`, `Gauge`,
`RatingStars`, `Splitter`, `WaveformSeekBar`, `Toolbar`, `Ribbon`, `MenuBar`,
`MenuButton`, `SplitButton`, `Popover`, `CustomWidget`, `ProgressRing`,
`Scrim`, `Snackbar`, `TooltipCard`. `ComboBox`, `Segmented`, `Pager` and
`StatusBar` declare no local attributes but do take the child lists above. All
of these still accept geometry, generic properties, style tokens, font tokens
and their content capabilities.

Aliases share the base widget's declarations: `Collapsible` = `Accordion`,
`PasswordBox`/`TextArea` = `TextEdit`, `Switch` = `Toggle`.

## Child elements

A child list replaces the widget's data model; it is not a live binding. Each
child attribute is a raw string validated by the widget.

| Parent | Child | Attributes |
|---|---|---|
| `ListView` | `<item>` | `title` (required, non-empty), `subtitle`, `trailing`, `current` (bool), `disabled` (bool) |
| `PlaylistView` | `<track>` | same as `<item>` |
| `ComboBox` | `<option>` | `label` (required, non-empty) |
| `Tabs`, `Segmented` | `<option>` | `label` (required, non-empty) |
| `CoverWall` | `<cover>` | `title` (required, non-empty), `subtitle`, `image` (integer `ImageId`), `corner-radius` (number) |
| `StatusBar` | `<segment>` | `text` (required, non-empty), `icon` (one character), `width` (number, clamped `>= 20`), `align` (`left`/`center`/`right`) |
| `Table` | `<column>` | `title` (required, non-empty), `width` (number `>= 0`), `flex` (bool), `align` (`left`/`center`/`right`) |
| `Table` | `<row>` | `disabled` (bool); children `<cell text="..."/>` |
| `Table` | `<cell>` | `text` (required; may be empty) |
| `TreeView` | `<node>` | `label` (required, non-empty), `glyph` (one character), `secondary`, `expanded` (bool), `disabled` (bool); nested `<node>` |
| `PropertyGrid` | `<section>` | `title` (text); children `<row>` |
| `PropertyGrid` | `<row>` | `key` (required), `value`, `kind`, `read-only` (bool), plus the kind's parameters |
| `Pager` | `<page>` | `label` (required, non-empty); the page count becomes the list length |
| `Breadcrumb` | `<step>` | `label` (required, non-empty) |
| `NavRail` | `<step>` | `label` (required, non-empty), `icon` (one character) |
| `RichText` | `<run>` | `text` (required, non-empty), `style` (`normal`/`strong`/`muted`/`link`), `link` (`u32`, link runs only) |
| `Sparkline` | `<point>` | `value` (required, finite number) |

`<row kind="...">` selects its editor and allowed parameters:

| `kind` | Parameters |
|---|---|
| `read-only` (default), `text` | `key`, `value`, `kind`, `read-only` |
| `number` | plus `min`, `max`, `step` (`> 0`), `decimals` (integer `0..=10`) |
| `choice` | plus `options` (pipe-separated, every option non-empty) |
| `check` | plus `on`, `off` (both non-empty) |
| `rating` | plus `stars` (integer `1..=10`) |

## Registered creation types

`WidgetRegistry::standard()` registers these. `command` is required for the
interactive types.

| `type` | Requires `command` | Constructor |
|---|---|---|
| `label` | no | `Label` |
| `button` | yes | `Button` |
| `toggle` | yes | `Toggle` (reports on every change; `M: Clone`) |
| `slider` | yes | `Slider` (reports on every change; `M: Clone`) |
| `progress` | no | `ProgressBar` |
| `icon` | no | `Icon` |
| `badge` | no | `Badge` |
| `card` | no | `Card` |
| `divider` | no | `Divider` |
| `kbd` | no | `Kbd` |
| `avatar` | no | `Avatar` |
| `spinner` | no | `Spinner` |

A created widget is page-layer, belongs to the document, and must be declared
with the same `type`/`command`/`parent` in every variant.

## Limits

| Limit | Value |
|---|---|
| `.buixml` file size | 4 MiB |
| Entries per document | 50 000 |
| Child elements per widget | 10 000 |
| Diagnostics per parse | 100 |
| Expression nesting | 32 |
| `.buitheme` file size | 1 MiB |
| `<color>` overrides per theme | 512 |

No DTD, CDATA, namespaces, includes, file watcher or event bindings. Style and
content values are literals, never expressions. Child attributes are literal
strings except geometry and constants.

## Diagnostics

Errors reject the whole document; warnings are reported and the rest applies.
Codes are stable.

| Code | Severity | Cause |
|---|---|---|
| `xml.syntax` | error | Malformed XML. |
| `xml.version` | error | Missing or unsupported `version`. |
| `xml.structure` | error | Wrong nesting, stray text, DTD, size/entry limit. |
| `xml.unknown-element` | error | Element outside the schema. |
| `xml.unknown-attribute` | error | Attribute outside the schema (non-widget elements). |
| `xml.missing-attribute` | error | Required attribute absent. |
| `xml.attr.value` | error | Malformed number/bool/token/color/name. |
| `xml.attr.conflict` | error | Mutually exclusive attributes (`radius`+`corners`, content pairs, ...). |
| `xml.duplicate-layout-id` | error | Two variants share `id`. |
| `xml.duplicate-widget-id` | error | Two entries in one variant share `id`. |
| `xml.layout.base` | error | No `(0,0)` variant, or duplicate minimums. |
| `xml.const.invalid` | error | Constant name outside `[a-z][a-z0-9_]*`. |
| `xml.const.reserved` | error | Constant shadows a space variable. |
| `xml.const.duplicate` | error | Constant declared twice. |
| `xml.const.undefined` | error | Unknown constant name. |
| `xml.const.vars` | error | Constant uses a space variable or `%`. |
| `xml.expr.syntax` | error | Malformed expression. |
| `xml.expr.div-zero` | error | Division by zero. |
| `xml.expr.non-finite` | error | Non-finite result or literal. |
| `xml.expr.depth` | error | Expression deeper than 32 levels. |
| `xml.geom.underdetermined` | error | Geometry supplies fewer than two values per axis. |
| `xml.geom.overdetermined` | error | Geometry supplies all three values on an axis. |
| `xml.geom.negative` | error | Negative `w`/`h`/stretched side, negative `right`/`bottom`. |
| `xml.geom.invalid` | error | The rectangle is not valid at the current size. |
| `xml.geom.non-finite` | error | A rectangle edge is not finite. |
| `xml.id.unknown-widget` | error | Addressed widget id does not exist. |
| `xml.id.unknown-scroll` | error | `<scroll>` id is not a registered panel. |
| `xml.id.ambiguous` | error | An id resolves to more than one target. |
| `xml.id.duplicate` | error | A created entry's id exists but is not document-owned. |
| `xml.parent.missing` | error | `parent` is not a scroll panel, or the id is not its child. |
| `xml.content.conflict` | error | Variants disagree on an id's content. |
| `xml.content.unsupported` | warning | The widget does not publish that content capability. |
| `xml.content.session-open` | warning | The target is being edited; the content write is skipped. |
| `xml.skin.conflict` | error | Variants disagree on an id's local skin data. |
| `xml.skin.reserved` | error | A widget declaration reuses a shared name. |
| `xml.skin.unknown` | error | Local attribute/child the widget does not declare. |
| `xml.skin.value` | error | A declared value fails its kind or the widget's validation. |
| `xml.style.unsupported` | warning | Surface token on a widget without a surface. |
| `xml.value.clamped` | warning | A value was clamped to the widget's range. |
| `xml.value.zero-size` | warning | `w` or `h` evaluates to zero. |
| `xml.rust.unsupported` | error | Inline Rust in the dynamic mode. |
| `xml.rust.context` | error | A context rectangle applied through the static descriptor. |
| `xml.rust.duplicate` | error | Two `<rust>` bindings target one widget. |
| `xml.rust.target` | error | `<rust>` targets an id the variant does not declare. |
| `xml.rust.conflict` | error | `<rust>` and a static child list disagree. |
| `xml.type.unknown` | error | `type` without a registry or without that registered type. |
| `xml.type.create` | error | A factory refused the entry or returned another id. |
| `xml.type.conflict` | error | Variants disagree on a created id's `type`/`command`/`parent`. |
| `xml.command.missing` | error | An interactive type has no resolvable command. |

## See also

- [XML layout authoring](../guides/xml-layout-authoring.md)
- [Palette tokens](palette-tokens.md)
- [Dynamic XML quickstart](../quickstart/05-xml-dynamic.md)
- [Compiled XML quickstart](../quickstart/06-xml-compiled.md)
- [The Ui container](../api/ui-container.md)
- [Debugging](../guides/debugging.md)
