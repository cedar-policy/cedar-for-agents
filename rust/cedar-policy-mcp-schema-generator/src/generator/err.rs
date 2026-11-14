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
    /// SchemaGenerator requires input schemas with exactly one named namespace that is not
    /// annotated with `@mcp_server` (the main namespace)
    #[error("Expected schema with a single namespace not annotated with `@mcp_server`")]
    #[diagnostic(
        code(schema_generator::no_namespace_provided),
        help("Input Cedar Schema stub should contain exactly 1 namespace without an `@mcp_server` annotation.")
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
        help("Server Descriptions cannot be merged. Consider pre-merging Server descriptions and using add_actions_from_server_description API.")
    )]
    ServerDescriptionMerge,
    /// SchemaGenerator failed to parse a Cedar schema string
    #[error("Failed to parse Cedar schema: {0}")]
    #[diagnostic(
        code(schema_generator::schema_parse_error),
        help("Ensure the input is a valid .cedarschema string.")
    )]
    SchemaParseError(String),
    /// A namespace of the input schema stub has an `@mcp_server` annotation without a value
    #[error("Namespace `{0}` has an `@mcp_server` annotation without a server name.")]
    #[diagnostic(
        code(schema_generator::missing_server_name),
        help("Give the server name as the annotation value, e.g., `@mcp_server(\"my-server\")`.")
    )]
    MissingServerName(String),
    /// Two namespaces of the input schema stub have the same `@mcp_server` name
    #[error("Server name `{0}` is used by more than one `@mcp_server` namespace.")]
    #[diagnostic(
        code(schema_generator::duplicate_server_name),
        help("Each `@mcp_server` namespace should have a distinct server name.")
    )]
    DuplicateServerName(SmolStr),
    /// A server namespace of the input schema stub is not a direct child of the main namespace
    #[error(
        "Server namespace `{namespace}` is not a direct child of the main namespace `{main}`."
    )]
    #[diagnostic(
        code(schema_generator::server_namespace_not_child_of_main),
        help("Name each `@mcp_server` namespace `{main}::<Name>`.")
    )]
    ServerNamespaceNotChildOfMain {
        /// The server namespace
        namespace: String,
        /// The main namespace (the only namespace without `@mcp_server`)
        main: String,
    },
    /// A server namespace of the input schema stub declares an MCP principal type
    #[error("Server namespace `{0}` declares an `@mcp_principal` entity type.")]
    #[diagnostic(
        code(schema_generator::principal_in_server_namespace),
        help("Declare `@mcp_principal` entity types in the main namespace only.")
    )]
    PrincipalInServerNamespace(String),
    /// The main namespace of a schema stub with `@mcp_server` namespaces declares an MCP resource type
    #[error("The main namespace `{0}` declares an `@mcp_resource` entity type, but the schema stub has `@mcp_server` namespaces.")]
    #[diagnostic(
        code(schema_generator::resource_in_main_namespace),
        help("Tools are only added to `@mcp_server` namespaces. Declare one `@mcp_resource` entity type in each `@mcp_server` namespace instead.")
    )]
    ResourceInMainNamespace(String),
    /// A server namespace of the input schema stub does not declare exactly one MCP resource type
    #[error("Server namespace `{namespace}` declares {count} `@mcp_resource` entity types.")]
    #[diagnostic(
        code(schema_generator::wrong_number_of_server_resources),
        help("Each `@mcp_server` namespace should declare exactly 1 `@mcp_resource` entity type.")
    )]
    WrongNumberOfServerResources {
        /// The server namespace
        namespace: String,
        /// The number of `@mcp_resource` entity types it declares
        count: usize,
    },
    /// An `@mcp_context` key conflicts with an attribute added by the generator
    #[error("`@mcp_context(\"{0}\")` uses a context key reserved by the schema generator.")]
    #[diagnostic(
        code(schema_generator::reserved_context_key),
        help("The schema generator adds the `input` and `output` context attributes. Use another key.")
    )]
    ReservedContextKey(SmolStr),
    /// `flatten_namespaces` was set for a schema stub with `@mcp_server` namespaces
    #[error("The `flatten_namespaces` option is not supported for schema stubs with `@mcp_server` namespaces.")]
    #[diagnostic(
        code(schema_generator::flatten_with_server_namespaces),
        help("Disable `flatten_namespaces`, or use a schema stub with a single namespace.")
    )]
    FlattenWithServerNamespaces,
    /// The action of a tool has the same name as an action of the schema stub
    #[error("The action of tool `{action}` conflicts with the action `{namespace}::Action::\"{action}\"`.")]
    #[diagnostic(
        code(schema_generator::conflicting_action_name),
        help("Rename the action of the schema stub. Note that the generator adds an `AllActions` action to each `@mcp_server` namespace without an `@mcp_action`.")
    )]
    ConflictingActionName {
        /// The name of the tool and of the action
        action: SmolStr,
        /// The namespace of the action
        namespace: String,
    },
    /// A server namespace without `@mcp_action` declares an action named `AllActions`
    #[error("Server namespace `{0}` declares an action `AllActions`, but no `@mcp_action`.")]
    #[diagnostic(
        code(schema_generator::all_actions_in_stub),
        help("The generator adds an `AllActions` action to each `@mcp_server` namespace without an `@mcp_action`. Annotate the action with `@mcp_action`, or rename it.")
    )]
    AllActionsInStub(String),
    /// `StubBuilder` cannot use `namespace` as the namespace identifier of server `server`
    #[error("`{namespace}` is not a valid namespace identifier for server `{server}`.")]
    #[diagnostic(
        code(schema_generator::invalid_server_namespace),
        help("The identifier derived from a server name replaces every character that is not an ASCII letter, digit or `_` by `_`. Give a valid Cedar identifier with `add_server_with_namespace`.")
    )]
    InvalidServerNamespace {
        /// The server name
        server: SmolStr,
        /// The namespace identifier
        namespace: String,
    },
    /// The namespace of a server added by `StubBuilder` already exists in the schema stub
    #[error("Namespace `{0}` already exists in the schema stub.")]
    #[diagnostic(
        code(schema_generator::existing_server_namespace),
        help("Give another namespace identifier with `add_server_with_namespace`.")
    )]
    ExistingServerNamespace(String),
    /// The API used does not match the schema stub (with or without `@mcp_server` namespaces),
    /// or the server name is unknown
    #[error(transparent)]
    #[diagnostic(transparent)]
    ServerLookup(#[from] ServerLookupError),
}

/// A generator was asked for the tools of a server that does not exist, either because the
/// server name is unknown or because the API used does not match the schema stub.
#[derive(Debug, Clone, Error, Diagnostic)]
pub enum ServerLookupError {
    /// The schema stub has `@mcp_server` namespaces, but no server name was given
    #[error("The schema stub declares `@mcp_server` namespaces, so a server name is required.")]
    #[diagnostic(
        code(schema_generator::server_name_required),
        help("Use the methods taking a server name, e.g., `add_actions_from_named_server_description` or `generate_named_request`.")
    )]
    ServerNameRequired,
    /// The schema stub has no `@mcp_server` namespace, but a server name was given
    #[error(
        "The schema stub declares no `@mcp_server` namespace, so a server name cannot be used."
    )]
    #[diagnostic(
        code(schema_generator::no_server_namespaces),
        help("Use the methods without a server name, e.g., `add_actions_from_server_description` or `generate_request`.")
    )]
    NoServerNamespaces,
    /// No `@mcp_server` namespace has this server name
    #[error("Unknown server `{0}`.")]
    #[diagnostic(
        code(schema_generator::unknown_server),
        help("The server name should be the value of an `@mcp_server` annotation of the schema stub.")
    )]
    UnknownServer(SmolStr),
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
    #[error(transparent)]
    #[diagnostic(transparent)]
    ServerLookup(#[from] ServerLookupError),
}
