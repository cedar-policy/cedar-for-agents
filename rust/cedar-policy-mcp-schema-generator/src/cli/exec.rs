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

use crate::cli::{
    CliArgs, CliError, Command, ConfigOptions, ErrorFormat, OutputFormat, PoliciesArgs, RequestArgs,
};
use crate::{SchemaGenerator, SchemaGeneratorConfig};

use cedar_policy_core::ast::{Context, EntityUID, PolicySet};
use cedar_policy_core::entities::Entities;
use cedar_policy_core::extensions::Extensions;
use cedar_policy_core::validator::{json_schema::Fragment, RawName};

use mcp_tools_sdk::data::{Input, Output};
use mcp_tools_sdk::description::ServerDescription;

use std::path::{Path, PathBuf};

fn get_config(config_options: &ConfigOptions) -> SchemaGeneratorConfig {
    SchemaGeneratorConfig::default()
        .include_outputs(config_options.include_outputs)
        .objects_as_records(config_options.objects_as_records)
        .erase_annotations(!config_options.keep_annotations)
        .flatten_namespaces(config_options.flatten_namespaces)
        .encode_numbers_as_decimal(config_options.encode_numbers_as_decimal)
}

fn read_schema(file: impl AsRef<Path>) -> Result<Fragment<RawName>, CliError> {
    let file = file.as_ref();
    match file.extension().and_then(|ext| ext.to_str()) {
        Some("json") => {
            let json_file = match std::fs::File::open(file) {
                Ok(json_file) => json_file,
                Err(e) => return Err(CliError::schema_file_open(file.to_path_buf(), e)),
            };
            Ok(Fragment::from_json_file(json_file)?)
        }
        Some("cedarschema") => {
            let schema_file = match std::fs::File::open(file) {
                Ok(schema_file) => schema_file,
                Err(e) => return Err(CliError::schema_file_open(file.to_path_buf(), e)),
            };
            Ok(Fragment::from_cedarschema_file(schema_file, Extensions::all_available())?.0)
        }
        _ => Err(CliError::UnrecognizedSchemaExtension),
    }
}

#[allow(clippy::ref_option)]
fn output_schema(
    schema: &Fragment<RawName>,
    output_location: &Option<PathBuf>,
    output_format: OutputFormat,
) -> Result<(), CliError> {
    let mut writer: Box<dyn std::io::Write> = match output_location {
        None => Box::new(std::io::stdout()),
        Some(file) => match std::fs::File::create(file) {
            Ok(fs) => Box::new(fs),
            Err(e) => return Err(CliError::write_file_open(file.clone(), e)),
        },
    };
    match output_format {
        OutputFormat::Human => writeln!(writer, "{}", schema.to_cedarschema()?),
        OutputFormat::Json => writeln!(writer, "{}", serde_json::to_string(schema)?),
    }
    .map_err(|e| {
        CliError::write_schema_file(
            output_location
                .clone()
                .unwrap_or_else(|| <str as AsRef<Path>>::as_ref("stdout").to_path_buf()),
            e,
        )
    })
}

impl CliArgs {
    pub fn exec(&self) -> Result<(), CliError> {
        match &self.command {
            Command::Generate {
                schema_stub,
                tool_descriptions,
                output,
                output_format,
                config,
                ..
            } => {
                let config = get_config(config);
                let schema_stub = read_schema(schema_stub)?;
                let tool_descriptions = ServerDescription::from_json_file(tool_descriptions)?;
                let mut schema_generator = SchemaGenerator::new_with_config(schema_stub, config)?;
                schema_generator.add_actions_from_server_description(&tool_descriptions)?;
                output_schema(schema_generator.get_schema(), output, *output_format)
            }
            Command::Authorize {
                schema_stub,
                tool_descriptions,
                config,
                request,
                policies,
                entities,
                mcp_tool_input,
                mcp_tool_output,
                ..
            } => {
                let config = get_config(config);
                let schema_stub = read_schema(schema_stub)?;
                let tool_descriptions = ServerDescription::from_json_file(tool_descriptions)?;
                let mut schema_generator = SchemaGenerator::new_with_config(schema_stub, config)?;
                schema_generator.add_actions_from_server_description(&tool_descriptions)?;
                let request_generator = schema_generator.new_request_generator()?;
                let policies = read_policies(policies)?;
                let entities = read_entities(entities)?;
                let (principal, resource, context) = read_request(request)?;
                let input = Input::from_json_file(mcp_tool_input)?;
                let output = mcp_tool_output
                    .as_ref()
                    .map(Output::from_json_file)
                    .transpose()?;
                let (request, entities) = request_generator.generate_request(
                    principal,
                    resource,
                    context.into_iter(),
                    entities,
                    &input,
                    output.as_ref(),
                )?;
                let authorizer = cedar_policy_core::authorizer::Authorizer::new();
                match authorizer
                    .is_authorized(request, &policies, &entities)
                    .decision
                {
                    cedar_policy_core::authorizer::Decision::Allow => println!("ALLOW"),
                    cedar_policy_core::authorizer::Decision::Deny => println!("DENY"),
                };
                Ok(())
            }
        }
    }

    pub fn get_error_format(&self) -> ErrorFormat {
        match &self.command {
            Command::Generate { error_format, .. } => *error_format,
            Command::Authorize { error_format, .. } => *error_format,
        }
    }
}

fn read_policies(args: &PoliciesArgs) -> Result<PolicySet, CliError> {
    let policy_str = match std::fs::read_to_string(args.policies_file.clone()) {
        Ok(str) => str,
        Err(e) => return Err(CliError::policies_file_open(args.policies_file.clone(), e)),
    };
    Ok(cedar_policy_core::parser::parse_policyset(&policy_str)?)
}

fn read_entities(file: impl AsRef<Path>) -> Result<Entities, CliError> {
    let file = file.as_ref();
    let entities_str = match std::fs::read_to_string(file) {
        Ok(str) => str,
        Err(e) => return Err(CliError::entities_file_open(file.to_path_buf(), e)),
    };
    let eparser = cedar_policy_core::entities::EntityJsonParser::new(
        None::<&cedar_policy_core::validator::CoreSchema<'_>>,
        cedar_policy_core::extensions::Extensions::all_available(),
        cedar_policy_core::entities::TCComputation::ComputeNow,
    );
    Ok(eparser.from_json_str(&entities_str)?)
}

/// This struct is the serde structure expected for --request-json
#[derive(Clone, Debug, serde::Deserialize)]
struct RequestJSON {
    /// Principal for the request
    #[serde(default)]
    principal: String,
    /// Resource for the request
    #[serde(default)]
    resource: String,
    /// Context for the request
    context: serde_json::Value,
}

fn read_request(args: &RequestArgs) -> Result<(EntityUID, EntityUID, Context), CliError> {
    match &args.request_json_file {
        Some(file) => match std::fs::read_to_string(file) {
            Ok(s) => {
                let qjson: RequestJSON =
                    serde_json::from_str(&s).map_err(CliError::RequestReadError)?;
                let principal = qjson
                    .principal
                    .parse()
                    .map_err(CliError::MalformedPrincipal)?;
                let resource = qjson
                    .resource
                    .parse()
                    .map_err(CliError::MalformedResource)?;
                let context = Context::from_json_value(qjson.context)?;
                Ok((principal, resource, context))
            }
            Err(e) => Err(CliError::request_json_file_open(file.into(), e)),
        },
        None => {
            let principal = args
                .principal
                .as_ref()
                .ok_or_else(|| CliError::MissingPrincipal)?;
            let principal: EntityUID = principal.parse()?;
            let resource = args
                .resource
                .as_ref()
                .ok_or_else(|| CliError::MissingResource)?;
            let resource: EntityUID = resource.parse()?;
            let context = match &args.context_json_file {
                Some(ctx_file) => {
                    let context_str = match std::fs::read_to_string(ctx_file) {
                        Ok(str) => str,
                        Err(e) => return Err(CliError::context_file_open(ctx_file.into(), e)),
                    };
                    Context::from_json_str(&context_str)?
                }
                None => Context::empty(),
            };
            Ok((principal, resource, context))
        }
    }
}
