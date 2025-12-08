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

use super::data::{Input, Output};
use super::description::{
    Parameters, Property, PropertyType, PropertyTypeDef, ServerDescription, ToolDescription,
};
use super::err::{ContentType, DeserializationError};
use super::parser::json_value::{LocatedString, LocatedValue};

use linked_hash_map::LinkedHashMap;
use smol_str::SmolStr;

use std::collections::{HashMap, HashSet};

/// Deserialize an MCP `tools/list` json response into a `ServerDescription`
#[allow(clippy::needless_pass_by_value, reason = "Better interface")]
pub(crate) fn server_description_from_json_value(
    json_value: LocatedValue,
) -> Result<ServerDescription, DeserializationError> {
    if json_value.is_object() {
        match json_value.get("result") {
            Some(result) => {
                let tools_json = result.get("tools").ok_or_else(|| {
                    DeserializationError::missing_attribute(result, "tools", Vec::new())
                })?;
                let tools = tools_json.get_array().ok_or_else(|| DeserializationError::unexpected_type(
                    tools_json,
                    "Expected `tools` attribute of MCP tool_list response to be an array of MCP tool descriptions.",
                    ContentType::ServerDescription
                ))?;
                Ok(ServerDescription::new(
                    tools
                        .iter()
                        .map(tool_description_from_json_value_inner)
                        .collect::<Result<Vec<_>, _>>()?
                        .into_iter(),
                    typedefs_from_json_value(result.get("$defs"), ContentType::ToolParameters)?,
                ))
            }
            None => Ok(ServerDescription::new(
                std::iter::once(tool_description_from_json_value_inner(&json_value)?),
                HashMap::new(),
            )),
        }
    } else {
        let tools = json_value.get_array().ok_or_else(|| DeserializationError::unexpected_type(
            &json_value,
            "Expected either a JSON object containing an MCP list_tool response, a JSON array of tool descriptions, or a JSON object describing a single MCP tool.",
            ContentType::ServerDescription
        ))?;
        Ok(ServerDescription::new(
            tools
                .iter()
                .map(tool_description_from_json_value_inner)
                .collect::<Result<Vec<_>, _>>()?
                .into_iter(),
            HashMap::new(),
        ))
    }
}

/// Deserialize an MCP Tool Description json into a `ToolDescription`
#[allow(clippy::needless_pass_by_value, reason = "Better interface")]
pub(crate) fn tool_description_from_json_value(
    json_value: LocatedValue,
) -> Result<ToolDescription, DeserializationError> {
    // Public interface consumes the value, but ref is sufficient for deserialization
    tool_description_from_json_value_inner(&json_value)
}

fn tool_description_from_json_value_inner(
    json_value: &LocatedValue,
) -> Result<ToolDescription, DeserializationError> {
    let tool_obj = json_value.get_object().ok_or_else(|| {
        DeserializationError::unexpected_type(
            json_value,
            "Expected a JSON object containing an MCP tool description.",
            ContentType::ToolDescription,
        )
    })?;
    let name = tool_obj
        .get("name")
        .ok_or_else(|| DeserializationError::missing_attribute(json_value, "name", Vec::new()))?;
    let name = name.get_smolstr().ok_or_else(|| {
        DeserializationError::unexpected_type(
            name,
            "Expected `name` attribute of a MCP tool description to be a String.",
            ContentType::ToolDescription,
        )
    })?;
    let inputs = get_value_from_map(tool_obj, &["parameters", "inputSchema"]).ok_or_else(|| {
        DeserializationError::missing_attribute(
            json_value,
            "parameters",
            vec!["inputSchema".to_string()],
        )
    })?;
    let inputs = parameters_from_json_value(inputs)?;
    let outputs = tool_obj
        .get("outputSchema")
        .map(parameters_from_json_value)
        .transpose()?
        .unwrap_or_else(|| Parameters::new(Vec::new(), HashMap::new()));
    let type_defs = typedefs_from_json_value(tool_obj.get("$defs"), ContentType::ToolParameters)?;
    let description = tool_obj
        .get("description")
        .map(|json| {
            json.get_string().ok_or_else(|| {
                DeserializationError::unexpected_type(
                    json,
                    "Expected `description` attribute of a MCP Tool Description to be a string.",
                    ContentType::ToolDescription,
                )
            })
        })
        .transpose()?;
    Ok(ToolDescription::new(
        name,
        inputs,
        outputs,
        type_defs,
        description,
    ))
}

fn parameters_from_json_value(
    json_value: &LocatedValue,
) -> Result<Parameters, DeserializationError> {
    // Unwrap "json" wrapper it exists
    let json_value = json_value.get("json").unwrap_or(json_value);
    let params_obj = json_value.get_object().ok_or_else(|| {
        DeserializationError::unexpected_type(
            json_value,
            "Expected Input/Output schema of a MCP Tool Description to be a JSON object.",
            ContentType::ToolParameters,
        )
    })?;
    let type_defs = typedefs_from_json_value(params_obj.get("$defs"), ContentType::ToolParameters)?;
    let required =
        required_from_json_value(params_obj.get("required"), ContentType::ToolParameters)?;
    let properties = properties_from_json_value(
        params_obj.get("properties"),
        &required,
        ContentType::ToolParameters,
    )?;
    Ok(Parameters::new(properties, type_defs))
}

fn typedefs_from_json_value(
    json_value: Option<&LocatedValue>,
    content_type: ContentType,
) -> Result<HashMap<SmolStr, PropertyTypeDef>, DeserializationError> {
    let type_defs = json_value.map(|json_value| {
        let defs = json_value.get_object().ok_or_else(|| DeserializationError::unexpected_type(
            json_value,
            "Expected attribute `$defs` to be a JSON object mapping type names to JSON Type Schemas.",
            content_type
        ))?;
        defs.iter()
            .map(|(name, val)| {
                let name = name.to_smolstr();
                let description = val.get("description").and_then(|desc| desc.get_string());
                property_type_from_json_value(val).map(|ptype| {
                    (name.clone(), PropertyTypeDef::new(name, ptype, description))
                })
            })
            .collect::<Result<_, _>>()
    }).unwrap_or_else(|| Ok(HashMap::new()))?;
    typedefs_are_well_founded(&type_defs)?;
    Ok(type_defs)
}

fn required_from_json_value(
    json_value: Option<&LocatedValue>,
    content_type: ContentType,
) -> Result<HashSet<SmolStr>, DeserializationError> {
    json_value
        .map(|json_value| {
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
            } else if json_value.get_bool() == Some(false) {
                Ok(HashSet::new())
            } else {
                Err(DeserializationError::unexpected_type(
                    json_value,
                    "Expected `required` attribute to be a JSON array of strings.",
                    content_type,
                ))
            }
        })
        .unwrap_or_else(|| Ok(HashSet::new()))
}

fn properties_from_json_value(
    json_value: Option<&LocatedValue>,
    required: &HashSet<SmolStr>,
    content_type: ContentType,
) -> Result<Vec<Property>, DeserializationError> {
    json_value
        .map(|json_value| {
            if let Some(props_obj) = json_value.get_object() {
                props_obj
                    .iter()
                    .map(|(name, ptype_json)| {
                        let name = name.to_smolstr();
                        let required = required.contains(&name);
                        property_from_json_value(ptype_json, name, required)
                    })
                    .collect::<Result<_, _>>()
            } else if json_value.get_bool() == Some(false) {
                Ok(Vec::new())
            } else {
                Err(DeserializationError::unexpected_type(
                    json_value,
                    "Expected `properties` attribute to be a JSON object.",
                    content_type,
                ))
            }
        })
        .unwrap_or_else(|| Ok(Vec::new()))
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
    let ptype_obj = json_value.get_object().ok_or_else(|| {
        DeserializationError::unexpected_type(
            json_value,
            "Expected Property Type Schema to be a JSON object.",
            ContentType::Property,
        )
    })?;
    if let Some(type_json) = ptype_obj.get("type") {
        match type_json.get_str() {
            Some("boolean") => Ok(PropertyType::Bool),
            Some("integer") => Ok(PropertyType::Integer),
            Some("float") => Ok(PropertyType::Float),
            Some("number") => Ok(PropertyType::Number),
            Some("string") => {
                if let Some(enum_json) = ptype_obj.get("enum") {
                    let enum_variants = enum_json.get_array().ok_or_else(|| DeserializationError::unexpected_type(
                        enum_json,
                        "Expected `enum` attributed to be a JSON array of strings.",
                        ContentType::PropertyType
                    ))?;
                    let variants = enum_variants.iter().map(|variant| {
                        variant.get_smolstr().ok_or_else(|| DeserializationError::unexpected_type(
                            variant,
                            "Expected element of `enum` attribute to be a string.",
                            ContentType::PropertyType
                        ))
                    }).collect::<Result<Vec<_>,_>>()?;
                    if variants.is_empty() {
                        Err(DeserializationError::unexpected_value(
                            enum_json,
                            "Expected non-empty list of variants for `enum` attribute.",
                            ContentType::PropertyType
                        ))
                    } else {
                        Ok(PropertyType::Enum { variants })
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
                ptype_obj.get("items").map(|items_json| {
                    if items_json.is_object() {
                        let items_type = property_type_from_json_value(items_json)?;
                        Ok(PropertyType::Array { element_ty: Box::new(items_type) })
                    } else if items_json.is_bool() || items_json.is_null() {
                        Ok(PropertyType::Array { element_ty: Box::new(PropertyType::Unknown) })
                    } else {
                        Err(DeserializationError::unexpected_type(
                            items_json,
                            "Expected `items` attribute to be a JSON Schema (object) describing the type of array items.",
                            ContentType::PropertyType
                        ))
                    }
                }).unwrap_or_else(|| Ok(PropertyType::Array { element_ty: Box::new(PropertyType::Unknown) }))
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
                } else if type_json.is_bool() || type_json.is_null() {
                    Ok(PropertyType::Unknown)
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
        let typ_arr = union_json.get_array().ok_or_else(|| {
            DeserializationError::unexpected_type(
                json_value,
                "Expected `anyOf` or `oneOf` attribute to be an array of JSON Schemas.",
                ContentType::PropertyType,
            )
        })?;
        let types = typ_arr
            .iter()
            .map(property_type_from_json_value)
            .collect::<Result<_, _>>()?;
        Ok(PropertyType::Union { types })
    } else if let Some(ref_json) = ptype_obj.get("$ref") {
        let s = ref_json.get_str().ok_or_else(|| {
            DeserializationError::unexpected_type(
                ref_json,
                "Expected `$ref` attribute to be a string.",
                ContentType::PropertyType,
            )
        })?;
        let s = s.strip_prefix("#/$defs/").ok_or_else(|| {
            DeserializationError::unexpected_value(
                ref_json,
                "Expected `$ref` attribute to begin with `#/$defs/`",
                ContentType::Property,
            )
        })?;
        Ok(PropertyType::Ref { name: s.into() })
    } else {
        Ok(PropertyType::Unknown)
    }
}

fn get_value_from_map<'a, T: AsRef<str>>(
    map: &'a LinkedHashMap<LocatedString, LocatedValue>,
    key_aliases: &[T],
) -> Option<&'a LocatedValue> {
    key_aliases.iter().find_map(|key| map.get(key.as_ref()))
}

/// Deserialize an MCP `tools/call` json request into an `Input`
pub(crate) fn mcp_tool_input_from_json_value(
    json_value: &LocatedValue,
) -> Result<Input, DeserializationError> {
    let obj = json_value.get_object().ok_or_else(|| {
        DeserializationError::unexpected_type(
            json_value,
            "MCP `tools/call` request should be an object",
            ContentType::ToolInputRequest,
        )
    })?;
    let params = obj
        .get("params")
        .ok_or_else(|| DeserializationError::missing_attribute(json_value, "params", vec![]))?;
    let params_obj = params.get_object().ok_or_else(|| {
        DeserializationError::unexpected_type(
            json_value,
            "MCP `tools/call` request \"params\" attribute should be an object",
            ContentType::ToolInputRequest,
        )
    })?;
    let tool = params_obj
        .get("tool")
        .ok_or_else(|| DeserializationError::missing_attribute(params, "tool", vec![]))?;
    let tool = tool.get_smolstr().ok_or_else(|| {
        DeserializationError::unexpected_type(
            tool,
            "Expected \"tool\" attribute to be a string",
            ContentType::ToolInputRequest,
        )
    })?;
    let args = params_obj
        .get("args")
        .ok_or_else(|| DeserializationError::missing_attribute(params, "args", vec![]))?;
    let args = args.get_object().ok_or_else(|| {
        DeserializationError::unexpected_type(
            args,
            "Expected \"args\" attribute to be an object",
            ContentType::ToolInputRequest,
        )
    })?;
    let args = args
        .iter()
        .map(|(k, v)| (k.to_smolstr(), v.clone()))
        .collect();
    Ok(Input { name: tool, args })
}

/// Deserialize an MCP `tools/call` json response into an `Output`
pub(crate) fn mcp_tool_output_from_json_value(
    json_value: &LocatedValue,
) -> Result<Output, DeserializationError> {
    let obj = json_value.get_object().ok_or_else(|| {
        DeserializationError::unexpected_type(
            json_value,
            "MCP `tools/call` response should be an object",
            ContentType::ToolOutputResponse,
        )
    })?;
    let result = obj
        .get("result")
        .ok_or_else(|| DeserializationError::missing_attribute(json_value, "result", vec![]))?;
    let result_obj = result.get_object().ok_or_else(|| {
        DeserializationError::unexpected_type(
            result,
            "MCP `tools/call` response \"result\" attribute should be an object",
            ContentType::ToolOutputResponse,
        )
    })?;
    let content = result_obj.get("structuredContent").ok_or_else(|| {
        DeserializationError::missing_attribute(result, "structuredContent", vec![])
    })?;
    let results = content.get_object().ok_or_else(|| {
        DeserializationError::unexpected_type(
            content,
            "MCP `tools/call` response `\"structuredContent\"` is expected to be an object",
            ContentType::ToolOutputResponse,
        )
    })?;
    let results = results
        .iter()
        .map(|(k, v)| (k.to_smolstr(), v.clone()))
        .collect();
    Ok(Output { results })
}

fn typedefs_are_well_founded(
    type_defs: &HashMap<SmolStr, PropertyTypeDef>,
) -> Result<(), DeserializationError> {
    // Effectively perform a BFS from each type def to see if
    // there is any cycle of type defs that directly reference each other
    // e.g., type A = B; type B = C; type C = a;
    // Note: other mutually recursive types are acceptable
    // e.g., type A = B; type B = { "a": A }
    for (name, ty_def) in type_defs {
        let mut cycle = vec![name.clone()];
        let mut ty_def = ty_def;
        while let PropertyType::Ref { name } = ty_def.property_type() {
            if cycle.contains(name) {
                cycle.push(name.clone());
                return Err(DeserializationError::type_definition_cycle(cycle));
            }
            match type_defs.get(name) {
                Some(tdef) => {
                    cycle.push(name.clone());
                    ty_def = tdef
                }
                _ => break,
            }
        }
    }
    Ok(())
}
