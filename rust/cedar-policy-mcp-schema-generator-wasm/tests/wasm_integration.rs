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
//! These tests exercise the `generateSchema` function through wasm-bindgen,
//! verifying correct behavior at the JS/WASM boundary.

use cedar_policy_mcp_schema_generator_wasm::generate_schema;
use wasm_bindgen_test::*;

/// Helper: parse the JSON result and return the deserialized fields.
fn parse_result(json: &str) -> serde_json::Value {
    serde_json::from_str(json).expect("Result should be valid JSON")
}

/// Shared schema stub for tests.
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

#[wasm_bindgen_test]
fn test_basic_schema_generation() {
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

    let result_json = generate_schema(STUB, tools, None);
    let result = parse_result(&result_json);

    assert_eq!(result["isOk"], true);
    assert!(result["error"].is_null());

    let schema = result["schema"].as_str().unwrap();
    assert!(
        schema.contains("read_file"),
        "Schema should contain read_file action"
    );
    assert!(
        schema.contains("read_fileInput"),
        "Schema should contain input type"
    );
    assert!(
        schema.contains("String"),
        "Schema should contain String type for path"
    );

    // schemaJson should also be present and valid JSON
    let schema_json_str = result["schemaJson"].as_str().unwrap();
    let _schema_json: serde_json::Value =
        serde_json::from_str(schema_json_str).expect("schemaJson should be valid JSON");
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
                "properties": {
                    "path": { "type": "string" }
                },
                "required": ["path"]
            }
        }
    ]"#;

    let result_json = generate_schema(STUB, tools, None);
    let result = parse_result(&result_json);

    assert_eq!(result["isOk"], true);
    let schema = result["schema"].as_str().unwrap();
    assert!(
        schema.contains("execute_command"),
        "Should contain execute_command"
    );
    assert!(schema.contains("read_file"), "Should contain read_file");
    assert!(
        schema.contains("Long"),
        "Integer should map to Long by default"
    );
}

#[wasm_bindgen_test]
fn test_config_numbers_as_decimal() {
    let tools = r#"[
        {
            "name": "calculate",
            "description": "Calculate something",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "value": { "type": "number" }
                },
                "required": ["value"]
            }
        }
    ]"#;

    let config = r#"{"numbersAsDecimal": true}"#;
    let result_json = generate_schema(STUB, tools, Some(config.to_string()));
    let result = parse_result(&result_json);

    assert_eq!(result["isOk"], true);
    let schema = result["schema"].as_str().unwrap();
    assert!(
        schema.contains("Decimal"),
        "With numbersAsDecimal, number types should map to Decimal"
    );
}

#[wasm_bindgen_test]
fn test_invalid_schema_stub_returns_error() {
    let result_json = generate_schema("this is not valid cedar schema", "[]", None);
    let result = parse_result(&result_json);

    assert_eq!(result["isOk"], false);
    assert!(!result["error"].is_null(), "Should have an error message");
    assert!(result["schema"].is_null(), "Schema should be null on error");
}

#[wasm_bindgen_test]
fn test_invalid_tools_json_returns_error() {
    let result_json = generate_schema(STUB, "not valid json", None);
    let result = parse_result(&result_json);

    assert_eq!(result["isOk"], false);
    assert!(!result["error"].is_null());
}

#[wasm_bindgen_test]
fn test_invalid_config_returns_error() {
    let tools =
        r#"[{"name":"t","description":"d","inputSchema":{"type":"object","properties":{}}}]"#;
    let result_json = generate_schema(STUB, tools, Some("not valid json".to_string()));
    let result = parse_result(&result_json);

    assert_eq!(result["isOk"], false);
    assert!(result["error"].as_str().unwrap().contains("Invalid config"),);
}

#[wasm_bindgen_test]
fn test_empty_tools_produces_minimal_schema() {
    let result_json = generate_schema(STUB, "[]", None);
    let result = parse_result(&result_json);

    assert_eq!(result["isOk"], true);
    let schema = result["schema"].as_str().unwrap();
    assert!(
        schema.contains("TestServer"),
        "Should contain the namespace"
    );
    assert!(
        schema.contains("call_tool"),
        "Should contain the base action"
    );
}

#[wasm_bindgen_test]
fn test_optional_config_defaults() {
    // Passing None for config should use defaults (same as empty config)
    let tools = r#"[{"name":"t","description":"d","inputSchema":{"type":"object","properties":{"x":{"type":"string"}}}}]"#;

    let result_none = generate_schema(STUB, tools, None);
    let result_empty = generate_schema(STUB, tools, Some("{}".to_string()));

    let r1 = parse_result(&result_none);
    let r2 = parse_result(&result_empty);

    assert_eq!(r1["isOk"], true);
    assert_eq!(r2["isOk"], true);
    // Both should produce the same schema
    assert_eq!(r1["schema"], r2["schema"]);
}
