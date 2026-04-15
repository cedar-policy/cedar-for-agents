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

//! WASM bindings for the Cedar MCP Schema Generator.
//!
//! Exposes [`SchemaGenerator`] to JavaScript/TypeScript via `wasm-bindgen`,
//! enabling Node.js and browser environments to generate Cedar schemas from
//! MCP tool descriptions with the exact same behavior as the Rust implementation.
//!
//! This crate is a thin wrapper: all schema generation logic is delegated to
//! [`cedar_policy_mcp_schema_generator`], including schema stub parsing. This
//! avoids a direct dependency on `cedar-policy-core` in the bindings crate.

use cedar_policy_mcp_schema_generator::{SchemaGenerator, SchemaGeneratorConfig};
use mcp_tools_sdk::description::ServerDescription;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

/// Configuration options for schema generation, matching the Rust
/// [`SchemaGeneratorConfig`] options.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WasmConfig {
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
}

fn default_true() -> bool {
    true
}

impl Default for WasmConfig {
    fn default() -> Self {
        Self {
            include_outputs: false,
            objects_as_records: false,
            erase_annotations: true,
            flatten_namespaces: false,
            numbers_as_decimal: false,
        }
    }
}

impl From<WasmConfig> for SchemaGeneratorConfig {
    fn from(c: WasmConfig) -> Self {
        SchemaGeneratorConfig::default()
            .include_outputs(c.include_outputs)
            .objects_as_records(c.objects_as_records)
            .erase_annotations(c.erase_annotations)
            .flatten_namespaces(c.flatten_namespaces)
            .encode_numbers_as_decimal(c.numbers_as_decimal)
    }
}

/// Result returned to JavaScript from schema generation.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WasmSchemaResult {
    /// The generated Cedar schema as human-readable `.cedarschema` text.
    /// `null` if generation failed.
    schema: Option<String>,
    /// The generated Cedar schema as JSON (for `isAuthorized()`).
    /// `null` if generation failed.
    schema_json: Option<String>,
    /// Error message, `null` if successful.
    error: Option<String>,
    /// Whether generation succeeded.
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
/// * `config_json` - Optional configuration as a JSON string. If `null` or
///   empty, defaults are used.
///
/// # Returns
///
/// A JSON object with `schema` (human-readable), `schemaJson` (for Cedar
/// WASM evaluation), `error`, and `isOk` fields.
#[wasm_bindgen(js_name = "generateSchema")]
pub fn generate_schema(
    schema_stub: &str,
    tools_json: &str,
    // wasm-bindgen requires Option<String>, not Option<&str>, for optional parameters.
    config_json: Option<String>,
) -> String {
    let config_ref = config_json.as_deref();
    let result = generate_schema_inner(schema_stub, tools_json, config_ref);
    drop(config_json);
    serde_json::to_string(&result).unwrap_or_else(|e| {
        format!(
            r#"{{"isOk":false,"error":"Serialization error: {}","schema":null,"schemaJson":null}}"#,
            e
        )
    })
}

/// Convenience constructor for error results.
fn err_result(error: String) -> WasmSchemaResult {
    WasmSchemaResult {
        schema: None,
        schema_json: None,
        error: Some(error),
        is_ok: false,
    }
}

fn generate_schema_inner(
    schema_stub: &str,
    tools_json: &str,
    config_json: Option<&str>,
) -> WasmSchemaResult {
    // Parse config
    let config: SchemaGeneratorConfig = match config_json {
        Some(json) if !json.is_empty() => {
            let Ok(c) = serde_json::from_str::<WasmConfig>(json) else {
                return err_result(format!(
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

    // Parse schema stub and create generator via the generator crate's
    // convenience method, avoiding a direct cedar-policy-core dependency.
    let Ok(mut generator) = SchemaGenerator::from_cedarschema_str_with_config(schema_stub, config)
    else {
        return err_result("Schema error: failed to parse schema stub".to_string());
    };

    // Parse tool descriptions
    let Ok(server_desc) = ServerDescription::from_json_str(tools_json) else {
        return err_result("Invalid tool descriptions: failed to parse JSON".to_string());
    };

    if let Err(e) = generator.add_actions_from_server_description(&server_desc) {
        return err_result(format!("Error adding tools: {e}"));
    }

    // Get the generated schema as a human-readable string
    let schema_text = generator.get_schema_as_str();

    // Convert to JSON for Cedar WASM isAuthorized()
    let Ok(json) = serde_json::to_string_pretty(generator.get_schema()) else {
        return WasmSchemaResult {
            schema: Some(schema_text),
            schema_json: None,
            error: Some("JSON serialization warning: failed to serialize schema".to_string()),
            is_ok: true,
        };
    };

    WasmSchemaResult {
        schema: Some(schema_text),
        schema_json: Some(json),
        error: None,
        is_ok: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_schema_basic() {
        let stub = r#"
            namespace TestServer {
                @mcp_principal
                entity User;
                @mcp_resource
                entity McpServer;
                action "call_tool" appliesTo {
                    principal: [User],
                    resource: [McpServer]
                };
            }
        "#;

        let tools = r#"[
            {
                "name": "read_file",
                "description": "Read a file from disk",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string" }
                    },
                    "required": ["path"]
                }
            }
        ]"#;

        let result_json = generate_schema(stub, tools, None);
        #[expect(clippy::expect_used, reason = "Test assertion")]
        let result: WasmSchemaResult =
            serde_json::from_str(&result_json).expect("Should parse result");

        assert!(
            result.is_ok,
            "Expected success, got error: {:?}",
            result.error
        );
        let schema = result.schema.expect("Schema should be present");
        assert!(
            schema.contains("read_file"),
            "Schema should contain read_file action"
        );
    }

    #[test]
    fn test_invalid_stub() {
        let result_json = generate_schema("not a valid schema", "[]", None);
        #[expect(clippy::expect_used, reason = "Test assertion")]
        let result: WasmSchemaResult =
            serde_json::from_str(&result_json).expect("Should parse result");
        assert!(!result.is_ok);
        assert!(result.error.is_some());
    }

    #[test]
    fn test_empty_tools() {
        let stub = r#"
            namespace TestServer {
                @mcp_principal
                entity User;
                @mcp_resource
                entity McpServer;
                action "call_tool" appliesTo {
                    principal: [User],
                    resource: [McpServer]
                };
            }
        "#;

        let result_json = generate_schema(stub, "[]", None);
        #[expect(clippy::expect_used, reason = "Test assertion")]
        let result: WasmSchemaResult =
            serde_json::from_str(&result_json).expect("Should parse result");
        // Empty tools should still produce a valid (minimal) schema
        assert!(result.is_ok);
    }

    #[test]
    fn test_invalid_config_json() {
        let stub = r#"
            namespace TestServer {
                @mcp_principal
                entity User;
                @mcp_resource
                entity McpServer;
                action "call_tool" appliesTo {
                    principal: [User],
                    resource: [McpServer]
                };
            }
        "#;

        let result_json = generate_schema(stub, "[]", Some("not valid json".to_string()));
        #[expect(clippy::expect_used, reason = "Test assertion")]
        let result: WasmSchemaResult =
            serde_json::from_str(&result_json).expect("Should parse result");
        assert!(!result.is_ok);
        assert!(result
            .error
            .as_deref()
            .unwrap_or("")
            .contains("Invalid config"));
    }

    #[test]
    fn test_invalid_tools_json() {
        let stub = r#"
            namespace TestServer {
                @mcp_principal
                entity User;
                @mcp_resource
                entity McpServer;
                action "call_tool" appliesTo {
                    principal: [User],
                    resource: [McpServer]
                };
            }
        "#;

        let result_json = generate_schema(stub, "not valid json", None);
        #[expect(clippy::expect_used, reason = "Test assertion")]
        let result: WasmSchemaResult =
            serde_json::from_str(&result_json).expect("Should parse result");
        assert!(!result.is_ok);
        assert!(result
            .error
            .as_deref()
            .unwrap_or("")
            .contains("Invalid tool descriptions"));
    }

    #[test]
    fn test_config_with_options() {
        let stub = r#"
            namespace TestServer {
                @mcp_principal
                entity User;
                @mcp_resource
                entity McpServer;
                action "call_tool" appliesTo {
                    principal: [User],
                    resource: [McpServer]
                };
            }
        "#;

        let tools = r#"[
            {
                "name": "calculate",
                "description": "Perform calculation",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "value": { "type": "number" }
                    }
                }
            }
        ]"#;

        let config = r#"{"numbersAsDecimal": true, "includeOutputs": false}"#;

        let result_json = generate_schema(stub, tools, Some(config.to_string()));
        #[expect(clippy::expect_used, reason = "Test assertion")]
        let result: WasmSchemaResult =
            serde_json::from_str(&result_json).expect("Should parse result");
        assert!(
            result.is_ok,
            "Expected success, got error: {:?}",
            result.error
        );
        // Config options should be accepted and produce a valid schema
        assert!(result.schema.is_some());
        assert!(result.schema_json.is_some());
    }

    #[test]
    fn test_empty_config_string() {
        let stub = r#"
            namespace TestServer {
                @mcp_principal
                entity User;
                @mcp_resource
                entity McpServer;
                action "call_tool" appliesTo {
                    principal: [User],
                    resource: [McpServer]
                };
            }
        "#;

        // Empty string config should use defaults (same as None)
        let result_json = generate_schema(stub, "[]", Some(String::new()));
        #[expect(clippy::expect_used, reason = "Test assertion")]
        let result: WasmSchemaResult =
            serde_json::from_str(&result_json).expect("Should parse result");
        assert!(result.is_ok);
    }

    #[test]
    fn test_default_config_values() {
        let config = WasmConfig::default();
        assert!(!config.include_outputs);
        assert!(!config.objects_as_records);
        assert!(config.erase_annotations);
        assert!(!config.flatten_namespaces);
        assert!(!config.numbers_as_decimal);
    }

    #[test]
    fn test_wasm_config_to_schema_config() {
        let wasm_config = WasmConfig {
            include_outputs: true,
            objects_as_records: true,
            erase_annotations: false,
            flatten_namespaces: true,
            numbers_as_decimal: true,
        };
        let _config: SchemaGeneratorConfig = wasm_config.into();
        // Conversion should not panic
    }

    #[test]
    fn test_default_true_helper() {
        assert!(default_true());
    }

    #[test]
    fn test_schema_json_present_on_success() {
        let stub = r#"
            namespace TestServer {
                @mcp_principal
                entity User;
                @mcp_resource
                entity McpServer;
                action "call_tool" appliesTo {
                    principal: [User],
                    resource: [McpServer]
                };
            }
        "#;

        let tools = r#"[
            {
                "name": "test_tool",
                "description": "A test tool",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string" }
                    }
                }
            }
        ]"#;

        let result_json = generate_schema(stub, tools, None);
        #[expect(clippy::expect_used, reason = "Test assertion")]
        let result: WasmSchemaResult =
            serde_json::from_str(&result_json).expect("Should parse result");
        assert!(result.is_ok);
        assert!(result.schema.is_some());
        assert!(result.schema_json.is_some());
        assert!(result.error.is_none());

        // Verify schema_json is valid JSON
        let schema_json = result.schema_json.unwrap();
        assert!(
            serde_json::from_str::<serde_json::Value>(&schema_json).is_ok(),
            "schemaJson should be valid JSON"
        );
    }
}

#[cfg(test)]
mod coverage_tests {
    use super::*;

    /// Stub shared across coverage tests.
    const STUB: &str = r#"
        namespace TestServer {
            @mcp_principal
            entity User;
            @mcp_resource
            entity McpServer;
            action "call_tool" appliesTo {
                principal: [User],
                resource: [McpServer]
            };
        }
    "#;

    #[test]
    fn test_multi_tool_with_diverse_types() {
        // Exercises add_actions_from_server_description with multiple tools
        // and diverse property types (string, integer, boolean) to cover
        // deeper code paths in generate_schema_inner.
        let tools = r#"[
            {
                "name": "search",
                "description": "Search for items",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": { "type": "string" },
                        "limit": { "type": "integer" },
                        "offset": { "type": "integer" }
                    },
                    "required": ["query"]
                }
            },
            {
                "name": "get_item",
                "description": "Get a specific item",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "string" },
                        "include_metadata": { "type": "boolean" }
                    },
                    "required": ["id"]
                }
            }
        ]"#;

        let result_json = generate_schema(STUB, tools, None);
        #[expect(clippy::expect_used, reason = "Test assertion")]
        let result: WasmSchemaResult =
            serde_json::from_str(&result_json).expect("Should parse result");
        assert!(
            result.is_ok,
            "Expected success, got error: {:?}",
            result.error
        );

        let schema = result.schema.expect("Schema should be present");
        assert!(schema.contains("search"), "Should contain search action");
        assert!(
            schema.contains("get_item"),
            "Should contain get_item action"
        );
        assert!(schema.contains("Long"), "Integer should map to Long");
    }

    #[test]
    fn test_all_config_options_enabled() {
        // Exercises the WasmConfig -> SchemaGeneratorConfig conversion
        // with all non-default values to ensure full coverage of the
        // From<WasmConfig> impl.
        let tools = r#"[
            {
                "name": "calc",
                "description": "Calculate",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "value": { "type": "number" },
                        "name": { "type": "string" }
                    }
                }
            }
        ]"#;

        let config = r#"{
            "numbersAsDecimal": true,
            "includeOutputs": true,
            "objectsAsRecords": true,
            "eraseAnnotations": false,
            "flattenNamespaces": true
        }"#;

        let result_json = generate_schema(STUB, tools, Some(config.to_string()));
        #[expect(clippy::expect_used, reason = "Test assertion")]
        let result: WasmSchemaResult =
            serde_json::from_str(&result_json).expect("Should parse result");
        assert!(
            result.is_ok,
            "Expected success with all config, got error: {:?}",
            result.error
        );
        assert!(result.schema.is_some());
        assert!(result.schema_json.is_some());
    }

    #[test]
    fn test_error_result_fields_complete() {
        // Verifies all fields of the WasmSchemaResult on error:
        // schema and schema_json should be None, error should explain
        // the failure, is_ok should be false.
        let result_json = generate_schema("invalid", "[]", None);
        #[expect(clippy::expect_used, reason = "Test assertion")]
        let result: WasmSchemaResult =
            serde_json::from_str(&result_json).expect("Should parse result");
        assert!(!result.is_ok);
        assert!(result.schema.is_none(), "Schema should be None on error");
        assert!(
            result.schema_json.is_none(),
            "SchemaJson should be None on error"
        );
        assert!(result.error.is_some(), "Error should be present");
        assert!(
            result
                .error
                .as_deref()
                .unwrap_or("")
                .contains("Schema error"),
            "Error should indicate schema parsing failure"
        );
    }

    #[test]
    fn test_tool_with_nested_object() {
        // Exercises object type mapping paths in schema generation.
        let tools = r#"[
            {
                "name": "create_record",
                "description": "Create a record",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string" },
                        "metadata": {
                            "type": "object",
                            "properties": {
                                "created_by": { "type": "string" },
                                "priority": { "type": "integer" }
                            }
                        }
                    },
                    "required": ["name"]
                }
            }
        ]"#;

        let result_json = generate_schema(STUB, tools, None);
        #[expect(clippy::expect_used, reason = "Test assertion")]
        let result: WasmSchemaResult =
            serde_json::from_str(&result_json).expect("Should parse result");
        assert!(
            result.is_ok,
            "Expected success, got error: {:?}",
            result.error
        );
        assert!(result.schema.is_some());
        assert!(result.schema_json.is_some());
    }

    #[test]
    fn test_tool_with_array_property() {
        // Exercises array type mapping.
        let tools = r#"[
            {
                "name": "process_batch",
                "description": "Process items",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "items": {
                            "type": "array",
                            "items": { "type": "string" }
                        }
                    },
                    "required": ["items"]
                }
            }
        ]"#;

        let result_json = generate_schema(STUB, tools, None);
        #[expect(clippy::expect_used, reason = "Test assertion")]
        let result: WasmSchemaResult =
            serde_json::from_str(&result_json).expect("Should parse result");
        assert!(
            result.is_ok,
            "Expected success, got error: {:?}",
            result.error
        );
        let schema = result.schema.expect("Schema");
        assert!(
            schema.contains("process_batch"),
            "Should contain action name"
        );
    }

    #[test]
    fn test_config_partial_options() {
        // Only some config options set (exercises serde defaults).
        let tools = r#"[
            {
                "name": "test",
                "description": "test",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "x": { "type": "string" }
                    }
                }
            }
        ]"#;

        let config = r#"{"objectsAsRecords": true}"#;
        let result_json = generate_schema(STUB, tools, Some(config.to_string()));
        #[expect(clippy::expect_used, reason = "Test assertion")]
        let result: WasmSchemaResult =
            serde_json::from_str(&result_json).expect("Should parse result");
        assert!(
            result.is_ok,
            "Expected success, got error: {:?}",
            result.error
        );
    }

    #[test]
    fn test_generate_schema_inner_directly() {
        // Calls generate_schema_inner with various config_json values
        // to ensure the match arm coverage.
        let result = generate_schema_inner(STUB, "[]", None);
        assert!(result.is_ok);

        let result = generate_schema_inner(STUB, "[]", Some(""));
        assert!(result.is_ok);

        let result = generate_schema_inner(STUB, "[]", Some("{}"));
        assert!(result.is_ok);
    }
}
