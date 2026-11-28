# 控件

Bamboo Native UI 提供 62 个公开控件。每个页面都遵循相同的结构：
介绍、Rust 示例、XML 示例、属性表、事件章节，以及 XML 定义规则。
能够通过 `type="..."` 从布局文档创建的控件会特别说明；其余控件在 Rust 中创建，
并在文档中用 `id` 引用。

如果你要编写自己的控件，请从 [控件 trait](../api/widget-trait.md) 开始；如果你要从
`.buixml` 文件中定位这些控件，请参阅 [XML 布局编写指南](../guides/xml-layout-authoring.md)。

## 文本与内容

| 控件 | 摘要 |
|---|---|
| [Label](content/label.md) | 带链接样式和可选 chip 背景的静态文本或换行文本。 |
| [Icon](content/icon.md) | 单个居中的字形（Segoe MDL2 或任意文本字符）。 |
| [Badge](content/badge.md) | 带色调颜色和可选移除目标的小型状态 chip。 |
| [Divider](content/divider.md) | 水平或垂直分隔线，可选居中的标签。 |
| [Avatar](content/avatar.md) | 圆形首字母头像，或裁剪为圆形的图片，可选圆环。 |
| [Kbd](content/kbd.md) | 键盘快捷键提示 chip。 |
| [CopyableText](content/copyable-text.md) | 带复制提示和复制消息的只读文本。 |
| [MarqueeText](content/marquee-text.md) | 放不下时滚动的单行标题。 |
| [RichText](content/rich-text.md) | 带内联加粗、弱化和链接片段换行显示的文本。 |

## 输入

| 控件 | 摘要 |
|---|---|
| [Checkbox](inputs/checkbox.md) | 带标签的开关框，会报告新的勾选状态。 |
| [RadioButton](inputs/radio-button.md) | 独立单选框，会报告选中状态。 |
| [RadioGroup](inputs/radio-group.md) | 支持指针和键盘选择的垂直单选组。 |
| [SearchBox](inputs/search-box.md) | 带字形、提示文本、清除按钮和提交消息的搜索框。 |
| [TextEdit](inputs/text-edit.md) | 单行/多行字段，可为原生或框架绘制（别名 `PasswordBox`、`TextArea`）。 |
| [NumberInput](inputs/number-input.md) | 带范围、步进器、小数位数和自定义格式化/解析的整数字段（别名 `NumberStepper`）。 |
| [ComboBox](inputs/combo-box.md) | 带覆盖层选项列表的下拉列表。 |
| [MultiSelectList](inputs/multi-select-list.md) | 支持 Ctrl/Shift 多选的虚拟化可勾选列表。 |

## 控件

| 控件 | 摘要 |
|---|---|
| [Button](controls/button.md) | 带变体、字形和启用状态的标准点击按钮。 |
| [Toggle](controls/toggle.md) | 带可选侧边标签的开关（别名 `Switch`）。 |
| [Segmented](controls/segmented.md) | 可按选项禁用的分段单选条。 |
| [Tabs](controls/tabs.md) | 带可关闭标签页、徽标和标签对齐的标签条。 |
| [Slider](controls/slider.md) | 带步长、刻度和拖动回调的范围滑块。 |
| [ProgressBar](controls/progress-bar.md) | 确定或不确定进度条。 |
| [Pager](controls/pager.md) | 带页面标签的上一页/下一页控件。 |
| [ColorSwatch](controls/color-swatch.md) | 带标签的颜色矩形。 |

## 选择器

| 控件 | 摘要 |
|---|---|
| [ColorPicker](pickers/color-picker.md) | 带调色板弹出层和可选透明度条的颜色字段。 |
| [DatePicker](pickers/date-picker.md) | 带月历网格弹出层和范围限制的日期字段。 |
| [TimePicker](pickers/time-picker.md) | 带小时/分钟列弹出层和范围限制的时间字段。 |

## 数据与集合

| 控件 | 摘要 |
|---|---|
| [Sparkline](data/sparkline.md) | 用于数值序列的紧凑条形条。 |
| [Gauge](data/gauge.md) | 从零到一的开口弧形仪表。 |
| [RatingStars](data/rating-stars.md) | 可点击并支持键盘调整的星级评分。 |
| [ListView](data/list-view.md) | 虚拟化的标题/副标题/尾部行列表。 |
| [Table](data/table.md) | 支持排序、调整大小、重排序和内联编辑的虚拟化表格。 |
| [TreeView](data/tree-view.md) | 支持重命名和拖动移动的可展开层级结构。 |
| [PropertyGrid](data/property-grid.md) | 带类型化编辑器的分节键/值检查器。 |

## 导航

| 控件 | 摘要 |
|---|---|
| [Accordion](navigation/accordion.md) | 可折叠的区块堆栈（别名 `Collapsible`）。 |
| [Breadcrumb](navigation/breadcrumb.md) | 带不导航的当前页面的可点击路径。 |
| [NavRail](navigation/nav-rail.md) | 带可选图标的单选侧边栏。 |
| [Splitter](navigation/splitter.md) | 报告新位置的可拖动分栏分隔条。 |

## 反馈

| 控件 | 摘要 |
|---|---|
| [Spinner](feedback/spinner.md) | 由应用驱动的活动圆环。 |
| [ProgressRing](feedback/progress-ring.md) | 带可选百分比的圆形确定性指示器。 |
| [Banner](feedback/banner.md) | 带种类颜色、操作和关闭目标的内联通知。 |
| [TooltipCard](feedback/tooltip-card.md) | 页内提示面板，区别于原生宿主工具提示。 |
| [Scrim](feedback/scrim.md) | 启动覆盖层的变暗且阻止输入的背景。 |
| [Toast](feedback/toast.md) | 带持续时间、操作和错误标志的临时状态行（别名 `Snackbar`）。 |

## 显示

| 控件 | 摘要 |
|---|---|
| [Card](display/card.md) | 带内边距和强调的带框标题/主体表面。 |
| [GroupBox](display/group-box.md) | 带标题的字段集式框架。 |
| [StatCard](display/stat-card.md) | 带大号数值和说明文字的紧凑指标。 |
| [EmptyState](display/empty-state.md) | 居中的空视图标题和提示。 |
| [SectionTitle](display/section-title.md) | 小型弱化的章节标题。 |

## 播放器与媒体

| 控件 | 摘要 |
|---|---|
| [WaveformSeekBar](player/waveform-seek-bar.md) | 带拖拽回调的波形绘制进度条。 |
| [PlaybackTransport](player/playback-transport.md) | 随机/上一首/播放暂停/下一首/重复传输条。 |
| [CoverWall](player/cover-wall.md) | 带说明文字和选择的虚拟化封面网格。 |
| [PlaylistView](player/playlist-view.md) | 支持拖动和 Alt+方向键重排序的虚拟化曲目列表。 |

## 窗口装饰

| 控件 | 摘要 |
|---|---|
| [MenuBar](chrome/menu-bar.md) | 绘制式顶层菜单栏，其菜单项打开原生菜单。 |
| [MenuButton](chrome/menu-button.md) | 带标签、字形和原生下拉菜单的按钮。 |
| [SplitButton](chrome/split-button.md) | 主操作主体，外加包含备选菜单的尖角符号。 |
| [Toolbar](chrome/toolbar.md) | 带分隔符和溢出功能的固定单元格命令条。 |
| [StatusBar](chrome/status-bar.md) | 带可选进度和大小调整手柄的只读状态分段。 |
| [Ribbon](chrome/ribbon.md) | 带分组大/小命令磁贴的选项卡式功能区。 |

## 覆盖层与自定义

| 控件 | 摘要 |
|---|---|
| [Popover](overlay/popover.md) | 感知锚点、带尖角和视口限制的覆盖层卡片。 |
| [CustomWidget](custom/custom-widget.md) | 带输入、命中测试和无障碍钩子的闭包绘制控件。 |

## 别名

这些类型别名让草图可以使用它偏好的词汇；它们解析为相同的类型，
并记录在规范页面上。

| 别名 | 规范 |
|---|---|
| `Switch` | [Toggle](controls/toggle.md) |
| `NumberStepper` | [NumberInput](inputs/number-input.md) |
| `PasswordBox` | [TextEdit](inputs/text-edit.md) |
| `TextArea` | [TextEdit](inputs/text-edit.md) |
| `Collapsible` | [Accordion](navigation/accordion.md) |
| `Snackbar` | [Toast](feedback/toast.md) |

## 另见

- [控件 trait](../api/widget-trait.md)
- [控件注册表与创建](../api/widget-registry-and-creation.md)
- [XML 元素参考](../reference/xml-elements.md)
- [主题](../guides/theming.md)
