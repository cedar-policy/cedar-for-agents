/*
 * Copyright Cedar Contributors
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *      https://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use super::loc::Loc;
use miette::Diagnostic;
use thiserror::Error;

#[derive(Error, Debug, Diagnostic)]
pub enum ParseError {
    #[error(transparent)]
    #[diagnostic(transparent)]
    TokenizerError(#[from] TokenizerError),

    #[error("Encountered unexpected token while parsing.")]
    #[diagnostic(transparent)]
    UnexpectedToken(LocationFound),

    #[error("Duplicate key found")]
    #[diagnostic(transparent)]
    DuplicateKey(DuplicateFound),
}

impl ParseError {
    pub fn unexpected_token(loc: Loc, msg: &str) -> Self {
        Self::UnexpectedToken(LocationFound {
            src: loc,
            label: "Found".to_string(),
            msg: msg.to_string(),
            code: "parse::unexpected_token".to_string(),
        })
    }

    pub fn duplicate_key(first: miette::SourceSpan, second: Loc) -> Self {
        let loc = second;
        let second = (&loc).into();
        Self::DuplicateKey(DuplicateFound {
            src: loc,
            first,
            second,
            msg: "All keys should be unique.".to_string(),
            code: "parse::duplicate_key".to_string(),
        })
    }
}

#[derive(Error, Debug, Diagnostic)]
pub enum TokenizerError {
    #[error("Encountered EOF while parsing.")]
    #[diagnostic(transparent)]
    UnexpectedEof(LocationFound),
    #[error("Encountered unexpected token while parsing.")]
    #[diagnostic(transparent)]
    UnexpectedToken(LocationFound),
    #[error("Encountered unknown escape sequence while parsing string literal")]
    #[diagnostic(transparent)]
    UnexpectedEscapeSequence(LocationFound),
    #[error("Encountered invalid number literal")]
    #[diagnostic(transparent)]
    InvalidNumberLiteral(LocationFound),
}

impl TokenizerError {
    pub fn unexpected_eof(loc: Loc, msg: &str) -> Self {
        Self::UnexpectedEof(LocationFound {
            src: loc,
            label: "End of Input".to_string(),
            msg: msg.to_string(),
            code: "parse::unexpected_eof".to_string(),
        })
    }

    pub fn unexpected_token(loc: Loc, msg: &str) -> Self {
        Self::UnexpectedToken(LocationFound {
            src: loc,
            label: "Found".to_string(),
            msg: msg.to_string(),
            code: "parse::unexpected_token".to_string(),
        })
    }

    pub fn unknown_escape_sequence(loc: Loc, msg: &str) -> Self {
        Self::UnexpectedEscapeSequence(LocationFound {
            src: loc,
            label: "Found".to_string(),
            msg: msg.to_string(),
            code: "parse::invalid_string_literal".to_string(),
        })
    }

    pub fn invalid_number(loc: Loc, msg: &str) -> Self {
        Self::InvalidNumberLiteral(LocationFound {
            src: loc,
            label: "Found".to_string(),
            msg: msg.to_string(),
            code: "parse::invalid_number_literal".to_string(),
        })
    }
}

#[derive(Debug, Error)]
#[error("Problem found.")]
pub struct LocationFound {
    src: Loc,
    label: String,
    msg: String,
    code: String,
}

impl Diagnostic for LocationFound {
    fn code<'a>(&'a self) -> Option<Box<dyn std::fmt::Display + 'a>> {
        Some(Box::new(&self.code))
    }

    fn source_code(&self) -> Option<&dyn miette::SourceCode> {
        Some(&self.src)
    }

    fn help<'a>(&'a self) -> Option<Box<dyn std::fmt::Display + 'a>> {
        Some(Box::new(&self.msg))
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = miette::LabeledSpan> + '_>> {
        Some(Box::new(std::iter::once(miette::LabeledSpan::new(
            Some(self.label.clone()),
            self.src.start(),
            self.src.end() - self.src.start(),
        ))))
    }
}

#[derive(Debug, Error)]
#[error("Duplicates found.")]
pub struct DuplicateFound {
    src: Loc,
    first: miette::SourceSpan,
    second: miette::SourceSpan,
    msg: String,
    code: String,
}

impl Diagnostic for DuplicateFound {
    fn code<'a>(&'a self) -> Option<Box<dyn std::fmt::Display + 'a>> {
        Some(Box::new(&self.code))
    }

    fn source_code(&self) -> Option<&dyn miette::SourceCode> {
        Some(&self.src)
    }

    fn help<'a>(&'a self) -> Option<Box<dyn std::fmt::Display + 'a>> {
        Some(Box::new(&self.msg))
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = miette::LabeledSpan> + '_>> {
        Some(Box::new(
            [
                miette::LabeledSpan::new(
                    Some("First occurence here".to_string()),
                    self.first.offset(),
                    self.first.len(),
                ),
                miette::LabeledSpan::new(
                    Some("Second occurence here".to_string()),
                    self.second.offset(),
                    self.second.len(),
                ),
            ]
            .into_iter(),
        ))
    }
}
