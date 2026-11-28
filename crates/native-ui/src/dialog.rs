// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Native dialogs, in two presentations.
//!
//! - **Modal** (the default): the owner window is disabled while the dialog is
//!   up, so the user must close it before using the application again.
//! - **Parallel** (`.modal(false)`): the dialog has no owner, so the
//!   application stays enabled and usable while it is open.
//!
//! Both kinds return their result asynchronously: a worker thread hosts the
//! native dialog (a `MessageBox`, the shell file picker, the font/color
//! choosers or the date/time picker), and the answer is delivered to
//! [`crate::Host::dialog_finished`]
//! on the UI thread after the dialog closes. The UI thread never enters the
//! dialog's nested message loop, so paints and timers keep working for a
//! parallel dialog and no window state is borrowed twice for a modal one.
//!
//! A modal dialog disables its owner across thread boundaries; both message
//! loops must stay responsive while it is up. The framework's UI thread always
//! pumps, and the dialog thread is inside the native dialog's own loop until it
//! closes, so the required condition holds.

use crate::color::{Color, Rgba};

/// Correlates a dialog request with the result delivered to
/// [`crate::Host::dialog_finished`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DialogId(pub u32);

/// Icon shown by a message dialog.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MessageIcon {
    #[default]
    Info,
    Warning,
    Error,
    Question,
}

/// Button set shown by a message dialog.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MessageButtons {
    #[default]
    Ok,
    OkCancel,
    YesNo,
    YesNoCancel,
    RetryCancel,
    AbortRetryIgnore,
}

/// A custom button on a message dialog: its label (with `&` access keys) and
/// the nonzero command id reported back through
/// [`MessageAnswer::Custom`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MessageButton {
    pub(crate) label: String,
    pub(crate) command: u32,
}

impl MessageButton {
    pub fn new(label: impl Into<String>, command: u32) -> Self {
        Self {
            label: label.into(),
            command,
        }
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn command(&self) -> u32 {
        self.command
    }
}

/// The button the user pressed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MessageAnswer {
    Ok,
    Cancel,
    Yes,
    No,
    Retry,
    Abort,
    Ignore,
    /// One of [`MessageDialog::button`]'s custom buttons (its command id).
    Custom(u32),
}

/// Which button owns the keyboard focus when the dialog opens.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MessageDefault {
    #[default]
    First,
    Second,
    Third,
}

/// Text, icon and buttons of a platform message box.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MessageDialog {
    pub(crate) text: String,
    pub(crate) title: String,
    pub(crate) icon: MessageIcon,
    pub(crate) buttons: MessageButtons,
    pub(crate) default_button: MessageDefault,
    pub(crate) modal: bool,
    /// Custom buttons: when non-empty the dialog is a task dialog with these
    /// buttons instead of the standard [`MessageButtons`] set.
    pub(crate) custom_buttons: Vec<MessageButton>,
    /// Optional bold main instruction above the text (task-dialog path).
    pub(crate) instruction: String,
    /// Optional `.ico` shown as the dialog's main icon (task-dialog path).
    pub(crate) icon_file: Option<String>,
}

impl MessageDialog {
    /// An informational `OK` dialog, modal by default.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            title: String::new(),
            icon: MessageIcon::Info,
            buttons: MessageButtons::Ok,
            default_button: MessageDefault::First,
            modal: true,
            custom_buttons: Vec::new(),
            instruction: String::new(),
            icon_file: None,
        }
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    pub fn icon(mut self, icon: MessageIcon) -> Self {
        self.icon = icon;
        self
    }

    pub fn buttons(mut self, buttons: MessageButtons) -> Self {
        self.buttons = buttons;
        self
    }

    pub fn default_button(mut self, default_button: MessageDefault) -> Self {
        self.default_button = default_button;
        self
    }

    /// Add a custom button. A dialog with at least one custom button is shown
    /// as a task dialog: the labels are the application's (with `&` access
    /// keys), the selected id arrives as [`MessageAnswer::Custom`], and the
    /// standard [`MessageDialog::buttons`] set is ignored. Escape and the
    /// title-bar close report [`MessageAnswer::Cancel`].
    pub fn button(mut self, label: impl Into<String>, command: u32) -> Self {
        self.custom_buttons.push(MessageButton::new(label, command));
        self
    }

    /// Bold main instruction above the text (task-dialog path only).
    pub fn instruction(mut self, instruction: impl Into<String>) -> Self {
        self.instruction = instruction.into();
        self
    }

    /// Show a custom main icon from an `.ico` file (task-dialog path only); a
    /// failed load falls back to the [`MessageIcon`] glyph.
    pub fn icon_file(mut self, path: impl Into<String>) -> Self {
        self.icon_file = Some(path.into());
        self
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn dialog_title(&self) -> &str {
        &self.title
    }

    /// The custom buttons (empty for a standard `MessageBox` dialog).
    pub fn custom_buttons(&self) -> &[MessageButton] {
        &self.custom_buttons
    }
}

/// Which shell picker to show.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileDialogMode {
    Open,
    Save,
    Folder,
}

/// One entry of a file dialog's type filter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileFilter {
    pub(crate) label: String,
    pub(crate) pattern: String,
}

impl FileFilter {
    /// `label` is the visible name (`"音频"`); `pattern` is a semicolon list
    /// (`"*.mp3;*.flac"`).
    pub fn new(label: impl Into<String>, pattern: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            pattern: pattern.into(),
        }
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn pattern(&self) -> &str {
        &self.pattern
    }
}

/// The shell's open/save/folder picker.
#[derive(Clone, Debug, PartialEq)]
pub struct FileDialog {
    pub(crate) mode: FileDialogMode,
    pub(crate) title: String,
    pub(crate) filters: Vec<FileFilter>,
    pub(crate) initial_dir: Option<String>,
    pub(crate) initial_name: Option<String>,
    /// The open picker accepts more than one file (ignored for save/folder).
    pub(crate) multi_select: bool,
    pub(crate) modal: bool,
}

impl FileDialog {
    fn new(mode: FileDialogMode) -> Self {
        Self {
            mode,
            title: String::new(),
            filters: Vec::new(),
            initial_dir: None,
            initial_name: None,
            multi_select: false,
            modal: true,
        }
    }

    /// Pick an existing file.
    pub fn open() -> Self {
        Self::new(FileDialogMode::Open)
    }

    /// Choose a path to save to.
    pub fn save() -> Self {
        Self::new(FileDialogMode::Save)
    }

    /// Pick a folder.
    pub fn folder() -> Self {
        Self::new(FileDialogMode::Folder)
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// Append a type filter; the first one is selected initially.
    pub fn filter(mut self, label: impl Into<String>, pattern: impl Into<String>) -> Self {
        self.filters.push(FileFilter::new(label, pattern));
        self
    }

    /// Folder the dialog opens in (existing directory).
    pub fn initial_dir(mut self, dir: impl Into<String>) -> Self {
        self.initial_dir = Some(dir.into());
        self
    }

    /// Pre-filled file name (open dialogs may use it as the default selection).
    pub fn initial_name(mut self, name: impl Into<String>) -> Self {
        self.initial_name = Some(name.into());
        self
    }

    /// Let the open picker return several files. The result arrives as
    /// [`DialogResult::Files`] instead of [`DialogResult::File`]; save and
    /// folder modes ignore it.
    pub fn multi_select(mut self, multiple: bool) -> Self {
        self.multi_select = multiple;
        self
    }

    pub fn is_multi_select(&self) -> bool {
        self.multi_select
    }

    /// Whether this dialog produces [`DialogResult::Files`].
    pub fn returns_multiple(&self) -> bool {
        self.multi_select && self.mode == FileDialogMode::Open
    }

    pub fn mode(&self) -> FileDialogMode {
        self.mode
    }
}

/// The subset of a `LOGFONT` a dialog can choose.
#[derive(Clone, Debug, PartialEq)]
pub struct FontChoice {
    pub family: String,
    /// Point size.
    pub size: f32,
    pub bold: bool,
    pub italic: bool,
}

impl FontChoice {
    pub fn new(family: impl Into<String>, size: f32) -> Self {
        Self {
            family: family.into(),
            size,
            bold: false,
            italic: false,
        }
    }

    pub fn bold(mut self, bold: bool) -> Self {
        self.bold = bold;
        self
    }

    pub fn italic(mut self, italic: bool) -> Self {
        self.italic = italic;
        self
    }
}

/// The system font chooser.
#[derive(Clone, Debug, PartialEq)]
pub struct FontDialog {
    pub(crate) initial: Option<FontChoice>,
    pub(crate) modal: bool,
}

impl Default for FontDialog {
    fn default() -> Self {
        Self::new()
    }
}

impl FontDialog {
    pub fn new() -> Self {
        Self {
            initial: None,
            modal: true,
        }
    }

    /// Start with this face/size/style (also shown in the sample box).
    pub fn initial(mut self, font: FontChoice) -> Self {
        self.initial = Some(font);
        self
    }
}

/// The system color picker.
#[derive(Clone, Debug, PartialEq)]
pub struct ColorDialog {
    pub(crate) initial: Option<Color>,
    pub(crate) modal: bool,
    /// Seed for the 16 custom-color wells; `None` keeps the process-wide wells.
    pub(crate) custom: Option<[Color; 16]>,
}

impl Default for ColorDialog {
    fn default() -> Self {
        Self::new()
    }
}

impl ColorDialog {
    pub fn new() -> Self {
        Self {
            initial: None,
            modal: true,
            custom: None,
        }
    }

    /// Pre-select this color.
    pub fn initial(mut self, color: Color) -> Self {
        self.initial = Some(color);
        self
    }

    /// Seed the 16 custom-color wells (the row under the palette). Without
    /// this the wells keep whatever the process last showed: the shell writes
    /// a picked custom color back into them, so the next picker offers it
    /// again. Use [`custom_colors`] to read them for saving in the
    /// application's settings.
    pub fn custom_colors(mut self, colors: [Color; 16]) -> Self {
        self.custom = Some(colors);
        self
    }
}

/// The library's own color window (D118): a themed top-level dialog with an
/// HSV field, RGB/HSV channel rows (slider + number editor), the built-in
/// presets, an optional transparency row and OK/Cancel. It follows the
/// application's theme and is owned by the window that opened it; nothing
/// native is involved (unlike [`ColorDialog`], the system chooser).
#[derive(Clone, Debug, PartialEq)]
pub struct ColorWindow {
    pub(crate) initial: Rgba,
    pub(crate) allow_alpha: bool,
    pub(crate) modal: bool,
}

impl ColorWindow {
    pub fn new(initial: Rgba) -> Self {
        Self {
            initial,
            allow_alpha: false,
            modal: true,
        }
    }

    /// Start from this color and alpha.
    pub fn initial(mut self, rgba: Rgba) -> Self {
        self.initial = rgba;
        self
    }

    /// Show the transparency row (`0..=255`). Without it the window edits an
    /// opaque color and reports one.
    pub fn allow_alpha(mut self, allow: bool) -> Self {
        self.allow_alpha = allow;
        self
    }

    /// Do not disable the owner while the window is up.
    pub fn parallel(mut self) -> Self {
        self.modal = false;
        self
    }
}

/// The process-wide custom-color wells the color picker shows. They start
/// white, and a custom color the user mixes is written back by the shell.
pub fn custom_colors() -> [Color; 16] {
    let mut wells = [Color::new(255, 255, 255); 16];
    for (well, colorref) in wells.iter_mut().zip(colors::load()) {
        *well = Color::from_colorref(colorref);
    }
    wells
}

/// Replace the process-wide custom-color wells (restore a set the application
/// saved); the next color picker starts from them.
pub fn set_custom_colors(colors: [Color; 16]) {
    colors::store(colors.map(|color| color.to_colorref()));
}

#[cfg(windows)]
mod colors {
    use std::sync::Mutex;

    /// The shell writes into this array; it lives for the process so the next
    /// picker (including one on another worker thread) starts with the same
    /// wells.
    static WELLS: Mutex<[u32; 16]> = Mutex::new([0x00FF_FFFF; 16]);

    pub fn load() -> [u32; 16] {
        WELLS.lock().map(|wells| *wells).unwrap_or([0x00FF_FFFF; 16])
    }

    pub fn store(wells: [u32; 16]) {
        if let Ok(mut stored) = WELLS.lock() {
            *stored = wells;
        }
    }
}

#[cfg(not(windows))]
mod colors {
    pub fn load() -> [u32; 16] {
        [0x00FF_FFFF; 16]
    }

    pub fn store(_wells: [u32; 16]) {}
}

impl FontDialog {
    /// Builders default to modal; this opens it in parallel instead.
    pub fn parallel(mut self) -> Self {
        self.modal = false;
        self
    }
}

impl ColorDialog {
    /// Builders default to modal; this opens it in parallel instead.
    pub fn parallel(mut self) -> Self {
        self.modal = false;
        self
    }
}

impl FileDialog {
    /// Builders default to modal; this opens it in parallel instead.
    pub fn parallel(mut self) -> Self {
        self.modal = false;
        self
    }
}

impl MessageDialog {
    /// Builders default to modal; this opens it in parallel instead.
    pub fn parallel(mut self) -> Self {
        self.modal = false;
        self
    }
}


/// A dialog request: a message or a shell picker.
#[derive(Clone, Debug, PartialEq)]
pub enum Dialog {
    Message(MessageDialog),
    File(FileDialog),
    Font(FontDialog),
    Color(ColorDialog),
    /// The library's themed color window (D118).
    ColorWindow(ColorWindow),
}

impl Dialog {
    /// Whether the owner window is disabled while the dialog is up.
    pub fn modal(&self) -> bool {
        match self {
            Dialog::Message(dialog) => dialog.modal,
            Dialog::File(dialog) => dialog.modal,
            Dialog::Font(dialog) => dialog.modal,
            Dialog::Color(dialog) => dialog.modal,
            Dialog::ColorWindow(dialog) => dialog.modal,
        }
    }

    /// The result to report when the dialog could not be shown at all.
    pub fn canceled_result(&self) -> DialogResult {
        match self {
            Dialog::Message(_) => DialogResult::Message(None),
            Dialog::File(dialog) if dialog.returns_multiple() => DialogResult::Files(None),
            Dialog::File(_) => DialogResult::File(None),
            Dialog::Font(_) => DialogResult::Font(None),
            Dialog::Color(_) => DialogResult::Color(None),
            Dialog::ColorWindow(_) => DialogResult::ColorWindow(None),
        }
    }
}

impl From<MessageDialog> for Dialog {
    fn from(dialog: MessageDialog) -> Self {
        Dialog::Message(dialog)
    }
}

impl From<FileDialog> for Dialog {
    fn from(dialog: FileDialog) -> Self {
        Dialog::File(dialog)
    }
}

impl From<FontDialog> for Dialog {
    fn from(dialog: FontDialog) -> Self {
        Dialog::Font(dialog)
    }
}

impl From<ColorDialog> for Dialog {
    fn from(dialog: ColorDialog) -> Self {
        Dialog::Color(dialog)
    }
}

impl From<ColorWindow> for Dialog {
    fn from(dialog: ColorWindow) -> Self {
        Dialog::ColorWindow(dialog)
    }
}

/// What the user chose. `None` means the dialog was dismissed (Cancel, the
/// close box or a failure to show it).
#[derive(Clone, Debug, PartialEq)]
pub enum DialogResult {
    Message(Option<MessageAnswer>),
    /// Open/save/folder path.
    File(Option<String>),
    /// A multi-select open picker's paths (empty selection is `None`).
    Files(Option<Vec<String>>),
    Font(Option<FontChoice>),
    Color(Option<Color>),
    /// A [`ColorWindow`] result: the picked color and alpha.
    ColorWindow(Option<Rgba>),
}

impl DialogResult {
    /// Whether the dialog produced a value (as opposed to a dismissal).
    pub fn is_some(&self) -> bool {
        match self {
            DialogResult::Message(answer) => answer.is_some(),
            DialogResult::File(path) => path.is_some(),
            DialogResult::Files(paths) => paths.as_ref().is_some_and(|paths| !paths.is_empty()),
            DialogResult::Font(font) => font.is_some(),
            DialogResult::Color(color) => color.is_some(),
            DialogResult::ColorWindow(rgba) => rgba.is_some(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builders_are_modal_by_default_and_parallel_on_request() {
        let message = MessageDialog::new("Saved")
            .title("提示")
            .icon(MessageIcon::Warning)
            .buttons(MessageButtons::OkCancel);
        assert!(Dialog::from(message.clone()).modal());
        assert!(!Dialog::from(message.parallel()).modal());
        assert!(Dialog::from(FileDialog::open()).modal());
        assert!(!Dialog::from(FileDialog::save().parallel()).modal());
        assert!(Dialog::from(FontDialog::new()).modal());
        assert!(!Dialog::from(ColorDialog::new().parallel()).modal());
    }

    #[test]
    fn file_builder_keeps_filters_and_initial_values() {
        let dialog = FileDialog::open()
            .title("打开音频")
            .filter("音频", "*.mp3;*.flac")
            .filter("所有文件", "*.*")
            .initial_dir("F:\\Music")
            .initial_name("song.mp3");
        assert_eq!(dialog.mode(), FileDialogMode::Open);
        assert_eq!(dialog.filters.len(), 2);
        assert_eq!(dialog.filters[1].pattern(), "*.*");
        assert_eq!(dialog.initial_dir.as_deref(), Some("F:\\Music"));
        assert_eq!(dialog.initial_name.as_deref(), Some("song.mp3"));
    }

    #[test]
    fn font_choice_carries_style() {
        let font = FontChoice::new("Segoe UI", 12.5).bold(true).italic(true);
        assert_eq!(font.family, "Segoe UI");
        assert!((font.size - 12.5).abs() < f32::EPSILON);
        assert!(font.bold && font.italic);
    }

    #[test]
    fn a_multi_select_open_reports_files_and_other_modes_ignore_it() {
        let dialog = FileDialog::open().multi_select(true);
        assert!(dialog.is_multi_select() && dialog.returns_multiple());
        assert_eq!(
            Dialog::from(dialog).canceled_result(),
            DialogResult::Files(None)
        );
        // Save and folder pickers stay single-valued.
        assert!(!FileDialog::save().multi_select(true).returns_multiple());
        assert!(!FileDialog::folder().multi_select(true).returns_multiple());
        assert!(DialogResult::Files(Some(vec!["a.mp3".to_string()])).is_some());
        assert!(!DialogResult::Files(None).is_some());
        assert!(!DialogResult::Files(Some(Vec::new())).is_some());
    }

    #[test]
    fn canceled_results_match_the_request_kind() {
        assert_eq!(
            Dialog::from(MessageDialog::new("x")).canceled_result(),
            DialogResult::Message(None)
        );
        assert_eq!(
            Dialog::from(FileDialog::folder()).canceled_result(),
            DialogResult::File(None)
        );
        assert_eq!(
            Dialog::from(ColorDialog::new()).canceled_result(),
            DialogResult::Color(None)
        );
        assert!(!DialogResult::Font(None).is_some());
        assert!(DialogResult::Message(Some(MessageAnswer::Yes)).is_some());
    }

    #[test]
    fn a_message_dialog_keeps_custom_buttons_instruction_and_icon() {
        let dialog = MessageDialog::new("正文")
            .title("标题")
            .instruction("保存本次更改？")
            .icon_file("app.ico")
            .button("保存(&S)", 501)
            .button("放弃(&D)", 502);
        assert_eq!(dialog.custom_buttons().len(), 2);
        assert_eq!(dialog.custom_buttons()[0].command(), 501);
        assert_eq!(dialog.custom_buttons()[0].label(), "保存(&S)");
        assert_eq!(dialog.custom_buttons()[1].command(), 502);
        assert_eq!(dialog.instruction, "保存本次更改？");
        assert_eq!(dialog.icon_file.as_deref(), Some("app.ico"));
        // The standard set stays untouched for the fallback path.
        assert_eq!(dialog.buttons, MessageButtons::Ok);
        assert_eq!(
            MessageAnswer::Custom(501),
            MessageAnswer::Custom(501)
        );
        assert_ne!(MessageAnswer::Custom(501), MessageAnswer::Cancel);
    }

    #[test]
    fn custom_color_wells_round_trip_through_the_process_store() {
        let wells = [Color::new(12, 34, 56); 16];
        set_custom_colors(wells);
        assert_eq!(custom_colors()[0], wells[0]);
        assert_eq!(custom_colors()[15], wells[15]);
        let dialog = ColorDialog::new().custom_colors([Color::new(1, 2, 3); 16]);
        assert_eq!(dialog.custom.map(|wells| wells[3]), Some(Color::new(1, 2, 3)));
        set_custom_colors([Color::new(255, 255, 255); 16]);
    }
}