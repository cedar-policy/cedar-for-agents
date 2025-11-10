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
}

/// Command Line Interface for Cedar MCP Schema Generator
#[derive(Parser, Debug)]
#[clap(name = "cedar-policy-mcp-schema-generator", version)]
pub struct CliArgs {
    #[clap(subcommand)]
    pub(crate) command: Command,
}
