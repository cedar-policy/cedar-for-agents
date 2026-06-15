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

//! Python bindings for the Cedar MCP Schema Generator.
//!
//! Exposes [`SchemaGenerator`] to Python via PyO3, enabling Python environments
//! to generate Cedar schemas from MCP tool descriptions with the exact same
//! behavior as the Rust implementation.
//!
//! This crate is a thin wrapper: all schema generation logic is delegated to
//! [`cedar_policy_mcp_schema_generator`].

use cedar_policy_mcp_schema_generator::{SchemaGenerator, SchemaGeneratorConfig};
use mcp_tools_sdk::data::Input;
use mcp_tools_sdk::description::ServerDescription;
use pyo3::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PyConfig {
    #[serde(default)]
    include_outputs: bool,
    #[serde(default)]
    objects_as_records: bool,
    #[serde(default = "default_true")]
    erase_annotations: bool,
    #[serde(default)]
    flatten_namespaces: bool,
    #[serde(default)]
    numbers_as_decimal: bool,
    #[serde(default)]
    deduplicate_entity_types: bool,
}

fn default_true() -> bool {
    true
}

impl Default for PyConfig {
    fn default() -> Self {
        Self {
            include_outputs: false,
            objects_as_records: false,
            erase_annotations: true,
            flatten_namespaces: false,
            numbers_as_decimal: false,
            deduplicate_entity_types: false,
        }
    }
}

impl From<PyConfig> for SchemaGeneratorConfig {
    fn from(c: PyConfig) -> Self {
        SchemaGeneratorConfig::default()
            .include_outputs(c.include_outputs)
            .objects_as_records(c.objects_as_records)
            .erase_annotations(c.erase_annotations)
            .flatten_namespaces(c.flatten_namespaces)
            .encode_numbers_as_decimal(c.numbers_as_decimal)
            .deduplicate_entity_types(c.deduplicate_entity_types)
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SchemaResult {
    schema: Option<String>,
    schema_json: Option<String>,
    error: Option<String>,
    is_ok: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RequestResult {
    principal: Option<String>,
    action: Option<String>,
    resource: Option<String>,
    entities_json: Option<String>,
    error: Option<String>,
    is_ok: bool,
}

/// Generate a Cedar schema from a schema stub and MCP tool descriptions.
///
/// # Arguments
///
/// * `schema_stub` - A Cedar schema stub as a `.cedarschema` string. Must
///   contain entity types annotated with `@mcp_principal` and `@mcp_resource`.
/// * `tools_json` - MCP tool descriptions as a JSON string. This should be
///   the `tools` array from an MCP `tools/list` response.
/// * `config_json` - Optional configuration as a JSON string. If `None` or
///   empty, defaults are used.
///
/// # Returns
///
/// A JSON string with `schema`, `schemaJson`, `error`, and `isOk` fields.
#[pyfunction]
#[pyo3(signature = (schema_stub, tools_json, config_json=None))]
fn generate_schema(schema_stub: &str, tools_json: &str, config_json: Option<&str>) -> String {
    let result = generate_schema_inner(schema_stub, tools_json, config_json);
    serde_json::to_string(&result).unwrap_or_else(|e| {
        format!(
            r#"{{"isOk":false,"error":"Serialization error: {}","schema":null,"schemaJson":null}}"#,
            e
        )
    })
}

/// Generate a Cedar authorization request from an MCP tool call.
///
/// # Arguments
///
/// * `schema_stub` - A Cedar schema stub as a `.cedarschema` string.
/// * `tools_json` - MCP tool descriptions as a JSON string.
/// * `input_json` - MCP tool call input as a JSON string. Format:
///   `{"params": {"tool": "tool_name", "args": {"key": "value"}}}`.
/// * `principal_type` - The Cedar entity type for the principal (e.g., `"User"`).
/// * `principal_id` - The principal identifier (e.g., `"alice"`).
/// * `resource_type` - The Cedar entity type for the resource (e.g., `"McpServer"`).
/// * `resource_id` - The resource identifier (e.g., `"my-server"`).
/// * `config_json` - Optional configuration as a JSON string.
///
/// # Returns
///
/// A JSON string with `principal`, `action`, `resource`, `entitiesJson`,
/// `error`, and `isOk` fields.
#[pyfunction]
#[pyo3(signature = (schema_stub, tools_json, input_json, principal_type, principal_id, resource_type, resource_id, config_json=None))]
fn generate_request(
    schema_stub: &str,
    tools_json: &str,
    input_json: &str,
    principal_type: &str,
    principal_id: &str,
    resource_type: &str,
    resource_id: &str,
    config_json: Option<&str>,
) -> String {
    let result = generate_request_inner(
        schema_stub,
        tools_json,
        input_json,
        principal_type,
        principal_id,
        resource_type,
        resource_id,
        config_json,
    );
    serde_json::to_string(&result).unwrap_or_else(|e| {
        format!(
            r#"{{"isOk":false,"error":"Serialization error: {}","principal":null,"action":null,"resource":null,"entitiesJson":null}}"#,
            e
        )
    })
}

fn generate_schema_inner(
    schema_stub: &str,
    tools_json: &str,
    config_json: Option<&str>,
) -> SchemaResult {
    let config: SchemaGeneratorConfig = match config_json {
        Some(json) if !json.is_empty() => {
            let Ok(c) = serde_json::from_str::<PyConfig>(json) else {
                return schema_err(format!(
                    "Invalid config: {}",
                    serde_json::from_str::<serde_json::Value>(json)
                        .err()
                        .map_or_else(|| "unrecognized fields".to_string(), |e| e.to_string())
                ));
            };
            c.into()
        }
        _ => SchemaGeneratorConfig::default(),
    };

    let Ok(mut generator) = SchemaGenerator::from_cedarschema_str_with_config(schema_stub, config)
    else {
        return schema_err("Schema error: failed to parse schema stub".to_string());
    };

    let Ok(server_desc) = ServerDescription::from_json_str(tools_json) else {
        return schema_err("Invalid tool descriptions: failed to parse JSON".to_string());
    };

    if let Err(e) = generator.add_actions_from_server_description(&server_desc) {
        return schema_err(format!("Error adding tools: {e}"));
    }

    let schema_text = generator.get_schema_as_str();

    let Ok(json) = serde_json::to_string_pretty(generator.get_schema()) else {
        return SchemaResult {
            schema: Some(schema_text),
            schema_json: None,
            error: Some("JSON serialization warning: failed to serialize schema".to_string()),
            is_ok: true,
        };
    };

    SchemaResult {
        schema: Some(schema_text),
        schema_json: Some(json),
        error: None,
        is_ok: true,
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "Mirrors the flat parameter list from the WASM crate"
)]
fn generate_request_inner(
    schema_stub: &str,
    tools_json: &str,
    input_json: &str,
    principal_type: &str,
    principal_id: &str,
    resource_type: &str,
    resource_id: &str,
    config_json: Option<&str>,
) -> RequestResult {
    let config: SchemaGeneratorConfig = match config_json {
        Some(json) if !json.is_empty() => {
            let Ok(c) = serde_json::from_str::<PyConfig>(json) else {
                return request_err(format!(
                    "Invalid config: {}",
                    serde_json::from_str::<serde_json::Value>(json)
                        .err()
                        .map_or_else(|| "unrecognized fields".to_string(), |e| e.to_string())
                ));
            };
            c.into()
        }
        _ => SchemaGeneratorConfig::default(),
    };

    let Ok(mut generator) = SchemaGenerator::from_cedarschema_str_with_config(schema_stub, config)
    else {
        return request_err("Schema error: failed to parse schema stub".to_string());
    };

    let Ok(server_desc) = ServerDescription::from_json_str(tools_json) else {
        return request_err("Invalid tool descriptions: failed to parse JSON".to_string());
    };

    if let Err(e) = generator.add_actions_from_server_description(&server_desc) {
        return request_err(format!("Error adding tools: {e}"));
    }

    let Ok(req_gen) = generator.new_request_generator() else {
        return request_err("Failed to create request generator".to_string());
    };

    let Ok(input) = Input::from_json_str(input_json) else {
        return request_err("Invalid tool input: failed to parse JSON".to_string());
    };

    match req_gen.generate_request_components_from_strings(
        &input,
        principal_type,
        principal_id,
        resource_type,
        resource_id,
    ) {
        Ok(components) => RequestResult {
            principal: Some(components.principal),
            action: Some(components.action),
            resource: Some(components.resource),
            entities_json: Some(components.entities_json),
            error: None,
            is_ok: true,
        },
        Err(e) => request_err(format!("Request generation error: {e}")),
    }
}

fn schema_err(error: String) -> SchemaResult {
    SchemaResult {
        schema: None,
        schema_json: None,
        error: Some(error),
        is_ok: false,
    }
}

fn request_err(error: String) -> RequestResult {
    RequestResult {
        principal: None,
        action: None,
        resource: None,
        entities_json: None,
        error: Some(error),
        is_ok: false,
    }
}

#[pymodule]
#[pyo3(name = "_native")]
fn cedar_mcp_schema_generator(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(generate_schema, m)?)?;
    m.add_function(wrap_pyfunction!(generate_request, m)?)?;
    Ok(())
}
