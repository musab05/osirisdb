//! SQL lexer and tokenizer subsystem.
//!
//! This module implements a hand-written, zero-copy lexical analyzer that transforms
//! raw SQL source text into a stream of [`Token`]s. Each token carries a [`TokenKind`]
//! discriminant and a [`Span`] that records byte offsets plus line/column positions
//! for precise error reporting.
//!
//! # Sub-modules
//!
//! - [`token`] — defines [`TokenKind`], [`Token`], and [`Modifier`].
//! - [`lexer`] — the [`Lexer`] struct and its scanning logic.
//! - [`lookup_keyword()`] — maps identifier strings to SQL keyword token kinds.
//! - [`spanned_token`] — the [`Span`] type used for source-location tracking.

pub mod token;
pub mod lexer;
pub mod lookup_keyword;
pub mod spanned_token;

pub use lexer::*;
pub use token::*;
pub use lookup_keyword::*;
pub use spanned_token::*;
