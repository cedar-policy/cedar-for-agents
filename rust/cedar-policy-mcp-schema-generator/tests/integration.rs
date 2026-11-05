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

/// Read an expected schema file. The expected files have comments in them to make it
/// nicer to read the examples, but they need to be stripped before comparison.
fn read_expected_schema(schema_fname: &str) -> String {
    let raw_schema =
        std::fs::read_to_string(schema_fname).expect("Failed to read expected schema file");
    raw_schema
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
        + "\n"
}

mod lib {
    use std::collections::BTreeMap;

    use cedar_policy_mcp_schema_generator::{
        SchemaGenerator, SchemaGeneratorConfig, SchemaGeneratorError, StubBuilder,
    };

    use cedar_policy_core::extensions::Extensions;
    use cedar_policy_core::validator::json_schema::Fragment;
    use cedar_policy_core::validator::{ValidationMode, Validator, ValidatorSchema};

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

        assert_schema_matches_file(&generator, schema_fname);
    }

    /// Assert that the schema of `generator` is the schema of file `schema_fname`.
    fn assert_schema_matches_file(generator: &SchemaGenerator, schema_fname: &str) {
        let expected_schema = super::read_expected_schema(schema_fname);
        let actual_schema = generator
            .get_schema()
            .to_cedarschema()
            .expect("Failed to resolve generated schema");
        assert!(
            expected_schema == actual_schema,
            "{} != {}",
            expected_schema,
            actual_schema
        );
    }

    fn multi_namespace_generator(config: SchemaGeneratorConfig) -> SchemaGenerator {
        let stub = std::fs::read_to_string("examples/multi/stub.cedarschema")
            .expect("Failed to read schema file");
        SchemaGenerator::from_cedarschema_str_with_config(&stub, config)
            .expect("input schema file is malformed")
    }

    fn multi_namespace_servers() -> BTreeMap<String, ServerDescription> {
        BTreeMap::from([
            (
                "weather-api".to_string(),
                ServerDescription::from_json_file("examples/multi/weather.json")
                    .expect("Failed to read tools file"),
            ),
            (
                "billing".to_string(),
                ServerDescription::from_json_file("examples/multi/billing.json")
                    .expect("Failed to read tools file"),
            ),
        ])
    }

    fn run_multi_namespace_test(schema_fname: &str, config: SchemaGeneratorConfig) {
        let mut generator = multi_namespace_generator(config);
        generator
            .add_actions_from_named_server_descriptions(&multi_namespace_servers())
            .expect("Failed to add tool actions to schema generator");
        assert_schema_matches_file(&generator, schema_fname);

        let policies = std::fs::read_to_string("examples/multi/policies.cedar")
            .expect("Failed to read policies file");
        let policies = cedar_policy_core::parser::parse_policyset(&policies)
            .expect("Failed to parse policies");
        let schema = ValidatorSchema::try_from(generator.get_schema().clone())
            .expect("Failed to resolve generated schema");
        let result = Validator::new(schema).validate(&policies, ValidationMode::Strict);
        assert!(
            result.validation_passed(),
            "{:?}",
            result.validation_errors().collect::<Vec<_>>()
        );
    }

    #[test]
    fn multi_namespace_default() {
        run_multi_namespace_test(
            "examples/multi/multi_default.cedarschema",
            SchemaGeneratorConfig::default(),
        );
    }

    #[test]
    fn multi_namespace_deduplicate() {
        run_multi_namespace_test(
            "examples/multi/multi_deduplicate.cedarschema",
            SchemaGeneratorConfig::default().deduplicate_entity_types(true),
        );
    }

    #[test]
    fn multi_namespace_objects_as_records() {
        run_multi_namespace_test(
            "examples/multi/multi_objects_as_records.cedarschema",
            SchemaGeneratorConfig::default().objects_as_records(true),
        );
    }

    #[test]
    fn multi_namespace_include_outputs() {
        run_multi_namespace_test(
            "examples/multi/multi_include_outputs.cedarschema",
            SchemaGeneratorConfig::default().include_outputs(true),
        );
    }

    #[test]
    fn multi_namespace_one_server_at_a_time() {
        let mut generator = multi_namespace_generator(SchemaGeneratorConfig::default());
        for (server, description) in multi_namespace_servers() {
            generator
                .add_actions_from_named_server_description(&server, &description)
                .expect("Failed to add tool actions to schema generator");
        }
        assert_schema_matches_file(&generator, "examples/multi/multi_default.cedarschema");
    }

    #[test]
    fn multi_namespace_failure_adds_no_server() {
        let mut generator = multi_namespace_generator(SchemaGeneratorConfig::default());
        let before = generator.get_schema_as_str();
        let mut servers = multi_namespace_servers();
        let weather = servers["weather-api"].clone();
        servers.insert("unknown".to_string(), weather);
        assert!(generator
            .add_actions_from_named_server_descriptions(&servers)
            .is_err());
        assert_eq!(generator.get_schema_as_str(), before);

        // Each server still accepts its tools after the failure
        generator
            .add_actions_from_named_server_descriptions(&multi_namespace_servers())
            .expect("Failed to add tool actions to schema generator");
        assert_schema_matches_file(&generator, "examples/multi/multi_default.cedarschema");
    }

    #[test]
    fn stub_builder() {
        let main = std::fs::read_to_string("examples/multi/main.cedarschema")
            .expect("Failed to read schema file");
        let builder = StubBuilder::from_cedarschema_str(&main)
            .and_then(|builder| builder.add_server("weather-api"))
            .and_then(|builder| builder.add_server_with_namespace("billing", "Billing"))
            .expect("Failed to build stub");
        assert_eq!(
            builder.get_stub().to_cedarschema().unwrap(),
            super::read_expected_schema("examples/multi/built_stub.cedarschema")
        );

        let mut generator =
            SchemaGenerator::new(builder.into_stub()).expect("built stub is malformed");
        generator
            .add_actions_from_named_server_descriptions(&multi_namespace_servers())
            .expect("Failed to add tool actions to schema generator");
        generator
            .new_request_generator()
            .expect("Failed to resolve generated schema");
    }

    #[test]
    fn multi_namespace_server_tools_added_once() {
        let mut generator = multi_namespace_generator(SchemaGeneratorConfig::default());
        let servers = multi_namespace_servers();
        generator
            .add_actions_from_named_server_description("billing", &servers["billing"])
            .expect("Failed to add tool actions to schema generator");
        assert!(matches!(
            generator.add_actions_from_named_server_description("billing", &servers["billing"]),
            Err(SchemaGeneratorError::ServerDescriptionMerge)
        ));
        assert!(matches!(
            generator.add_actions_from_named_server_descriptions(&servers),
            Err(SchemaGeneratorError::ServerDescriptionMerge)
        ));
    }

    fn run_inline_test(tools_json: &str, expected_schema: &str, config: SchemaGeneratorConfig) {
        let description =
            ServerDescription::from_json_str(tools_json).expect("Failed to parse tools JSON");
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

    fn run_inline_test_with_stub(
        tools_json: &str,
        stub_schema: &str,
        expected_schema: &str,
        config: SchemaGeneratorConfig,
    ) {
        let description =
            ServerDescription::from_json_str(tools_json).expect("Failed to parse tools JSON");
        let extensions = Extensions::all_available();
        let (input_schema, _) = Fragment::from_cedarschema_str(stub_schema, extensions)
            .expect("Failed to parse custom stub schema");

        let mut generator = SchemaGenerator::new_with_config(input_schema, config)
            .expect("input schema file is malformed");
        generator
            .add_actions_from_server_description(&description)
            .expect("Failed to add tool actions to schema generator");

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
    fn dedup_leaf_record_different_required() {
        // Two tools have "metadata" with same field names and types, but different
        // required status (version? vs version). They should NOT be deduplicated.
        run_integration_test(
            "examples/dedup/dedup_leaf_record_different_required.json",
            "examples/dedup/dedup_leaf_record_different_required.cedarschema",
            SchemaGeneratorConfig::default().deduplicate_entity_types(true),
        );
    }

    #[test]
    fn dedup_leaf_record() {
        // tool_a and tool_b share the same "metadata" leaf record (author: String, version?: Long).
        // tool_c has a different "metadata" (title: String, count: Long).
        // The shared one should be deduplicated to the LCA namespace; tool_c keeps its own.
        run_integration_test(
            "examples/dedup/dedup_leaf_record.json",
            "examples/dedup/dedup_leaf_record.cedarschema",
            SchemaGeneratorConfig::default().deduplicate_entity_types(true),
        );
    }

    #[test]
    fn dedup_mixed_enum_and_record() {
        // Both an enum ("format") and a leaf record ("options") are shared across two tools.
        // Both should be deduplicated to the LCA namespace.
        run_integration_test(
            "examples/dedup/dedup_mixed_enum_and_record.json",
            "examples/dedup/dedup_mixed_enum_and_record.cedarschema",
            SchemaGeneratorConfig::default().deduplicate_entity_types(true),
        );
    }

    #[test]
    fn dedup_enum_record_name_conflict() {
        // An enum and a leaf record both named "status" target the same LCA.
        // Both should be skipped (no dedup) because of the name collision.
        run_integration_test(
            "examples/dedup/dedup_enum_record_name_conflict.json",
            "examples/dedup/dedup_enum_record_name_conflict.cedarschema",
            SchemaGeneratorConfig::default().deduplicate_entity_types(true),
        );
    }

    #[test]
    fn dedup_leaf_record_no_match() {
        // Three tools have "config" with: different field names (tool_a vs tool_b),
        // same field names but different types (tool_a vs tool_c).
        // None should be deduplicated.
        run_integration_test(
            "examples/dedup/dedup_leaf_record_no_match.json",
            "examples/dedup/dedup_leaf_record_no_match.cedarschema",
            SchemaGeneratorConfig::default().deduplicate_entity_types(true),
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
        run_integration_test(
            "examples/dedup/dedup_collision_existing_entity.json",
            "examples/dedup/dedup_collision_existing_entity.cedarschema",
            SchemaGeneratorConfig::default().deduplicate_entity_types(true),
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

        let expected_schema = "\
namespace MyMcpServer {
  type CommonContext = {
    currentTimestamp: datetime,
    ipaddr: ipaddr
  };

  type tool_aInput = {
    query: String,
    status?: MyMcpServer::status
  };

  type tool_bInput = {
    data: String,
    status?: MyMcpServer::status
  };

  entity McpServer;

  entity User = {
    id: String,
    username: String
  };

  entity status enum [\"active\", \"inactive\"];

  action \"call_tool\";

  action \"tool_a\" in [Action::\"call_tool\"] appliesTo {
    principal: [User],
    resource: [McpServer],
    context: {
      input: tool_aInput,
      session: CommonContext
    }
  };

  action \"tool_b\" in [Action::\"call_tool\"] appliesTo {
    principal: [User],
    resource: [McpServer],
    context: {
      input: tool_bInput,
      session: CommonContext
    }
  };
}
";

        run_inline_test_with_stub(
            tools_json,
            stub_schema,
            expected_schema,
            SchemaGeneratorConfig::default().deduplicate_entity_types(true),
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

        let expected_schema = "\
namespace MyMcpServer::tool_a::Input {
  entity status enum [\"active\", \"inactive\"];
}

namespace MyMcpServer::tool_b::Input {
  entity status enum [\"active\", \"inactive\"];
}

namespace MyMcpServer {
  type CommonContext = {
    currentTimestamp: datetime,
    ipaddr: ipaddr
  };

  type tool_aInput = {
    query: String,
    status?: MyMcpServer::tool_a::Input::status
  };

  type tool_bInput = {
    data: String,
    status?: MyMcpServer::tool_b::Input::status
  };

  entity McpServer;

  entity User = {
    id: String,
    username: String
  };

  entity status enum [\"open\", \"closed\"];

  action \"call_tool\";

  action \"tool_a\" in [Action::\"call_tool\"] appliesTo {
    principal: [User],
    resource: [McpServer],
    context: {
      input: tool_aInput,
      session: CommonContext
    }
  };

  action \"tool_b\" in [Action::\"call_tool\"] appliesTo {
    principal: [User],
    resource: [McpServer],
    context: {
      input: tool_bInput,
      session: CommonContext
    }
  };
}
";

        run_inline_test_with_stub(
            tools_json,
            stub_schema,
            expected_schema,
            SchemaGeneratorConfig::default().deduplicate_entity_types(true),
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

        let expected_schema = "\
namespace MyMcpServer::tool_a::Input {
  entity mode enum [\"fast\", \"slow\"];
}

namespace MyMcpServer::tool_b::Input {
  entity mode enum [\"fast\", \"slow\"];
}

namespace MyMcpServer::tool_c::Input {
  entity mode enum [\"sync\", \"async\"];
}

namespace MyMcpServer::tool_d::Input {
  entity mode enum [\"sync\", \"async\"];
}

namespace MyMcpServer {
  type CommonContext = {
    currentTimestamp: datetime,
    ipaddr: ipaddr
  };

  type tool_aInput = {
    mode: MyMcpServer::tool_a::Input::mode
  };

  type tool_bInput = {
    mode: MyMcpServer::tool_b::Input::mode
  };

  type tool_cInput = {
    mode: MyMcpServer::tool_c::Input::mode
  };

  type tool_dInput = {
    mode: MyMcpServer::tool_d::Input::mode
  };

  entity McpServer;

  entity User = {
    id: String,
    username: String
  };

  action \"call_tool\";

  action \"tool_a\" in [Action::\"call_tool\"] appliesTo {
    principal: [User],
    resource: [McpServer],
    context: {
      input: tool_aInput,
      session: CommonContext
    }
  };

  action \"tool_b\" in [Action::\"call_tool\"] appliesTo {
    principal: [User],
    resource: [McpServer],
    context: {
      input: tool_bInput,
      session: CommonContext
    }
  };

  action \"tool_c\" in [Action::\"call_tool\"] appliesTo {
    principal: [User],
    resource: [McpServer],
    context: {
      input: tool_cInput,
      session: CommonContext
    }
  };

  action \"tool_d\" in [Action::\"call_tool\"] appliesTo {
    principal: [User],
    resource: [McpServer],
    context: {
      input: tool_dInput,
      session: CommonContext
    }
  };
}
";

        run_inline_test(
            tools_json,
            expected_schema,
            SchemaGeneratorConfig::default().deduplicate_entity_types(true),
        );
    }

    #[test]
    fn dedup_same_variants_different_order_not_deduplicated() {
        // Two tools have an enum with the same variants but in different order.
        // Order is significant, so they should NOT be deduplicated.
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
                        "inputSchema": { "json": { "type": "object", "properties": { "mode": { "type": "string", "enum": ["slow", "fast"] } }, "required": ["mode"] } }
                    }
                ]
            }
        }"#;

        let expected_schema = "\
namespace MyMcpServer::tool_a::Input {
  entity mode enum [\"fast\", \"slow\"];
}

namespace MyMcpServer::tool_b::Input {
  entity mode enum [\"slow\", \"fast\"];
}

namespace MyMcpServer {
  type CommonContext = {
    currentTimestamp: datetime,
    ipaddr: ipaddr
  };

  type tool_aInput = {
    mode: MyMcpServer::tool_a::Input::mode
  };

  type tool_bInput = {
    mode: MyMcpServer::tool_b::Input::mode
  };

  entity McpServer;

  entity User = {
    id: String,
    username: String
  };

  action \"call_tool\";

  action \"tool_a\" in [Action::\"call_tool\"] appliesTo {
    principal: [User],
    resource: [McpServer],
    context: {
      input: tool_aInput,
      session: CommonContext
    }
  };

  action \"tool_b\" in [Action::\"call_tool\"] appliesTo {
    principal: [User],
    resource: [McpServer],
    context: {
      input: tool_bInput,
      session: CommonContext
    }
  };
}
";

        run_inline_test(
            tools_json,
            expected_schema,
            SchemaGeneratorConfig::default().deduplicate_entity_types(true),
        );
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

        let expected_schema = "\
namespace MyMcpServer {
  type CommonContext = {
    currentTimestamp: datetime,
    ipaddr: ipaddr
  };

  type tool_aInput = {
    priorities: Set<MyMcpServer::priorities>
  };

  type tool_bInput = {
    priorities: Set<MyMcpServer::priorities>
  };

  entity McpServer;

  entity User = {
    id: String,
    username: String
  };

  entity priorities enum [\"high\", \"medium\", \"low\"];

  action \"call_tool\";

  action \"tool_a\" in [Action::\"call_tool\"] appliesTo {
    principal: [User],
    resource: [McpServer],
    context: {
      input: tool_aInput,
      session: CommonContext
    }
  };

  action \"tool_b\" in [Action::\"call_tool\"] appliesTo {
    principal: [User],
    resource: [McpServer],
    context: {
      input: tool_bInput,
      session: CommonContext
    }
  };
}
";

        run_inline_test(
            tools_json,
            expected_schema,
            SchemaGeneratorConfig::default().deduplicate_entity_types(true),
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

        let expected_schema = "\
namespace MyMcpServer {
  type CommonContext = {
    currentTimestamp: datetime,
    ipaddr: ipaddr
  };

  type tool_aInput = {
    query: String
  };

  type tool_aOutput = {
    status: MyMcpServer::status
  };

  type tool_bInput = {
    id: String
  };

  type tool_bOutput = {
    status: MyMcpServer::status
  };

  entity McpServer;

  entity User = {
    id: String,
    username: String
  };

  entity status enum [\"success\", \"failure\", \"pending\"];

  action \"call_tool\";

  action \"tool_a\" in [Action::\"call_tool\"] appliesTo {
    principal: [User],
    resource: [McpServer],
    context: {
      input: tool_aInput,
      output?: tool_aOutput,
      session: CommonContext
    }
  };

  action \"tool_b\" in [Action::\"call_tool\"] appliesTo {
    principal: [User],
    resource: [McpServer],
    context: {
      input: tool_bInput,
      output?: tool_bOutput,
      session: CommonContext
    }
  };
}
";

        run_inline_test(
            tools_json,
            expected_schema,
            SchemaGeneratorConfig::default()
                .include_outputs(true)
                .deduplicate_entity_types(true),
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

        let expected_schema = "\
namespace MyMcpServer {
  type CommonContext = {
    currentTimestamp: datetime,
    ipaddr: ipaddr
  };

  type tool_aInput = {
    priority: MyMcpServer::priority
  };

  type tool_aOutput = {  };

  type tool_bInput = {
    id: String
  };

  type tool_bOutput = {
    priority: MyMcpServer::priority
  };

  entity McpServer;

  entity User = {
    id: String,
    username: String
  };

  entity priority enum [\"high\", \"medium\", \"low\"];

  action \"call_tool\";

  action \"tool_a\" in [Action::\"call_tool\"] appliesTo {
    principal: [User],
    resource: [McpServer],
    context: {
      input: tool_aInput,
      output?: tool_aOutput,
      session: CommonContext
    }
  };

  action \"tool_b\" in [Action::\"call_tool\"] appliesTo {
    principal: [User],
    resource: [McpServer],
    context: {
      input: tool_bInput,
      output?: tool_bOutput,
      session: CommonContext
    }
  };
}
";

        run_inline_test(
            tools_json,
            expected_schema,
            SchemaGeneratorConfig::default()
                .include_outputs(true)
                .deduplicate_entity_types(true),
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

        let expected_schema = "\
namespace MyMcpServer::tool_a::Input {
  entity status enum [\"success\", \"failure\"];
}

namespace MyMcpServer {
  type CommonContext = {
    currentTimestamp: datetime,
    ipaddr: ipaddr
  };

  type tool_aInput = {
    status: MyMcpServer::tool_a::Input::status
  };

  type tool_bInput = {
    id: String
  };

  entity McpServer;

  entity User = {
    id: String,
    username: String
  };

  action \"call_tool\";

  action \"tool_a\" in [Action::\"call_tool\"] appliesTo {
    principal: [User],
    resource: [McpServer],
    context: {
      input: tool_aInput,
      session: CommonContext
    }
  };

  action \"tool_b\" in [Action::\"call_tool\"] appliesTo {
    principal: [User],
    resource: [McpServer],
    context: {
      input: tool_bInput,
      session: CommonContext
    }
  };
}
";

        run_inline_test(
            tools_json,
            expected_schema,
            SchemaGeneratorConfig::default().deduplicate_entity_types(true),
        );
    }

    #[test]
    fn dedup_nested_record_not_deduplicated() {
        // Two tools have the same nested structure: a wrapping object "config" containing
        // a leaf object "settings". The innermost "settings" entity (all primitive fields)
        // should be deduplicated to the LCA, but the wrapping "config" entity should NOT
        // be deduplicated because it is not a leaf record (it references another entity type).
        run_integration_test(
            "examples/dedup/dedup_nested_record_not_deduplicated.json",
            "examples/dedup/dedup_nested_record_not_deduplicated.cedarschema",
            SchemaGeneratorConfig::default().deduplicate_entity_types(true),
        );
    }

    #[test]
    fn dedup_leaf_record_in_output() {
        // Two tools share the same leaf record "metadata" in their outputSchema.
        // With include_outputs + deduplicate_entity_types, the leaf record should be
        // deduplicated to the LCA namespace.
        run_integration_test(
            "examples/dedup/dedup_leaf_record_in_output.json",
            "examples/dedup/dedup_leaf_record_in_output.cedarschema",
            SchemaGeneratorConfig::default()
                .include_outputs(true)
                .deduplicate_entity_types(true),
        );
    }

    #[test]
    fn inline_empty_records_inlines_empty_record() {
        // A field whose shape is an empty record (no properties, no additionalProperties).
        // With `inline_empty_records`, it is encoded inline as the Cedar record type `{}`
        // rather than as a named entity type declaration that is referenced.
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
                                    "metadata": { "type": "object", "properties": {} }
                                },
                                "required": ["metadata"]
                            }
                        }
                    }
                ]
            }
        }"#;

        let expected_schema = "namespace MyMcpServer {
  type CommonContext = {
    currentTimestamp: datetime,
    ipaddr: ipaddr
  };

  type tool_aInput = {
    metadata: {    }
  };

  entity McpServer;

  entity User = {
    id: String,
    username: String
  };

  action \"call_tool\";

  action \"tool_a\" in [Action::\"call_tool\"] appliesTo {
    principal: [User],
    resource: [McpServer],
    context: {
      input: tool_aInput,
      session: CommonContext
    }
  };
}
";

        run_inline_test(
            tools_json,
            expected_schema,
            SchemaGeneratorConfig::default().inline_empty_records(true),
        );
    }

    #[test]
    fn inline_empty_records_default_emits_named_type() {
        // Same tool as `inline_empty_records_inlines_empty_record`, but with the default
        // config. The empty record is emitted as a named entity type and referenced,
        // confirming the inlining only happens when the option is enabled.
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
                                    "metadata": { "type": "object", "properties": {} }
                                },
                                "required": ["metadata"]
                            }
                        }
                    }
                ]
            }
        }"#;

        let expected_schema = "namespace MyMcpServer::tool_a::Input {
  entity metadata;
}

namespace MyMcpServer {
  type CommonContext = {
    currentTimestamp: datetime,
    ipaddr: ipaddr
  };

  type tool_aInput = {
    metadata: MyMcpServer::tool_a::Input::metadata
  };

  entity McpServer;

  entity User = {
    id: String,
    username: String
  };

  action \"call_tool\";

  action \"tool_a\" in [Action::\"call_tool\"] appliesTo {
    principal: [User],
    resource: [McpServer],
    context: {
      input: tool_aInput,
      session: CommonContext
    }
  };
}
";

        run_inline_test(
            tools_json,
            expected_schema,
            SchemaGeneratorConfig::default(),
        );
    }
}

#[cfg(feature = "cli")]
mod cli {
    use assert_cmd::{assert::OutputAssertExt, cargo_bin_cmd};
    use tempfile::TempDir;

    fn assert_stdout_snapshot(snapshot_name: &'static str, cmd: &mut assert_cmd::Command) {
        let output = cmd.assert().success();
        let stdout = String::from_utf8_lossy(&output.get_output().stdout);
        insta::assert_snapshot!(snapshot_name, stdout);
    }

    #[test]
    fn test_simple_default_cedar_schema() {
        let mut cmd = cargo_bin_cmd!("cedar-policy-mcp-schema-generator");
        let cmd = cmd
            .arg("generate")
            .arg("examples/stub.cedarschema")
            .arg("examples/simple/tool.json");
        assert_stdout_snapshot("simple_default_cedar_schema", cmd);
    }

    #[test]
    fn test_simple_default_json_schema() {
        let mut cmd = cargo_bin_cmd!("cedar-policy-mcp-schema-generator");
        let cmd = cmd
            .arg("generate")
            .arg("examples/stub.cedarschema")
            .arg("examples/simple/tool.json")
            .arg("--output-format")
            .arg("json");
        assert_stdout_snapshot("simple_default_json_schema", cmd);
    }

    #[test]
    fn test_simple_keep_annotations_cedar_schema() {
        let mut cmd = cargo_bin_cmd!("cedar-policy-mcp-schema-generator");
        let cmd = cmd
            .arg("generate")
            .arg("examples/stub.cedarschema")
            .arg("examples/simple/tool.json")
            .arg("--keep-annotations");
        assert_stdout_snapshot("simple_keep_annotations_cedar_schema", cmd);
    }

    #[test]
    fn test_simple_keep_annotations_json_schema() {
        let mut cmd = cargo_bin_cmd!("cedar-policy-mcp-schema-generator");
        let cmd = cmd
            .arg("generate")
            .arg("examples/stub.cedarschema")
            .arg("examples/simple/tool.json")
            .arg("--keep-annotations")
            .arg("--output-format")
            .arg("json");
        assert_stdout_snapshot("simple_keep_annotations_json_schema", cmd);
    }

    #[test]
    fn test_simple_object_as_records_cedar_schema() {
        let mut cmd = cargo_bin_cmd!("cedar-policy-mcp-schema-generator");
        let cmd = cmd
            .arg("generate")
            .arg("examples/stub.cedarschema")
            .arg("examples/simple/tool.json")
            .arg("--objects-as-records");
        assert_stdout_snapshot("simple_object_as_records_cedar_schema", cmd);
    }

    #[test]
    fn test_simple_object_as_records_json_schema() {
        let mut cmd = cargo_bin_cmd!("cedar-policy-mcp-schema-generator");
        let cmd = cmd
            .arg("generate")
            .arg("examples/stub.cedarschema")
            .arg("examples/simple/tool.json")
            .arg("--objects-as-records")
            .arg("--output-format")
            .arg("json");
        assert_stdout_snapshot("simple_object_as_records_json_schema", cmd);
    }

    #[test]
    fn test_simple_include_outputs_cedar_schema() {
        let mut cmd = cargo_bin_cmd!("cedar-policy-mcp-schema-generator");
        let cmd = cmd
            .arg("generate")
            .arg("examples/stub.cedarschema")
            .arg("examples/simple/tool.json")
            .arg("--include-outputs");
        assert_stdout_snapshot("simple_include_outputs_cedar_schema", cmd);
    }

    #[test]
    fn test_simple_include_outputs_json_schema() {
        let mut cmd = cargo_bin_cmd!("cedar-policy-mcp-schema-generator");
        let cmd = cmd
            .arg("generate")
            .arg("examples/stub.cedarschema")
            .arg("examples/simple/tool.json")
            .arg("--include-outputs")
            .arg("--output-format")
            .arg("json");
        assert_stdout_snapshot("simple_include_outputs_json_schema", cmd);
    }

    #[test]
    fn test_simple_flattened_namespace_cedar_schema() {
        let mut cmd = cargo_bin_cmd!("cedar-policy-mcp-schema-generator");
        let cmd = cmd
            .arg("generate")
            .arg("examples/stub.cedarschema")
            .arg("examples/simple/tool.json")
            .arg("--flatten-namespaces")
            .arg("--error-format")
            .arg("plain");
        assert_stdout_snapshot("simple_flattened_namespace_cedar_schema", cmd);
    }

    #[test]
    fn test_simple_flattened_namespace_json_schema() {
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
        assert_stdout_snapshot("simple_flattened_namespace_json_schema", cmd);
    }

    #[test]
    fn test_simple_encode_numbers_as_decimal_cedar_schema() {
        let mut cmd = cargo_bin_cmd!("cedar-policy-mcp-schema-generator");
        let cmd = cmd
            .arg("generate")
            .arg("examples/stub.cedarschema")
            .arg("examples/simple/tool.json")
            .arg("--encode-numbers-as-decimal");
        assert_stdout_snapshot("simple_encode_numbers_as_decimal_cedar_schema", cmd);
    }

    #[test]
    fn test_simple_encode_numbers_as_decimal_json_schema() {
        let mut cmd = cargo_bin_cmd!("cedar-policy-mcp-schema-generator");
        let cmd = cmd
            .arg("generate")
            .arg("examples/stub.cedarschema")
            .arg("examples/simple/tool.json")
            .arg("--encode-numbers-as-decimal")
            .arg("--output-format")
            .arg("json");
        assert_stdout_snapshot("simple_encode_numbers_as_decimal_json_schema", cmd);
    }
    #[test]
    fn test_nullable_objects_cedar_schema() {
        let mut cmd = cargo_bin_cmd!("cedar-policy-mcp-schema-generator");
        let cmd = cmd
            .arg("generate")
            .arg("examples/stub.cedarschema")
            .arg("examples/simple/tool_nullable_objects.json");
        assert_stdout_snapshot("nullable_objects_cedar_schema", cmd);
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

        let mut cmd = cargo_bin_cmd!("cedar-policy-mcp-schema-generator");
        let cmd = cmd
            .arg("generate")
            .arg(input_file)
            .arg("examples/simple/tool.json");
        assert_stdout_snapshot("default_with_json_schema", cmd);
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

        let actual = std::fs::read_to_string(output_file).unwrap();
        insta::assert_snapshot!("default_write_to_file", actual);
    }

    #[test]
    fn test_authorize_simple_default_allow() {
        authorize_simple_default_allow("permit(principal, action, resource);");
    }

    #[test]
    fn test_authorize_simple_default_action_in_allow() {
        authorize_simple_default_allow(
            r#"permit(principal, action in MyMcpServer::Action::"call_tool", resource);"#,
        );
    }

    /// Assert that `policy` allows a call to `test_tool` of `examples/simple/tool.json`
    fn authorize_simple_default_allow(policy: &str) {
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
        std::fs::write(&policy_fname, policy).unwrap();

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

    #[test]
    fn test_authorize_dedup_leaf_record_allow() {
        // Two tools share a leaf record "metadata" {author: String, version?: Long}.
        // With dedup, the entity type is placed in the LCA namespace.
        // Policy checks the entity's attributes via has/access on the entity.
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
    context.input.query == "hello"
};"#,
        )
        .unwrap();

        let input = r#"{
    "params": {
        "tool": "tool_a",
        "args": {
            "metadata": { "author": "alice", "version": 2 },
            "query": "hello"
        }
    }
}"#;
        let input_fname = temp_dir.path().join("input.json");
        std::fs::write(&input_fname, input).unwrap();

        let mut cmd = cargo_bin_cmd!("cedar-policy-mcp-schema-generator");
        let cmd = cmd
            .arg("authorize")
            .arg("examples/stub.cedarschema")
            .arg("examples/dedup/dedup_leaf_record.json")
            .arg("--deduplicate-entity-types")
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
    fn test_authorize_dedup_leaf_record_deny() {
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
    context.input.query == "world"
};"#,
        )
        .unwrap();

        let input = r#"{
    "params": {
        "tool": "tool_a",
        "args": {
            "metadata": { "author": "alice", "version": 2 },
            "query": "hello"
        }
    }
}"#;
        let input_fname = temp_dir.path().join("input.json");
        std::fs::write(&input_fname, input).unwrap();

        let mut cmd = cargo_bin_cmd!("cedar-policy-mcp-schema-generator");
        let cmd = cmd
            .arg("authorize")
            .arg("examples/stub.cedarschema")
            .arg("examples/dedup/dedup_leaf_record.json")
            .arg("--deduplicate-entity-types")
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

    #[test]
    fn test_authorize_dedup_mixed_enum_and_record_allow() {
        // Tests authorization with both a deduplicated enum and a deduplicated leaf record.
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
    context.input has format &&
    context.input.format == MyMcpServer::format::"markdown"
};"#,
        )
        .unwrap();

        let input = r#"{
    "params": {
        "tool": "tool_a",
        "args": {
            "format": "markdown",
            "options": { "verbose": true, "timeout": 30 },
            "query": "test"
        }
    }
}"#;
        let input_fname = temp_dir.path().join("input.json");
        std::fs::write(&input_fname, input).unwrap();

        let mut cmd = cargo_bin_cmd!("cedar-policy-mcp-schema-generator");
        let cmd = cmd
            .arg("authorize")
            .arg("examples/stub.cedarschema")
            .arg("examples/dedup/dedup_mixed_enum_and_record.json")
            .arg("--deduplicate-entity-types")
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
    fn test_authorize_dedup_mixed_enum_and_record_deny() {
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
    context.input has format &&
    context.input.format == MyMcpServer::format::"text"
};"#,
        )
        .unwrap();

        let input = r#"{
    "params": {
        "tool": "tool_b",
        "args": {
            "format": "markdown",
            "options": { "verbose": false },
            "data": "payload"
        }
    }
}"#;
        let input_fname = temp_dir.path().join("input.json");
        std::fs::write(&input_fname, input).unwrap();

        let mut cmd = cargo_bin_cmd!("cedar-policy-mcp-schema-generator");
        let cmd = cmd
            .arg("authorize")
            .arg("examples/stub.cedarschema")
            .arg("examples/dedup/dedup_mixed_enum_and_record.json")
            .arg("--deduplicate-entity-types")
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

    fn multi_namespace_generate_cmd() -> assert_cmd::Command {
        let mut cmd = cargo_bin_cmd!("cedar-policy-mcp-schema-generator");
        cmd.arg("generate")
            .arg("examples/multi/stub.cedarschema")
            .arg("--server")
            .arg("weather-api=examples/multi/weather.json")
            .arg("--server")
            .arg("billing=examples/multi/billing.json");
        cmd
    }

    #[test]
    fn test_multi_namespace_generate() {
        multi_namespace_generate_cmd()
            .assert()
            .success()
            .stdout(super::read_expected_schema(
                "examples/multi/multi_default.cedarschema",
            ));
    }

    #[test]
    fn test_multi_namespace_generate_deduplicate() {
        multi_namespace_generate_cmd()
            .arg("--deduplicate-entity-types")
            .assert()
            .success()
            .stdout(super::read_expected_schema(
                "examples/multi/multi_deduplicate.cedarschema",
            ));
    }

    #[test]
    fn test_stub() {
        let mut cmd = cargo_bin_cmd!("cedar-policy-mcp-schema-generator");
        cmd.arg("stub")
            .arg("examples/multi/main.cedarschema")
            .arg("--server")
            .arg("weather-api")
            .arg("--server")
            .arg("billing=Billing")
            .assert()
            .success()
            .stdout(super::read_expected_schema(
                "examples/multi/built_stub.cedarschema",
            ));
    }

    #[test]
    fn test_stub_invalid_namespace() {
        let mut cmd = cargo_bin_cmd!("cedar-policy-mcp-schema-generator");
        let assert = cmd
            .arg("stub")
            .arg("examples/multi/main.cedarschema")
            .arg("--server")
            .arg("in")
            .arg("--error-format")
            .arg("plain")
            .assert()
            .failure();
        assert_stderr_contains(
            assert,
            "`in` is not a valid namespace identifier for server `in`.",
        );
    }

    #[test]
    fn test_multi_namespace_generate_duplicate_server() {
        let assert = multi_namespace_generate_cmd()
            .arg("--server")
            .arg("billing=examples/multi/billing.json")
            .arg("--error-format")
            .arg("plain")
            .assert()
            .failure();
        assert_stderr_contains(
            assert,
            "Server `billing` is given more than once with `--server`",
        );
    }

    #[test]
    fn test_multi_namespace_generate_without_server() {
        let mut cmd = cargo_bin_cmd!("cedar-policy-mcp-schema-generator");
        let assert = cmd
            .arg("generate")
            .arg("examples/multi/stub.cedarschema")
            .arg("examples/multi/weather.json")
            .arg("--error-format")
            .arg("plain")
            .assert()
            .failure();
        assert_stderr_contains(
            assert,
            "The schema stub declares `@mcp_server` namespaces, so a server name is required.",
        );
    }

    #[test]
    fn test_single_namespace_generate_with_server() {
        let mut cmd = cargo_bin_cmd!("cedar-policy-mcp-schema-generator");
        let assert = cmd
            .arg("generate")
            .arg("examples/stub.cedarschema")
            .arg("--server")
            .arg("weather-api=examples/multi/weather.json")
            .arg("--error-format")
            .arg("plain")
            .assert()
            .failure();
        assert_stderr_contains(
            assert,
            "The schema stub declares no `@mcp_server` namespace, so a server name cannot be used.",
        );
    }

    fn assert_stderr_contains(assert: assert_cmd::assert::Assert, expected: &str) {
        let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
        assert!(stderr.contains(expected), "{expected} not in {stderr}");
    }

    /// `authorize` a call to server `server`, using the files `examples/multi/{prefix}*`
    fn multi_namespace_authorize_cmd(server: &str, prefix: &str) -> assert_cmd::Command {
        let mut cmd = cargo_bin_cmd!("cedar-policy-mcp-schema-generator");
        cmd.arg("authorize")
            .arg("examples/multi/stub.cedarschema")
            .arg(format!("examples/multi/{prefix}.json"))
            .arg("--server")
            .arg(server)
            .arg("--policies")
            .arg("examples/multi/policies.cedar")
            .arg("--entities")
            .arg("examples/multi/entities.json")
            .arg("--request-json")
            .arg(format!("examples/multi/{prefix}_request.json"))
            .arg("--mcp-tool-input")
            .arg(format!("examples/multi/{prefix}_input.json"));
        cmd
    }

    #[test]
    fn test_multi_namespace_authorize_allow() {
        multi_namespace_authorize_cmd("weather-api", "weather")
            .assert()
            .success()
            .stdout("ALLOW\n")
            .stderr("");
    }

    #[test]
    fn test_multi_namespace_authorize_deny() {
        // The billing policy only permits `account_id == "acct2"`
        multi_namespace_authorize_cmd("billing", "billing")
            .assert()
            .success()
            .stdout("DENY\n")
            .stderr("");
    }
}
