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

use crate::cli::{CliArgs, CliError, ConfigOptions, Command, ErrorFormat, OutputFormat};
use crate::{SchemaGenerator, SchemaGeneratorConfig, ServerDescription};

use cedar_policy_core::validator::{json_schema::Fragment, RawName};
use cedar_policy_core::extensions::Extensions;

use std::path::{Path, PathBuf};

fn get_config(config_options: &ConfigOptions) -> SchemaGeneratorConfig {
    SchemaGeneratorConfig::default()
        .include_outputs(config_options.include_outputs)
        .objects_as_records(config_options.objects_as_records)
        .erase_annotations(!config_options.keep_annotations)
        .flatten_namespaces(config_options.flatten_namespaces)
}

fn read_schema(file: impl AsRef<Path>) -> Result<Fragment<RawName>, CliError> {
    let file = file.as_ref();
    match file.extension().and_then(|ext| ext.to_str()) {
        Some("json") => {
            let json_file = match std::fs::File::open(file) {
                Ok(json_file) => json_file,
                Err(e) => return Err(CliError::schema_file_open(file.to_path_buf(), e))
            };
            Ok(Fragment::from_json_file(json_file)?)
        }
        Some("cedarschema") => {
            let schema_file = match std::fs::File::open(file) {
                Ok(schema_file) => schema_file,
                Err(e) => return Err(CliError::schema_file_open(file.to_path_buf(), e))
            };
            Ok(Fragment::from_cedarschema_file(schema_file, Extensions::all_available())?.0)
        }
        _ => Err(CliError::UnrecognizedSchemaExtension)
    }
}

fn output_schema(schema: &Fragment<RawName>, output_location: &Option<PathBuf>, output_format: &OutputFormat) -> Result<(), CliError> {
    let mut writer: Box<dyn std::io::Write> = match output_location {
        None => Box::new(std::io::stdout()),
        Some(file) => match std::fs::File::create(file) {
            Ok(fs) => Box::new(fs),
            Err(e) => return Err(CliError::write_file_open(file.clone(), e)),
        }
    };
    match output_format {
        OutputFormat::Human => writeln!(writer, "{}", schema.to_cedarschema()?),
        OutputFormat::Json => writeln!(writer, "{}", serde_json::to_string(schema)?),
    }.map_err(|e| CliError::write_schema_file(output_location.clone().unwrap_or_else(|| <str as AsRef<Path>>::as_ref("stdout").to_path_buf()), e))
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
                output_schema(schema_generator.get_schema(), output, output_format)
            }
        }
    }

    pub fn get_error_format(&self) -> ErrorFormat {
        match &self.command {
            Command::Generate { error_format, .. } => error_format.clone(),
        }
    }
}