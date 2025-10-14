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

#[derive(Debug, Clone)]
pub struct PropertyTypeDef {
    pub(crate) name: SmolStr,
    pub(crate) prop_type: PropertyType,
    pub(crate) description: Option<String>,
}

impl PropertyTypeDef {
    pub fn new(name: SmolStr, prop_type: PropertyType, description: Option<String>) -> Self {
        Self {
            name,
            prop_type,
            description,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn property_type(&self) -> &PropertyType {
        &self.prop_type
    }

    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }
}

#[derive(Debug, Clone)]
pub(crate) struct PropertyTypeDefs {
    type_defs: HashMap<SmolStr, PropertyTypeDef>,
}

impl PropertyTypeDefs {
    pub(crate) fn new(type_defs: HashMap<SmolStr, PropertyTypeDef>) -> Self {
        Self { type_defs }
    }

    pub(crate) fn values(&self) -> impl Iterator<Item = &PropertyTypeDef> {
        self.type_defs.values()
    }
}

#[derive(Debug, Clone)]
pub struct Parameters {
    pub(crate) properties: Vec<Property>,
    pub(crate) type_defs: PropertyTypeDefs,
}

impl Parameters {
    pub fn new(properties: Vec<Property>, type_defs: HashMap<SmolStr, PropertyTypeDef>) -> Self {
        Self {
            properties,
            type_defs: PropertyTypeDefs::new(type_defs),
        }
    }

    pub fn properties(&self) -> impl Iterator<Item = &Property> {
        self.properties.iter()
    }

    pub fn type_definitions(&self) -> impl Iterator<Item = &PropertyTypeDef> {
        self.type_defs.values()
    }
}

#[derive(Debug, Clone)]
pub struct ToolDescription {
    pub(crate) name: SmolStr,
    pub(crate) description: Option<String>,
    pub(crate) inputs: Parameters,
    pub(crate) outputs: Parameters,
    pub(crate) type_defs: PropertyTypeDefs,
}

impl ToolDescription {
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

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    pub fn inputs(&self) -> &Parameters {
        &self.inputs
    }

    pub fn outputs(&self) -> &Parameters {
        &self.outputs
    }

    pub fn type_definitions(&self) -> impl Iterator<Item = &PropertyTypeDef> {
        self.type_defs.values()
    }

    pub fn from_json_str(json_str: &str) -> Result<Self, DeserializationError> {
        let mut parser = parser::json_parser::JsonParser::new(json_str);
        deserializer::tool_description_from_json_value(parser.get_value()?)
    }

    pub fn from_json_file<P: AsRef<Path>>(json_file: P) -> Result<Self, DeserializationError> {
        let contents = std::fs::read_to_string(json_file.as_ref()).map_err(|e| {
            DeserializationError::ReadError {
                file_name: json_file.as_ref().into(),
                error: format!("{e}"),
            }
        })?;
        Self::from_json_str(&contents)
    }
}

#[derive(Debug, Clone)]
pub struct ServerDescription {
    pub(crate) tools: Vec<ToolDescription>,
    pub(crate) type_defs: PropertyTypeDefs,
}

impl ServerDescription {
    pub fn new(tools: Vec<ToolDescription>, type_defs: HashMap<SmolStr, PropertyTypeDef>) -> Self {
        Self {
            tools,
            type_defs: PropertyTypeDefs::new(type_defs),
        }
    }

    pub fn tool_descriptions(&self) -> impl Iterator<Item = &ToolDescription> {
        self.tools.iter()
    }

    pub fn type_definitions(&self) -> impl Iterator<Item = &PropertyTypeDef> {
        self.type_defs.values()
    }

    pub fn from_json_str(json_str: &str) -> Result<Self, DeserializationError> {
        let mut parser = parser::json_parser::JsonParser::new(json_str);
        deserializer::server_description_from_json_value(parser.get_value()?)
    }

    pub fn from_json_file<P: AsRef<Path>>(json_file: P) -> Result<Self, DeserializationError> {
        let contents = std::fs::read_to_string(json_file.as_ref()).map_err(|e| {
            DeserializationError::ReadError {
                file_name: json_file.as_ref().into(),
                error: format!("{e}"),
            }
        })?;
        Self::from_json_str(&contents)
    }
}
