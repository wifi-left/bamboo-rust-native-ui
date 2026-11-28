// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Diagnostics of the layout-skin parser and applier (M7 §9).
//!
//! Codes are stable strings: a test may pin one, and the debug form groups by
//! them. Errors reject the whole document; warnings are reported and the rest
//! still applies.

use std::fmt;

/// How bad a [`LayoutDiagnostic`] is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayoutSeverity {
    Error,
    Warning,
}

/// One parse or apply problem, pointing at the element that caused it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayoutDiagnostic {
    pub severity: LayoutSeverity,
    /// Stable, testable identifier (`xml.geom.overdetermined`, …).
    pub code: &'static str,
    /// 1-based line in the skin file.
    pub line: usize,
    /// 1-based column in the skin file.
    pub column: usize,
    /// Best-effort element path: `layout/wide/m7_action`.
    pub path: String,
    pub message: String,
}

impl LayoutDiagnostic {
    pub fn error(
        code: &'static str,
        line: usize,
        column: usize,
        path: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity: LayoutSeverity::Error,
            code,
            line,
            column,
            path: path.into(),
            message: message.into(),
        }
    }

    pub fn warning(
        code: &'static str,
        line: usize,
        column: usize,
        path: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity: LayoutSeverity::Warning,
            code,
            line,
            column,
            path: path.into(),
            message: message.into(),
        }
    }

    pub fn is_error(&self) -> bool {
        self.severity == LayoutSeverity::Error
    }
}

impl fmt::Display for LayoutDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let severity = match self.severity {
            LayoutSeverity::Error => "error",
            LayoutSeverity::Warning => "warning",
        };
        if self.path.is_empty() {
            write!(
                f,
                "{}:{}: {}[{}]: {}",
                self.line, self.column, severity, self.code, self.message
            )
        } else {
            write!(
                f,
                "{}:{}: {}[{}] ({}): {}",
                self.line, self.column, severity, self.code, self.path, self.message
            )
        }
    }
}

/// The mutable diagnostic sink shared by the parser and the applier.
#[derive(Default)]
pub struct Diagnostics {
    items: Vec<LayoutDiagnostic>,
    limit: usize,
}

impl Diagnostics {
    pub fn new(limit: usize) -> Self {
        Self {
            items: Vec::new(),
            limit,
        }
    }

    pub fn push(&mut self, diagnostic: LayoutDiagnostic) {
        if self.items.len() < self.limit {
            self.items.push(diagnostic);
        }
    }

    pub fn error(
        &mut self,
        code: &'static str,
        line: usize,
        column: usize,
        path: impl Into<String>,
        message: impl Into<String>,
    ) {
        self.push(LayoutDiagnostic::error(code, line, column, path, message));
    }

    pub fn warning(
        &mut self,
        code: &'static str,
        line: usize,
        column: usize,
        path: impl Into<String>,
        message: impl Into<String>,
    ) {
        self.push(LayoutDiagnostic::warning(code, line, column, path, message));
    }

    pub fn has_errors(&self) -> bool {
        self.items.iter().any(LayoutDiagnostic::is_error)
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn into_vec(self) -> Vec<LayoutDiagnostic> {
        self.items
    }
}
