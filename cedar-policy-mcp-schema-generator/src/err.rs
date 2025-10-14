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

use miette::Diagnostic;
use smol_str::SmolStr;
use thiserror::Error;

#[derive(Debug, Error, Diagnostic)]
pub enum SchemaGeneratorError {
    #[error("Expected schema with a single namespace")]
    #[diagnostic(
        code(schema_generator::no_namespace_provided),
        help("Input Cedar Schema stub should contain exactly 1 namespace.")
    )]
    WrongNumberOfNamespaces,
    #[error("Input Schema's should not use global namespace.")]
    #[diagnostic(
        code(schema_generator::global_namespace_used),
        help("Input Cedar Schema stub should not use the global namespace.")
    )]
    GlobalNamespaceUsed,
    #[error("No MCP Principal Entity types specified.")]
    #[diagnostic(
        code(schema_generator::expected_mcp_principal),
        help("Input Cedar Schema stub should specify at least 1 Entity Type as an MCP Principal.")
    )]
    NoPrincipalTypes,
    #[error("No MCP Resource Entity types specified.")]
    #[diagnostic(
        code(schema_generator::expected_mcp_resource),
        help("Input Cedar Schema stub should specify at least 1 Entity Type as an MCP Resource.")
    )]
    NoResourceTypes,
    #[error("Multiple MCP contexts are labeled with the same attribute name: `{0}`")]
    #[diagnostic(
        code(schema_generator::conflicting_mcp_contexts),
        help("Input Cedar Schema stub should assign each MCP context a unique attribute name.")
    )]
    ConflictingContextNames(String),
    #[error("{0}")]
    #[diagnostic(
        code(schema_generator::use_of_reserved_name),
        help("MCP Tool Description Schemas make use of reserved keyword.")
    )]
    ReservedName(#[from] cedar_policy_core::parser::err::ParseErrors),
    #[error("{0}")]
    #[diagnostic(
        code(schema_generator::use_of_reserved_name),
        help("MCP Tool Description Schemas make use of reserved keyword.")
    )]
    ReservedCommonTypeName(#[from] cedar_policy_core::validator::json_schema::ReservedCommonTypeBasenameError),
    #[error("Conflicting type definitions between MCP Tool Description and input Cedar Schema Stub File.")]
    #[diagnostic(
        code(schema_generator::conflicting_name),
        help("MCP Tool Description's Schema makes use of a type name `{0}` that conflicts with a type defined in the input Cedar Schema stub file.")
    )]
    ConflictingSchemaNameError(SmolStr),
    #[error("Undefined Reference Type.")]
    #[diagnostic(
        code(schema_generator::undefined_reference),
        help("`{name}` not found in `{namespace}` (or any containing namespace). Ensure that every `$ref` type in input MCP Tool Description references a defined type definition.")
    )]
    UndefinedReferenceType {
        name: String,
        namespace: String,
    },


    //     #[error("Annotation of MCP Shared Context cannot be used as a Context attribute name.")]
//     #[diagnostic(
//         code(schema_generator::invalid_mcp_context_name),
//         help("Input Cedar Schema stub should annotate all MCP Shared contexts with valid attribute names.")
//     )]
//     ContextNameAnnotation(#[from] cedar_policy_core::parser::err::ParseError),
}

impl SchemaGeneratorError {
    pub(crate) fn conflicting_name(name: SmolStr) -> Self {
        Self::ConflictingSchemaNameError(name)
    }

    pub(crate) fn undefined_ref(name: String, namespace: String) -> Self {
        Self::UndefinedReferenceType { name, namespace }
    }
}