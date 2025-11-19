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

use smol_str::{SmolStr, ToSmolStr};
use std::collections::HashMap;
use std::path::Path;

use super::data::{Input, Output};
use super::deserializer;
use super::err::{DeserializationError, ValidationError};
use super::parser;
use super::validation::{validate_input, validate_output};

/// The type a `Property` can take
#[derive(Debug, Clone)]
pub enum PropertyType {
    Bool,
    Integer,
    Float,
    Number,
    String,
    Decimal,
    Datetime,
    Duration,
    IpAddr,
    Null,
    Enum {
        variants: Vec<SmolStr>,
    },
    Array {
        element_ty: Box<PropertyType>,
    },
    Tuple {
        types: Vec<PropertyType>,
    },
    Union {
        types: Vec<PropertyType>,
    },
    Object {
        properties: Vec<Property>,
        additional_properties: Option<Box<PropertyType>>,
    },
    Ref {
        name: SmolStr,
    },
}

/// Representation of an input (or output) `Property`
/// I.e., an attribute of an JSON Object Schema type
#[derive(Debug, Clone)]
pub struct Property {
    pub(crate) name: SmolStr,
    pub(crate) description: Option<String>,
    pub(crate) required: bool,
    pub(crate) prop_type: PropertyType,
}

impl Property {
    /// Create a new `Property`
    pub fn new(
        name: SmolStr,
        required: bool,
        prop_type: PropertyType,
        description: Option<String>,
    ) -> Self {
        Self {
            name,
            description,
            required,
            prop_type,
        }
    }

    /// Returns the `name` of the `Property`.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns if this is a required `Property`.
    pub fn is_required(&self) -> bool {
        self.required
    }

    /// Returns the `Property`'s `description` if it exists.
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    pub fn property_type(&self) -> &PropertyType {
        &self.prop_type
    }
}

/// Representation of a TypeDef used for defining `Property`s
#[derive(Debug, Clone)]
pub struct PropertyTypeDef {
    pub(crate) name: SmolStr,
    pub(crate) prop_type: PropertyType,
    pub(crate) description: Option<String>,
}

impl PropertyTypeDef {
    /// Create a new `PropertyTypeDef` from its components
    pub fn new(name: SmolStr, prop_type: PropertyType, description: Option<String>) -> Self {
        Self {
            name,
            prop_type,
            description,
        }
    }

    /// Get the name of the TypeDef
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the definition of the TypeDef
    pub fn property_type(&self) -> &PropertyType {
        &self.prop_type
    }

    /// Retrieve the description of the type (if it exists)
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }
}

/// Container for convienently representing a collection of TypeDefs
#[derive(Debug, Clone)]
pub(crate) struct PropertyTypeDefs {
    pub(crate) type_defs: HashMap<SmolStr, PropertyTypeDef>,
}

impl PropertyTypeDefs {
    /// Create a new collection of TypeDefs
    pub(crate) fn new(type_defs: HashMap<SmolStr, PropertyTypeDef>) -> Self {
        Self { type_defs }
    }

    /// Get the collection of TypeDefs
    pub(crate) fn values(&self) -> impl Iterator<Item = &PropertyTypeDef> {
        self.type_defs.values()
    }
}

/// A collection of Input (or Output) Properties of an MCP tool Description
/// I.e., a Representation of the data in the `Parameters`, `InputSchema` or `OutputSchema`
/// attribute of an MCP tool Descritpion
#[derive(Debug, Clone)]
pub struct Parameters {
    pub(crate) properties: Vec<Property>,
    pub(crate) type_defs: PropertyTypeDefs,
}

impl Parameters {
    /// Create a new Parameters collection
    pub fn new(properties: Vec<Property>, type_defs: HashMap<SmolStr, PropertyTypeDef>) -> Self {
        Self {
            properties,
            type_defs: PropertyTypeDefs::new(type_defs),
        }
    }

    /// Iterate over the `Property`s within this `Parameters`
    pub fn properties(&self) -> impl Iterator<Item = &Property> {
        self.properties.iter()
    }

    /// Iterate over the TypeDefs defined within this `Parameters`
    pub fn type_definitions(&self) -> impl Iterator<Item = &PropertyTypeDef> {
        self.type_defs.values()
    }
}

/// A Representation of a Single Tool Description
#[derive(Debug, Clone)]
pub struct ToolDescription {
    pub(crate) name: SmolStr,
    pub(crate) description: Option<String>,
    pub(crate) inputs: Parameters,
    pub(crate) outputs: Parameters,
    pub(crate) type_defs: PropertyTypeDefs,
}

impl ToolDescription {
    /// Construct a Tool Description from its components
    pub fn new(
        name: SmolStr,
        inputs: Parameters,
        outputs: Parameters,
        type_defs: HashMap<SmolStr, PropertyTypeDef>,
        description: Option<String>,
    ) -> Self {
        Self {
            name,
            description,
            inputs,
            outputs,
            type_defs: PropertyTypeDefs::new(type_defs),
        }
    }

    /// Get the name of this tool
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the description of this tool (if it exists)
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    /// Get the input `Parameters` of this tool
    pub fn inputs(&self) -> &Parameters {
        &self.inputs
    }

    /// Get the output `Parameters` of this tool
    pub fn outputs(&self) -> &Parameters {
        &self.outputs
    }

    /// Get the TypeDefs defined within this tool (i.e., TypeDefs shared between Input and Output Parameters)
    /// This does not return the TypeDefs specific to either Input or Output Parameters.
    pub fn type_definitions(&self) -> impl Iterator<Item = &PropertyTypeDef> {
        self.type_defs.values()
    }

    /// Deserialize an MCP Tool Description JSON into a `ToolDescription`
    pub fn from_json_str(json_str: &str) -> Result<Self, DeserializationError> {
        let mut parser = parser::json_parser::JsonParser::new(json_str);
        deserializer::tool_description_from_json_value(parser.get_value()?)
    }

    /// Deserialize an MCP Tool Description JSON into a `ToolDescription`
    pub fn from_json_file<P: AsRef<Path>>(json_file: P) -> Result<Self, DeserializationError> {
        let contents = std::fs::read_to_string(json_file.as_ref()).map_err(|e| {
            DeserializationError::read_error(json_file.as_ref().into(), format!("{e}"))
        })?;
        Self::from_json_str(&contents)
    }

    /// Validates the `Input` matches this `ToolDescription`'s input schema.
    pub fn validate_input(
        &self,
        input: &Input,
        type_defs: &HashMap<SmolStr, PropertyTypeDef>,
    ) -> Result<(), ValidationError> {
        validate_input(self, input, type_defs)
    }

    /// Validates the `Output` matches this `ToolDescription`'s output schema.
    pub fn validate_output(
        &self,
        output: &Output,
        type_defs: &HashMap<SmolStr, PropertyTypeDef>,
    ) -> Result<(), ValidationError> {
        validate_output(self, output, type_defs)
    }
}

/// A representation of a collection of MCP Tools (e.g., all the tools provided by an MCP Server)
#[derive(Debug, Clone)]
pub struct ServerDescription {
    tools: HashMap<SmolStr, ToolDescription>,
    type_defs: PropertyTypeDefs,
}

impl ServerDescription {
    /// Create a new Server Description from its components
    pub fn new(tools: Vec<ToolDescription>, type_defs: HashMap<SmolStr, PropertyTypeDef>) -> Self {
        let tools = tools
            .into_iter()
            .map(|tool| (tool.name().to_smolstr(), tool))
            .collect();
        Self {
            tools,
            type_defs: PropertyTypeDefs::new(type_defs),
        }
    }

    /// Get an iterator to all tool descriptions within this `ServerDescription`
    pub fn tool_descriptions(&self) -> impl Iterator<Item = &ToolDescription> {
        self.tools.values()
    }

    /// Get any TypeDefs defined within this ServerDescription (i.e., TypeDefs shared between tools)
    pub fn type_definitions(&self) -> impl Iterator<Item = &PropertyTypeDef> {
        self.type_defs.values()
    }

    /// Deserialize an MCP `tools/list` json response (or JSON Array of Tool Descriptions) into a `ServerDescription`
    pub fn from_json_str(json_str: &str) -> Result<Self, DeserializationError> {
        let mut parser = parser::json_parser::JsonParser::new(json_str);
        deserializer::server_description_from_json_value(parser.get_value()?)
    }

    /// Deserialize an MCP `tools/list` json response (or JSON Array of Tool Descriptions) into a `ServerDescription`
    pub fn from_json_file<P: AsRef<Path>>(json_file: P) -> Result<Self, DeserializationError> {
        let contents = std::fs::read_to_string(json_file.as_ref()).map_err(|e| {
            DeserializationError::read_error(json_file.as_ref().into(), format!("{e}"))
        })?;
        Self::from_json_str(&contents)
    }

    /// Validate the `Input` against the corresponding tool within this `ServerDescription`
    pub fn validate_input(&self, input: &Input) -> Result<(), ValidationError> {
        match self.tools.get(input.name()) {
            Some(tool) => tool.validate_input(input, &self.type_defs.type_defs),
            None => Err(ValidationError::tool_not_found(input.name().into())),
        }
    }

    /// Validate the `Output` against the corresponding tool within this `ServerDescription`
    pub fn validate_output(&self, tool_name: &str, output: &Output) -> Result<(), ValidationError> {
        match self.tools.get(tool_name) {
            Some(tool) => tool.validate_output(output, &self.type_defs.type_defs),
            None => Err(ValidationError::tool_not_found(tool_name.into())),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use cool_asserts::assert_matches;
    use smol_str::ToSmolStr;

    #[test]
    fn test_property() {
        let property = Property::new(
            "Prop".into(),
            true,
            PropertyType::Bool,
            Some("Banana".to_string()),
        );
        assert!(property.name() == "Prop");
        assert!(property.is_required());
        assert_matches!(property.property_type(), PropertyType::Bool);
        assert_matches!(property.description(), Some("Banana"));
    }

    #[test]
    fn test_type_def() {
        let type_def = PropertyTypeDef::new(
            "my_type".into(),
            PropertyType::Datetime,
            Some("My Type".to_string()),
        );
        assert!(type_def.name() == "my_type");
        assert_matches!(type_def.property_type(), PropertyType::Datetime);
        assert_matches!(type_def.description(), Some("My Type"));
    }

    #[test]
    fn test_parameters() {
        let props = vec![
            Property::new("first".into(), true, PropertyType::Bool, None),
            Property::new("second".into(), false, PropertyType::Float, None),
        ];
        let type_defs = vec![
            (
                "my_bool".to_smolstr(),
                PropertyTypeDef::new("my_bool".into(), PropertyType::Bool, None),
            ),
            (
                "my_int".to_smolstr(),
                PropertyTypeDef::new("my_int".into(), PropertyType::Integer, None),
            ),
        ]
        .into_iter()
        .collect();
        let params = Parameters::new(props, type_defs);

        assert_matches!(
            params.properties().map(Property::name).collect::<Vec<_>>(),
            ["first", "second"]
        );
        assert_matches!(
            params
                .properties()
                .map(Property::is_required)
                .collect::<Vec<_>>(),
            [true, false]
        );
        assert_matches!(
            params
                .properties()
                .map(Property::property_type)
                .collect::<Vec<_>>(),
            [PropertyType::Bool, PropertyType::Float]
        );
        assert_matches!(
            params
                .properties()
                .map(Property::description)
                .collect::<Vec<_>>(),
            [None, None]
        );

        let type_defs = params.type_definitions().cloned().collect::<Vec<_>>();
        assert!(type_defs.len() == 2);
        if type_defs.get(0).map(PropertyTypeDef::name) == Some("my_bool") {
            assert_matches!(
                type_defs
                    .iter()
                    .map(PropertyTypeDef::name)
                    .collect::<Vec<_>>(),
                ["my_bool", "my_int"]
            );
            assert_matches!(
                type_defs
                    .iter()
                    .map(PropertyTypeDef::property_type)
                    .collect::<Vec<_>>(),
                [PropertyType::Bool, PropertyType::Integer]
            );
            assert_matches!(
                type_defs
                    .iter()
                    .map(PropertyTypeDef::description)
                    .collect::<Vec<_>>(),
                [None, None]
            );
        } else {
            assert_matches!(
                type_defs
                    .iter()
                    .map(PropertyTypeDef::name)
                    .collect::<Vec<_>>(),
                ["my_int", "my_bool"]
            );
            assert_matches!(
                type_defs
                    .iter()
                    .map(PropertyTypeDef::property_type)
                    .collect::<Vec<_>>(),
                [PropertyType::Integer, PropertyType::Bool]
            );
            assert_matches!(
                type_defs
                    .iter()
                    .map(PropertyTypeDef::description)
                    .collect::<Vec<_>>(),
                [None, None]
            );
        }
    }

    #[test]
    fn test_tool_from_json_str_simple() {
        let tool_description = r#"{
            "name": "check_task_status",
            "description": "Check if a task is ready for work",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task_id": {"type": "string"}
                },
                "required": ["task_id"]
            }
        }"#;
        let tool = ToolDescription::from_json_str(tool_description)
            .expect("Failed to parse MCP Description");
        assert!(tool.name() == "check_task_status");
        assert_matches!(
            tool.description(),
            Some("Check if a task is ready for work")
        );
        assert!(tool.type_definitions().count() == 0);
        assert!(tool.inputs().type_definitions().count() == 0);
        assert!(tool.outputs().properties().count() == 0);
        assert!(tool.outputs().type_definitions().count() == 0);

        let inputs = tool.inputs().properties().cloned().collect::<Vec<_>>();
        assert!(inputs.len() == 1);

        assert_matches!(inputs.get(0).map(Property::name), Some("task_id"));
        assert_matches!(inputs.get(0).map(Property::is_required), Some(true));
        assert_matches!(
            inputs.get(0).map(Property::property_type),
            Some(PropertyType::String)
        );
        assert_matches!(inputs.get(0).and_then(Property::description), None);
    }

    #[test]
    fn test_server_from_json_str_simple() {
        let server_description = r#"{
            "result": {
                "tools": [{
                    "name": "check_task_status",
                    "description": "Check if a task is ready for work",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "task_id": {"type": "string"}
                        },
                        "required": ["task_id"]
                    }
                }]
            }
        }"#;
        let tools = ServerDescription::from_json_str(server_description)
            .expect("Failed to parse server description");
        assert!(tools.type_definitions().count() == 0);
        assert!(tools.tool_descriptions().count() == 1);

        let tool = tools.tool_descriptions().next().unwrap();
        assert!(tool.name() == "check_task_status");
        assert_matches!(
            tool.description(),
            Some("Check if a task is ready for work")
        );
        assert!(tool.type_definitions().count() == 0);
        assert!(tool.inputs().type_definitions().count() == 0);
        assert!(tool.outputs().properties().count() == 0);
        assert!(tool.outputs().type_definitions().count() == 0);

        let inputs = tool.inputs().properties().cloned().collect::<Vec<_>>();
        assert!(inputs.len() == 1);

        assert_matches!(inputs.get(0).map(Property::name), Some("task_id"));
        assert_matches!(inputs.get(0).map(Property::is_required), Some(true));
        assert_matches!(
            inputs.get(0).map(Property::property_type),
            Some(PropertyType::String)
        );
        assert_matches!(inputs.get(0).and_then(Property::description), None);
    }

    #[test]
    fn test_result_file_but_result_not_object_error() {
        let server_description = r#"{
    "result": false
}"#;
        assert_matches!(
            ServerDescription::from_json_str(server_description),
            Err(DeserializationError::MissingExpectedAttribute(..))
        );
    }

    #[test]
    fn test_result_file_without_tools_list_error() {
        let server_description = r#"{
    "result": {}
}"#;
        assert_matches!(
            ServerDescription::from_json_str(server_description),
            Err(DeserializationError::MissingExpectedAttribute(..))
        );
    }

    #[test]
    fn test_result_file_tool_not_array_error() {
        let server_description = r#"{
    "result": {
        "tools": {}
    }
}"#;
        assert_matches!(
            ServerDescription::from_json_str(server_description),
            Err(DeserializationError::UnexpectedType(..))
        );
    }

    #[test]
    fn test_empty_array_of_tools() {
        let server_description = "[]";
        let tools = ServerDescription::from_json_str(server_description).unwrap();

        assert!(tools.tool_descriptions().count() == 0);
        assert!(tools.type_definitions().count() == 0);
    }

    #[test]
    fn test_deserialize_wrong_type() {
        let server_description = "true";
        assert_matches!(
            ServerDescription::from_json_str(server_description),
            Err(DeserializationError::UnexpectedType(..))
        );
    }

    #[test]
    fn test_tool_name_wrong_type() {
        let server_description = r#"[
    { "name": false }
]"#;
        assert_matches!(
            ServerDescription::from_json_str(server_description),
            Err(DeserializationError::UnexpectedType(..))
        );
    }

    #[test]
    fn test_tool_name_not_found_error() {
        let server_description = r#"[{}]"#;
        assert_matches!(
            ServerDescription::from_json_str(server_description),
            Err(DeserializationError::MissingExpectedAttribute(..))
        );
    }

    #[test]
    fn test_tool_with_no_inputs_error() {
        let server_description = r#"{
    "result": {
        "tools": [
            {
                "name": "test_tool",
                "description": "a tool for testing",
                "properties": {
                    "comment": "properties should be \"parameters\" or \"inputSchema\""
                }
            }
        ]
    }
}"#;
        assert_matches!(
            ServerDescription::from_json_str(server_description),
            Err(DeserializationError::MissingExpectedAttribute(..))
        );
    }

    #[test]
    fn test_tool_description_not_string_error() {
        let tool_description = r#"{
    "name": "test_tool",
    "description": false,
    "inputSchema": {}
}"#;
        assert_matches!(
            ToolDescription::from_json_str(tool_description),
            Err(DeserializationError::UnexpectedType(..))
        );
    }

    #[test]
    fn test_tool_no_description() {
        let tool_description = r#"{
    "name": "test_tool",
    "inputSchema": {}
}"#;
        let tool = ToolDescription::from_json_str(tool_description).unwrap();
        assert!(tool.description().is_none());
    }

    #[test]
    fn test_tool_not_object_error() {
        let tool_description = "[]";
        assert_matches!(
            ToolDescription::from_json_str(tool_description),
            Err(DeserializationError::UnexpectedType(..))
        );
    }

    #[test]
    fn test_inputs_not_object_error() {
        let tool_description = r#"{
    "name": "test_tool",
    "inputSchema": []
}"#;
        assert_matches!(
            ToolDescription::from_json_str(tool_description),
            Err(DeserializationError::UnexpectedType(..))
        );
    }

    #[test]
    fn test_typedefs_not_object_error() {
        let tool_description = r#"{
    "name": "test_tool",
    "$defs": [],
    "inputSchema": {}
}"#;
        assert_matches!(
            ToolDescription::from_json_str(tool_description),
            Err(DeserializationError::UnexpectedType(..))
        );
    }

    #[test]
    fn test_required_contains_non_string_error() {
        let tool_description = r#"{
    "name": "test_tool",
    "inputSchema": {
        "type": "object",
        "properties": {},
        "required": [true]
    }
}"#;
        assert_matches!(
            ToolDescription::from_json_str(tool_description),
            Err(DeserializationError::UnexpectedType(..))
        );
    }

    #[test]
    fn test_required_is_false() {
        let tool_description = r#"{
    "name": "test_tool",
    "inputSchema": {
        "type": "object",
        "properties": {
            "test_attr": { "type": "string" }
        },
        "required": false
    }
}"#;
        let tool = ToolDescription::from_json_str(tool_description).unwrap();

        assert!(tool.inputs().properties().count() == 1);

        let property = tool.inputs().properties().next().unwrap();
        assert!(!property.is_required());
        assert!(property.name() == "test_attr");
    }

    #[test]
    fn test_required_is_true_error() {
        let tool_description = r#"{
    "name": "test_tool",
    "inputSchema": {
        "type": "object",
        "properties": {
            "test_attr": { "type": "string" }
        },
        "required": true
    }
}"#;
        assert_matches!(
            ToolDescription::from_json_str(tool_description),
            Err(DeserializationError::UnexpectedType(..))
        );
    }

    #[test]
    fn test_properties_is_false() {
        let tool_description = r#"{
    "name": "test_tool",
    "inputSchema": {
        "type": "object",
        "properties": false,
        "required": false
    }
}"#;
        let tool = ToolDescription::from_json_str(tool_description).unwrap();

        assert!(tool.inputs().properties().count() == 0);
    }

    #[test]
    fn test_properties_is_true_error() {
        let tool_description = r#"{
    "name": "test_tool",
    "inputSchema": {
        "type": "object",
        "properties": true,
        "required": false
    }
}"#;
        assert_matches!(
            ToolDescription::from_json_str(tool_description),
            Err(DeserializationError::UnexpectedType(..))
        );
    }

    #[test]
    fn test_property_with_non_string_description_error() {
        let tool_description = r#"{
    "name": "test_tool",
    "inputSchema": {
        "type": "object",
        "properties": {
            "test_attr": {
                "type": "string",
                "description": false
            }
        },
        "required": false
    }
}"#;
        assert_matches!(
            ToolDescription::from_json_str(tool_description),
            Err(DeserializationError::UnexpectedType(..))
        );
    }

    #[test]
    fn test_empty_enum_errors() {
        let tool_description = r#"{
    "name": "test_tool",
    "inputSchema": {
        "type": "object",
        "properties": {
            "test_attr": {
                "type": "string",
                "enum": []
            }
        }
    }
}"#;
        assert_matches!(
            ToolDescription::from_json_str(tool_description),
            Err(DeserializationError::UnexpectedValue(..))
        );
    }

    #[test]
    fn test_enum_non_string_errors() {
        let tool_description = r#"{
    "name": "test_tool",
    "inputSchema": {
        "type": "object",
        "properties": {
            "test_attr": {
                "type": "string",
                "enum": [true]
            }
        }
    }
}"#;
        assert_matches!(
            ToolDescription::from_json_str(tool_description),
            Err(DeserializationError::UnexpectedType(..))
        );
    }

    #[test]
    fn test_enum_non_array_errors() {
        let tool_description = r#"{
    "name": "test_tool",
    "inputSchema": {
        "type": "object",
        "properties": {
            "test_attr": {
                "type": "string",
                "enum": true
            }
        }
    }
}"#;
        assert_matches!(
            ToolDescription::from_json_str(tool_description),
            Err(DeserializationError::UnexpectedType(..))
        );
    }

    #[test]
    fn test_unrecognized_string_format_is_string() {
        let tool_description = r#"{
    "name": "test_tool",
    "inputSchema": {
        "type": "object",
        "properties": {
            "test_attr": {
                "type": "string",
                "format": "unknown"
            }
        }
    }
}"#;
        let tool = ToolDescription::from_json_str(tool_description).unwrap();
        assert_matches!(
            tool.inputs().properties().next(),
            Some(v) if matches!(v.property_type(), PropertyType::String)
        );
    }

    #[test]
    fn test_string_format_is_not_string_errors() {
        let tool_description = r#"{
    "name": "test_tool",
    "inputSchema": {
        "type": "object",
        "properties": {
            "test_attr": {
                "type": "string",
                "format": false
            }
        }
    }
}"#;
        assert_matches!(
            ToolDescription::from_json_str(tool_description),
            Err(DeserializationError::UnexpectedType(..))
        );
    }

    #[test]
    fn test_array_items_declaration_not_object_errors() {
        let tool_description = r#"{
    "name": "test_tool",
    "inputSchema": {
        "type": "object",
        "properties": {
            "test_attr": {
                "type": "array",
                "items": false
            }
        }
    }
}"#;
        assert_matches!(
            ToolDescription::from_json_str(tool_description),
            Err(DeserializationError::UnexpectedType(..))
        );
    }

    #[test]
    fn test_array_items_declaration_missing_is_error() {
        let tool_description = r#"{
    "name": "test_tool",
    "inputSchema": {
        "type": "object",
        "properties": {
            "test_attr": {
                "type": "array"
            }
        }
    }
}"#;
        assert_matches!(
            ToolDescription::from_json_str(tool_description),
            Err(DeserializationError::MissingExpectedAttribute(..))
        );
    }

    #[test]
    fn test_unrecognized_type_name_errors() {
        let tool_description = r#"{
    "name": "test_tool",
    "inputSchema": {
        "type": "object",
        "properties": {
            "test_attr": {
                "type": "some weird type name"
            }
        }
    }
}"#;
        assert_matches!(
            ToolDescription::from_json_str(tool_description),
            Err(DeserializationError::UnexpectedValue(..))
        );
    }

    #[test]
    fn test_unrecognized_base_type_in_tuple_type_errors() {
        let tool_description = r#"{
    "name": "test_tool",
    "inputSchema": {
        "type": "object",
        "properties": {
            "test_attr": {
                "type": ["null", "some weird type name"]
            }
        }
    }
}"#;
        assert_matches!(
            ToolDescription::from_json_str(tool_description),
            Err(DeserializationError::UnexpectedValue(..))
        );
    }

    #[test]
    fn test_type_not_string_or_array_of_types_errors() {
        let tool_description = r#"{
    "name": "test_tool",
    "inputSchema": {
        "type": "object",
        "properties": {
            "test_attr": {
                "type": false
            }
        }
    }
}"#;
        assert_matches!(
            ToolDescription::from_json_str(tool_description),
            Err(DeserializationError::UnexpectedType(..))
        );
    }

    #[test]
    fn test_union_type_not_array_of_types_errors() {
        let tool_description = r#"{
    "name": "test_tool",
    "inputSchema": {
        "type": "object",
        "properties": {
            "test_attr": {
                "anyOf": false
            }
        }
    }
}"#;
        assert_matches!(
            ToolDescription::from_json_str(tool_description),
            Err(DeserializationError::UnexpectedType(..))
        );
    }

    #[test]
    fn test_reftype_has_unrecognized_prefix_errors() {
        let tool_description = r#"{
    "name": "test_tool",
    "inputSchema": {
        "type": "object",
        "properties": {
            "test_attr": {
                "$ref": "does not start with required prefix"
            }
        }
    }
}"#;
        assert_matches!(
            ToolDescription::from_json_str(tool_description),
            Err(DeserializationError::UnexpectedValue(..))
        );
    }

    #[test]
    fn test_reftype_is_not_string_errors() {
        let tool_description = r#"{
    "name": "test_tool",
    "inputSchema": {
        "type": "object",
        "properties": {
            "test_attr": {
                "$ref": false
            }
        }
    }
}"#;
        assert_matches!(
            ToolDescription::from_json_str(tool_description),
            Err(DeserializationError::UnexpectedType(..))
        );
    }

    #[test]
    fn test_no_type_information_provided_errors() {
        let tool_description = r#"{
    "name": "test_tool",
    "inputSchema": {
        "type": "object",
        "properties": {
            "test_attr": {
            }
        }
    }
}"#;
        assert_matches!(
            ToolDescription::from_json_str(tool_description),
            Err(DeserializationError::MissingExpectedAttribute(..))
        );
    }

    //--------------- Test Input/Output Validation -------------------------
    #[test]
    fn test_validate_input_simple() {
        let tool_description = r#"{
    "name": "test_tool",
    "inputSchema": {
        "type": "object",
        "properties": {
            "attr": { "type": "boolean" }
        },
        "required": ["attr"]
    }
}"#;
        let tools = ServerDescription::from_json_str(tool_description).unwrap();

        let tool_input = r#"{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "tools/call",
    "params": {
        "tool": "test_tool",
        "args": {
            "attr": false
        }
    }
}"#;
        let input = Input::from_json_str(tool_input).unwrap();
        tools.validate_input(&input).unwrap()
    }

    #[test]
    fn test_validate_input_all_types() {
        let tool_description = r##"{
    "name": "test_tool",
    "inputSchema": {
        "type": "object",
        "$defs": {
            "my_bool": { "type": "boolean" }
        },
        "properties": {
            "bool_attr": { "type": "boolean" },
            "int_attr": { "type": "integer" },
            "float_attr": { "type": "float" },
            "num_attr": { "type": "number" },
            "str_attr": { "type": "string" },
            "dec_attr": { "type": "string", "format": "decimal" },
            "date_attr": { "type": "string", "format": "date" },
            "dt_attr": { "type": "string", "format": "date-time" },
            "dur_attr": { "type": "string", "format": "duration" },
            "ipv4_attr": { "type": "string", "format": "ipv4" },
            "ipv6_attr": { "type": "string", "format": "ipv6" },
            "null_attr": { "type": "null" },
            "enum_attr": { "type": "string", "enum": ["this", "that"] },
            "arr_attr": { "type": "array", "items": { "type": "integer" } },
            "tuple_attr": { "type": ["string", "boolean"] },
            "union_attr": {
                "anyOf": [
                    { "type": "string" },
                    { "type": "null" }
                ]
            },
            "ref_attr": { "$ref": "#/$defs/my_bool" }
        },
        "required": [
            "bool_attr", "int_attr", "float_attr", "num_attr",
            "str_attr", "dec_attr", "date_attr", "dt_attr",
            "dur_attr", "ipv4_attr", "ipv6_attr", "null_attr",
            "enum_attr", "arr_attr", "tuple_attr", "union_attr",
            "ref_attr"
        ]
    }
}"##;
        let tools = ServerDescription::from_json_str(tool_description).unwrap();

        let tool_input = r#"{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "tools/call",
    "params": {
        "tool": "test_tool",
        "args": {
            "bool_attr": false,
            "int_attr": 1,
            "float_attr": 1.0,
            "num_attr": 1.2e12,
            "str_attr": "my cool str",
            "dec_attr": "0.0001",
            "date_attr": "2025-11-19",
            "dt_attr": "2025-11-19T12:11:00",
            "dur_attr": "PT1D",
            "ipv4_attr": "0.0.0.0",
            "ipv6_attr": "::1",
            "null_attr": null,
            "enum_attr": "this",
            "arr_attr": [0, 1, 2],
            "tuple_attr": ["a part of a pair", true],
            "union_attr": null,
            "ref_attr": false
        }
    }
}"#;
        let input = Input::from_json_str(tool_input).unwrap();
        tools.validate_input(&input).unwrap()
    }

    #[test]
    fn test_validate_input_none_required() {
        let tool_description = r#"{
    "name": "test_tool",
    "inputSchema": {
        "type": "object",
        "properties": {
            "attr": { "type": "boolean" }
        },
        "required": []
    }
}"#;
        let tools = ServerDescription::from_json_str(tool_description).unwrap();

        let tool_input = r#"{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "tools/call",
    "params": {
        "tool": "test_tool",
        "args": {}
    }
}"#;
        let input = Input::from_json_str(tool_input).unwrap();
        tools.validate_input(&input).unwrap()
    }

    #[test]
    fn test_validate_input_tool_not_found_errors() {
        let tool_description = r#"{
    "name": "test_tool",
    "inputSchema": {
        "type": "object",
        "properties": {
            "attr": { "type": "boolean" }
        },
        "required": []
    }
}"#;
        let tools = ServerDescription::from_json_str(tool_description).unwrap();

        let tool_input = r#"{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "tools/call",
    "params": {
        "tool": "test_tool2",
        "args": {}
    }
}"#;
        let input = Input::from_json_str(tool_input).unwrap();
        assert_matches!(
            tools.validate_input(&input),
            Err(ValidationError::ToolNotFound(..))
        )
    }

    #[test]
    fn test_validate_input_wrong_name_errors() {
        let tool_description = r#"{
    "name": "test_tool",
    "inputSchema": {
        "type": "object",
        "properties": {
            "attr": { "type": "boolean" }
        },
        "required": []
    }
}"#;
        let tool = ToolDescription::from_json_str(tool_description).unwrap();

        let tool_input = r#"{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "tools/call",
    "params": {
        "tool": "test_tool2",
        "args": {}
    }
}"#;
        let input = Input::from_json_str(tool_input).unwrap();
        assert_matches!(
            tool.validate_input(&input, &HashMap::new()),
            Err(ValidationError::MismatchedToolNames(..))
        )
    }

    #[test]
    fn test_validate_output_simple() {
        let tool_description = r#"{
    "name": "test_tool",
    "inputSchema": {
        "type": "object",
        "properties": {}
    },
    "outputSchema": {
        "type": "object",
        "properties": {
            "attr": { "type": "boolean" }
        },
        "required": ["attr"]
    }
}"#;
        let tools = ServerDescription::from_json_str(tool_description).unwrap();

        let tool_output = r#"{
    "jsonrpc": "2.0",
    "id": 1,
    "result": {
        "structuredContent": {
            "attr": false
        }
    }
}"#;
        let output = Output::from_json_str(tool_output).unwrap();
        tools.validate_output("test_tool", &output).unwrap()
    }

    #[test]
    fn test_validate_output_all_types() {
        let tool_description = r##"{
    "name": "test_tool",
    "inputSchema": {
        "type": "object",
        "properties": {}
    },
    "outputSchema": {
        "type": "object",
        "$defs": {
            "my_bool": { "type": "boolean" }
        },
        "properties": {
            "bool_attr": { "type": "boolean" },
            "int_attr": { "type": "integer" },
            "float_attr": { "type": "float" },
            "num_attr": { "type": "number" },
            "str_attr": { "type": "string" },
            "dec_attr": { "type": "string", "format": "decimal" },
            "date_attr": { "type": "string", "format": "date" },
            "dt_attr": { "type": "string", "format": "date-time" },
            "dur_attr": { "type": "string", "format": "duration" },
            "ipv4_attr": { "type": "string", "format": "ipv4" },
            "ipv6_attr": { "type": "string", "format": "ipv6" },
            "null_attr": { "type": "null" },
            "enum_attr": { "type": "string", "enum": ["this", "that"] },
            "arr_attr": { "type": "array", "items": { "type": "integer" } },
            "tuple_attr": { "type": ["string", "boolean"] },
            "union_attr": {
                "anyOf": [
                    { "type": "string" },
                    { "type": "null" }
                ]
            },
            "ref_attr": { "$ref": "#/$defs/my_bool" }
        },
        "required": [
            "bool_attr", "int_attr", "float_attr", "num_attr",
            "str_attr", "dec_attr", "date_attr", "dt_attr",
            "dur_attr", "ipv4_attr", "ipv6_attr", "null_attr",
            "enum_attr", "arr_attr", "tuple_attr", "union_attr",
            "ref_attr"
        ]
    }
}"##;
        let tools = ServerDescription::from_json_str(tool_description).unwrap();

        let tool_output = r#"{
    "jsonrpc": "2.0",
    "id": 1,
    "result": {
        "structuredContent": {
            "bool_attr": false,
            "int_attr": 1,
            "float_attr": 1.0,
            "num_attr": 1.2e12,
            "str_attr": "my cool str",
            "dec_attr": "0.0001",
            "date_attr": "2025-11-19",
            "dt_attr": "2025-11-19T12:11:00",
            "dur_attr": "PT1D",
            "ipv4_attr": "0.0.0.0",
            "ipv6_attr": "::1",
            "null_attr": null,
            "enum_attr": "this",
            "arr_attr": [0, 1, 2],
            "tuple_attr": ["a part of a pair", true],
            "union_attr": null,
            "ref_attr": false
        }
    }
}"#;
        let output = Output::from_json_str(tool_output).unwrap();
        tools.validate_output("test_tool", &output).unwrap()
    }

    #[test]
    fn test_validate_ouptut_none_required() {
        let tool_description = r#"{
    "name": "test_tool",
    "inputSchema": {
        "type": "object",
        "properties": {}
    },
    "outputSchema": {
        "type": "object",
        "properties": {
            "attr": { "type": "boolean" }
        }
    }
}"#;
        let tools = ServerDescription::from_json_str(tool_description).unwrap();

        let tool_output = r#"{
    "jsonrpc": "2.0",
    "id": 1,
    "result": {
        "structuredContent": {}
    }
}"#;
        let output = Output::from_json_str(tool_output).unwrap();
        tools.validate_output("test_tool", &output).unwrap()
    }

    #[test]
    fn test_validate_output_tool_not_found_errors() {
        let tool_description = r#"{
    "name": "test_tool",
    "inputSchema": {
        "type": "object",
        "properties": {}
    },
    "outputSchema": {
        "type": "object",
        "properties": {
            "attr": { "type": "boolean" }
        }
    }
}"#;
        let tools = ServerDescription::from_json_str(tool_description).unwrap();

        let tool_output = r#"{
    "jsonrpc": "2.0",
    "id": 1,
    "result": {
        "structuredContent": {}
    }
}"#;
        let output = Output::from_json_str(tool_output).unwrap();
        assert_matches!(
            tools.validate_output("test_tool2", &output),
            Err(ValidationError::ToolNotFound(..))
        )
    }
}
