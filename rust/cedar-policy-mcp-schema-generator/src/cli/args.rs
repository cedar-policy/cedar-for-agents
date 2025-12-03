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

use clap::{Args, Parser, Subcommand, ValueEnum};
use serde::Serialize;
use std::path::PathBuf;

/// This struct contains the arguments that together specify a request.
#[derive(Args, Clone, Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct RequestArgs {
    /// Principal for the request, e.g., MyMcpServer::User::"Alice"
    #[arg(short = 'l', long)]
    pub principal: Option<String>,
    /// Resource for the request, e.g., MyMcpServer::McpServer::"Server 0"
    #[arg(short, long)]
    pub resource: Option<String>,
    /// File containing a JSON object representing the context for the request.
    /// Should be a (possibly empty) map from keys to values.
    #[arg(short, long = "context", value_name = "FILE")]
    pub context_json_file: Option<String>,
    /// File containing a JSON object representing the entire request. Must have
    /// fields "principal", "action", "resource", and "context", where "context"
    /// is a (possibly empty) map from keys to values. This option replaces
    /// --principal, --resource, etc.
    #[arg(long = "request-json", value_name = "FILE", conflicts_with_all = &["principal", "resource", "context_json_file"])]
    pub request_json_file: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum, Serialize)]
pub enum PolicyFormat {
    /// The standard Cedar policy format, documented at <https://docs.cedarpolicy.com/policies/syntax-policy.html>
    #[default]
    Cedar,
    /// Cedar's JSON policy format, documented at <https://docs.cedarpolicy.com/policies/json-format.html>
    Json,
}

/// This struct contains the arguments that together specify an input policy or policy set.
#[derive(Args, Clone, Debug, Serialize)]
pub struct PoliciesArgs {
    /// File containing the static Cedar policies and/or templates. If not provided, read policies from stdin.
    #[arg(short, long = "policies", value_name = "FILE", required = true)]
    pub policies_file: PathBuf,
    /// Format of policies in the `--policies` file
    #[arg(long = "policy-format", default_value_t, value_enum)]
    pub policy_format: PolicyFormat,
}

#[derive(Args, Clone, Debug, Serialize)]
#[clap(next_help_heading = "Configuration Options")]
#[serde(rename_all = "kebab-case")]
pub(crate) struct ConfigOptions {
    /// Whether to encode the `OutputSchema` of each tool as an optional attribute of the tool's action's context (default: false).
    #[arg(long, default_value_t = false)]
    pub(crate) include_outputs: bool,
    /// Whether to encode "object" typed properties as a Record Type when the object does not have `additionalProperties` (default: false).
    #[arg(long, default_value_t = false)]
    pub(crate) objects_as_records: bool,
    /// Whether to keep `mcp_principal`, `mcp_resource`, `mcp_context`, and `mcp_action` annotations in the final schema (default: false).
    #[arg(long, default_value_t = false)]
    pub(crate) keep_annotations: bool,
    /// Whether to create an output schema with a single namespace by flattening names---e.g., `Foo::Baz::Bar` becomes `Foo_Baz_Bar`---(default: false).
    #[arg(long, default_value_t = false)]
    pub(crate) flatten_namespaces: bool,
    /// Whether to encode all `"number"` and `"float"` typed paramaters in input MCP tool descriptions as Cedar `decimal` type (default: false).
    ///
    /// Note: Representing `"number"` and `"float"` type parameters as `decimals` results
    /// in a loss of precision as `decimal`s only have four decimal places of precision.
    /// This may result in unsound authorization policies. For example `x < y` is true for
    /// `x = 2` and `y = 2.00004`. However, when converted to decimals, `x < y` evaluates to
    /// false as `x == y == 2.0000`. Additionally, numbers & floats have a significantly larger
    /// range than decimals. Decimals are limited between [-922337203685477.5808, 922337203685477.5807].
    #[arg(long, default_value_t = false)]
    pub(crate) encode_numbers_as_decimal: bool,
}

#[derive(ValueEnum, Clone, Copy, Debug, Serialize)]
pub(crate) enum OutputFormat {
    /// Human Readable Cedar Schema Format.
    Human,
    /// Json Cedar Schema Format.
    Json,
}

#[derive(ValueEnum, Clone, Copy, Debug, Serialize)]
pub enum ErrorFormat {
    /// Human-readable error messages with terminal graphics and inline code snippets.
    Human,
    /// Plain-text error messages without fancy graphics or colors, suitable for screen readers.
    Plain,
    /// Machine-readable JSON output.
    Json,
}

#[derive(Clone, Debug, Serialize, Subcommand)]
pub(crate) enum Command {
    /// Generate a Cedar Schema by adding an action for each tool description of an MCP
    /// Tools Description file to an initial Cedar Schema stub file.
    Generate {
        /// A Cedar Schema stub file used as the basis of the ouptut schema.
        #[clap(required = true)]
        schema_stub: PathBuf,
        /// A file containing the MCP Tool Descriptions to add as actions to schema stub file.
        #[clap(required = true)]
        tool_descriptions: PathBuf,
        /// The location to save the output Cedar Schema (default: stdout).
        #[arg(long, value_name = "OUTPUT_FILE")]
        output: Option<PathBuf>,
        #[arg(long, default_value = "human")]
        output_format: OutputFormat,
        #[arg(long, default_value = "human")]
        error_format: ErrorFormat,
        #[clap(flatten)]
        config: ConfigOptions,
    },
    /// Convert MCP tool Input & Output to a Cedar Authorization Request and check authorization
    /// against a set of policies.
    ///
    /// Requires:
    /// 1. MCP tool description to successfully generate to a Schema,
    /// 2. Policies to validate against the generated Schema,
    /// 3. MCP Input/Output data to validate against MCP tool description, and
    /// 4. Input Entities / Context / Principal / Resource and generated Request components to validate against generated Schema.
    Authorize {
        /// A Cedar Schema stub file used as the basis of the ouptut schema.
        #[clap(required = true)]
        schema_stub: PathBuf,
        /// A file containing the MCP Tool Descriptions to add as actions to schema stub file.
        #[clap(required = true)]
        tool_descriptions: PathBuf,
        #[arg(long, default_value = "human")]
        output_format: OutputFormat,
        #[arg(long, default_value = "human")]
        error_format: ErrorFormat,
        #[clap(flatten)]
        request: RequestArgs,
        #[clap(flatten)]
        policies: PoliciesArgs,
        #[arg(long = "entities", value_name = "FILE")]
        entities: PathBuf,
        #[arg(long = "mcp-tool-input", value_name = "FILE")]
        mcp_tool_input: PathBuf,
        #[arg(long = "mcp-tool-output", value_name = "FILE")]
        mcp_tool_output: Option<PathBuf>,
        #[clap(flatten)]
        config: ConfigOptions,
    },
}

/// Command Line Interface for Cedar MCP Schema Generator
#[derive(Parser, Debug)]
#[clap(name = "cedar-policy-mcp-schema-generator", version)]
pub struct CliArgs {
    #[clap(subcommand)]
    pub(crate) command: Command,
}
