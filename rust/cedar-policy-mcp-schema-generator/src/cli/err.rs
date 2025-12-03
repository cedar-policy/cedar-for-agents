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
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
#[error("Could not open file `{}`: {}", .file.display(), .error)]
pub struct FileOpenError {
    file: PathBuf,
    error: std::io::Error,
}

#[derive(Debug, Error, Diagnostic)]
pub enum CliError {
    #[error("Expected schema file to end with either `.cedarschema` or `.json`")]
    #[diagnostic(
        code(cli_error::unrecognized_schema_file),
        help("Provide schema in either .json or .cedarschema format")
    )]
    UnrecognizedSchemaExtension,
    #[error("Could not open cedar schema file `{}`: {}", .0.file.display(), .0.error)]
    #[diagnostic(code(cli_error::file_open_error), help("Make sure {} exists and you have permissions to read it.", .0.file.display()))]
    SchemaFileOpen(FileOpenError),
    #[error("Error while parsing schema: {}", .0)]
    #[diagnostic(transparent)]
    SchemaParseError(#[from] cedar_policy_core::validator::CedarSchemaError),
    #[error(transparent)]
    #[diagnostic(transparent)]
    JsonSchemaParseError(#[from] cedar_policy_core::validator::SchemaError),
    #[error("Error while deserializing tool descriptions: {}", .0)]
    #[diagnostic(transparent)]
    ToolDezerialization(#[from] mcp_tools_sdk::err::DeserializationError),
    #[error("Error while generating schema: {}", .0)]
    #[diagnostic(transparent)]
    SchemaGenerator(#[from] crate::SchemaGeneratorError),
    #[error("Error trying to create file for writing {}: {}", .0.file.display(), .0.error)]
    #[diagnostic(code(cli_error::file_open_error), help("Make sure to write to/create {}.", .0.file.display()))]
    OpeningSchemaWriteFile(FileOpenError),
    #[error("Error trying to write schema to file {}: {}", .0.file.display(), .0.error)]
    #[diagnostic(code(cli_error::file_write_error), help("Make sure to write to {}.", .0.file.display()))]
    WritingSchemaFile(FileOpenError),
    #[error("Error while trying to serialize schema to JSON: {}", .0)]
    #[diagnostic(
        code(cli_error::serialize_schema_to_json),
        help("Could not serialize produced schema to json")
    )]
    JsonSerializeSchema(#[from] serde_json::Error),
    #[error("Error while trying to serialize schema to Cedar format: {}", .0)]
    #[diagnostic(transparent)]
    CedarSerializeSchema(
        #[from] cedar_policy_core::validator::cedar_schema::fmt::ToCedarSchemaSyntaxError,
    ),
    #[error("Could not open cedar policies file `{}`: {}", .0.file.display(), .0.error)]
    #[diagnostic(code(cli_error::file_open_error), help("Make sure {} exists and you have permissions to read it.", .0.file.display()))]
    PoliciesFileOpen(FileOpenError),
    #[error(transparent)]
    #[diagnostic(transparent)]
    PolicySetReadErrors(#[from] cedar_policy_core::parser::err::ParseErrors),
    #[error("Could not open cedar entities file `{}`: {}", .0.file.display(), .0.error)]
    #[diagnostic(code(cli_error::file_open_error), help("Make sure {} exists and you have permissions to read it.", .0.file.display()))]
    EntitiesFileOpen(FileOpenError),
    #[error(transparent)]
    #[diagnostic(transparent)]
    EntitiesSetReadErrors(#[from] cedar_policy_core::entities::err::EntitiesError),
    #[error("Missing required principal argument")]
    #[diagnostic(
        code(cli_error::missing_principal),
        help("Provide a principal, e.g., using `--principal PrincipalType::\"principal_id\".")
    )]
    MissingPrincipal,
    #[error("Missing required resource argument")]
    #[diagnostic(
        code(cli_error::missing_principal),
        help("Provide a principal, e.g., using `--resource ResourceType::\"resource_id\".")
    )]
    MissingResource,
    #[error("Could not open cedar context file `{}`: {}", .0.file.display(), .0.error)]
    #[diagnostic(code(cli_error::file_open_error), help("Make sure {} exists and you have permissions to read it.", .0.file.display()))]
    ContextFileOpen(FileOpenError),
    #[error(transparent)]
    #[diagnostic(transparent)]
    ContextReadError(#[from] cedar_policy_core::entities::json::ContextJsonDeserializationError),
    #[error(transparent)]
    #[diagnostic(transparent)]
    RequestGeneration(#[from] crate::RequestGeneratorError)
}

impl CliError {
    pub(crate) fn schema_file_open(file: PathBuf, error: std::io::Error) -> Self {
        Self::SchemaFileOpen(FileOpenError { file, error })
    }

    pub(crate) fn write_file_open(file: PathBuf, error: std::io::Error) -> Self {
        Self::OpeningSchemaWriteFile(FileOpenError { file, error })
    }

    pub(crate) fn write_schema_file(file: PathBuf, error: std::io::Error) -> Self {
        Self::WritingSchemaFile(FileOpenError { file, error })
    }

    pub(crate) fn policies_file_open(file: PathBuf, error: std::io::Error) -> Self {
        Self::PoliciesFileOpen(FileOpenError { file, error })
    }

    pub(crate) fn entities_file_open(file: PathBuf, error: std::io::Error) -> Self {
        Self::PoliciesFileOpen(FileOpenError { file, error })
    }

    pub(crate) fn context_file_open(file: PathBuf, error: std::io::Error) -> Self {
        Self::PoliciesFileOpen(FileOpenError { file, error })
    }
}
