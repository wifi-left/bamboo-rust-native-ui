// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! System notifications.
//!
//! [`Notification`] is the application-facing description of one notification;
//! [`crate::WindowHandle::notify`] posts it through the Windows notification
//! platform (WinRT `ToastNotificationManager`). A title and a body are enough:
//! the window's `WindowBuilder::app_id` selects the AppUserModelID, and when
//! the toast cannot be shown (an unpackaged process without a registered
//! AUMID, or the shell notification platform unavailable) the window falls
//! back to a tray-icon balloon when one is installed.
//!
//! Toasts are visual only: clicking one starts the application through the
//! shell's own activation path, so there is no in-process activation callback
//! here. Use [`Notification::launch`] to pass arguments the next launch reads
//! from the command line.

/// One system notification.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Notification {
    title: String,
    body: String,
    app_id: Option<String>,
    tag: Option<String>,
    group: Option<String>,
    launch: Option<String>,
    silent: bool,
}

impl Notification {
    /// A notification with a title and a body (both may be empty, though the
    /// shell shows an empty toast poorly).
    pub fn new(title: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            body: body.into(),
            ..Self::default()
        }
    }

    /// Replace the body.
    pub fn body(mut self, body: impl Into<String>) -> Self {
        self.body = body.into();
        self
    }

    /// The AppUserModelID used for this notification. `None` uses the window's
    /// `WindowBuilder::app_id`, then the process AUMID.
    pub fn app_id(mut self, id: impl Into<String>) -> Self {
        self.app_id = Some(id.into());
        self
    }

    /// A stable tag for replacing/removing this notification in the Action
    /// Center (per `group`).
    pub fn tag(mut self, tag: impl Into<String>) -> Self {
        self.tag = Some(tag.into());
        self
    }

    /// The tag group; see [`Notification::tag`].
    pub fn group(mut self, group: impl Into<String>) -> Self {
        self.group = Some(group.into());
        self
    }

    /// Arguments the application receives when the user activates the toast.
    pub fn launch(mut self, arguments: impl Into<String>) -> Self {
        self.launch = Some(arguments.into());
        self
    }

    /// Suppress the notification sound (the toast still appears).
    pub fn silent(mut self, silent: bool) -> Self {
        self.silent = silent;
        self
    }

    pub fn title_text(&self) -> &str {
        &self.title
    }

    pub fn body_text(&self) -> &str {
        &self.body
    }

    pub fn app_id_str(&self) -> Option<&str> {
        self.app_id.as_deref()
    }

    pub fn tag_text(&self) -> Option<&str> {
        self.tag.as_deref()
    }

    pub fn group_text(&self) -> Option<&str> {
        self.group.as_deref()
    }

    pub fn launch_text(&self) -> Option<&str> {
        self.launch.as_deref()
    }

    pub fn is_silent(&self) -> bool {
        self.silent
    }

    /// Escape text for the toast XML. Public for tests and applications that
    /// build a custom payload through the same escaping rules.
    pub fn escape(text: &str) -> String {
        let mut escaped = String::with_capacity(text.len());
        for ch in text.chars() {
            match ch {
                '&' => escaped.push_str("&amp;"),
                '<' => escaped.push_str("&lt;"),
                '>' => escaped.push_str("&gt;"),
                '"' => escaped.push_str("&quot;"),
                '\'' => escaped.push_str("&apos;"),
                _ => escaped.push(ch),
            }
        }
        escaped
    }

    /// The `ToastGeneric` XML this notification is shown with.
    pub fn to_toast_xml(&self) -> String {
        let launch = self
            .launch
            .as_ref()
            .map(|launch| format!(" launch=\"{}\"", Notification::escape(launch)))
            .unwrap_or_default();
        let audio = if self.silent {
            "<audio silent=\"true\"/>"
        } else {
            ""
        };
        format!(
            "<toast{launch}><visual><binding template=\"ToastGeneric\"><text>{title}</text><text>{body}</text></binding></visual>{audio}</toast>",
            title = Notification::escape(&self.title),
            body = Notification::escape(&self.body),
        )
    }
}

/// Whether the shell accepted a notification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotificationResult {
    /// Shown through the Windows notification platform.
    Toast,
    /// The toast was refused; shown as a tray balloon instead.
    Balloon,
    /// Neither path could show it (no notification platform and no tray icon).
    Failed,
}

impl NotificationResult {
    pub fn shown(self) -> bool {
        self != Self::Failed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toast_xml_escapes_text_and_marks_silence() {
        let notification = Notification::new("<标题>", "a & b");
        assert_eq!(
            notification.to_toast_xml(),
            "<toast><visual><binding template=\"ToastGeneric\"><text>&lt;标题&gt;</text><text>a &amp; b</text></binding></visual></toast>"
        );
        let silent = Notification::new("t", "b").silent(true).launch("app://x?a=1&b=2");
        let xml = silent.to_toast_xml();
        assert!(xml.contains("<audio silent=\"true\"/>"));
        assert!(xml.contains("launch=\"app://x?a=1&amp;b=2\""));
        let escaped = Notification::escape("\"'&<>");
        for entity in ["&quot;", "&apos;", "&amp;", "&lt;", "&gt;"] {
            assert!(escaped.contains(entity), "{entity} missing from {escaped}");
        }
    }

    #[test]
    fn builders_keep_their_values() {
        let notification = Notification::new("t", "b")
            .app_id("Demo.App")
            .tag("track")
            .group("player")
            .launch("open");
        assert_eq!(notification.app_id_str(), Some("Demo.App"));
        assert_eq!(notification.tag_text(), Some("track"));
        assert_eq!(notification.group_text(), Some("player"));
        assert_eq!(notification.launch_text(), Some("open"));
        assert_eq!(notification.title_text(), "t");
        assert_eq!(notification.body_text(), "b");
        assert!(!notification.is_silent());
    }
}
