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

use crate::data::{self, Input, Output};
use crate::description::{self, PropertyType, ToolDescription};
use crate::err::ValidationError;
use smol_str::{SmolStr, ToSmolStr};
use std::collections::{HashMap, HashSet};

pub(crate) fn validate_input(
    tool: &ToolDescription,
    input: &Input,
    type_defs: &HashMap<SmolStr, description::PropertyTypeDef>,
) -> Result<(), ValidationError> {
    if tool.name() != input.name() {
        return Err(ValidationError::mismatched_names(
            tool.name().to_smolstr(),
            input.name().to_smolstr(),
        ));
    }

    let mut type_defs = type_defs.clone();
    type_defs.extend(tool.type_defs.type_defs.clone());

    let args = input.get_args().collect();
    validate_parameters(&tool.inputs, &args, &mut type_defs)
}

pub(crate) fn validate_output(
    tool: &ToolDescription,
    output: &Output,
    type_defs: &HashMap<SmolStr, description::PropertyTypeDef>,
) -> Result<(), ValidationError> {
    let mut type_defs = type_defs.clone();
    type_defs.extend(tool.type_defs.type_defs.clone());

    let results = output.get_results().collect();
    validate_parameters(&tool.outputs, &results, &mut type_defs)
}

fn validate_parameters(
    types: &description::Parameters,
    vals: &HashMap<&str, data::BorrowedValue<'_>>,
    type_defs: &mut HashMap<SmolStr, description::PropertyTypeDef>,
) -> Result<(), ValidationError> {
    type_defs.extend(types.type_defs.type_defs.clone());

    for property in types.properties() {
        match vals.get(property.name()) {
            Some(val) => validate_property_type(property.property_type(), val, type_defs)?,
            None if property.is_required() => {
                return Err(ValidationError::missing_required_property(
                    property.name().into(),
                ))
            }
            None => (),
        }
    }

    Ok(())
}

fn validate_property_type(
    ty: &PropertyType,
    val: &data::BorrowedValue<'_>,
    type_defs: &HashMap<SmolStr, description::PropertyTypeDef>,
) -> Result<(), ValidationError> {
    match ty {
        PropertyType::Bool if val.is_bool() => (),
        PropertyType::Integer if val.is_number() => {
            if val.get_i64().is_none() {
                // PANIC SAFETY: the value is a number. Converting to a number should not error
                #[allow(
                    clippy::unwrap_used,
                    reason = "val is a number. Converting to number should not error"
                )]
                let val = val.get_number().unwrap();
                return Err(ValidationError::invalid_integer_literal(val.as_str()));
            }
        }
        PropertyType::Float if val.is_number() => {
            if val.get_f64().is_none() {
                // PANIC SAFETY: the value is a number. Converting to a number should not error
                #[allow(
                    clippy::unwrap_used,
                    reason = "val is a number. Converting to number should not error"
                )]
                let val = val.get_number().unwrap();
                return Err(ValidationError::invalid_float_literal(val.as_str()));
            }
        }
        PropertyType::Number if val.is_number() => (),
        PropertyType::String if val.is_string() => (),
        PropertyType::Decimal if val.is_string() => {
            // PANIC SAFETY: the value is a string. Converting to str should not error
            #[allow(
                clippy::unwrap_used,
                reason = "val is a string. Converstion to str should not error"
            )]
            let val = val.get_str().unwrap();
            if !is_decimal(val) {
                return Err(ValidationError::invalid_decimal_literal(val));
            }
        }
        PropertyType::Datetime if val.is_string() => {
            // PANIC SAFETY: the value is a string. Converting to str should not error
            #[allow(
                clippy::unwrap_used,
                reason = "val is a string. Converstion to str should not error"
            )]
            let val = val.get_str().unwrap();
            if !is_datetime(val) {
                return Err(ValidationError::invalid_datetime_literal(val));
            }
        }
        PropertyType::Duration if val.is_string() => {
            // PANIC SAFETY: the value is a string. Converting to str should not error
            #[allow(
                clippy::unwrap_used,
                reason = "val is a string. Converstion to str should not error"
            )]
            let val = val.get_str().unwrap();
            if !is_duration(val) {
                return Err(ValidationError::invalid_duration_literal(val));
            }
        }
        PropertyType::IpAddr if val.is_string() => {
            // PANIC SAFETY: the value is a string. Converting to str should not error
            #[allow(
                clippy::unwrap_used,
                reason = "val is a string. Converstion to str should not error"
            )]
            let val = val.get_str().unwrap();
            if !is_ipaddr(val) {
                return Err(ValidationError::invalid_ipaddr_literal(val));
            }
        }
        PropertyType::Null if val.is_null() => (),
        PropertyType::Enum { variants } if val.is_string() => {
            // PANIC SAFETY: the value is a string. Converting to smolstr should not error
            #[allow(
                clippy::unwrap_used,
                reason = "val is a string. Converting to smolstr should not error"
            )]
            let val = val.get_smolstr().unwrap();
            if !variants.contains(&val) {
                return Err(ValidationError::invalid_enum_variant(val.as_str()));
            }
        }
        PropertyType::Array { element_ty } if val.is_array() => {
            // PANIC SAFETY: the value is an array. Getting as an array should not error
            #[allow(
                clippy::unwrap_used,
                reason = "val is an array. Getting as an array should not error"
            )]
            let val = val.get_array().unwrap();
            for v in val {
                validate_property_type(element_ty, &v, type_defs)?
            }
        }
        PropertyType::Tuple { types } if val.is_array() => {
            // PANIC SAFETY: the value is an array. Getting as an array should not error
            #[allow(
                clippy::unwrap_used,
                reason = "val is an array. Getting as an array should not error"
            )]
            let val = val.get_array().unwrap();
            if types.len() != val.len() {
                return Err(ValidationError::wrong_tuple_size(types.len(), val.len()));
            }
            for (ty, val) in types.iter().zip(val.iter()) {
                validate_property_type(ty, val, type_defs)?
            }
        }
        PropertyType::Union { types } => {
            if !types
                .iter()
                .any(|ty| validate_property_type(ty, val, type_defs).is_ok())
            {
                return Err(ValidationError::InvalidValueForUnionType);
            }
        }
        PropertyType::Object {
            properties,
            additional_properties,
        } if val.is_map() => {
            // PANIC SAFETY: the value is a map, conversion to a map should not error
            #[allow(
                clippy::unwrap_used,
                reason = "val is a map, conversion to a map should not error"
            )]
            let val = val.get_map().unwrap();
            let mut props = HashSet::new();
            for property in properties {
                props.insert(property.name());
                match val.get(property.name()) {
                    Some(v) => validate_property_type(property.property_type(), v, type_defs)?,
                    None if property.is_required() => {
                        return Err(ValidationError::missing_required_property(
                            property.name().into(),
                        ))
                    }
                    None => (),
                }
            }
            for (name, v) in val.iter() {
                if !props.contains(name.as_str()) {
                    match additional_properties {
                        Some(ty) => validate_property_type(ty, v, type_defs)?,
                        None => return Err(ValidationError::unexpected_property(name.as_str())),
                    }
                }
            }
        }
        PropertyType::Ref { name } => match type_defs.get(name) {
            Some(ty) => validate_property_type(ty.property_type(), val, type_defs)?,
            None => return Err(ValidationError::unexpected_type_name(name.as_str())),
        },
        _ => return Err(ValidationError::InvalidValueForType),
    }
    Ok(())
}

// PANIC SAFETY: Indexing vec of length 2 by 0 and 1 should not panic
#[allow(
    clippy::indexing_slicing,
    reason = "Indexing vec of length 2 by 0 and 1 should not panic"
)]
fn is_decimal(str: &str) -> bool {
    let parts: Vec<&str> = str.split('.').collect();

    if parts.len() != 2 {
        return false;
    }
    let integer_part = parts[0];
    let fractional_part = parts[1];

    // Validate integer part: 0 or [1-9][0-9]*
    if integer_part.is_empty()
        || (integer_part.len() > 1 && integer_part.starts_with('0'))
        || !integer_part.chars().all(|c| c.is_ascii_digit())
    {
        return false;
    }

    // Validate fractional part: [0-9]{1,4}
    if fractional_part.is_empty()
        || fractional_part.len() > 4
        || !fractional_part.chars().all(|c| c.is_ascii_digit())
    {
        return false;
    }

    // Construct the scaled integer value
    // Result = (integer_part * 10^frac_len + fractional_part)
    let frac_len = fractional_part.len();
    #[allow(
        clippy::cast_possible_truncation,
        reason = "Casting length of 0-4 will not truncate"
    )]
    let scale = 10_i64.pow(frac_len as u32);

    // Parse parts
    let int_val: i64 = match integer_part.parse() {
        Ok(v) => v,
        Err(_) => return false,
    };
    let frac_val: i64 = match fractional_part.parse() {
        Ok(v) => v,
        Err(_) => return false,
    };

    // Check for overflow when scaling integer part
    let scaled_int = match int_val.checked_mul(scale) {
        Some(v) => v,
        None => return false,
    };

    // Check for overflow when adding fractional part
    scaled_int.checked_add(frac_val).is_some()
}

fn is_datetime(str: &str) -> bool {
    chrono::NaiveDate::parse_from_str(str, "%Y-%m-%d").is_ok()
        || chrono::DateTime::parse_from_rfc3339(str).is_ok()
        || chrono::NaiveDateTime::parse_from_str(str, "%Y-%m-%dT%H:%M:%S%.f").is_ok()
        || chrono::NaiveDateTime::parse_from_str(str, "%Y-%m-%dT%H:%M:%S").is_ok()
}

fn is_duration(str: &str) -> bool {
    iso8601::duration(str).is_ok()
}

fn is_ipaddr(str: &str) -> bool {
    str.parse::<std::net::IpAddr>().is_ok()
}
