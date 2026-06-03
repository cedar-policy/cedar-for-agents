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

//! WASM integration tests for the Cedar MCP Schema Generator bindings.
//!
//! These tests exercise `generateSchema` and `generateRequest` through
//! wasm-bindgen, verifying correct behavior at the JS/WASM boundary.

#![expect(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "test assertions"
)]

use cedar_policy_mcp_schema_generator_wasm::{generate_request, generate_schema};
use wasm_bindgen_test::*;

// ─── Shared constants ───────────────────────────────────────────────────────

/// Minimal schema stub used across most tests.
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

/// A single-tool description (string-typed input) reused across tests.
const SINGLE_TOOL: &str = r#"[{
    "name": "read_file",
    "description": "Read a file from disk",
    "inputSchema": {
        "type": "object",
        "properties": { "path": { "type": "string" } },
        "required": ["path"]
    }
}]"#;

// ─── Helpers ────────────────────────────────────────────────────────────────

/// Parse a JSON result string into a `serde_json::Value`.
fn parse_result(json: &str) -> serde_json::Value {
    serde_json::from_str(json).expect("Result should be valid JSON")
}

/// Assert a schema result succeeded and return the schema text.
fn assert_schema_ok(result: &serde_json::Value) -> &str {
    assert_eq!(result["isOk"], true, "Error: {:?}", result["error"]);
    assert!(result["error"].is_null());
    result["schema"]
        .as_str()
        .expect("schema should be a string")
}

/// Assert a schema result failed and return the error message.
fn assert_schema_err(result: &serde_json::Value) -> &str {
    assert_eq!(result["isOk"], false);
    assert!(result["schema"].is_null());
    result["error"]
        .as_str()
        .expect("error should be a string")
}

/// Assert a request result succeeded and return the parsed value.
fn assert_request_ok(result: &serde_json::Value) {
    assert_eq!(result["isOk"], true, "Error: {:?}", result["error"]);
    assert!(result["error"].is_null());
    assert!(!result["principal"].is_null());
    assert!(!result["action"].is_null());
    assert!(!result["resource"].is_null());
    assert!(!result["entitiesJson"].is_null());
}

/// Assert a request result failed and return the error message.
fn assert_request_err(result: &serde_json::Value) -> &str {
    assert_eq!(result["isOk"], false);
    assert!(result["principal"].is_null());
    assert!(result["action"].is_null());
    assert!(result["resource"].is_null());
    assert!(result["entitiesJson"].is_null());
    result["error"]
        .as_str()
        .expect("error should be a string")
}

/// Build an MCP tool call input JSON string for a given tool name and args object.
fn tool_input(tool_name: &str, args_json: &str) -> String {
    format!(r#"{{"params": {{"tool": "{tool_name}", "args": {args_json}}}}}"#)
}

// ─── Schema generation tests ────────────────────────────────────────────────

#[wasm_bindgen_test]
fn test_basic_schema_generation() {
    let result = parse_result(&generate_schema(STUB, SINGLE_TOOL, None));
    let schema = assert_schema_ok(&result);

    assert!(schema.contains("read_file"));
    assert!(schema.contains("read_fileInput"));
    assert!(schema.contains("String"));

    // schemaJson should be valid JSON
    let schema_json_str = result["schemaJson"].as_str().unwrap();
    serde_json::from_str::<serde_json::Value>(schema_json_str)
        .expect("schemaJson should be valid JSON");
}

#[wasm_bindgen_test]
fn test_multi_tool_schema() {
    let tools = r#"[
        {
            "name": "execute_command",
            "description": "Execute a shell command",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "command": { "type": "string" },
                    "timeout": { "type": "integer" }
                },
                "required": ["command"]
            }
        },
        {
            "name": "read_file",
            "description": "Read a file",
            "inputSchema": {
                "type": "object",
                "properties": { "path": { "type": "string" } },
                "required": ["path"]
            }
        }
    ]"#;

    let result = parse_result(&generate_schema(STUB, tools, None));
    let schema = assert_schema_ok(&result);

    assert!(schema.contains("execute_command"));
    assert!(schema.contains("read_file"));
    assert!(schema.contains("Long"), "integer should map to Long");
}

#[wasm_bindgen_test]
fn test_config_numbers_as_decimal() {
    let tools = r#"[{
        "name": "calculate",
        "description": "Calculate",
        "inputSchema": {
            "type": "object",
            "properties": { "value": { "type": "number" } },
            "required": ["value"]
        }
    }]"#;

    let result = parse_result(&generate_schema(
        STUB,
        tools,
        Some(r#"{"numbersAsDecimal": true}"#.to_string()),
    ));
    let schema = assert_schema_ok(&result);
    assert!(schema.contains("Decimal"));
}

#[wasm_bindgen_test]
fn test_config_objects_as_records() {
    let tools = r#"[{
        "name": "create",
        "description": "Create item",
        "inputSchema": {
            "type": "object",
            "properties": {
                "meta": {
                    "type": "object",
                    "properties": { "key": { "type": "string" } },
                    "required": ["key"]
                }
            }
        }
    }]"#;

    let without = parse_result(&generate_schema(STUB, tools, None));
    let with = parse_result(&generate_schema(
        STUB,
        tools,
        Some(r#"{"objectsAsRecords": true}"#.to_string()),
    ));

    assert_schema_ok(&without);
    assert_schema_ok(&with);
    // objectsAsRecords changes representation; schemas should differ
    assert_ne!(without["schema"], with["schema"]);
}

#[wasm_bindgen_test]
fn test_config_flatten_namespaces() {
    let tools = r#"[{
        "name": "do_thing",
        "description": "Do",
        "inputSchema": {
            "type": "object",
            "properties": { "x": { "type": "string" } }
        }
    }]"#;

    let result = parse_result(&generate_schema(
        STUB,
        tools,
        Some(r#"{"flattenNamespaces": true}"#.to_string()),
    ));
    assert_schema_ok(&result);
}

#[wasm_bindgen_test]
fn test_config_deduplicate_entity_types() {
    // Two tools with identical enums; deduplication should consolidate them.
    let tools = r#"[
        {
            "name": "tool_a",
            "description": "A",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "status": { "type": "string", "enum": ["on", "off"] }
                }
            }
        },
        {
            "name": "tool_b",
            "description": "B",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "status": { "type": "string", "enum": ["on", "off"] }
                }
            }
        }
    ]"#;

    let without = parse_result(&generate_schema(STUB, tools, None));
    let with = parse_result(&generate_schema(
        STUB,
        tools,
        Some(r#"{"deduplicateEntityTypes": true}"#.to_string()),
    ));

    let schema_without = assert_schema_ok(&without);
    let schema_with = assert_schema_ok(&with);

    // Deduplicated schema should be shorter or equal (fewer entity types).
    assert!(schema_with.len() <= schema_without.len());
}

#[wasm_bindgen_test]
fn test_config_erase_annotations_false() {
    let result = parse_result(&generate_schema(
        STUB,
        SINGLE_TOOL,
        Some(r#"{"eraseAnnotations": false}"#.to_string()),
    ));
    let schema = assert_schema_ok(&result);
    assert!(
        schema.contains("mcp_principal"),
        "Annotations should be preserved"
    );
}

#[wasm_bindgen_test]
fn test_config_erase_annotations_true_is_default() {
    let result = parse_result(&generate_schema(STUB, SINGLE_TOOL, None));
    let schema = assert_schema_ok(&result);
    assert!(
        !schema.contains("mcp_principal"),
        "Annotations should be erased by default"
    );
}

#[wasm_bindgen_test]
fn test_empty_tools_produces_minimal_schema() {
    let result = parse_result(&generate_schema(STUB, "[]", None));
    let schema = assert_schema_ok(&result);
    assert!(schema.contains("TestServer"));
    assert!(schema.contains("call_tool"));
}

#[wasm_bindgen_test]
fn test_optional_config_defaults() {
    let r1 = parse_result(&generate_schema(STUB, SINGLE_TOOL, None));
    let r2 = parse_result(&generate_schema(
        STUB,
        SINGLE_TOOL,
        Some("{}".to_string()),
    ));

    assert_schema_ok(&r1);
    assert_schema_ok(&r2);
    assert_eq!(r1["schema"], r2["schema"]);
}

// ─── Schema generation error tests ─────────────────────────────────────────

#[wasm_bindgen_test]
fn test_invalid_schema_stub_returns_error() {
    let result = parse_result(&generate_schema("not valid cedar schema", "[]", None));
    let error = assert_schema_err(&result);
    assert!(error.contains("Schema error"));
}

#[wasm_bindgen_test]
fn test_invalid_tools_json_returns_error() {
    let result = parse_result(&generate_schema(STUB, "not valid json", None));
    let error = assert_schema_err(&result);
    assert!(error.contains("Invalid tool descriptions"));
}

#[wasm_bindgen_test]
fn test_invalid_config_returns_error() {
    let result = parse_result(&generate_schema(
        STUB,
        SINGLE_TOOL,
        Some("not valid json".to_string()),
    ));
    let error = assert_schema_err(&result);
    assert!(error.contains("Invalid config"));
}

#[wasm_bindgen_test]
fn test_unknown_config_field_returns_error() {
    let result = parse_result(&generate_schema(
        STUB,
        SINGLE_TOOL,
        Some(r#"{"typoField": true}"#.to_string()),
    ));
    let error = assert_schema_err(&result);
    assert!(error.contains("Invalid config"));
}

// ─── Request generation tests ───────────────────────────────────────────────

#[wasm_bindgen_test]
fn test_generate_request_basic() {
    let input = tool_input("read_file", r#"{"path": "/tmp/test.txt"}"#);
    let result = parse_result(&generate_request(
        STUB, SINGLE_TOOL, &input, "User", "alice", "McpServer", "s1", None,
    ));

    assert_request_ok(&result);
    assert!(result["principal"].as_str().unwrap().contains("User"));
    assert!(result["principal"].as_str().unwrap().contains("alice"));
    assert!(result["action"].as_str().unwrap().contains("read_file"));
    assert!(result["resource"].as_str().unwrap().contains("McpServer"));
    assert!(result["resource"].as_str().unwrap().contains("s1"));

    // entities_json should be a valid JSON array
    let entities: serde_json::Value =
        serde_json::from_str(result["entitiesJson"].as_str().unwrap()).unwrap();
    assert!(entities.is_array());
}

#[wasm_bindgen_test]
fn test_generate_request_multi_tool_selects_correct_action() {
    let tools = r#"[
        {
            "name": "read_file",
            "description": "Read",
            "inputSchema": {
                "type": "object",
                "properties": { "path": { "type": "string" } },
                "required": ["path"]
            }
        },
        {
            "name": "write_file",
            "description": "Write",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "content": { "type": "string" }
                },
                "required": ["path", "content"]
            }
        }
    ]"#;

    let input = tool_input("write_file", r#"{"path": "/out", "content": "hi"}"#);
    let result = parse_result(&generate_request(
        STUB, tools, &input, "User", "bob", "McpServer", "prod", None,
    ));

    assert_request_ok(&result);
    assert!(result["action"].as_str().unwrap().contains("write_file"));
    assert!(!result["action"].as_str().unwrap().contains("read_file"));
}

#[wasm_bindgen_test]
fn test_generate_request_with_nested_object_produces_entities() {
    let tools = r#"[{
        "name": "ingest",
        "description": "Ingest",
        "inputSchema": {
            "type": "object",
            "properties": {
                "metadata": {
                    "type": "object",
                    "properties": {
                        "source": { "type": "string" }
                    },
                    "required": ["source"]
                }
            },
            "required": ["metadata"]
        }
    }]"#;

    let input = tool_input("ingest", r#"{"metadata": {"source": "sensor-1"}}"#);
    let result = parse_result(&generate_request(
        STUB, tools, &input, "User", "alice", "McpServer", "s1", None,
    ));

    assert_request_ok(&result);
    let entities: serde_json::Value =
        serde_json::from_str(result["entitiesJson"].as_str().unwrap()).unwrap();
    let arr = entities.as_array().unwrap();
    assert!(
        !arr.is_empty(),
        "Nested objects should produce entities"
    );
}

#[wasm_bindgen_test]
fn test_generate_request_with_config() {
    let tools = r#"[{
        "name": "calc",
        "description": "Calculate",
        "inputSchema": {
            "type": "object",
            "properties": { "value": { "type": "number" } },
            "required": ["value"]
        }
    }]"#;

    let input = tool_input("calc", r#"{"value": 3.14}"#);
    let result = parse_result(&generate_request(
        STUB, tools, &input, "User", "alice", "McpServer", "s1",
        Some(r#"{"numbersAsDecimal": true}"#.to_string()),
    ));
    assert_request_ok(&result);
}

#[wasm_bindgen_test]
fn test_generate_request_empty_config_uses_defaults() {
    let input = tool_input("read_file", r#"{"path": "/tmp"}"#);
    let result = parse_result(&generate_request(
        STUB, SINGLE_TOOL, &input, "User", "u", "McpServer", "s", Some(String::new()),
    ));
    assert_request_ok(&result);
}

// ─── Request generation error tests ────────────────────────────────────────

#[wasm_bindgen_test]
fn test_generate_request_invalid_input_returns_error() {
    let result = parse_result(&generate_request(
        STUB, SINGLE_TOOL, "not json", "User", "u1", "McpServer", "s1", None,
    ));
    let error = assert_request_err(&result);
    assert!(error.contains("Invalid tool input"));
}

#[wasm_bindgen_test]
fn test_generate_request_invalid_stub_returns_error() {
    let input = tool_input("read_file", r#"{"path": "/tmp"}"#);
    let result = parse_result(&generate_request(
        "bad stub", SINGLE_TOOL, &input, "User", "u", "McpServer", "s", None,
    ));
    let error = assert_request_err(&result);
    assert!(error.contains("Schema error"));
}

#[wasm_bindgen_test]
fn test_generate_request_invalid_tools_returns_error() {
    let input = tool_input("read_file", r#"{"path": "/tmp"}"#);
    let result = parse_result(&generate_request(
        STUB, "bad json", &input, "User", "u", "McpServer", "s", None,
    ));
    let error = assert_request_err(&result);
    assert!(error.contains("Invalid tool descriptions"));
}

#[wasm_bindgen_test]
fn test_generate_request_invalid_config_returns_error() {
    let input = tool_input("read_file", r#"{"path": "/tmp"}"#);
    let result = parse_result(&generate_request(
        STUB, SINGLE_TOOL, &input, "User", "u", "McpServer", "s",
        Some("bad json".to_string()),
    ));
    let error = assert_request_err(&result);
    assert!(error.contains("Invalid config"));
}

#[wasm_bindgen_test]
fn test_generate_request_unknown_config_field_returns_error() {
    let input = tool_input("read_file", r#"{"path": "/tmp"}"#);
    let result = parse_result(&generate_request(
        STUB, SINGLE_TOOL, &input, "User", "u", "McpServer", "s",
        Some(r#"{"badField": true}"#.to_string()),
    ));
    let error = assert_request_err(&result);
    assert!(error.contains("Invalid config"));
}

// ─── Consistency tests ──────────────────────────────────────────────────────

#[wasm_bindgen_test]
fn test_schema_and_request_use_same_namespace() {
    let input = tool_input("read_file", r#"{"path": "/tmp"}"#);

    let schema_result = parse_result(&generate_schema(STUB, SINGLE_TOOL, None));
    let schema = assert_schema_ok(&schema_result);

    let req_result = parse_result(&generate_request(
        STUB, SINGLE_TOOL, &input, "User", "alice", "McpServer", "s1", None,
    ));
    assert_request_ok(&req_result);

    // Both should reference the same namespace
    assert!(schema.contains("TestServer"));
    assert!(req_result["principal"]
        .as_str()
        .unwrap()
        .contains("TestServer"));
    assert!(req_result["action"]
        .as_str()
        .unwrap()
        .contains("TestServer"));
    assert!(req_result["resource"]
        .as_str()
        .unwrap()
        .contains("TestServer"));
}

#[wasm_bindgen_test]
fn test_schema_json_is_parseable_and_contains_actions() {
    let result = parse_result(&generate_schema(STUB, SINGLE_TOOL, None));
    assert_schema_ok(&result);

    let schema_json: serde_json::Value =
        serde_json::from_str(result["schemaJson"].as_str().unwrap()).unwrap();

    // The JSON schema should be an object with namespace keys
    assert!(schema_json.is_object());
    // Should contain at least one namespace with actions
    let ns = schema_json.as_object().unwrap();
    assert!(!ns.is_empty(), "schemaJson should have namespace entries");
}
