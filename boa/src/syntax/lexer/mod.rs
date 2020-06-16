//! A lexical analyzer for JavaScript source code.
//!
//! The Lexer splits its input source code into a sequence of input elements called tokens, represented by the [Token](../ast/token/struct.Token.html) structure.
//! It also removes whitespace and comments and attaches them to the next token.

#[macro_use]
mod comment;

mod cursor;
pub mod error;

#[macro_use]
mod string;
pub mod token;

#[macro_use]
mod template;

mod number;

mod operator;

mod spread;

mod regex;

mod identifier;

// Temporary disabled while lexer in progress.
#[cfg(test)]
mod tests;

pub use self::error::Error;

use self::{
    comment::Comment, cursor::Cursor, identifier::Identifier, number::NumberLiteral,
    operator::Operator, regex::RegexLiteral, spread::SpreadLiteral, string::StringLiteral,
    template::TemplateLiteral,
};
use crate::syntax::ast::{Position, Punctuator, Span};
use std::io::Read;
pub use token::{Token, TokenKind};

trait Tokenizer<R> {
    /// Lexes the next token.
    fn lex(&mut self, cursor: &mut Cursor<R>, start_pos: Position) -> Result<Token, Error>
    where
        R: Read;
}

/// Lexer or tokenizer for the Boa JavaScript Engine.
#[derive(Debug)]
pub struct Lexer<R> {
    cursor: Cursor<R>,
    goal_symbol: InputElement,
}

impl<R> Lexer<R> {
    /// Checks if a character is whitespace as per ECMAScript standards.
    ///
    /// The Rust `char::is_whitespace` function and the ECMAScript standard use different sets of
    /// characters as whitespaces:
    ///  * Rust uses `\p{White_Space}`,
    ///  * ECMAScript standard uses `\{Space_Separator}` + `\u{0009}`, `\u{000B}`, `\u{000C}`, `\u{FEFF}`
    ///
    /// [More information](https://tc39.es/ecma262/#table-32)
    fn is_whitespace(ch: char) -> bool {
        match ch {
            '\u{0020}' | '\u{0009}' | '\u{000B}' | '\u{000C}' | '\u{00A0}' | '\u{FEFF}' |
            // Unicode Space_Seperator category (minus \u{0020} and \u{00A0} which are allready stated above)
            '\u{1680}' | '\u{2000}'..='\u{200A}' | '\u{202F}' | '\u{205F}' | '\u{3000}' => true,
            _ => false,
        }
    }

    /// Sets the goal symbol for the lexer.
    pub(crate) fn _set_goal(&mut self, elm: InputElement) {
        self.goal_symbol = elm;
    }
}

impl<R> Lexer<R>
where
    R: Read,
{
    /// Creates a new lexer.
    #[inline]
    pub fn new(reader: R) -> Self {
        Self {
            cursor: Cursor::new(reader),
            goal_symbol: Default::default(),
        }
    }
}

/// ECMAScript goal symbols.
///
/// <https://tc39.es/ecma262/#sec-ecmascript-language-lexical-grammar>
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InputElement {
    Div,
    _RegExp,
    _RegExpOrTemplateTail,
    _TemplateTail,
}

impl Default for InputElement {
    fn default() -> Self {
        InputElement::Div
        // Decided on InputElementDiv as default for now based on documentation from
        // <https://tc39.es/ecma262/#sec-ecmascript-language-lexical-grammar>
    }
}

impl<R> Iterator for Lexer<R>
where
    R: Read,
{
    type Item = Result<Token, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        let (start, next_chr) = loop {
            let start = self.cursor.pos();
            let next_chr = match self.cursor.next()? {
                Ok(c) => c,
                Err(e) => return Some(Err(e.into())),
            };

            // Ignore whitespace
            if !Self::is_whitespace(next_chr) {
                break (start, next_chr);
            }
        };

        // TODO, setting strict mode on/off.
        let strict_mode = false;

        let token = match next_chr {
            '\r' | '\n' | '\u{2028}' | '\u{2029}' => Ok(Token::new(
                TokenKind::LineTerminator,
                Span::new(start, self.cursor.pos()),
            )),
            '"' | '\'' => StringLiteral::new(next_chr).lex(&mut self.cursor, start),
            template_match!() => TemplateLiteral::new().lex(&mut self.cursor, start),
            _ if next_chr.is_digit(10) => {
                NumberLiteral::new(next_chr, strict_mode).lex(&mut self.cursor, start)
            }
            _ if next_chr.is_alphabetic() || next_chr == '$' || next_chr == '_' => {
                Identifier::new(next_chr).lex(&mut self.cursor, start)
            }
            ';' => Ok(Token::new(
                Punctuator::Semicolon.into(),
                Span::new(start, self.cursor.pos()),
            )),
            ':' => Ok(Token::new(
                Punctuator::Colon.into(),
                Span::new(start, self.cursor.pos()),
            )),
            '.' => SpreadLiteral::new().lex(&mut self.cursor, start),
            '(' => Ok(Token::new(
                Punctuator::OpenParen.into(),
                Span::new(start, self.cursor.pos()),
            )),
            ')' => Ok(Token::new(
                Punctuator::CloseParen.into(),
                Span::new(start, self.cursor.pos()),
            )),
            ',' => Ok(Token::new(
                Punctuator::Comma.into(),
                Span::new(start, self.cursor.pos()),
            )),
            '{' => Ok(Token::new(
                Punctuator::OpenBlock.into(),
                Span::new(start, self.cursor.pos()),
            )),
            '}' => Ok(Token::new(
                Punctuator::CloseBlock.into(),
                Span::new(start, self.cursor.pos()),
            )),
            '[' => Ok(Token::new(
                Punctuator::OpenBracket.into(),
                Span::new(start, self.cursor.pos()),
            )),
            ']' => Ok(Token::new(
                Punctuator::CloseBracket.into(),
                Span::new(start, self.cursor.pos()),
            )),
            '?' => Ok(Token::new(
                Punctuator::Question.into(),
                Span::new(start, self.cursor.pos()),
            )),
            comment_match!() => Comment::new().lex(&mut self.cursor, start),
            '*' | '+' | '-' | '%' | '|' | '&' | '^' | '=' | '<' | '>' | '!' | '~' => {
                Operator::new(next_chr).lex(&mut self.cursor, start)
            }
            _ => {
                let details = format!(
                    "Unexpected '{}' at line {}, column {}",
                    next_chr,
                    start.line_number(),
                    start.column_number()
                );
                Err(Error::syntax(details))
            }
        };

        if let Ok(t) = token {
            if t.kind() == &TokenKind::Comment {
                // Skip comment
                self.next()
            } else {
                Some(Ok(t))
            }
        } else {
            Some(token)
        }
    }
}

// impl<R> Tokenizer<R> for Lexer<R> {
//     fn lex(&mut self, cursor: &mut Cursor<R>, start_pos: Position) -> io::Result<Token>
//     where
//         R: Read,
//     {

//     }
// }