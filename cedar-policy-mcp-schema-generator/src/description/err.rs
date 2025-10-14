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

use super::parser::{json_value::LocatedValue, loc::Loc};
use miette::Diagnostic;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug, Diagnostic)]
pub enum DeserializationError {
    #[error(transparent)]
    #[diagnostic(transparent)]
    ParseError(#[from] super::parser::err::ParseError),

    #[error("Encountered unexpected JSON type while deserializing {content_type}.")]
    #[diagnostic(code(deserialization::unexpected_type), help("{msg}"))]
    UnexpectedType {
        #[source_code]
        loc: Loc,
        #[label("found")]
        found: miette::SourceSpan,
        msg: String,
        content_type: ContentType,
    },

    #[error(transparent)]
    #[diagnostic(transparent)]
    MissingExpectedAttribute(MissingExpectedAttributeError),

    #[error("Unexpected value.")]
    #[diagnostic(code(deserialization::unexpected_value), help("{msg}"))]
    UnexpectedValue {
        #[source_code]
        loc: Loc,
        #[label("found")]
        found: miette::SourceSpan,
        msg: String,
        content_type: ContentType,
    },

    #[error("Error reading {file_name}: {error}")]
    #[diagnostic()]
    ReadError { file_name: PathBuf, error: String },
}

impl DeserializationError {
    pub(crate) fn unexpected_type(
        json_value: &LocatedValue,
        msg: &str,
        content_type: ContentType,
    ) -> Self {
        let loc = json_value.as_loc().clone();
        let found = (&loc).into();
        Self::UnexpectedType {
            loc,
            found,
            msg: msg.to_string(),
            content_type,
        }
    }

    pub(crate) fn missing_attribute(
        json_value: &LocatedValue,
        expected_key: &str,
        aliases: Vec<String>,
    ) -> Self {
        let loc = json_value.as_loc().clone();
        let existing_keys = json_value.get_object().map_or(Vec::new(), |obj| {
            obj.keys().map(|key| key.as_loc().into()).collect()
        });
        Self::MissingExpectedAttribute(MissingExpectedAttributeError {
            loc,
            expected_key: expected_key.to_string(),
            aliases,
            existing_keys,
        })
    }

    pub(crate) fn unexpected_value(
        json_value: &LocatedValue,
        msg: &str,
        content_type: ContentType,
    ) -> Self {
        let loc = json_value.as_loc().clone();
        let found = (&loc).into();
        Self::UnexpectedType {
            loc,
            found,
            msg: msg.to_string(),
            content_type,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ContentType {
    ServerDescription,
    ToolDescription,
    ToolParameters,
    Property,
    PropertyType,
}

impl std::fmt::Display for ContentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ServerDescription => write!(f, "MCP Server Description"),
            Self::ToolDescription => write!(f, "MCP Tool Description"),
            Self::ToolParameters => write!(f, "MCP Tool Input/Output Schema"),
            Self::Property => write!(f, "JSON Schema Property Description"),
            Self::PropertyType => write!(f, "JSON Schema Property Type"),
        }
    }
}

#[derive(Error, Debug)]
#[error("Missing expected attribute")]
pub struct MissingExpectedAttributeError {
    pub loc: Loc,
    pub expected_key: String,
    pub aliases: Vec<String>,
    pub existing_keys: Vec<miette::SourceSpan>,
}

impl Diagnostic for MissingExpectedAttributeError {
    fn code<'a>(&'a self) -> Option<Box<dyn std::fmt::Display + 'a>> {
        Some(Box::new("deserialization::missing_attribute"))
    }

    fn source_code(&self) -> Option<&dyn miette::SourceCode> {
        Some(&self.loc)
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = miette::LabeledSpan> + '_>> {
        if self.existing_keys.len() == 0 {
            Some(Box::new(std::iter::once(miette::LabeledSpan::new(
                Some("Empty object".into()),
                self.loc.start(),
                self.loc.end() - self.loc.start(),
            ))))
        } else {
            Some(Box::new(self.existing_keys.iter().map(|span| {
                miette::LabeledSpan::new(Some("Existing key".into()), span.offset(), span.len())
            })))
        }
    }

    fn help<'a>(&'a self) -> Option<Box<dyn std::fmt::Display + 'a>> {
        let aliases = if self.aliases.len() == 0 {
            String::new()
        } else {
            format!(" (aliases: `{}`)", self.aliases.join("`, `"))
        };
        Some(Box::new(format!(
            "Expected key `{}`{}",
            self.expected_key, aliases
        )))
    }
}
