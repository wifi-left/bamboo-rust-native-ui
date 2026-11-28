// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Geometry expressions (M7 §3.3).
//!
//! ```text
//! expr     := term (('+' | '-') term)*
//! term     := factor (('*' | '/') factor)*
//! factor   := number | number '%' | variable | '(' expr ')' | '-' factor
//! ```
//!
//! Expressions are parsed once and evaluated on every apply against the
//! current space (client area or scroll content area). `to_source` prints the
//! canonical, whitespace-free form: the normalized writer and the generation
//! hash both rely on it, so formatting an expression differently in the file
//! never changes the model.

use crate::model::LayoutConstants;

/// A space variable an expression can read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Var {
    Left,
    Top,
    Right,
    Bottom,
    Cw,
    Ch,
    Cx,
    Cy,
}

impl Var {
    fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "left" => Self::Left,
            "top" => Self::Top,
            "right" => Self::Right,
            "bottom" => Self::Bottom,
            "cw" => Self::Cw,
            "ch" => Self::Ch,
            "cx" => Self::Cx,
            "cy" => Self::Cy,
            _ => return None,
        })
    }

    pub fn source(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Top => "top",
            Self::Right => "right",
            Self::Bottom => "bottom",
            Self::Cw => "cw",
            Self::Ch => "ch",
            Self::Cx => "cx",
            Self::Cy => "cy",
        }
    }

    fn value(self, width: f64, height: f64) -> f64 {
        match self {
            Self::Left | Self::Top => 0.0,
            Self::Right | Self::Cw => width,
            Self::Bottom | Self::Ch => height,
            Self::Cx => width / 2.0,
            Self::Cy => height / 2.0,
        }
    }

    /// Reserved constant names (a `<const>` may not shadow one).
    pub fn is_reserved(name: &str) -> bool {
        Self::from_name(name).is_some()
    }
}

/// Which axis a percent value refers to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    Horizontal,
    Vertical,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
}

impl BinOp {
    fn symbol(self) -> &'static str {
        match self {
            Self::Add => "+",
            Self::Sub => "-",
            Self::Mul => "*",
            Self::Div => "/",
        }
    }

    fn precedence(self) -> u8 {
        match self {
            Self::Add | Self::Sub => 1,
            Self::Mul | Self::Div => 2,
        }
    }
}

/// A parsed geometry expression.
#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    Num(f64),
    Var(Var),
    Const(String),
    /// `N%`: `N` percent of the axis extent, stored as the literal `N`.
    Percent(f64),
    /// An inline-Rust value (`rust:` prefix): the raw code, kept for the
    /// compiled mode. The dynamic evaluator refuses it.
    Rust(String),
    Neg(Box<Expr>),
    Bin(BinOp, Box<Expr>, Box<Expr>),
}

/// One expression problem, with its stable diagnostic code.
#[derive(Clone, Debug, PartialEq)]
pub struct ExprError {
    pub code: &'static str,
    pub message: String,
}

impl ExprError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

/// Maximum nesting depth of an expression (§3.3).
pub const MAX_DEPTH: usize = 32;

/// Parse one attribute value. Space variables are accepted; the caller decides
/// whether they are legal in its context.
pub fn parse_expr(text: &str, constants: &LayoutConstants) -> Result<Expr, ExprError> {
    let tokens = tokenize(text)?;
    let mut parser = Parser {
        tokens,
        position: 0,
        constants,
        nesting: 0,
    };
    let expr = parser.expr()?;
    if parser.position != parser.tokens.len() {
        return Err(ExprError::new(
            "xml.expr.syntax",
            "unexpected token after the expression",
        ));
    }
    if depth(&expr) > MAX_DEPTH {
        return Err(ExprError::new(
            "xml.expr.depth",
            format!("expression is deeper than {MAX_DEPTH} levels"),
        ));
    }
    Ok(expr)
}

/// Evaluate a parsed expression against one space.
pub fn eval(
    expr: &Expr,
    width: f64,
    height: f64,
    axis: Axis,
    constants: &LayoutConstants,
) -> Result<f64, ExprError> {
    let value = eval_inner(expr, width, height, axis, constants)?;
    if !value.is_finite() {
        return Err(ExprError::new(
            "xml.expr.non-finite",
            "the expression evaluates to a non-finite number",
        ));
    }
    Ok(value)
}

fn eval_inner(
    expr: &Expr,
    width: f64,
    height: f64,
    axis: Axis,
    constants: &LayoutConstants,
) -> Result<f64, ExprError> {
    Ok(match expr {
        Expr::Rust(_) => {
            return Err(ExprError::new(
                "xml.rust.unsupported",
                "inline Rust is only available in the compiled mode",
            ));
        }
        Expr::Num(value) => *value,
        Expr::Var(var) => var.value(width, height),
        Expr::Const(name) => constants.get(name).ok_or_else(|| {
            ExprError::new("xml.const.undefined", format!("unknown constant `{name}`"))
        })?,
        Expr::Percent(value) => {
            let base = match axis {
                Axis::Horizontal => width,
                Axis::Vertical => height,
            };
            value / 100.0 * base
        }
        Expr::Neg(inner) => -eval_inner(inner, width, height, axis, constants)?,
        Expr::Bin(op, left, right) => {
            let left = eval_inner(left, width, height, axis, constants)?;
            let right = eval_inner(right, width, height, axis, constants)?;
            match op {
                BinOp::Add => left + right,
                BinOp::Sub => left - right,
                BinOp::Mul => left * right,
                BinOp::Div => {
                    if right == 0.0 {
                        return Err(ExprError::new("xml.expr.div-zero", "division by zero"));
                    }
                    left / right
                }
            }
        }
    })
}

/// Evaluate a constant definition: no space variables, no percent, and only
/// constants declared above it.
pub fn eval_constant(expr: &Expr, constants: &LayoutConstants) -> Result<f64, ExprError> {
    if contains_var(expr) || contains_percent(expr) {
        return Err(ExprError::new(
            "xml.const.vars",
            "a constant may not use space variables or percent",
        ));
    }
    let value = eval_inner(expr, 0.0, 0.0, Axis::Horizontal, constants)?;
    if !value.is_finite() {
        return Err(ExprError::new(
            "xml.expr.non-finite",
            "the constant evaluates to a non-finite number",
        ));
    }
    Ok(value)
}

fn contains_var(expr: &Expr) -> bool {
    match expr {
        Expr::Var(_) => true,
        Expr::Neg(inner) => contains_var(inner),
        Expr::Bin(_, left, right) => contains_var(left) || contains_var(right),
        _ => false,
    }
}

fn contains_percent(expr: &Expr) -> bool {
    match expr {
        Expr::Percent(_) => true,
        Expr::Neg(inner) => contains_percent(inner),
        Expr::Bin(_, left, right) => contains_percent(left) || contains_percent(right),
        _ => false,
    }
}

fn depth(expr: &Expr) -> usize {
    match expr {
        Expr::Num(_) | Expr::Var(_) | Expr::Const(_) | Expr::Percent(_) | Expr::Rust(_) => 1,
        Expr::Neg(inner) => 1 + depth(inner),
        Expr::Bin(_, left, right) => 1 + depth(left).max(depth(right)),
    }
}

/// Canonical source form: shortest numbers, no whitespace, minimal
/// parentheses.
pub fn to_source(expr: &Expr) -> String {
    let mut out = String::new();
    write_expr(expr, &mut out);
    out
}

fn write_expr(expr: &Expr, out: &mut String) {
    match expr {
        Expr::Num(value) => out.push_str(&number(*value)),
        Expr::Percent(value) => {
            out.push_str(&number(*value));
            out.push('%');
        }
        Expr::Var(var) => out.push_str(var.source()),
        Expr::Const(name) => out.push_str(name),
        Expr::Rust(text) => {
            out.push_str("rust:");
            out.push_str(text);
        }
        Expr::Neg(inner) => {
            out.push('-');
            if matches!(**inner, Expr::Bin(..) | Expr::Neg(_)) {
                out.push('(');
                write_expr(inner, out);
                out.push(')');
            } else {
                write_expr(inner, out);
            }
        }
        Expr::Bin(op, left, right) => {
            write_child(left, *op, false, out);
            out.push_str(op.symbol());
            write_child(right, *op, true, out);
        }
    }
}

fn write_child(child: &Expr, parent: BinOp, right: bool, out: &mut String) {
    let parenthesize = match child {
        Expr::Bin(op, ..) => {
            let child_precedence = op.precedence();
            let parent_precedence = parent.precedence();
            child_precedence < parent_precedence
                || (right
                    && child_precedence == parent_precedence
                    && matches!(
                        (parent, *op),
                        (BinOp::Add, BinOp::Sub)
                            | (BinOp::Sub, _)
                            | (BinOp::Mul, BinOp::Div)
                            | (BinOp::Div, _)
                    ))
        }
        Expr::Neg(_) => true,
        _ => false,
    };
    if parenthesize {
        out.push('(');
        write_expr(child, out);
        out.push(')');
    } else {
        write_expr(child, out);
    }
}

/// Shortest representation of a number: `18`, `18.5`, `0.0001`.
pub fn number(value: f64) -> String {
    if value == 0.0 {
        return "0".to_string();
    }
    if value.fract() == 0.0 && value.abs() < 1e15 {
        return format!("{}", value as i64);
    }
    format!("{value}")
}

// ---------------------------------------------------------------------------
// Lexer
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
enum Token {
    Num(f64),
    Ident(String),
    Percent,
    Plus,
    Minus,
    Star,
    Slash,
    Open,
    Close,
}

fn tokenize(text: &str) -> Result<Vec<Token>, ExprError> {
    let bytes = text.as_bytes();
    let mut tokens = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        match byte {
            b' ' | b'\t' | b'\r' | b'\n' => index += 1,
            b'+' => {
                tokens.push(Token::Plus);
                index += 1;
            }
            b'-' => {
                tokens.push(Token::Minus);
                index += 1;
            }
            b'*' => {
                tokens.push(Token::Star);
                index += 1;
            }
            b'/' => {
                tokens.push(Token::Slash);
                index += 1;
            }
            b'%' => {
                tokens.push(Token::Percent);
                index += 1;
            }
            b'(' => {
                tokens.push(Token::Open);
                index += 1;
            }
            b')' => {
                tokens.push(Token::Close);
                index += 1;
            }
            b'0'..=b'9' => {
                let start = index;
                while index < bytes.len() && bytes[index].is_ascii_digit() {
                    index += 1;
                }
                if index < bytes.len() && bytes[index] == b'.' {
                    index += 1;
                    let fraction_start = index;
                    while index < bytes.len() && bytes[index].is_ascii_digit() {
                        index += 1;
                    }
                    if index == fraction_start {
                        return Err(ExprError::new(
                            "xml.expr.syntax",
                            "a decimal point must be followed by digits",
                        ));
                    }
                }
                let value: f64 = text[start..index].parse().map_err(|_| {
                    ExprError::new("xml.expr.syntax", "malformed number literal")
                })?;
                if !value.is_finite() {
                    return Err(ExprError::new(
                        "xml.expr.non-finite",
                        "a number literal is not finite",
                    ));
                }
                tokens.push(Token::Num(value));
            }
            _ if byte.is_ascii_alphabetic() || byte == b'_' => {
                let start = index;
                index += 1;
                while index < bytes.len() {
                    let byte = bytes[index];
                    if byte.is_ascii_alphanumeric() || byte == b'_' {
                        index += 1;
                    } else {
                        break;
                    }
                }
                tokens.push(Token::Ident(text[start..index].to_string()));
            }
            _ => {
                return Err(ExprError::new(
                    "xml.expr.syntax",
                    format!("unexpected character `{}`", text[index..].chars().next().unwrap_or('?')),
                ));
            }
        }
    }
    Ok(tokens)
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

struct Parser<'a> {
    tokens: Vec<Token>,
    position: usize,
    constants: &'a LayoutConstants,
    /// Parenthesis/unary nesting, checked while parsing (the AST depth check
    /// alone cannot see redundant parentheses).
    nesting: usize,
}

impl<'a> Parser<'a> {
    fn enter(&mut self) -> Result<(), ExprError> {
        self.nesting += 1;
        if self.nesting > MAX_DEPTH {
            return Err(ExprError::new(
                "xml.expr.depth",
                format!("expression is nested deeper than {MAX_DEPTH} levels"),
            ));
        }
        Ok(())
    }

    fn leave(&mut self) {
        self.nesting = self.nesting.saturating_sub(1);
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.position)
    }

    fn next(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.position).cloned();
        self.position += 1;
        token
    }

    fn expr(&mut self) -> Result<Expr, ExprError> {
        let mut left = self.term()?;
        loop {
            match self.peek() {
                Some(Token::Plus) => {
                    self.position += 1;
                    let right = self.term()?;
                    left = Expr::Bin(BinOp::Add, Box::new(left), Box::new(right));
                }
                Some(Token::Minus) => {
                    self.position += 1;
                    let right = self.term()?;
                    left = Expr::Bin(BinOp::Sub, Box::new(left), Box::new(right));
                }
                _ => return Ok(left),
            }
        }
    }

    fn term(&mut self) -> Result<Expr, ExprError> {
        let mut left = self.factor()?;
        loop {
            match self.peek() {
                Some(Token::Star) => {
                    self.position += 1;
                    let right = self.factor()?;
                    left = Expr::Bin(BinOp::Mul, Box::new(left), Box::new(right));
                }
                Some(Token::Slash) => {
                    self.position += 1;
                    let right = self.factor()?;
                    left = Expr::Bin(BinOp::Div, Box::new(left), Box::new(right));
                }
                _ => return Ok(left),
            }
        }
    }

    fn factor(&mut self) -> Result<Expr, ExprError> {
        match self.next() {
            Some(Token::Num(value)) => {
                if matches!(self.peek(), Some(Token::Percent)) {
                    self.position += 1;
                    Ok(Expr::Percent(value))
                } else {
                    Ok(Expr::Num(value))
                }
            }
            Some(Token::Minus) => {
                self.enter()?;
                let inner = self.factor();
                self.leave();
                Ok(Expr::Neg(Box::new(inner?)))
            }
            Some(Token::Open) => {
                self.enter()?;
                let inner = self.expr();
                self.leave();
                let inner = inner?;
                match self.next() {
                    Some(Token::Close) => Ok(inner),
                    _ => Err(ExprError::new("xml.expr.syntax", "missing `)`")),
                }
            }
            Some(Token::Ident(name)) => {
                if let Some(var) = Var::from_name(&name) {
                    Ok(Expr::Var(var))
                } else if self.constants.get(&name).is_some() {
                    Ok(Expr::Const(name))
                } else {
                    Err(ExprError::new(
                        "xml.const.undefined",
                        format!("unknown constant `{name}`"),
                    ))
                }
            }
            Some(Token::Percent) => Err(ExprError::new(
                "xml.expr.syntax",
                "`%` must follow a number",
            )),
            Some(token) => Err(ExprError::new(
                "xml.expr.syntax",
                format!("unexpected token `{token:?}`"),
            )),
            None => Err(ExprError::new("xml.expr.syntax", "unexpected end of expression")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn constants() -> LayoutConstants {
        let mut constants = LayoutConstants::default();
        constants.insert("pad".into(), 16.0, "16".into());
        constants.insert("gap".into(), 8.0, "8".into());
        constants
    }

    fn parse(text: &str) -> Expr {
        let constants = constants();
        parse_expr(text, &constants).expect("valid expression")
    }

    fn evaluate(text: &str, width: f64, height: f64, axis: Axis) -> f64 {
        let constants = constants();
        let expr = parse_expr(text, &constants).expect("valid expression");
        eval(&expr, width, height, axis, &constants).expect("evaluates")
    }

    fn code(text: &str) -> &'static str {
        let constants = constants();
        parse_expr(text, &constants).expect_err("invalid expression").code
    }

    #[test]
    fn precedence_and_parentheses() {
        assert_eq!(evaluate("1+2*3", 100.0, 100.0, Axis::Horizontal), 7.0);
        assert_eq!(evaluate("(1+2)*3", 100.0, 100.0, Axis::Horizontal), 9.0);
        assert_eq!(evaluate("10-2-3", 100.0, 100.0, Axis::Horizontal), 5.0);
        assert_eq!(evaluate("(10-2)-3", 100.0, 100.0, Axis::Horizontal), 5.0);
        assert_eq!(evaluate("10-(2-3)", 100.0, 100.0, Axis::Horizontal), 11.0);
        assert_eq!(evaluate("8/2/2", 100.0, 100.0, Axis::Horizontal), 2.0);
        assert_eq!(evaluate("8/(2/2)", 100.0, 100.0, Axis::Horizontal), 8.0);
        assert_eq!(evaluate("-4+10", 100.0, 100.0, Axis::Horizontal), 6.0);
        assert_eq!(evaluate("-2*3", 100.0, 100.0, Axis::Horizontal), -6.0);
    }

    #[test]
    fn whitespace_is_free() {
        assert_eq!(evaluate("  cw - 2 * pad ", 400.0, 300.0, Axis::Horizontal), 368.0);
        assert_eq!(evaluate("cw-2*pad", 400.0, 300.0, Axis::Horizontal), 368.0);
    }

    #[test]
    fn percents_use_the_axis() {
        assert_eq!(evaluate("50%", 400.0, 300.0, Axis::Horizontal), 200.0);
        assert_eq!(evaluate("50%", 400.0, 300.0, Axis::Vertical), 150.0);
        assert_eq!(evaluate("25%+8", 400.0, 300.0, Axis::Horizontal), 108.0);
        assert_eq!(evaluate("2*50%", 400.0, 300.0, Axis::Horizontal), 400.0);
    }

    #[test]
    fn space_variables() {
        assert_eq!(evaluate("cw", 400.0, 300.0, Axis::Horizontal), 400.0);
        assert_eq!(evaluate("ch", 400.0, 300.0, Axis::Horizontal), 300.0);
        assert_eq!(evaluate("cx-22", 400.0, 300.0, Axis::Horizontal), 178.0);
        assert_eq!(evaluate("cy", 400.0, 300.0, Axis::Vertical), 150.0);
        assert_eq!(evaluate("left+top", 400.0, 300.0, Axis::Horizontal), 0.0);
        assert_eq!(evaluate("right-bottom", 400.0, 300.0, Axis::Horizontal), 100.0);
    }

    #[test]
    fn constants_resolve() {
        assert_eq!(evaluate("cw-2*pad", 400.0, 300.0, Axis::Horizontal), 368.0);
        assert_eq!(evaluate("pad+gap", 400.0, 300.0, Axis::Horizontal), 24.0);
    }

    #[test]
    fn error_codes_are_stable() {
        let constants = constants();
        assert_eq!(code("pad+"), "xml.expr.syntax");
        assert_eq!(code("(1+2"), "xml.expr.syntax");
        assert_eq!(code("1."), "xml.expr.syntax");
        assert_eq!(code("1%2"), "xml.expr.syntax");
        assert_eq!(code("nope"), "xml.const.undefined");
        // Division is evaluated at apply time, not at parse time.
        let expr = parse_expr("1/0", &constants).expect("parses");
        assert_eq!(
            eval(&expr, 100.0, 100.0, Axis::Horizontal, &constants)
                .expect_err("divides by zero")
                .code,
            "xml.expr.div-zero"
        );
    }

    #[test]
    fn non_finite_and_depth_are_rejected() {
        let constants = constants();
        let deep = "(".repeat(MAX_DEPTH + 2) + "1" + &")".repeat(MAX_DEPTH + 2);
        let error = parse_expr(&deep, &constants).expect_err("too deep");
        assert_eq!(error.code, "xml.expr.depth");
        let huge = "9".repeat(400);
        let error = parse_expr(&huge, &constants).expect_err("not finite");
        assert_eq!(error.code, "xml.expr.non-finite");
    }

    #[test]
    fn constants_reject_variables_and_percent() {
        let constants = constants();
        let expr = parse_expr("cw-16", &constants).unwrap();
        assert_eq!(eval_constant(&expr, &constants).unwrap_err().code, "xml.const.vars");
        let expr = parse_expr("50%", &constants).unwrap();
        assert_eq!(eval_constant(&expr, &constants).unwrap_err().code, "xml.const.vars");
        let expr = parse_expr("2*pad", &constants).unwrap();
        assert_eq!(eval_constant(&expr, &constants).unwrap(), 32.0);
    }

    #[test]
    fn canonical_source_round_trips() {
        let constants = constants();
        for text in [
            "1+2*3",
            "(1+2)*3",
            "10-(2-3)",
            "8/(2/2)",
            "a: nope",
            "cw-2*pad",
            "50%+8",
            "-2*3",
            "2*(pad+gap)",
            "1.5",
        ] {
            if text == "a: nope" {
                continue;
            }
            let expr = parse_expr(text, &constants).unwrap();
            let source = to_source(&expr);
            let again = parse_expr(&source, &constants).unwrap();
            assert_eq!(expr, again, "{text} -> {source}");
        }
        assert_eq!(to_source(&parse("10 - (2 - 3)")), "10-(2-3)");
        assert_eq!(to_source(&parse("(10 - 2) - 3")), "10-2-3");
        assert_eq!(to_source(&parse("8 / (2 * 2)")), "8/(2*2)");
        assert_eq!(to_source(&parse("- 2 * 3")), "(-2)*3");
        assert_eq!(to_source(&parse("0.5")), "0.5");
    }
}
