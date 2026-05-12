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

mod lib {
    use std::io::Read;

    use cedar_policy_mcp_schema_generator::{SchemaGenerator, SchemaGeneratorConfig};

    use cedar_policy_core::extensions::Extensions;
    use cedar_policy_core::validator::json_schema::Fragment;

    use mcp_tools_sdk::description::ServerDescription;

    fn run_integration_test(tools_fname: &str, schema_fname: &str, config: SchemaGeneratorConfig) {
        let description =
            ServerDescription::from_json_file(tools_fname).expect("Failed to read tools file");
        let stub_file =
            std::fs::File::open("examples/stub.cedarschema").expect("Failed to read schema file");
        let input_schema = Fragment::from_cedarschema_file(stub_file, Extensions::all_available())
            .expect("Failed to parse input schema")
            .0;

        let mut generator = SchemaGenerator::new_with_config(input_schema, config)
            .expect("input schema file is malformed");
        generator
            .add_actions_from_server_description(&description)
            .expect("Failed to add tool actions to schema generator");

        // Read expected schema file
        let mut schema_file =
            std::fs::File::open(schema_fname).expect("Failed to read expected output file");
        let mut expected_schema = String::new();
        let _ = schema_file
            .read_to_string(&mut expected_schema)
            .expect("Failed to read expected schema file");

        let actual_schema = generator
            .get_schema()
            .clone()
            .to_cedarschema()
            .expect("Failed to resolve generated schema");
        assert!(
            expected_schema == actual_schema,
            "{} != {}",
            expected_schema,
            actual_schema
        );
    }

    #[test]
    fn strands_agent() {
        run_integration_test(
            "examples/strands/strands_tools.json",
            "examples/strands/strands_tools.cedarschema",
            SchemaGeneratorConfig::default(),
        );
    }

    #[test]
    fn strands_agent_flat() {
        run_integration_test(
            "examples/strands/strands_tools.json",
            "examples/strands/strands_tools_flat.cedarschema",
            SchemaGeneratorConfig::default().flatten_namespaces(true),
        );
    }

    #[test]
    fn tuple_tool() {
        run_integration_test(
            "examples/simple/tool_tuple.json",
            "examples/simple/tool_tuple.cedarschema",
            SchemaGeneratorConfig::default(),
        );
    }

    #[test]
    fn mixed_array_tool() {
        // This test has prefixItems with a different type than items results in Set<Unknown>.
        // You will not be able to write meanigful policies against those types.
        // See https://github.com/cedar-policy/cedar-for-agents/issues/90 for an improvement.
        run_integration_test(
            "examples/simple/tool_mixed_array.json",
            "examples/simple/tool_mixed_array.cedarschema",
            SchemaGeneratorConfig::default(),
        );
    }

    #[test]
    fn dedup_entity_types() {
        run_integration_test(
            "examples/dedup/dedup_tools.json",
            "examples/dedup/dedup_tools.cedarschema",
            SchemaGeneratorConfig::default().deduplicate_entity_types(true),
        );
    }

    #[test]
    fn dedup_entity_types_flat() {
        run_integration_test(
            "examples/dedup/dedup_tools.json",
            "examples/dedup/dedup_tools_flat.cedarschema",
            SchemaGeneratorConfig::default()
                .deduplicate_entity_types(true)
                .flatten_namespaces(true),
        );
    }

    #[test]
    fn dedup_same_name_different_variants() {
        // Two tools have an enum with the same name ("mode") but different variants.
        // They should NOT be deduplicated — each stays in its own Input namespace.
        run_integration_test(
            "examples/dedup/dedup_same_name_different_variants.json",
            "examples/dedup/dedup_same_name_different_variants.cedarschema",
            SchemaGeneratorConfig::default().deduplicate_entity_types(true),
        );
    }

    #[test]
    fn dedup_same_name_different_variants_flat() {
        run_integration_test(
            "examples/dedup/dedup_same_name_different_variants.json",
            "examples/dedup/dedup_same_name_different_variants_flat.cedarschema",
            SchemaGeneratorConfig::default()
                .deduplicate_entity_types(true)
                .flatten_namespaces(true),
        );
    }

    #[test]
    fn dedup_three_way_lca() {
        // Single tool with the same enum ("priority") at three different nested object depths:
        //   MyMcpServer::tool_x::Input::B::C::priority
        //   MyMcpServer::tool_x::Input::B::D::priority
        //   MyMcpServer::tool_x::Input::E::F::priority
        // The enum should be deduplicated to the LCA namespace (MyMcpServer::tool_x::Input).
        run_integration_test(
            "examples/dedup/dedup_three_way_lca.json",
            "examples/dedup/dedup_three_way_lca.cedarschema",
            SchemaGeneratorConfig::default().deduplicate_entity_types(true),
        );
    }

    #[test]
    fn dedup_three_way_lca_flat() {
        run_integration_test(
            "examples/dedup/dedup_three_way_lca.json",
            "examples/dedup/dedup_three_way_lca_flat.cedarschema",
            SchemaGeneratorConfig::default()
                .deduplicate_entity_types(true)
                .flatten_namespaces(true),
        );
    }

    #[test]
    fn dedup_collision_existing_entity_in_lca() {
        // Two tools share an enum named "McpServer" (same name as existing entity type
        // in the LCA namespace). Dedup should skip this enum — each tool keeps its own copy.
        let description = ServerDescription::from_json_file(
            "examples/dedup/dedup_collision_existing_entity.json",
        )
        .expect("Failed to read tools file");
        let stub_file =
            std::fs::File::open("examples/stub.cedarschema").expect("Failed to read schema file");
        let input_schema = Fragment::from_cedarschema_file(stub_file, Extensions::all_available())
            .expect("Failed to parse input schema")
            .0;

        let config = SchemaGeneratorConfig::default().deduplicate_entity_types(true);
        let mut generator =
            SchemaGenerator::new_with_config(input_schema, config).expect("schema is malformed");
        generator
            .add_actions_from_server_description(&description)
            .expect("Collision should be skipped, not produce an error");

        let schema = generator.get_schema();
        // The enum should remain local in each tool's Input namespace
        let tool_a_input_ns = Some("MyMcpServer::tool_a::Input".parse().unwrap());
        let tool_b_input_ns = Some("MyMcpServer::tool_b::Input".parse().unwrap());

        let tool_a_nsdef = schema
            .0
            .get(&tool_a_input_ns)
            .expect("Expected tool_a::Input namespace to exist");
        let tool_b_nsdef = schema
            .0
            .get(&tool_b_input_ns)
            .expect("Expected tool_b::Input namespace to exist");

        assert!(
            tool_a_nsdef
                .entity_types
                .contains_key(&"McpServer".parse().unwrap()),
            "McpServer enum should stay local in tool_a::Input"
        );
        assert!(
            tool_b_nsdef
                .entity_types
                .contains_key(&"McpServer".parse().unwrap()),
            "McpServer enum should stay local in tool_b::Input"
        );
    }

    #[test]
    fn dedup_collision_same_enum_already_in_lca() {
        // The LCA namespace already has an enum entity type with the same name AND same variants.
        // Dedup should reuse the existing entity — tools reference it instead of creating local copies.
        let stub_schema = r#"
namespace MyMcpServer {
    @mcp_principal("User")
    entity User {
        id: String,
        username: String,
    };

    @mcp_context("session")
    type CommonContext = {
        currentTimestamp: datetime,
        ipaddr: ipaddr,
    };

    @mcp_resource("McpServer")
    entity McpServer;

    @mcp_action("call_tool")
    action call_tool;

    // Pre-existing enum entity type with same variants as tools
    entity status enum ["active", "inactive"];
}
"#;

        let extensions = Extensions::all_available();
        let (input_schema, _) = Fragment::from_cedarschema_str(stub_schema, extensions)
            .expect("Failed to parse custom stub schema");

        let tools_json = r#"{
            "result": {
                "tools": [
                    {
                        "name": "tool_a",
                        "description": "Tool A with status enum",
                        "inputSchema": {
                            "json": {
                                "type": "object",
                                "properties": {
                                    "status": {
                                        "type": "string",
                                        "enum": ["active", "inactive"],
                                        "description": "Status"
                                    },
                                    "query": {
                                        "type": "string",
                                        "description": "Query"
                                    }
                                },
                                "required": ["query"]
                            }
                        }
                    },
                    {
                        "name": "tool_b",
                        "description": "Tool B with same status enum",
                        "inputSchema": {
                            "json": {
                                "type": "object",
                                "properties": {
                                    "status": {
                                        "type": "string",
                                        "enum": ["active", "inactive"],
                                        "description": "Status"
                                    },
                                    "data": {
                                        "type": "string",
                                        "description": "Data"
                                    }
                                },
                                "required": ["data"]
                            }
                        }
                    }
                ]
            }
        }"#;

        let description =
            ServerDescription::from_json_str(tools_json).expect("Failed to parse tools JSON");

        let config = SchemaGeneratorConfig::default().deduplicate_entity_types(true);
        let mut generator =
            SchemaGenerator::new_with_config(input_schema, config).expect("schema is malformed");
        generator
            .add_actions_from_server_description(&description)
            .expect("Should reuse existing enum, not produce an error");

        let schema = generator.get_schema();

        // The pre-existing status enum in the root namespace should still be there
        let root_ns = Some("MyMcpServer".parse().unwrap());
        let root_nsdef = schema.0.get(&root_ns).expect("Root namespace should exist");
        assert!(
            root_nsdef
                .entity_types
                .contains_key(&"status".parse().unwrap()),
            "Pre-existing status enum should remain in root namespace"
        );

        // Tools should NOT have local copies — they reference the existing one
        let tool_a_input_ns = Some("MyMcpServer::tool_a::Input".parse().unwrap());
        let tool_b_input_ns = Some("MyMcpServer::tool_b::Input".parse().unwrap());
        assert!(
            schema.0.get(&tool_a_input_ns).is_none()
                || !schema
                    .0
                    .get(&tool_a_input_ns)
                    .unwrap()
                    .entity_types
                    .contains_key(&"status".parse().unwrap()),
            "status should NOT be duplicated in tool_a::Input"
        );
        assert!(
            schema.0.get(&tool_b_input_ns).is_none()
                || !schema
                    .0
                    .get(&tool_b_input_ns)
                    .unwrap()
                    .entity_types
                    .contains_key(&"status".parse().unwrap()),
            "status should NOT be duplicated in tool_b::Input"
        );
    }

    #[test]
    fn dedup_collision_different_enum_already_in_lca() {
        // The LCA namespace already has an enum entity type with the same name but DIFFERENT variants.
        // Dedup should skip — each tool keeps its own local copy.
        let stub_schema = r#"
namespace MyMcpServer {
    @mcp_principal("User")
    entity User {
        id: String,
        username: String,
    };

    @mcp_context("session")
    type CommonContext = {
        currentTimestamp: datetime,
        ipaddr: ipaddr,
    };

    @mcp_resource("McpServer")
    entity McpServer;

    @mcp_action("call_tool")
    action call_tool;

    // Pre-existing enum with DIFFERENT variants than tools
    entity status enum ["open", "closed"];
}
"#;

        let extensions = Extensions::all_available();
        let (input_schema, _) = Fragment::from_cedarschema_str(stub_schema, extensions)
            .expect("Failed to parse custom stub schema");

        let tools_json = r#"{
            "result": {
                "tools": [
                    {
                        "name": "tool_a",
                        "description": "Tool A with status enum",
                        "inputSchema": {
                            "json": {
                                "type": "object",
                                "properties": {
                                    "status": {
                                        "type": "string",
                                        "enum": ["active", "inactive"],
                                        "description": "Status"
                                    },
                                    "query": {
                                        "type": "string",
                                        "description": "Query"
                                    }
                                },
                                "required": ["query"]
                            }
                        }
                    },
                    {
                        "name": "tool_b",
                        "description": "Tool B with same status enum",
                        "inputSchema": {
                            "json": {
                                "type": "object",
                                "properties": {
                                    "status": {
                                        "type": "string",
                                        "enum": ["active", "inactive"],
                                        "description": "Status"
                                    },
                                    "data": {
                                        "type": "string",
                                        "description": "Data"
                                    }
                                },
                                "required": ["data"]
                            }
                        }
                    }
                ]
            }
        }"#;

        let description =
            ServerDescription::from_json_str(tools_json).expect("Failed to parse tools JSON");

        let config = SchemaGeneratorConfig::default().deduplicate_entity_types(true);
        let mut generator =
            SchemaGenerator::new_with_config(input_schema, config).expect("schema is malformed");
        generator
            .add_actions_from_server_description(&description)
            .expect("Collision should be skipped, not produce an error");

        let schema = generator.get_schema();

        // The pre-existing status enum (different variants) remains unchanged
        let root_ns = Some("MyMcpServer".parse().unwrap());
        let root_nsdef = schema.0.get(&root_ns).expect("Root namespace should exist");
        assert!(
            root_nsdef
                .entity_types
                .contains_key(&"status".parse().unwrap()),
            "Pre-existing status enum should remain in root namespace"
        );

        // Each tool should have its own local status enum since dedup was skipped
        let tool_a_input_ns = Some("MyMcpServer::tool_a::Input".parse().unwrap());
        let tool_b_input_ns = Some("MyMcpServer::tool_b::Input".parse().unwrap());

        let tool_a_nsdef = schema
            .0
            .get(&tool_a_input_ns)
            .expect("Expected tool_a::Input namespace to exist");
        let tool_b_nsdef = schema
            .0
            .get(&tool_b_input_ns)
            .expect("Expected tool_b::Input namespace to exist");

        assert!(
            tool_a_nsdef
                .entity_types
                .contains_key(&"status".parse().unwrap()),
            "status enum should stay local in tool_a::Input"
        );
        assert!(
            tool_b_nsdef
                .entity_types
                .contains_key(&"status".parse().unwrap()),
            "status enum should stay local in tool_b::Input"
        );
    }

    #[test]
    fn dedup_skip_both_when_same_name_different_variants_both_duplicated() {
        // Two pairs of tools define `mode` with different variants:
        //   tool_a, tool_b: mode ["fast", "slow"]
        //   tool_c, tool_d: mode ["sync", "async"]
        // Both fingerprints have >1 occurrence, both compute LCA = MyMcpServer.
        // Since they'd collide at the same LCA, BOTH should be skipped.
        let tools_json = r#"{
            "result": {
                "tools": [
                    {
                        "name": "tool_a",
                        "description": "Tool A",
                        "inputSchema": { "json": { "type": "object", "properties": { "mode": { "type": "string", "enum": ["fast", "slow"] } }, "required": ["mode"] } }
                    },
                    {
                        "name": "tool_b",
                        "description": "Tool B",
                        "inputSchema": { "json": { "type": "object", "properties": { "mode": { "type": "string", "enum": ["fast", "slow"] } }, "required": ["mode"] } }
                    },
                    {
                        "name": "tool_c",
                        "description": "Tool C",
                        "inputSchema": { "json": { "type": "object", "properties": { "mode": { "type": "string", "enum": ["sync", "async"] } }, "required": ["mode"] } }
                    },
                    {
                        "name": "tool_d",
                        "description": "Tool D",
                        "inputSchema": { "json": { "type": "object", "properties": { "mode": { "type": "string", "enum": ["sync", "async"] } }, "required": ["mode"] } }
                    }
                ]
            }
        }"#;

        let description =
            ServerDescription::from_json_str(tools_json).expect("Failed to parse tools JSON");
        let stub_file =
            std::fs::File::open("examples/stub.cedarschema").expect("Failed to read schema file");
        let input_schema = Fragment::from_cedarschema_file(stub_file, Extensions::all_available())
            .expect("Failed to parse input schema")
            .0;

        let config = SchemaGeneratorConfig::default().deduplicate_entity_types(true);
        let mut generator =
            SchemaGenerator::new_with_config(input_schema, config).expect("schema is malformed");
        generator
            .add_actions_from_server_description(&description)
            .expect("Should succeed — conflicting dedup entries are skipped");

        let schema = generator.get_schema();

        // Neither enum should be deduplicated — each stays in its tool's Input namespace
        let root_ns = Some("MyMcpServer".parse().unwrap());
        let root_nsdef = schema.0.get(&root_ns).expect("Root namespace should exist");
        assert!(
            !root_nsdef
                .entity_types
                .contains_key(&"mode".parse().unwrap()),
            "mode should NOT be placed in the root namespace"
        );

        // All four tools should have their own local `mode` entity type
        for tool in &["tool_a", "tool_b", "tool_c", "tool_d"] {
            let input_ns = Some(format!("MyMcpServer::{}::Input", tool).parse().unwrap());
            let nsdef = schema
                .0
                .get(&input_ns)
                .unwrap_or_else(|| panic!("Expected {}::Input namespace to exist", tool));
            assert!(
                nsdef.entity_types.contains_key(&"mode".parse().unwrap()),
                "mode should stay local in {}::Input",
                tool
            );
        }
    }

    #[test]
    fn dedup_enum_inside_array() {
        // Two tools have the same enum nested inside an array property.
        // The enum should still be deduplicated.
        let tools_json = r#"{
            "result": {
                "tools": [
                    {
                        "name": "tool_a",
                        "description": "Tool A",
                        "inputSchema": {
                            "json": {
                                "type": "object",
                                "properties": {
                                    "priorities": {
                                        "type": "array",
                                        "items": {
                                            "type": "string",
                                            "enum": ["high", "medium", "low"]
                                        }
                                    }
                                },
                                "required": ["priorities"]
                            }
                        }
                    },
                    {
                        "name": "tool_b",
                        "description": "Tool B",
                        "inputSchema": {
                            "json": {
                                "type": "object",
                                "properties": {
                                    "priorities": {
                                        "type": "array",
                                        "items": {
                                            "type": "string",
                                            "enum": ["high", "medium", "low"]
                                        }
                                    }
                                },
                                "required": ["priorities"]
                            }
                        }
                    }
                ]
            }
        }"#;

        let description =
            ServerDescription::from_json_str(tools_json).expect("Failed to parse tools JSON");
        let stub_file =
            std::fs::File::open("examples/stub.cedarschema").expect("Failed to read schema file");
        let input_schema = Fragment::from_cedarschema_file(stub_file, Extensions::all_available())
            .expect("Failed to parse input schema")
            .0;

        let config = SchemaGeneratorConfig::default().deduplicate_entity_types(true);
        let mut generator =
            SchemaGenerator::new_with_config(input_schema, config).expect("schema is malformed");
        generator
            .add_actions_from_server_description(&description)
            .expect("Should succeed with array-nested enum dedup");

        let schema = generator.get_schema();

        // The enum should be deduplicated to the LCA namespace (MyMcpServer)
        let root_ns = Some("MyMcpServer".parse().unwrap());
        let root_nsdef = schema.0.get(&root_ns).expect("Root namespace should exist");
        assert!(
            root_nsdef
                .entity_types
                .contains_key(&"priorities".parse().unwrap()),
            "priorities enum should be deduplicated to the root namespace"
        );

        // Neither tool's Input namespace should have a local copy
        let tool_a_input_ns = Some("MyMcpServer::tool_a::Input".parse().unwrap());
        let tool_b_input_ns = Some("MyMcpServer::tool_b::Input".parse().unwrap());
        assert!(
            schema.0.get(&tool_a_input_ns).is_none()
                || !schema
                    .0
                    .get(&tool_a_input_ns)
                    .unwrap()
                    .entity_types
                    .contains_key(&"priorities".parse().unwrap()),
            "priorities should NOT be in tool_a::Input"
        );
        assert!(
            schema.0.get(&tool_b_input_ns).is_none()
                || !schema
                    .0
                    .get(&tool_b_input_ns)
                    .unwrap()
                    .entity_types
                    .contains_key(&"priorities".parse().unwrap()),
            "priorities should NOT be in tool_b::Input"
        );
    }

    #[test]
    fn dedup_enum_across_outputs() {
        // Two tools share an enum in their outputSchema.
        // With include_outputs + deduplicate_entity_types, the enum should be
        // deduplicated to the LCA namespace.
        let tools_json = r#"{
            "result": {
                "tools": [
                    {
                        "name": "tool_a",
                        "description": "Tool A",
                        "inputSchema": {
                            "json": {
                                "type": "object",
                                "properties": {
                                    "query": { "type": "string" }
                                },
                                "required": ["query"]
                            }
                        },
                        "outputSchema": {
                            "json": {
                                "type": "object",
                                "properties": {
                                    "status": {
                                        "type": "string",
                                        "enum": ["success", "failure", "pending"]
                                    }
                                },
                                "required": ["status"]
                            }
                        }
                    },
                    {
                        "name": "tool_b",
                        "description": "Tool B",
                        "inputSchema": {
                            "json": {
                                "type": "object",
                                "properties": {
                                    "id": { "type": "string" }
                                },
                                "required": ["id"]
                            }
                        },
                        "outputSchema": {
                            "json": {
                                "type": "object",
                                "properties": {
                                    "status": {
                                        "type": "string",
                                        "enum": ["success", "failure", "pending"]
                                    }
                                },
                                "required": ["status"]
                            }
                        }
                    }
                ]
            }
        }"#;

        let description =
            ServerDescription::from_json_str(tools_json).expect("Failed to parse tools JSON");
        let stub_file =
            std::fs::File::open("examples/stub.cedarschema").expect("Failed to read schema file");
        let input_schema = Fragment::from_cedarschema_file(stub_file, Extensions::all_available())
            .expect("Failed to parse input schema")
            .0;

        let config = SchemaGeneratorConfig::default()
            .include_outputs(true)
            .deduplicate_entity_types(true);
        let mut generator =
            SchemaGenerator::new_with_config(input_schema, config).expect("schema is malformed");
        generator
            .add_actions_from_server_description(&description)
            .expect("Should succeed with output enum dedup");

        let schema = generator.get_schema();

        // The status enum should be deduplicated to the LCA of the two Output namespaces.
        // tool_a::Output and tool_b::Output -> LCA is MyMcpServer
        let root_ns = Some("MyMcpServer".parse().unwrap());
        let root_nsdef = schema.0.get(&root_ns).expect("Root namespace should exist");
        assert!(
            root_nsdef
                .entity_types
                .contains_key(&"status".parse().unwrap()),
            "status enum should be deduplicated to the root namespace"
        );

        // Neither tool's Output namespace should have a local copy
        let tool_a_output_ns = Some("MyMcpServer::tool_a::Output".parse().unwrap());
        let tool_b_output_ns = Some("MyMcpServer::tool_b::Output".parse().unwrap());
        assert!(
            schema.0.get(&tool_a_output_ns).is_none()
                || !schema
                    .0
                    .get(&tool_a_output_ns)
                    .unwrap()
                    .entity_types
                    .contains_key(&"status".parse().unwrap()),
            "status should NOT be in tool_a::Output"
        );
        assert!(
            schema.0.get(&tool_b_output_ns).is_none()
                || !schema
                    .0
                    .get(&tool_b_output_ns)
                    .unwrap()
                    .entity_types
                    .contains_key(&"status".parse().unwrap()),
            "status should NOT be in tool_b::Output"
        );
    }

    #[test]
    fn dedup_enum_across_input_and_output() {
        // One tool has an enum in inputSchema, the other has the same enum in outputSchema.
        // They should still be deduplicated to the LCA.
        let tools_json = r#"{
            "result": {
                "tools": [
                    {
                        "name": "tool_a",
                        "description": "Tool A with enum in input",
                        "inputSchema": {
                            "json": {
                                "type": "object",
                                "properties": {
                                    "priority": {
                                        "type": "string",
                                        "enum": ["high", "medium", "low"]
                                    }
                                },
                                "required": ["priority"]
                            }
                        }
                    },
                    {
                        "name": "tool_b",
                        "description": "Tool B with same enum in output",
                        "inputSchema": {
                            "json": {
                                "type": "object",
                                "properties": {
                                    "id": { "type": "string" }
                                },
                                "required": ["id"]
                            }
                        },
                        "outputSchema": {
                            "json": {
                                "type": "object",
                                "properties": {
                                    "priority": {
                                        "type": "string",
                                        "enum": ["high", "medium", "low"]
                                    }
                                },
                                "required": ["priority"]
                            }
                        }
                    }
                ]
            }
        }"#;

        let description =
            ServerDescription::from_json_str(tools_json).expect("Failed to parse tools JSON");
        let stub_file =
            std::fs::File::open("examples/stub.cedarschema").expect("Failed to read schema file");
        let input_schema = Fragment::from_cedarschema_file(stub_file, Extensions::all_available())
            .expect("Failed to parse input schema")
            .0;

        let config = SchemaGeneratorConfig::default()
            .include_outputs(true)
            .deduplicate_entity_types(true);
        let mut generator =
            SchemaGenerator::new_with_config(input_schema, config).expect("schema is malformed");
        generator
            .add_actions_from_server_description(&description)
            .expect("Should succeed with cross input/output enum dedup");

        let schema = generator.get_schema();

        // LCA of tool_a::Input and tool_b::Output is MyMcpServer
        let root_ns = Some("MyMcpServer".parse().unwrap());
        let root_nsdef = schema.0.get(&root_ns).expect("Root namespace should exist");
        assert!(
            root_nsdef
                .entity_types
                .contains_key(&"priority".parse().unwrap()),
            "priority enum should be deduplicated to the root namespace"
        );

        // Neither source namespace should have a local copy
        let tool_a_input_ns = Some("MyMcpServer::tool_a::Input".parse().unwrap());
        let tool_b_output_ns = Some("MyMcpServer::tool_b::Output".parse().unwrap());
        assert!(
            schema.0.get(&tool_a_input_ns).is_none()
                || !schema
                    .0
                    .get(&tool_a_input_ns)
                    .unwrap()
                    .entity_types
                    .contains_key(&"priority".parse().unwrap()),
            "priority should NOT be in tool_a::Input"
        );
        assert!(
            schema.0.get(&tool_b_output_ns).is_none()
                || !schema
                    .0
                    .get(&tool_b_output_ns)
                    .unwrap()
                    .entity_types
                    .contains_key(&"priority".parse().unwrap()),
            "priority should NOT be in tool_b::Output"
        );
    }

    #[test]
    fn dedup_output_not_scanned_without_include_outputs() {
        // When include_outputs is false, output enums should NOT participate in dedup.
        // tool_a has "status" in input, tool_b has same "status" in output only.
        // Without include_outputs, the output enum is never generated, so no dedup happens.
        let tools_json = r#"{
            "result": {
                "tools": [
                    {
                        "name": "tool_a",
                        "description": "Tool A with enum in input",
                        "inputSchema": {
                            "json": {
                                "type": "object",
                                "properties": {
                                    "status": {
                                        "type": "string",
                                        "enum": ["success", "failure"]
                                    }
                                },
                                "required": ["status"]
                            }
                        }
                    },
                    {
                        "name": "tool_b",
                        "description": "Tool B with same enum in output only",
                        "inputSchema": {
                            "json": {
                                "type": "object",
                                "properties": {
                                    "id": { "type": "string" }
                                },
                                "required": ["id"]
                            }
                        },
                        "outputSchema": {
                            "json": {
                                "type": "object",
                                "properties": {
                                    "status": {
                                        "type": "string",
                                        "enum": ["success", "failure"]
                                    }
                                },
                                "required": ["status"]
                            }
                        }
                    }
                ]
            }
        }"#;

        let description =
            ServerDescription::from_json_str(tools_json).expect("Failed to parse tools JSON");
        let stub_file =
            std::fs::File::open("examples/stub.cedarschema").expect("Failed to read schema file");
        let input_schema = Fragment::from_cedarschema_file(stub_file, Extensions::all_available())
            .expect("Failed to parse input schema")
            .0;

        // deduplicate_entity_types = true, but include_outputs = false (default)
        let config = SchemaGeneratorConfig::default().deduplicate_entity_types(true);
        let mut generator =
            SchemaGenerator::new_with_config(input_schema, config).expect("schema is malformed");
        generator
            .add_actions_from_server_description(&description)
            .expect("Should succeed");

        let schema = generator.get_schema();

        // The enum only appears once (in tool_a::Input), so it should NOT be deduplicated
        let root_ns = Some("MyMcpServer".parse().unwrap());
        let root_nsdef = schema.0.get(&root_ns).expect("Root namespace should exist");
        assert!(
            !root_nsdef
                .entity_types
                .contains_key(&"status".parse().unwrap()),
            "status should NOT be deduplicated to root when include_outputs is false"
        );

        // tool_a should have its own local status enum
        let tool_a_input_ns = Some("MyMcpServer::tool_a::Input".parse().unwrap());
        let tool_a_nsdef = schema
            .0
            .get(&tool_a_input_ns)
            .expect("Expected tool_a::Input namespace to exist");
        assert!(
            tool_a_nsdef
                .entity_types
                .contains_key(&"status".parse().unwrap()),
            "status should remain local in tool_a::Input"
        );
    }
}

#[cfg(feature = "cli")]
mod cli {
    use assert_cmd::{assert::OutputAssertExt, cargo_bin_cmd};
    use tempfile::TempDir;

    #[test]
    fn test_simple_default_cedar_schema() {
        let expected = std::fs::read_to_string("examples/simple/tool_default.cedarschema").unwrap();

        let mut cmd = cargo_bin_cmd!("cedar-policy-mcp-schema-generator");
        let cmd = cmd
            .arg("generate")
            .arg("examples/stub.cedarschema")
            .arg("examples/simple/tool.json");
        cmd.unwrap().assert().success().stdout(expected);
    }

    #[test]
    fn test_simple_default_json_schema() {
        let expected =
            std::fs::read_to_string("examples/simple/tool_default.cedarschema.json").unwrap();

        let mut cmd = cargo_bin_cmd!("cedar-policy-mcp-schema-generator");
        let cmd = cmd
            .arg("generate")
            .arg("examples/stub.cedarschema")
            .arg("examples/simple/tool.json")
            .arg("--output-format")
            .arg("json");
        cmd.unwrap().assert().success().stdout(expected);
    }

    #[test]
    fn test_simple_keep_annotations_cedar_schema() {
        let expected =
            std::fs::read_to_string("examples/simple/tool_keep_annotations.cedarschema").unwrap();

        let mut cmd = cargo_bin_cmd!("cedar-policy-mcp-schema-generator");
        let cmd = cmd
            .arg("generate")
            .arg("examples/stub.cedarschema")
            .arg("examples/simple/tool.json")
            .arg("--keep-annotations");
        cmd.unwrap().assert().success().stdout(expected);
    }

    #[test]
    fn test_simple_keep_annotations_json_schema() {
        let expected =
            std::fs::read_to_string("examples/simple/tool_keep_annotations.cedarschema.json")
                .unwrap();

        let mut cmd = cargo_bin_cmd!("cedar-policy-mcp-schema-generator");
        let cmd = cmd
            .arg("generate")
            .arg("examples/stub.cedarschema")
            .arg("examples/simple/tool.json")
            .arg("--keep-annotations")
            .arg("--output-format")
            .arg("json");
        cmd.unwrap().assert().success().stdout(expected);
    }

    #[test]
    fn test_simple_object_as_records_cedar_schema() {
        let expected =
            std::fs::read_to_string("examples/simple/tool_objects_as_records.cedarschema").unwrap();

        let mut cmd = cargo_bin_cmd!("cedar-policy-mcp-schema-generator");
        let cmd = cmd
            .arg("generate")
            .arg("examples/stub.cedarschema")
            .arg("examples/simple/tool.json")
            .arg("--objects-as-records");
        cmd.unwrap().assert().success().stdout(expected);
    }

    #[test]
    fn test_simple_object_as_records_json_schema() {
        let expected =
            std::fs::read_to_string("examples/simple/tool_objects_as_records.cedarschema.json")
                .unwrap();

        let mut cmd = cargo_bin_cmd!("cedar-policy-mcp-schema-generator");
        let cmd = cmd
            .arg("generate")
            .arg("examples/stub.cedarschema")
            .arg("examples/simple/tool.json")
            .arg("--objects-as-records")
            .arg("--output-format")
            .arg("json");
        cmd.unwrap().assert().success().stdout(expected);
    }

    #[test]
    fn test_simple_include_outputs_cedar_schema() {
        let expected =
            std::fs::read_to_string("examples/simple/tool_include_outputs.cedarschema").unwrap();

        let mut cmd = cargo_bin_cmd!("cedar-policy-mcp-schema-generator");
        let cmd = cmd
            .arg("generate")
            .arg("examples/stub.cedarschema")
            .arg("examples/simple/tool.json")
            .arg("--include-outputs");
        cmd.unwrap().assert().success().stdout(expected);
    }

    #[test]
    fn test_simple_include_outputs_json_schema() {
        let expected =
            std::fs::read_to_string("examples/simple/tool_include_outputs.cedarschema.json")
                .unwrap();

        let mut cmd = cargo_bin_cmd!("cedar-policy-mcp-schema-generator");
        let cmd = cmd
            .arg("generate")
            .arg("examples/stub.cedarschema")
            .arg("examples/simple/tool.json")
            .arg("--include-outputs")
            .arg("--output-format")
            .arg("json");
        cmd.unwrap().assert().success().stdout(expected);
    }

    #[test]
    fn test_simple_flattened_namespace_cedar_schema() {
        let expected =
            std::fs::read_to_string("examples/simple/tool_flattened_namespace.cedarschema")
                .unwrap();

        let mut cmd = cargo_bin_cmd!("cedar-policy-mcp-schema-generator");
        let cmd = cmd
            .arg("generate")
            .arg("examples/stub.cedarschema")
            .arg("examples/simple/tool.json")
            .arg("--flatten-namespaces")
            .arg("--error-format")
            .arg("plain");
        cmd.unwrap().assert().success().stdout(expected);
    }

    #[test]
    fn test_simple_flattened_namespace_json_schema() {
        let expected =
            std::fs::read_to_string("examples/simple/tool_flattened_namespace.cedarschema.json")
                .unwrap();

        let mut cmd = cargo_bin_cmd!("cedar-policy-mcp-schema-generator");
        let cmd = cmd
            .arg("generate")
            .arg("examples/stub.cedarschema")
            .arg("examples/simple/tool.json")
            .arg("--flatten-namespaces")
            .arg("--output-format")
            .arg("json")
            .arg("--error-format")
            .arg("json");
        cmd.unwrap().assert().success().stdout(expected);
    }

    #[test]
    fn test_simple_encode_numbers_as_decimal_cedar_schema() {
        let expected =
            std::fs::read_to_string("examples/simple/tool_encode_numbers_as_decimal.cedarschema")
                .unwrap();

        let mut cmd = cargo_bin_cmd!("cedar-policy-mcp-schema-generator");
        let cmd = cmd
            .arg("generate")
            .arg("examples/stub.cedarschema")
            .arg("examples/simple/tool.json")
            .arg("--encode-numbers-as-decimal");
        cmd.unwrap().assert().success().stdout(expected);
    }

    #[test]
    fn test_simple_encode_numbers_as_decimal_json_schema() {
        let expected = std::fs::read_to_string(
            "examples/simple/tool_encode_numbers_as_decimal.cedarschema.json",
        )
        .unwrap();

        let mut cmd = cargo_bin_cmd!("cedar-policy-mcp-schema-generator");
        let cmd = cmd
            .arg("generate")
            .arg("examples/stub.cedarschema")
            .arg("examples/simple/tool.json")
            .arg("--encode-numbers-as-decimal")
            .arg("--output-format")
            .arg("json");
        cmd.unwrap().assert().success().stdout(expected);
    }
    #[test]
    fn test_nullable_objects_cedar_schema() {
        let expected =
            std::fs::read_to_string("examples/simple/tool_nullable_objects.cedarschema").unwrap();

        let mut cmd = cargo_bin_cmd!("cedar-policy-mcp-schema-generator");
        let cmd = cmd
            .arg("generate")
            .arg("examples/stub.cedarschema")
            .arg("examples/simple/tool_nullable_objects.json");
        cmd.unwrap().assert().success().stdout(expected);
    }

    #[test]
    fn test_input_schema_does_not_exist_error() {
        let mut cmd = cargo_bin_cmd!("cedar-policy-mcp-schema-generator");
        let cmd = cmd
            .arg("generate")
            .arg("examples/stu.cedarschema")
            .arg("examples/simple/tool.json");
        cmd.assert().failure();
    }

    #[test]
    fn test_input_schema_unrecognized_extension() {
        let mut cmd = cargo_bin_cmd!("cedar-policy-mcp-schema-generator");
        let cmd = cmd
            .arg("generate")
            .arg("examples/stub.jsonschema")
            .arg("examples/simple/tool.json");
        cmd.assert().failure();
    }

    #[test]
    fn test_default_with_json_schema() {
        let schema_file = std::fs::File::open("examples/stub.cedarschema").unwrap();
        let schema = cedar_policy_core::validator::json_schema::Fragment::from_cedarschema_file(
            schema_file,
            cedar_policy_core::extensions::Extensions::all_available(),
        )
        .unwrap()
        .0;
        let temp_dir = TempDir::new().unwrap();
        let input_file = temp_dir.path().join("stub.json");
        std::fs::write(&input_file, serde_json::to_string(&schema).unwrap()).unwrap();

        let expected = std::fs::read_to_string("examples/simple/tool_default.cedarschema").unwrap();

        let mut cmd = cargo_bin_cmd!("cedar-policy-mcp-schema-generator");
        let cmd = cmd
            .arg("generate")
            .arg(input_file)
            .arg("examples/simple/tool.json");
        cmd.unwrap().assert().success().stdout(expected);
    }

    #[test]
    fn test_default_write_to_file() {
        let temp_dir = TempDir::new().unwrap();
        let output_file = temp_dir.path().join("schema.cedarschema");

        let mut cmd = cargo_bin_cmd!("cedar-policy-mcp-schema-generator");
        let cmd = cmd
            .arg("generate")
            .arg("examples/stub.cedarschema")
            .arg("examples/simple/tool.json")
            .arg("--output")
            .arg(&output_file);
        cmd.unwrap().assert().success();

        let expected = std::fs::read_to_string("examples/simple/tool_default.cedarschema").unwrap();
        let actual = std::fs::read_to_string(output_file).unwrap();
        assert_eq!(expected, actual);
    }

    #[test]
    fn test_authorize_simple_default_allow() {
        let temp_dir = TempDir::new().unwrap();
        let entities_fname = temp_dir.path().join("entities.json");
        std::fs::write(&entities_fname, "[]").unwrap();

        let request_json = r#"{
    "principal": "MyMcpServer::User::\"test_user\"",
    "resource": "MyMcpServer::McpServer::\"test_server\"",
    "context": {
        "session": {
            "currentTimestamp": {
                "__extn": {
                    "fn": "datetime",
                    "arg": "2025-12-16"
                }
            },
            "ipaddr": {
                "__extn": {
                    "fn": "ip",
                    "arg": "10.0.0.1"
                }
            }
        }
    }
}"#;
        let request_fname = temp_dir.path().join("request.json");
        std::fs::write(&request_fname, request_json).unwrap();

        let policy_fname = temp_dir.path().join("policies.cedar");
        std::fs::write(&policy_fname, "permit(principal, action, resource);").unwrap();

        let input = r#"{
    "params": {
        "tool": "test_tool",
        "args": {
            "bool_attr": false,
            "int_attr": 0,
            "float_attr": 1.0,
            "str_attr": "howdy",
            "enum_attr": "variant2",
            "dt_attr": "2025-12-16",
            "null_attr": null
        }
    }
}"#;
        let input_fname = temp_dir.path().join("input.json");
        std::fs::write(&input_fname, input).unwrap();

        let mut cmd = cargo_bin_cmd!("cedar-policy-mcp-schema-generator");
        let cmd = cmd
            .arg("authorize")
            .arg("examples/stub.cedarschema")
            .arg("examples/simple/tool.json")
            .arg("--request-json")
            .arg(&request_fname)
            .arg("--policies")
            .arg(&policy_fname)
            .arg("--entities")
            .arg(&entities_fname)
            .arg("--mcp-tool-input")
            .arg(&input_fname);
        cmd.unwrap().assert().success().stdout("ALLOW\n").stderr("");
    }

    #[test]
    fn test_authorize_simple_default_deny() {
        let temp_dir = TempDir::new().unwrap();
        let entities_fname = temp_dir.path().join("entities.json");
        std::fs::write(&entities_fname, "[]").unwrap();

        let principal = "MyMcpServer::User::\"test_user\"";
        let resource = "MyMcpServer::McpServer::\"test_server\"";
        let context_json = r#"{
    "session": {
        "currentTimestamp": {
            "__extn": {
                "fn": "datetime",
                "arg": "2025-12-16"
            }
        },
        "ipaddr": {
            "__extn": {
                "fn": "ip",
                "arg": "10.0.0.1"
            }
        }
    }
}"#;
        let context_fname = temp_dir.path().join("context.json");
        std::fs::write(&context_fname, context_json).unwrap();

        let policy_fname = temp_dir.path().join("policies.cedar");
        std::fs::write(
            &policy_fname,
            r#"permit(principal, action, resource) when {
    (context.input has bool_attr && context.input.bool_attr) ||
    (context.input has int_attr && context.input.int_attr < 0)
};"#,
        )
        .unwrap();

        let input = r#"{
    "params": {
        "tool": "test_tool",
        "args": {
            "bool_attr": false,
            "int_attr": 0,
            "float_attr": 1.0,
            "str_attr": "howdy",
            "enum_attr": "variant2",
            "dt_attr": "2025-12-16",
            "null_attr": null
        }
    }
}"#;
        let input_fname = temp_dir.path().join("input.json");
        std::fs::write(&input_fname, input).unwrap();

        let mut cmd = cargo_bin_cmd!("cedar-policy-mcp-schema-generator");
        let cmd = cmd
            .arg("authorize")
            .arg("examples/stub.cedarschema")
            .arg("examples/simple/tool.json")
            .arg("-l")
            .arg(principal)
            .arg("-r")
            .arg(resource)
            .arg("--context")
            .arg(&context_fname)
            .arg("--policies")
            .arg(&policy_fname)
            .arg("--entities")
            .arg(&entities_fname)
            .arg("--mcp-tool-input")
            .arg(&input_fname);
        cmd.unwrap().assert().success().stdout("DENY\n").stderr("");
    }

    #[test]
    fn test_authorize_tuple_allow() {
        let temp_dir = TempDir::new().unwrap();
        let entities_fname = temp_dir.path().join("entities.json");
        std::fs::write(&entities_fname, "[]").unwrap();

        let request_json = r#"{
            "principal": "MyMcpServer::User::\"test_user\"",
            "resource": "MyMcpServer::McpServer::\"test_server\"",
            "context": {
                "session": {
                    "currentTimestamp": {
                        "__extn": {
                            "fn": "datetime",
                            "arg": "2025-12-16"
                        }
                    },
                    "ipaddr": {
                        "__extn": {
                            "fn": "ip",
                            "arg": "10.0.0.1"
                        }
                    }
                }
            }
        }"#;
        let request_fname = temp_dir.path().join("request.json");
        std::fs::write(&request_fname, request_json).unwrap();

        let policy_fname = temp_dir.path().join("policies.cedar");
        std::fs::write(
            &policy_fname,
            r#"permit(principal, action, resource) when {
                context.input.coordinate.proj0 == decimal("1.0") &&
                context.input.labeled_value.proj0 == "hello" &&
                context.input.labeled_value.proj1 == 42
            };"#,
        )
        .unwrap();

        let input = r#"{
            "params": {
                "tool": "tuple_tool",
                "args": {
                    "coordinate": [1.0, 2.5],
                    "labeled_value": ["hello", 42]
                }
            }
        }"#;
        let input_fname = temp_dir.path().join("input.json");
        std::fs::write(&input_fname, input).unwrap();

        let mut cmd = cargo_bin_cmd!("cedar-policy-mcp-schema-generator");
        let cmd = cmd
            .arg("authorize")
            .arg("examples/stub.cedarschema")
            .arg("examples/simple/tool_tuple.json")
            .arg("--encode-numbers-as-decimal")
            .arg("--request-json")
            .arg(&request_fname)
            .arg("--policies")
            .arg(&policy_fname)
            .arg("--entities")
            .arg(&entities_fname)
            .arg("--mcp-tool-input")
            .arg(&input_fname);
        cmd.unwrap().assert().success().stdout("ALLOW\n").stderr("");
    }

    #[test]
    fn test_authorize_tuple_deny() {
        let temp_dir = TempDir::new().unwrap();
        let entities_fname = temp_dir.path().join("entities.json");
        std::fs::write(&entities_fname, "[]").unwrap();

        let request_json = r#"{
            "principal": "MyMcpServer::User::\"test_user\"",
            "resource": "MyMcpServer::McpServer::\"test_server\"",
            "context": {
                "session": {
                    "currentTimestamp": {
                        "__extn": {
                            "fn": "datetime",
                            "arg": "2025-12-16"
                        }
                    },
                    "ipaddr": {
                        "__extn": {
                            "fn": "ip",
                            "arg": "10.0.0.1"
                        }
                    }
                }
            }
        }"#;
        let request_fname = temp_dir.path().join("request.json");
        std::fs::write(&request_fname, request_json).unwrap();

        let policy_fname = temp_dir.path().join("policies.cedar");
        std::fs::write(
            &policy_fname,
            r#"permit(principal, action, resource) when {
                context.input.coordinate.proj0 == decimal("99.0")
            };"#,
        )
        .unwrap();

        let input = r#"{
            "params": {
                "tool": "tuple_tool",
                "args": {
                    "coordinate": [1.0, 2.5]
                }
            }
        }"#;
        let input_fname = temp_dir.path().join("input.json");
        std::fs::write(&input_fname, input).unwrap();

        let mut cmd = cargo_bin_cmd!("cedar-policy-mcp-schema-generator");
        let cmd = cmd
            .arg("authorize")
            .arg("examples/stub.cedarschema")
            .arg("examples/simple/tool_tuple.json")
            .arg("--encode-numbers-as-decimal")
            .arg("--request-json")
            .arg(&request_fname)
            .arg("--policies")
            .arg(&policy_fname)
            .arg("--entities")
            .arg(&entities_fname)
            .arg("--mcp-tool-input")
            .arg(&input_fname);
        cmd.unwrap().assert().success().stdout("DENY\n").stderr("");
    }
}
