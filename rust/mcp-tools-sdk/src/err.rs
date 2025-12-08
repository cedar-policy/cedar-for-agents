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
use smol_str::SmolStr;
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

    /// Deserializer found non well-founded type definitions (infinite recursion)
    #[error("Non well-founded type definitions: {0:?}")]
    #[diagnostic(
        code(deserialization_error::non_well_founded_type_definitions),
        help("Ensure that the type definitions are well-founded (all type definitions have finite size)")
    )]
    NonWellFoundedTypeDefinitions(TypeDefinitionCycle)
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

    pub(crate) fn type_definition_cycle(cycle: Vec<SmolStr>) -> Self {
        Self::NonWellFoundedTypeDefinitions(TypeDefinitionCycle { cycle })
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
    ToolInputRequest,
    ToolOutputResponse,
}

impl std::fmt::Display for ContentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ServerDescription => write!(f, "MCP Server Description"),
            Self::ToolDescription => write!(f, "MCP Tool Description"),
            Self::ToolParameters => write!(f, "MCP Tool Input/Output Schema"),
            Self::Property => write!(f, "JSON Schema Property Description"),
            Self::PropertyType => write!(f, "JSON Schema Property Type"),
            Self::ToolInputRequest => write!(f, "MCP `tools/call` JSON request"),
            Self::ToolOutputResponse => write!(f, "MCP `tools/call` JSON response"),
        }
    }
}

#[derive(Debug)]
pub struct ReadError {
    file_name: PathBuf,
    error: String,
}

#[allow(dead_code, reason="cycle is used implicitly in error message")]
#[derive(Debug)]
pub struct TypeDefinitionCycle {
    cycle: Vec<SmolStr>
}

#[derive(Error, Debug, Diagnostic)]
pub enum ValidationError {
    #[error(transparent)]
    #[diagnostic(
        code = "validation_error::mismatched_names",
        help = "Expected input and tool to have matching names."
    )]
    MismatchedToolNames(MismatchedNamesError),

    #[error(transparent)]
    #[diagnostic(
        code = "validation_error::tool_not_found",
        help = "Cannot validate input/output against a tool not found in Server Description."
    )]
    ToolNotFound(ToolNotFoundError),

    #[error(transparent)]
    #[diagnostic(
        code = "validation_error::missing_required_property",
        help = "Ensure all required properties are provided"
    )]
    MissingRequiredProperty(MissingRequiredPropertyError),

    #[error("Invalid Integer Literal: {}", .0.literal)]
    #[diagnostic(
        code = "validation_error::invalid_integer_literal",
        help = "Ensure integer literal is a valid 64-bit integer"
    )]
    InvalidIntegerLiteral(InvalidLiteralError),

    #[error("Invalid Float Literal: {}", .0.literal)]
    #[diagnostic(
        code = "validation_error::invalid_float_literal",
        help = "Ensure float literal is a valid 64-bit float"
    )]
    InvalidFloatLiteral(InvalidLiteralError),

    #[error("Invalid Decimal Literal: {}", .0.literal)]
    #[diagnostic(
        code = "validation_error::invalid_decimal_literal",
        help = "Ensure string literal is formated as a valid decimal string"
    )]
    InvalidDecimalLiteral(InvalidLiteralError),

    #[error("Invalid Datetime Literal: {}", .0.literal)]
    #[diagnostic(
        code = "validation_error::invalid_datetime_literal",
        help = "Ensure string literal is formated as a valid datetime string"
    )]
    InvalidDatetimeLiteral(InvalidLiteralError),

    #[error("Invalid Duration Literal: {}", .0.literal)]
    #[diagnostic(
        code = "validation_error::invalid_duration_literal",
        help = "Ensure string literal is formated as a valid duration string"
    )]
    InvalidDurationLiteral(InvalidLiteralError),

    #[error("Invalid IpAddr Literal: {}", .0.literal)]
    #[diagnostic(
        code = "validation_error::invalid_ipaddr_literal",
        help = "Ensure string literal is formated as a valid ipaddr string"
    )]
    InvalidIpAddrLiteral(InvalidLiteralError),

    #[error("Invalid Enum Variant: {}", .0.literal)]
    #[diagnostic(
        code = "validation_error::invalid_enum_variant",
        help = "Ensure string value is a valid enum variant"
    )]
    InvalidEnumVariant(InvalidLiteralError),

    #[error("Wrong number of elements for tuple: expected {} elements, found {}", .0.expected, .0.found)]
    #[diagnostic(
        code = "validation_error::wrong_tuple_size",
        help = "Ensure tuple arguments provide the right number of arguments"
    )]
    WrongTupleSize(WrongTupleSizeError),

    #[error("Could not match value to union type")]
    #[diagnostic(
        code = "validation_error::invalid_value_for_union_type",
        help = "Ensure the input value matches one of the types within the union type"
    )]
    InvalidValueForUnionType,

    #[error("Unexpected property on object: {}", .0.name)]
    #[diagnostic(
        code = "validation_error::unexpected_property",
        help = "Ensure you only include expected properties"
    )]
    UnexpectedProperty(UnexpectedPropertyError),

    #[error("Type {} not found in type definitions", .0.name)]
    #[diagnostic(
        code = "validation_error::unrecognized_type_name",
        help = "Ensure MCP tool schema has well formed type references"
    )]
    UnexpectedTypeName(UnexpectedTypeNameError),

    #[error("Value does not match expected type")]
    #[diagnostic(
        code = "validation_error::invalid_value_for_type",
        help = "Ensure input value matches expected type in MCP tool description"
    )]
    InvalidValueForType,
}

impl ValidationError {
    pub(crate) fn mismatched_names(tool_name: SmolStr, input_for: SmolStr) -> Self {
        Self::MismatchedToolNames(MismatchedNamesError {
            tool_name,
            input_for,
        })
    }

    pub(crate) fn tool_not_found(tool_name: SmolStr) -> Self {
        Self::ToolNotFound(ToolNotFoundError { tool_name })
    }

    pub(crate) fn missing_required_property(property_name: SmolStr) -> Self {
        Self::MissingRequiredProperty(MissingRequiredPropertyError { property_name })
    }

    pub(crate) fn invalid_integer_literal(literal: &str) -> Self {
        Self::InvalidIntegerLiteral(InvalidLiteralError {
            literal: literal.to_string(),
        })
    }

    pub(crate) fn invalid_float_literal(literal: &str) -> Self {
        Self::InvalidFloatLiteral(InvalidLiteralError {
            literal: literal.to_string(),
        })
    }

    pub(crate) fn invalid_decimal_literal(literal: &str) -> Self {
        Self::InvalidDecimalLiteral(InvalidLiteralError {
            literal: literal.to_string(),
        })
    }

    pub(crate) fn invalid_datetime_literal(literal: &str) -> Self {
        Self::InvalidDatetimeLiteral(InvalidLiteralError {
            literal: literal.to_string(),
        })
    }

    pub(crate) fn invalid_duration_literal(literal: &str) -> Self {
        Self::InvalidDurationLiteral(InvalidLiteralError {
            literal: literal.to_string(),
        })
    }

    pub(crate) fn invalid_ipaddr_literal(literal: &str) -> Self {
        Self::InvalidIpAddrLiteral(InvalidLiteralError {
            literal: literal.to_string(),
        })
    }

    pub(crate) fn invalid_enum_variant(literal: &str) -> Self {
        Self::InvalidEnumVariant(InvalidLiteralError {
            literal: literal.to_string(),
        })
    }

    pub(crate) fn wrong_tuple_size(expected: usize, found: usize) -> Self {
        Self::WrongTupleSize(WrongTupleSizeError { expected, found })
    }

    pub(crate) fn unexpected_property(name: &str) -> Self {
        Self::UnexpectedProperty(UnexpectedPropertyError {
            name: name.to_string(),
        })
    }

    pub(crate) fn unexpected_type_name(name: &str) -> Self {
        Self::UnexpectedTypeName(UnexpectedTypeNameError {
            name: name.to_string(),
        })
    }
}

#[derive(Debug, Error)]
#[error("Validating input/output for {tool_name} but found input for {input_for}")]
pub struct MismatchedNamesError {
    tool_name: SmolStr,
    input_for: SmolStr,
}

#[derive(Debug, Error)]
#[error("Validation failed because no tool named {tool_name} was found.")]
pub struct ToolNotFoundError {
    tool_name: SmolStr,
}

#[derive(Debug, Error)]
#[error("Validation failed because required property {property_name} is missing.")]
pub struct MissingRequiredPropertyError {
    property_name: SmolStr,
}

#[derive(Debug)]
pub struct InvalidLiteralError {
    literal: String,
}

#[derive(Debug)]
pub struct WrongTupleSizeError {
    expected: usize,
    found: usize,
}

#[derive(Debug)]
pub struct UnexpectedPropertyError {
    name: String,
}

#[derive(Debug)]
pub struct UnexpectedTypeNameError {
    name: String,
}
