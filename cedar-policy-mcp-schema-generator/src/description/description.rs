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

use smol_str::SmolStr;
use std::collections::HashMap;
use std::path::Path;

use super::deserializer;
use super::err::DeserializationError;
use super::parser;

/// The type a `Property` Can take
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

/// Representation of an input/output Property
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

/// Representation of a TypeDef used for defining properties
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
    type_defs: HashMap<SmolStr, PropertyTypeDef>,
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
}

/// A representation of a collection of MCP Tools (e.g., all the tools provided by an MCP Server)
#[derive(Debug, Clone)]
pub struct ServerDescription {
    pub(crate) tools: Vec<ToolDescription>,
    pub(crate) type_defs: PropertyTypeDefs,
}

impl ServerDescription {
    /// Create a new Server Description from its components
    pub fn new(tools: Vec<ToolDescription>, type_defs: HashMap<SmolStr, PropertyTypeDef>) -> Self {
        Self {
            tools,
            type_defs: PropertyTypeDefs::new(type_defs),
        }
    }

    /// Get an iterator to all tool descriptions within this `ServerDescription`
    pub fn tool_descriptions(&self) -> impl Iterator<Item = &ToolDescription> {
        self.tools.iter()
    }

    /// Get any TypeDefs defined within this ServerDescription (i.e., TypeDefs shared between tools)
    pub fn type_definitions(&self) -> impl Iterator<Item = &PropertyTypeDef> {
        self.type_defs.values()
    }

    /// Deserialize an MCP `list_tool` json response (or JSON Array of Tool Descriptions) into a `ServerDescription`
    pub fn from_json_str(json_str: &str) -> Result<Self, DeserializationError> {
        let mut parser = parser::json_parser::JsonParser::new(json_str);
        deserializer::server_description_from_json_value(parser.get_value()?)
    }

    /// Deserialize an MCP `list_tool` json response (or JSON Array of Tool Descriptions) into a `ServerDescription`
    pub fn from_json_file<P: AsRef<Path>>(json_file: P) -> Result<Self, DeserializationError> {
        let contents = std::fs::read_to_string(json_file.as_ref()).map_err(|e| {
            DeserializationError::read_error(json_file.as_ref().into(), format!("{e}"))
        })?;
        Self::from_json_str(&contents)
    }
}
