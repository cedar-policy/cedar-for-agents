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
pub enum TokenizerError {
    #[error("Encountered EOF while parsing.")]
    #[diagnostic(code(parse::unexpected_eof), help("{msg}"))]
    UnexpectedEof {
        #[source_code]
        src: Loc,
        #[label("End of Input")]
        found: miette::SourceSpan,
        msg: String,
    },
    #[error("Encountered unexpected token while parsing.")]
    #[diagnostic(code(parse::unexpected_token), help("{msg}"))]
    UnexpectedToken {
        #[source_code]
        src: Loc,
        #[label("Found")]
        found: miette::SourceSpan,
        msg: String,
    },
    #[error("Encountered unknown escape sequence while parsing string literal")]
    #[diagnostic(code(parse::invalid_string_liter), help("{msg}"))]
    UnexpectedEscapeSequence {
        #[source_code]
        src: Loc,
        #[label("Found")]
        found: miette::SourceSpan,
        msg: String,
    },
    #[error("Encountered invalid number literal")]
    #[diagnostic(code(parse::invalid_number_literal), help("{msg}"))]
    InvalidNumberLiteral {
        #[source_code]
        src: Loc,
        #[label("Invalid number literal")]
        found: miette::SourceSpan,
        msg: String,
    },
}

impl TokenizerError {
    pub fn unexpected_eof(loc: Loc, msg: &str) -> Self {
        let found = (&loc).into();
        Self::UnexpectedEof {
            src: loc,
            found,
            msg: msg.to_string(),
        }
    }

    pub fn unexpected_token(loc: Loc, msg: &str) -> Self {
        let found = (&loc).into();
        Self::UnexpectedToken {
            src: loc,
            found,
            msg: msg.to_string(),
        }
    }

    pub fn unknown_escape_sequence(loc: Loc, msg: &str) -> Self {
        let found = (&loc).into();
        Self::UnexpectedEscapeSequence {
            src: loc,
            found,
            msg: msg.to_string(),
        }
    }

    pub fn invalid_number(loc: Loc, msg: &str) -> Self {
        let found = (&loc).into();
        Self::InvalidNumberLiteral {
            src: loc,
            found,
            msg: msg.to_string(),
        }
    }
}

#[derive(Error, Debug, Diagnostic)]
pub enum ParseError {
    #[error(transparent)]
    #[diagnostic(transparent)]
    TokenizerError(#[from] TokenizerError),

    #[error("Encountered unexpected token while parsing.")]
    #[diagnostic(code(parse::unexpected_token), help("{msg}"))]
    UnexpectedToken {
        #[source_code]
        src: Loc,
        #[label("Found")]
        found: miette::SourceSpan,
        msg: String,
    },

    #[error("Duplicate key found")]
    #[diagnostic(code(parse::duplicate_key), help("All keys should be unique."))]
    DuplicateKey {
        #[source_code]
        src: Loc,
        #[label("First Occurence")]
        first: miette::SourceSpan,
        #[label("Second Occurence")]
        second: miette::SourceSpan,
    },
}

impl ParseError {
    pub fn unexpected_token(loc: Loc, msg: &str) -> Self {
        let found = (&loc).into();
        Self::UnexpectedToken {
            src: loc,
            found,
            msg: msg.to_string(),
        }
    }

    pub fn duplicate_key(first: miette::SourceSpan, second: Loc) -> Self {
        let loc = second;
        let second = (&loc).into();
        Self::DuplicateKey {
            src: loc,
            first,
            second,
        }
    }
}
