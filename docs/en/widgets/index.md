# Widgets

Bamboo Native UI ships 62 public widgets. Every page follows the same shape:
introduction, a Rust example, an XML example, a property table, an Events
section, and the XML definition rules. Widgets that can be created from a
layout document by `type="..."` say so; the rest are created in Rust and
addressed by `id` from the document.

Start with the [widget trait](../api/widget-trait.md) if you are writing your
own, and with the [XML layout authoring guide](../guides/xml-layout-authoring.md)
if you are positioning these widgets from a `.buixml` file.

## Text and content

| Widget | Summary |
|---|---|
| [Label](content/label.md) | Static or wrapping text with link styling and an optional chip background. |
| [Icon](content/icon.md) | A single centered glyph (Segoe MDL2 or any text character). |
| [Badge](content/badge.md) | A small status chip with a tone and an optional remove target. |
| [Divider](content/divider.md) | A horizontal or vertical rule, optionally with a centered label. |
| [Avatar](content/avatar.md) | Circular initials avatar or cropped image with an optional ring. |
| [Kbd](content/kbd.md) | A keyboard-shortcut hint chip. |
| [CopyableText](content/copyable-text.md) | Read-only text with a copy affordance and a copy message. |
| [MarqueeText](content/marquee-text.md) | A single-line title that scrolls when it does not fit. |
| [RichText](content/rich-text.md) | Wrapped text with inline strong, muted and link runs. |

## Input

| Widget | Summary |
|---|---|
| [Checkbox](inputs/checkbox.md) | Labelled on/off box that reports the new checked state. |
| [RadioButton](inputs/radio-button.md) | Standalone radio button that reports selection. |
| [RadioGroup](inputs/radio-group.md) | Vertical radio group with pointer and keyboard selection. |
| [SearchBox](inputs/search-box.md) | Search field with glyph, cue, clear button and a commit message. |
| [TextEdit](inputs/text-edit.md) | Single/multiline field, native or framework-painted (aliases `PasswordBox`, `TextArea`). |
| [NumberInput](inputs/number-input.md) | Integer field with range, steppers, decimals and custom format/parse (alias `NumberStepper`). |
| [ComboBox](inputs/combo-box.md) | Drop-down list with an overlay option list. |
| [MultiSelectList](inputs/multi-select-list.md) | Virtualized checkable list with Ctrl/Shift multi-selection. |

## Controls

| Widget | Summary |
|---|---|
| [Button](controls/button.md) | Standard click button with variants, glyphs and enabled state. |
| [Toggle](controls/toggle.md) | On/off switch with optional side labels (alias `Switch`). |
| [Segmented](controls/segmented.md) | Segmented single-choice strip with per-option disabling. |
| [Tabs](controls/tabs.md) | Tab strip with closable tabs, badges and label alignment. |
| [Slider](controls/slider.md) | Range slider with steps, ticks and drag callbacks. |
| [ProgressBar](controls/progress-bar.md) | Determinate or indeterminate progress bar. |
| [Pager](controls/pager.md) | Previous/next page control with a page label. |
| [ColorSwatch](controls/color-swatch.md) | Color rectangle with a label. |

## Pickers

| Widget | Summary |
|---|---|
| [ColorPicker](pickers/color-picker.md) | Color field with a palette popup and optional alpha strip. |
| [DatePicker](pickers/date-picker.md) | Date field with a month-grid popup and range clamping. |
| [TimePicker](pickers/time-picker.md) | Time field with an hour/minute column popup and range clamping. |

## Data and collections

| Widget | Summary |
|---|---|
| [Sparkline](data/sparkline.md) | Compact bar strip for a numeric series. |
| [Gauge](data/gauge.md) | Open-arc zero-to-one gauge. |
| [RatingStars](data/rating-stars.md) | Clickable star rating with keyboard adjustment. |
| [ListView](data/list-view.md) | Virtualized title/subtitle/trailing row list. |
| [Table](data/table.md) | Virtualized table with sorting, resizing, reordering and inline editing. |
| [TreeView](data/tree-view.md) | Expandable hierarchy with rename and drag moves. |
| [PropertyGrid](data/property-grid.md) | Sectioned key/value inspector with typed editors. |

## Navigation

| Widget | Summary |
|---|---|
| [Accordion](navigation/accordion.md) | Collapsible section stack (alias `Collapsible`). |
| [Breadcrumb](navigation/breadcrumb.md) | Clickable path with a non-navigating current page. |
| [NavRail](navigation/nav-rail.md) | Single-selection sidebar with optional icons. |
| [Splitter](navigation/splitter.md) | Draggable pane divider reporting its new position. |

## Feedback

| Widget | Summary |
|---|---|
| [Spinner](feedback/spinner.md) | App-driven activity ring. |
| [ProgressRing](feedback/progress-ring.md) | Circular determinate indicator with optional percentage. |
| [Banner](feedback/banner.md) | Inline notification with a kind color, action and close target. |
| [TooltipCard](feedback/tooltip-card.md) | In-page hint panel, distinct from the native host tooltip. |
| [Scrim](feedback/scrim.md) | Dimmed, input-blocking backdrop that starts the overlay layer. |
| [Toast](feedback/toast.md) | Transient status line with duration, action and error flag (alias `Snackbar`). |

## Display

| Widget | Summary |
|---|---|
| [Card](display/card.md) | Framed title/body surface with padding and emphasis. |
| [GroupBox](display/group-box.md) | Titled fieldset-style frame. |
| [StatCard](display/stat-card.md) | Compact metric with a large value and a caption. |
| [EmptyState](display/empty-state.md) | Centered empty-view title and hint. |
| [SectionTitle](display/section-title.md) | Small muted section heading. |

## Player and media

| Widget | Summary |
|---|---|
| [WaveformSeekBar](player/waveform-seek-bar.md) | Waveform-drawn seek bar with scrub callbacks. |
| [PlaybackTransport](player/playback-transport.md) | Shuffle/previous/play-pause/next/repeat transport strip. |
| [CoverWall](player/cover-wall.md) | Virtualized cover grid with captions and selection. |
| [PlaylistView](player/playlist-view.md) | Virtualized track list with drag and Alt+arrow reorder. |

## Chrome

| Widget | Summary |
|---|---|
| [MenuBar](chrome/menu-bar.md) | Painted top-level menu bar whose items open native menus. |
| [MenuButton](chrome/menu-button.md) | Button with a label, glyph and native drop-down menu. |
| [SplitButton](chrome/split-button.md) | Primary-action body plus a chevron menu of alternatives. |
| [Toolbar](chrome/toolbar.md) | Fixed-cell command strip with separators and overflow. |
| [StatusBar](chrome/status-bar.md) | Read-only status segments with optional progress and size grip. |
| [Ribbon](chrome/ribbon.md) | Tabbed ribbon with grouped large/small command tiles. |

## Overlay and custom

| Widget | Summary |
|---|---|
| [Popover](overlay/popover.md) | Anchor-aware overlay card with notch and viewport clamping. |
| [CustomWidget](custom/custom-widget.md) | Closure-painted widget with input, hit-test and a11y hooks. |

## Aliases

These type aliases exist so a sketch can use the vocabulary it prefers; they
resolve to the same types and are documented on their canonical page.

| Alias | Canonical |
|---|---|
| `Switch` | [Toggle](controls/toggle.md) |
| `NumberStepper` | [NumberInput](inputs/number-input.md) |
| `PasswordBox` | [TextEdit](inputs/text-edit.md) |
| `TextArea` | [TextEdit](inputs/text-edit.md) |
| `Collapsible` | [Accordion](navigation/accordion.md) |
| `Snackbar` | [Toast](feedback/toast.md) |

## See also

- [Widget trait](../api/widget-trait.md)
- [Widget registry and creation](../api/widget-registry-and-creation.md)
- [XML elements reference](../reference/xml-elements.md)
- [Theming](../guides/theming.md)
