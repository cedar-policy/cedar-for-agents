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
        let expected = std::fs::read_to_string("examples/simple/tool_default.json").unwrap();

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
            std::fs::read_to_string("examples/simple/tool_keep_annotations.json").unwrap();

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
            std::fs::read_to_string("examples/simple/tool_objects_as_records.json").unwrap();

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
            std::fs::read_to_string("examples/simple/tool_include_outputs.json").unwrap();

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
            std::fs::read_to_string("examples/simple/tool_flattened_namespace.json").unwrap();

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
}
