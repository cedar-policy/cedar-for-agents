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

use cedar_policy_core::ast::{ContextCreationError, ExpressionConstructionError};
use miette::Diagnostic;
use smol_str::SmolStr;
use thiserror::Error;

/// SchemaGenerator found conflicting type definitions in input Cedar Schema Stub and MCP Tool Description Type Definitions.
#[derive(Debug, Clone)]
pub struct ConflictingSchemaNameError {
    name: SmolStr,
}

/// SchemaGenerator encountered a MCP Type Schema with a `$ref` type that has no corresponding definition within the MCP Tool/Server Description
#[derive(Debug, Clone)]
pub struct UndefinedReferenceType {
    name: String,
    namespace: String,
}

/// SchemaGenerator encountered a MCP Type Schema containing an enum type with an empty variant list
#[derive(Debug, Clone)]
pub struct EmptyEnum {
    name: String,
}

/// SchemaGenerator encountered an error during generation
#[derive(Debug, Error, Diagnostic)]
pub enum SchemaGeneratorError {
    /// SchemaGenerator only supports input schemas with a single named namespace
    #[error("Expected schema with a single namespace")]
    #[diagnostic(
        code(schema_generator::no_namespace_provided),
        help("Input Cedar Schema stub should contain exactly 1 namespace.")
    )]
    WrongNumberOfNamespaces,
    /// SchemaGenerator does not support input schemas that use the global (unnamed) namespace.
    #[error("Input Schema's should not use global namespace.")]
    #[diagnostic(
        code(schema_generator::global_namespace_used),
        help("Input Cedar Schema stub should not use the global namespace.")
    )]
    GlobalNamespaceUsed,
    /// SchemaGenerator requires input schema to specify at least 1 MCP Principal type
    #[error("No MCP Principal Entity types specified.")]
    #[diagnostic(
        code(schema_generator::expected_mcp_principal),
        help("Input Cedar Schema stub should specify at least 1 Entity Type as an MCP Principal.")
    )]
    NoPrincipalTypes,
    /// SchemaGenerator requires input schema to specify at least 1 MCP resource type
    #[error("No MCP Resource Entity types specified.")]
    #[diagnostic(
        code(schema_generator::expected_mcp_resource),
        help("Input Cedar Schema stub should specify at least 1 Entity Type as an MCP Resource.")
    )]
    NoResourceTypes,
    /// SchemaGenerator failed because it encountered an MCP type that conflicts with a reserved Cedar Name
    #[error(transparent)]
    #[diagnostic(
        code(schema_generator::use_of_reserved_name),
        help("MCP Tool Description Schemas make use of reserved keyword.")
    )]
    ReservedName(#[from] cedar_policy_core::parser::err::ParseErrors),
    /// SchemaGenerator failed because it encountered an MCP type that conflicts with a reserved Cedar Name
    #[error("{0}")]
    #[diagnostic(
        code(schema_generator::use_of_reserved_name),
        help("MCP Tool Description Schemas make use of reserved keyword.")
    )]
    ReservedCommonTypeName(
        #[from] cedar_policy_core::validator::json_schema::ReservedCommonTypeBasenameError,
    ),
    /// SchemaGenerator failed because it encountered an MCP type that conflicts with a type defined within the input Cedar Schema
    #[error("Conflicting type definitions between MCP Tool Description and input Cedar Schema Stub File.")]
    #[diagnostic(
        code(schema_generator::conflicting_name),
        help("MCP Tool Description's Schema makes use of a type name `{}` that conflicts with a type defined in the input Cedar Schema stub file.", .0.name)
    )]
    ConflictingSchemaNameError(ConflictingSchemaNameError),
    /// SchemaGenerator failed because it encountered an MCP `$ref` type that has no definition
    #[error("Undefined Reference Type.")]
    #[diagnostic(
        code(schema_generator::undefined_reference),
        help("`{}` not found in `{}` (or any containing namespace). Ensure that every `$ref` type in input MCP Tool Description references a defined type definition.", .0.name, .0.namespace)
    )]
    UndefinedReferenceType(UndefinedReferenceType),
    /// SchemaGenerator failed because it encountered an MCP enum type with no variant names.
    #[error("Empty Enum Type: {}.", .0.name)]
    #[diagnostic(
        code(schema_generator::empty_enum_type),
        help("Ensure MCP Description does not contain any enum types with empty array of variant names.")
    )]
    EmptyEnumChoice(EmptyEnum),
    #[error(transparent)]
    #[diagnostic(transparent)]
    SchemaResolutionError(#[from] cedar_policy_core::validator::SchemaError),
    #[error("Server Descriptions cannot be merged.")]
    #[diagnostic(
        code(schema_generator::mcp_server_description_merge),
        help("Server Descriptions cannot be merged. Consider pre-merging Server descriptions and using add_tools_from_serve_description API.")
    )]
    ServerDescriptionMerge,
}

impl SchemaGeneratorError {
    /// Construct a `SchemaGeneratorError` representing that the Schema Generator encountered an
    /// MCP Type name that conflicts with a type name in the input cedar schema
    pub(crate) fn conflicting_name(name: SmolStr) -> Self {
        Self::ConflictingSchemaNameError(ConflictingSchemaNameError { name })
    }

    /// Construct a `SchemaGeneratorError` representing that the Schema Generator encountered an
    /// MCP `$ref` type that has no definition
    pub(crate) fn undefined_ref(name: String, namespace: String) -> Self {
        Self::UndefinedReferenceType(UndefinedReferenceType { name, namespace })
    }

    /// Construct a `SchemaGeneratorError` representing that the Schema Generator encountered an
    /// MCP enum with no variants
    pub(crate) fn empty_enum_choice(name: String) -> Self {
        Self::EmptyEnumChoice(EmptyEnum { name })
    }
}

/// RequestGenerator encountered an error during generation
#[derive(Debug, Error, Diagnostic)]
pub enum RequestGeneratorError {
    #[error(transparent)]
    #[diagnostic(
        code(schema_generator::use_of_reserved_name),
        help("MCP Tool Description Schemas make use of reserved keyword.")
    )]
    ReservedName(#[from] cedar_policy_core::parser::err::ParseErrors),
    #[error(transparent)]
    #[diagnostic(transparent)]
    CedarExpressionConstructionError(#[from] ExpressionConstructionError),
    #[error(transparent)]
    #[diagnostic(transparent)]
    CedarContextConstructionError(#[from] ContextCreationError),
    #[error(transparent)]
    #[diagnostic(transparent)]
    MCPValidationError(#[from] mcp_tools_sdk::err::ValidationError),
    #[error(transparent)]
    #[diagnostic(transparent)]
    MalformedExpression(#[from] cedar_policy_core::ast::RestrictedExpressionError),
    #[error(transparent)]
    #[diagnostic(transparent)]
    MalformedRequest(#[from] cedar_policy_core::validator::RequestValidationError),
    #[error("Cannot convert number {0} to decimal literal")]
    #[diagnostic(
        code = "request_generator::malformed_decimal_number",
        help = "Ensure the number can be parsed as either a 64 bit floating point or integer number"
    )]
    MalformedDecimalNumber(String),
    #[error(transparent)]
    #[diagnostic(transparent)]
    MalformedEntityData(#[from] cedar_policy_core::ast::EntityAttrEvaluationError),
    #[error(transparent)]
    #[diagnostic(transparent)]
    DuplicateEntities(#[from] cedar_policy_core::entities::err::EntitiesError),
}
