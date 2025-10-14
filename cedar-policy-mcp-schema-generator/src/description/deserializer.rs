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

use linked_hash_map::LinkedHashMap;
use smol_str::SmolStr;

use super::err::{ContentType, DeserializationError};
use super::parser::json_value::LocatedValue;
use crate::description::parser::json_value::LocatedString;
use crate::description::{
    Parameters, Property, PropertyType, PropertyTypeDef, ServerDescription, ToolDescription,
};

use std::collections::{HashMap, HashSet};

pub(crate) fn server_description_from_json_value(
    json_value: LocatedValue,
) -> Result<ServerDescription, DeserializationError> {
    if json_value.is_object() {
        match json_value.get("result") {
            Some(result) => match result.get("tools") {
                Some(tools_json) => {
                    if let Some(tools) = tools_json.get_array() {
                        Ok(ServerDescription::new(
                            tools
                                .iter()
                                .map(|tool_json| tool_description_from_json_value_inner(tool_json))
                                .collect::<Result<_, _>>()?,
                            typedefs_from_json_value(
                                result.get("$defs"),
                                ContentType::ToolParameters,
                            )?,
                        ))
                    } else {
                        Err(DeserializationError::unexpected_type(
                            tools_json,
                            "Expected `tools` attribute of MCP tool_list response to be an array of MCP tool descriptions.",
                            ContentType::ServerDescription
                        ))
                    }
                }
                None => Err(DeserializationError::missing_attribute(
                    result,
                    "tools",
                    Vec::new(),
                )),
            },
            None => Ok(ServerDescription::new(
                vec![tool_description_from_json_value_inner(&json_value)?],
                HashMap::new(),
            )),
        }
    } else if let Some(tools) = json_value.get_array() {
        Ok(ServerDescription::new(
            tools
                .iter()
                .map(|tool_json| tool_description_from_json_value_inner(tool_json))
                .collect::<Result<_, _>>()?,
            HashMap::new(),
        ))
    } else {
        Err(DeserializationError::unexpected_type(
            &json_value,
            "Expected either a JSON object containing an MCP list_tool response, a JSON array of tool descriptions, or a JSON object describing a single MCP tool.",
            ContentType::ServerDescription
        ))
    }
}

pub(crate) fn tool_description_from_json_value(
    json_value: LocatedValue,
) -> Result<ToolDescription, DeserializationError> {
    // Public interface consumes the value, but ref is sufficient for deserialization
    tool_description_from_json_value_inner(&json_value)
}

fn tool_description_from_json_value_inner(
    json_value: &LocatedValue,
) -> Result<ToolDescription, DeserializationError> {
    if let Some(tool_obj) = json_value.get_object() {
        let name = match tool_obj.get("name") {
            Some(s) => {
                if let Some(s) = s.get_smolstr() {
                    Ok(s)
                } else {
                    Err(DeserializationError::unexpected_type(
                        s,
                        "Expected `name` attribute of a MCP tool description to be a String.",
                        ContentType::ToolDescription,
                    ))
                }
            }
            None => Err(DeserializationError::missing_attribute(
                json_value,
                "name",
                Vec::new(),
            )),
        }?;
        let inputs = if let Some(inputs_json) =
            get_value_from_map(tool_obj, &["parameters", "inputSchema"])
        {
            parameters_from_json_value(inputs_json)
        } else {
            Err(DeserializationError::missing_attribute(
                json_value,
                "parameters",
                vec!["inputSchema".to_string()],
            ))
        }?;
        let outputs = if let Some(outputs_json) = tool_obj.get("outputSchema") {
            parameters_from_json_value(outputs_json)?
        } else {
            Parameters::new(Vec::new(), HashMap::new())
        };
        let type_defs =
            typedefs_from_json_value(tool_obj.get("$defs"), ContentType::ToolParameters)?;
        let description = if let Some(desc_json) = tool_obj.get("description") {
            if let Some(s) = desc_json.get_string() {
                Ok(Some(s))
            } else {
                Err(DeserializationError::unexpected_type(
                    desc_json,
                    "Expected `description` attribute of a MCP Tool Description to be a string.",
                    ContentType::ToolDescription,
                ))
            }
        } else {
            Ok(None)
        }?;
        Ok(ToolDescription::new(
            name,
            inputs,
            outputs,
            type_defs,
            description,
        ))
    } else {
        Err(DeserializationError::unexpected_type(
            json_value,
            "Expected a JSON object containing an MCP tool description.",
            ContentType::ToolDescription,
        ))
    }
}

fn parameters_from_json_value(
    json_value: &LocatedValue,
) -> Result<Parameters, DeserializationError> {
    // Unwrap "json" wrapper it exists
    let json_value = if let Some(json_value) = json_value.get("json") {
        json_value
    } else {
        json_value
    };
    if let Some(params_obj) = json_value.get_object() {
        let type_defs =
            typedefs_from_json_value(params_obj.get("$defs"), ContentType::ToolParameters)?;
        let required =
            required_from_json_value(params_obj.get("required"), ContentType::ToolParameters)?;
        let properties = properties_from_json_value(
            params_obj.get("properties"),
            &required,
            ContentType::ToolParameters,
        )?;
        Ok(Parameters::new(properties, type_defs))
    } else {
        Err(DeserializationError::unexpected_type(
            json_value,
            "Expected Input/Output schema of a MCP Tool Description to be a JSON object.",
            ContentType::ToolParameters,
        ))
    }
}

fn typedefs_from_json_value(
    json_value: Option<&LocatedValue>,
    content_type: ContentType,
) -> Result<HashMap<SmolStr, PropertyTypeDef>, DeserializationError> {
    match json_value {
        None => Ok(HashMap::new()),
        Some(json_value) => {
            if let Some(defs) = json_value.get_object() {
                defs.iter()
                    .map(|(name, val)| {
                        let name = name.to_smolstr();
                        let description = val.get("description").and_then(|desc| desc.get_string());
                        property_type_from_json_value(val).map(|ptype| {
                            (name.clone(), PropertyTypeDef::new(name, ptype, description))
                        })
                    })
                    .collect::<Result<_, _>>()
            } else {
                Err(DeserializationError::unexpected_type(
                    json_value,
                    "Expected attribute `$defs` to be a JSON object mapping type names to JSON Type Schemas.",
                    content_type
                ))
            }
        }
    }
}

fn required_from_json_value(
    json_value: Option<&LocatedValue>,
    content_type: ContentType,
) -> Result<HashSet<SmolStr>, DeserializationError> {
    match json_value {
        Some(json_value) => {
            if let Some(reqs) = json_value.get_array() {
                reqs.iter()
                    .map(|req| {
                        req.get_smolstr().ok_or_else(|| {
                            DeserializationError::unexpected_type(
                                req,
                                "Expected element of `required` to be a string.",
                                content_type,
                            )
                        })
                    })
                    .collect::<Result<_, _>>()
            } else if let Some(false) = json_value.get_bool() {
                Ok(HashSet::new())
            } else {
                Err(DeserializationError::unexpected_type(
                    json_value,
                    "Expected `required` attribute to be a JSON array of strings.",
                    content_type,
                ))
            }
        }
        None => Ok(HashSet::new()),
    }
}

fn properties_from_json_value(
    json_value: Option<&LocatedValue>,
    required: &HashSet<SmolStr>,
    content_type: ContentType,
) -> Result<Vec<Property>, DeserializationError> {
    match json_value {
        Some(json_value) => {
            if let Some(props_obj) = json_value.get_object() {
                props_obj
                    .iter()
                    .map(|(name, ptype_json)| {
                        let name = name.to_smolstr();
                        let required = required.contains(&name);
                        property_from_json_value(ptype_json, name, required)
                    })
                    .collect::<Result<_, _>>()
            } else if let Some(false) = json_value.get_bool() {
                Ok(Vec::new())
            } else {
                Err(DeserializationError::unexpected_type(
                    json_value,
                    "Expected `properties` attribute to be a JSON object.",
                    content_type,
                ))
            }
        }
        None => Ok(Vec::new()),
    }
}

fn property_from_json_value(
    json_value: &LocatedValue,
    name: SmolStr,
    required: bool,
) -> Result<Property, DeserializationError> {
    let description = json_value
        .get("description")
        .map(|desc_json| {
            desc_json.get_string().ok_or_else(|| {
                DeserializationError::unexpected_type(
                    desc_json,
                    "Expected `description` attribute to be a string.",
                    ContentType::Property,
                )
            })
        })
        .transpose()?;
    Ok(Property::new(
        name,
        required,
        property_type_from_json_value(json_value)?,
        description,
    ))
}

fn property_type_from_json_value(
    json_value: &LocatedValue,
) -> Result<PropertyType, DeserializationError> {
    if let Some(ptype_obj) = json_value.get_object() {
        if let Some(type_json) = ptype_obj.get("type") {
            match type_json.get_str() {
                Some("boolean") => Ok(PropertyType::Bool),
                Some("integer") => Ok(PropertyType::Integer),
                Some("float") => Ok(PropertyType::Float),
                Some("number") => Ok(PropertyType::Number),
                Some("string") => {
                    if let Some(enum_json) = ptype_obj.get("enum") {
                        if let Some(enum_variants) = enum_json.get_array() {
                            let variants = enum_variants.iter().map(|variant| {
                                variant.get_smolstr().ok_or_else(|| DeserializationError::unexpected_type(
                                    variant,
                                    "Expected element of `enum` attribute to be a string.",
                                    ContentType::PropertyType
                                ))
                            }).collect::<Result<Vec<_>,_>>()?;
                            if variants.len() == 0 {
                                Err(DeserializationError::unexpected_value(
                                    enum_json,
                                    "Expected non-empty list of variants for `enum` attribute.",
                                    ContentType::PropertyType
                                ))
                            } else {
                                Ok(PropertyType::Enum { variants })
                            }
                        } else {
                            Err(DeserializationError::unexpected_type(
                                enum_json,
                                "Expected `enum` attributed to be a JSON array of strings.",
                                ContentType::PropertyType
                            ))
                        }
                    } else if let Some(format_json) = ptype_obj.get("format") {
                        match format_json.get_str() {
                            Some("date") => Ok(PropertyType::Datetime),
                            Some("date-time") => Ok(PropertyType::Datetime),
                            Some("duration") => Ok(PropertyType::Duration),
                            Some("ipv4") => Ok(PropertyType::IpAddr),
                            Some("ipv6") => Ok(PropertyType::IpAddr),
                            Some("decimal") => Ok(PropertyType::Decimal),
                            Some(_) => Ok(PropertyType::String),
                            None => Err(DeserializationError::unexpected_type(
                                format_json,
                                "Expected `format` attribute to be a string.",
                                ContentType::PropertyType
                            ))
                        }
                    } else {
                        Ok(PropertyType::String)
                    }
                }
                Some("null") => Ok(PropertyType::Null),
                Some("array") => {
                    if let Some(items_json) = ptype_obj.get("items") {
                        if items_json.is_object() {
                            let items_type = property_type_from_json_value(items_json)?;
                            Ok(PropertyType::Array { element_ty: Box::new(items_type) })
                        } else {
                            Err(DeserializationError::unexpected_type(
                                items_json,
                                "Expected `items` attribute to be a JSON Schema (object) describing the type of array items.",
                                ContentType::PropertyType
                            ))
                        }
                    } else {
                        Err(DeserializationError::missing_attribute(
                            json_value,
                            "items",
                            Vec::new()
                        ))
                    }
                }
                Some("object") => {
                    let required = required_from_json_value(ptype_obj.get("required"), ContentType::ToolParameters)?;
                    let properties = properties_from_json_value(ptype_obj.get("properties"), &required, ContentType::ToolParameters)?;
                    let additional_properties = ptype_obj.get("additionalProperties").and_then(|json| {
                        property_type_from_json_value(json).ok()
                    }).map(Box::new);
                    Ok(PropertyType::Object { properties, additional_properties })
                }
                Some(_) => Err(DeserializationError::unexpected_value(
                    type_json,
                    "Expected one of: `boolean`, `integer`, `float`, `number`, `string`, `null`, `array`, `object`.",
                    ContentType::PropertyType
                )),
                None => {
                    if let Some(types_json) = type_json.get_array() {
                        let types = types_json
                            .iter()
                            .map(|ty_json| {
                                match ty_json.get_str() {
                                    Some("boolean") => Ok(PropertyType::Bool),
                                    Some("integer") => Ok(PropertyType::Integer),
                                    Some("float") => Ok(PropertyType::Float),
                                    Some("number") => Ok(PropertyType::Number),
                                    Some("string") => Ok(PropertyType::String),
                                    Some("null") => Ok(PropertyType::Null),
                                    Some(_) => Err(DeserializationError::unexpected_value(
                                        ty_json,
                                        "Expected one of `boolean`, `integer`, `float`, `number`, `string`, `null`.",
                                        ContentType::PropertyType
                                    )),
                                    None => property_type_from_json_value(ty_json)
                                }
                            })
                            .collect::<Result<_,_>>()?;
                        Ok(PropertyType::Tuple { types })
                    } else {
                        Err(DeserializationError::unexpected_type(
                            type_json,
                            "Expected `type` attribute to be a string.",
                            ContentType::PropertyType
                        ))
                    }
                }
            }
        } else if let Some(union_json) = get_value_from_map(ptype_obj, &["anyOf", "oneOf"]) {
            if let Some(type_arr) = union_json.get_array() {
                let types = type_arr
                    .iter()
                    .map(|type_json| property_type_from_json_value(type_json))
                    .collect::<Result<_, _>>()?;
                Ok(PropertyType::Union { types })
            } else {
                Err(DeserializationError::unexpected_type(
                    json_value,
                    "Expected `anyOf` or `oneOf` attribute to be an array of JSON Schemas.",
                    ContentType::PropertyType,
                ))
            }
        } else if let Some(ref_json) = ptype_obj.get("$ref") {
            if let Some(s) = ref_json.get_str() {
                if let Some(s) = s.strip_prefix("#/$defs/") {
                    Ok(PropertyType::Ref { name: s.into() })
                } else {
                    Err(DeserializationError::unexpected_value(
                        ref_json,
                        "Expected `$ref` attribute to begin with `#/$defs/`",
                        ContentType::Property,
                    ))
                }
            } else {
                Err(DeserializationError::unexpected_type(
                    ref_json,
                    "Expected `$ref` attribute to be a string.",
                    ContentType::PropertyType,
                ))
            }
        } else {
            Err(DeserializationError::missing_attribute(
                json_value,
                "type",
                vec!["anyOf".to_string(), "oneOf".to_string(), "$ref".to_string()],
            ))
        }
    } else {
        Err(DeserializationError::unexpected_type(
            json_value,
            "Expected Property Type Schema to be a JSON object.",
            ContentType::Property,
        ))
    }
}

fn get_value_from_map<'a, T: AsRef<str>>(
    map: &'a LinkedHashMap<LocatedString, LocatedValue>,
    key_aliases: &[T],
) -> Option<&'a LocatedValue> {
    key_aliases.iter().find_map(|key| map.get(key.as_ref()))
}
