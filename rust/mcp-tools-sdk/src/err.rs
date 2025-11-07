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

/// The type of errors that may be encountered during deserialization of an MCP Server / Tool Description
#[derive(Error, Debug, Diagnostic)]
pub enum DeserializationError {
    /// Deserializer encountered an error while parsing a JSON Value
    #[error(transparent)]
    #[diagnostic(transparent)]
    ParseError(#[from] super::parser::err::ParseError),

    /// Deserializer encountered an unexpected JSON type
    #[error("Encountered unexpected JSON type while deserializing {}.", .0.content_type)]
    #[diagnostic(transparent)]
    UnexpectedType(LocationFound),

    /// Deserializer did not find an expected attribute
    #[error(transparent)]
    #[diagnostic(transparent)]
    MissingExpectedAttribute(MissingExpectedAttributeError),

    /// Deserializer found an unexpected value
    #[error("Unexpected value.")]
    #[diagnostic(transparent)]
    UnexpectedValue(LocationFound),

    /// Deserializer could not open the provided JSON file
    #[error("Error reading {}: {}", .0.file_name.display(), .0.error)]
    #[diagnostic()]
    ReadError(ReadError),
}

impl DeserializationError {
    /// Construct a new `DeserizlizerError` signaling an Unexpected JSON type was encountered while deserializing
    pub(crate) fn unexpected_type(
        json_value: &LocatedValue,
        msg: &str,
        content_type: ContentType,
    ) -> Self {
        let loc = json_value.as_loc().clone();
        Self::UnexpectedType(LocationFound {
            src: loc,
            label: "Found".to_string(),
            msg: msg.to_string(),
            code: "deserialization::unexpected_type".to_string(),
            content_type,
        })
    }

    /// Construct a new `DeserizlizerError` signaling an expected attribute is missing from a JSON Object
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

    /// Construct a new `DeserizlizerError` signaling an Unexpected JSON value was encountered while deserializing
    pub(crate) fn unexpected_value(
        json_value: &LocatedValue,
        msg: &str,
        content_type: ContentType,
    ) -> Self {
        let loc = json_value.as_loc().clone();
        Self::UnexpectedValue(LocationFound {
            src: loc,
            label: "Found".to_string(),
            msg: msg.to_string(),
            code: "deserialization::unexpected_value".to_string(),
            content_type,
        })
    }

    /// Construct a new `DeserizlizerError` signaling that input JSON file could not be read
    pub(crate) fn read_error(file_name: PathBuf, error: String) -> Self {
        Self::ReadError(ReadError { file_name, error })
    }
}

#[derive(Error, Debug)]
#[error("Missing expected attribute")]
pub struct MissingExpectedAttributeError {
    loc: Loc,
    expected_key: String,
    aliases: Vec<String>,
    existing_keys: Vec<miette::SourceSpan>,
}

impl Diagnostic for MissingExpectedAttributeError {
    fn code<'a>(&'a self) -> Option<Box<dyn std::fmt::Display + 'a>> {
        Some(Box::new("deserialization::missing_attribute"))
    }

    fn source_code(&self) -> Option<&dyn miette::SourceCode> {
        Some(&self.loc)
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = miette::LabeledSpan> + '_>> {
        if self.existing_keys.is_empty() {
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
        let aliases = if self.aliases.is_empty() {
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

/// The source location of an error
#[derive(Debug, Error)]
#[error("Problem found.")]
pub struct LocationFound {
    src: Loc,
    label: String,
    msg: String,
    code: String,
    content_type: ContentType,
}

impl Diagnostic for LocationFound {
    fn code<'a>(&'a self) -> Option<Box<dyn std::fmt::Display + 'a>> {
        Some(Box::new(&self.code))
    }

    fn source_code(&self) -> Option<&dyn miette::SourceCode> {
        Some(&self.src)
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = miette::LabeledSpan> + '_>> {
        Some(Box::new(std::iter::once(miette::LabeledSpan::new(
            Some(self.label.clone()),
            self.src.start(),
            self.src.end() - self.src.start(),
        ))))
    }

    fn help<'a>(&'a self) -> Option<Box<dyn std::fmt::Display + 'a>> {
        Some(Box::new(&self.msg))
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

#[derive(Debug)]
pub struct ReadError {
    file_name: PathBuf,
    error: String,
}
