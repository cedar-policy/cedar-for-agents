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

use crate::data::{self, Input, Output, TypedInput, TypedOutput, TypedValue, Value};
use crate::description::{self, PropertyType, ToolDescription};
use crate::err::ValidationError;
use itertools::Itertools;
use smol_str::{SmolStr, ToSmolStr};
use std::collections::HashMap;

pub(crate) fn validate_input(
    tool: &ToolDescription,
    input: &Input,
    mut type_defs: HashMap<SmolStr, description::PropertyTypeDef>,
) -> Result<TypedInput, ValidationError> {
    if tool.name() != input.name() {
        return Err(ValidationError::mismatched_names(
            tool.name().to_smolstr(),
            input.name().to_smolstr(),
        ));
    }

    type_defs.extend(tool.type_defs.type_defs.clone());

    let args = input.get_args().collect();
    let args = validate_parameters(&tool.inputs, &args, &mut type_defs)?;
    Ok(TypedInput {
        name: input.name.clone(),
        args,
    })
}

pub(crate) fn validate_output(
    tool: &ToolDescription,
    output: &Output,
    mut type_defs: HashMap<SmolStr, description::PropertyTypeDef>,
) -> Result<TypedOutput, ValidationError> {
    type_defs.extend(tool.type_defs.type_defs.clone());

    let results = output.get_results().collect();
    let results = validate_parameters(&tool.outputs, &results, &mut type_defs)?;
    Ok(TypedOutput { results })
}

fn validate_parameters(
    types: &description::Parameters,
    vals: &HashMap<&str, data::BorrowedValue<'_>>,
    type_defs: &mut HashMap<SmolStr, description::PropertyTypeDef>,
) -> Result<HashMap<SmolStr, TypedValue>, ValidationError> {
    type_defs.extend(types.type_defs.type_defs.clone());

    let mut props = HashMap::new();
    for property in types.properties() {
        match vals.get(property.name()) {
            Some(val) => {
                let ty_val = validate_property_type(
                    property.property_type(),
                    val.clone().into(),
                    type_defs,
                )?;
                props.insert(property.name().to_smolstr(), ty_val);
            }
            None if property.is_required() => {
                return Err(ValidationError::missing_required_property(
                    property.name().into(),
                ))
            }
            None => (),
        }
    }
    for val in vals.keys() {
        if !props.contains_key(*val) {
            return Err(ValidationError::unexpected_property(val));
        }
    }
    Ok(props)
}

fn validate_property_type(
    ty: &PropertyType,
    val: Value,
    type_defs: &HashMap<SmolStr, description::PropertyTypeDef>,
) -> Result<TypedValue, ValidationError> {
    match (ty, val) {
        (PropertyType::Bool, Value::Bool(b)) => Ok(TypedValue::Bool(b)),
        (PropertyType::Integer, Value::Number(num)) => match num.to_i64() {
            Some(i) => Ok(TypedValue::Integer(i)),
            None => Err(ValidationError::invalid_integer_literal(num.as_str())),
        },
        (PropertyType::Float, Value::Number(num)) => match num.to_f64() {
            Some(f) => Ok(TypedValue::Float(f)),
            None => Err(ValidationError::invalid_float_literal(num.as_str())),
        },
        (PropertyType::Number, Value::Number(num)) => Ok(TypedValue::Number(num)),
        (PropertyType::String, Value::String(s)) => Ok(TypedValue::String(s)),
        (PropertyType::Decimal, Value::String(s)) => {
            if !is_decimal(&s) {
                Err(ValidationError::invalid_decimal_literal(&s))
            } else {
                Ok(TypedValue::Decimal(s))
            }
        }
        (PropertyType::Datetime, Value::String(s)) => {
            if !is_datetime(&s) {
                Err(ValidationError::invalid_datetime_literal(&s))
            } else {
                Ok(TypedValue::Datetime(s))
            }
        }
        (PropertyType::Duration, Value::String(s)) => {
            if !is_duration(&s) {
                Err(ValidationError::invalid_duration_literal(&s))
            } else {
                Ok(TypedValue::Duration(s))
            }
        }
        (PropertyType::IpAddr, Value::String(s)) => {
            if !is_ipaddr(&s) {
                Err(ValidationError::invalid_ipaddr_literal(&s))
            } else {
                Ok(TypedValue::IpAddr(s))
            }
        }
        (PropertyType::Null, Value::Null) => Ok(TypedValue::Null),
        (PropertyType::Enum { variants }, Value::String(s)) => {
            if !variants.contains(&s) {
                Err(ValidationError::invalid_enum_variant(&s))
            } else {
                Ok(TypedValue::Enum(s))
            }
        }
        (PropertyType::Array { element_ty }, Value::Array(vals)) => {
            let vals = vals
                .into_iter()
                .map(|v| validate_property_type(element_ty, v, type_defs))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(TypedValue::Array(vals))
        }
        (PropertyType::Tuple { types }, Value::Array(vals)) => {
            if types.len() != vals.len() {
                return Err(ValidationError::wrong_tuple_size(types.len(), vals.len()));
            }
            let vals = vals
                .into_iter()
                .zip(types.iter())
                .map(|(val, ty)| validate_property_type(ty, val, type_defs))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(TypedValue::Tuple(vals))
        }
        (PropertyType::Union { types }, val) => {
            for (index, ty) in types.iter().enumerate() {
                if let Ok(ty_val) = validate_property_type(ty, val.clone(), type_defs) {
                    return Ok(TypedValue::Union {
                        index,
                        value: Box::new(ty_val),
                    });
                }
            }
            Err(ValidationError::InvalidValueForUnionType)
        }
        (
            PropertyType::Object {
                properties,
                additional_properties,
            },
            Value::Map(mut vals),
        ) => {
            let mut props = HashMap::new();
            for property in properties {
                match vals.remove(property.name()) {
                    Some(v) => {
                        let ty_val =
                            validate_property_type(property.property_type(), v.clone(), type_defs)?;
                        props.insert(property.name().to_smolstr(), ty_val);
                    }
                    None if property.is_required() => {
                        return Err(ValidationError::missing_required_property(
                            property.name().into(),
                        ))
                    }
                    None => (),
                }
            }

            let mut additional_props = HashMap::new();
            for (name, v) in vals.into_iter() {
                if !props.contains_key(&name) {
                    match additional_properties {
                        Some(ty) => {
                            let ty_val = validate_property_type(ty, v, type_defs)?;
                            additional_props.insert(name, ty_val);
                        }
                        None => return Err(ValidationError::unexpected_property(&name)),
                    }
                }
            }
            Ok(TypedValue::Object {
                properties: props,
                additional_properties: additional_props,
            })
        }
        (PropertyType::Ref { name }, val) => match type_defs.get(name) {
            Some(ty) => {
                let ty_val = validate_property_type(ty.property_type(), val, type_defs)?;
                Ok(TypedValue::Ref {
                    name: name.clone(),
                    val: Box::new(ty_val),
                })
            }
            None => Err(ValidationError::unexpected_type_name(name.as_str())),
        },
        (PropertyType::Unknown, val) => Ok(TypedValue::Unknown(val)),
        _ => Err(ValidationError::InvalidValueForType),
    }
}

fn is_decimal(str: &str) -> bool {
    let Some((integer_part, fractional_part)) = str.split('.').collect_tuple() else {
        return false;
    };

    // Validate integer part: 0 or [1-9][0-9]*
    if integer_part.is_empty()
        || (integer_part.len() > 1 && integer_part.starts_with('0'))
        || !integer_part.chars().all(|c| c.is_ascii_digit() || c == '-')
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
    let is_neg = integer_part.starts_with("-");

    // Construct the scaled integer value
    // Result = (integer_part * 10^4 + fractional_part * 10^(4 - fractional_part.len()))
    #[expect(
        clippy::cast_possible_truncation,
        reason = "Casting usize between 0 and 4 will not truncate."
    )]
    let frac_scale = 10_i64.pow(4 - (fractional_part.len() as u32));
    let scale = 10_i64.pow(4);

    // Parse parts
    let Ok(int_val) = integer_part.parse::<i64>() else {
        return false;
    };

    // only ascii digits and has length 1-4 (thus parsing will not fail)
    #[expect(
        clippy::unwrap_used,
        reason = "An `i64` whose string representation has length <= 4 cannot overflow."
    )]
    let frac_val: i64 = fractional_part.parse().unwrap();
    let frac_val = frac_val * frac_scale * (if is_neg { -1 } else { 1 });

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
        || chrono::DateTime::parse_from_str(str, "%Y-%m-%dT%H:%M:%S%z").is_ok()
        || chrono::DateTime::parse_from_str(str, "%Y-%m-%dT%H:%M:%S%.f%z").is_ok()
        || chrono::NaiveDateTime::parse_from_str(str, "%Y-%m-%dT%H:%M:%S%.f").is_ok()
}

fn is_duration(str: &str) -> bool {
    iso8601::duration(str).is_ok()
}

fn is_ipaddr(str: &str) -> bool {
    str.parse::<std::net::IpAddr>().is_ok() || str.parse::<ipnet::IpNet>().is_ok()
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_is_decimal_positive_tests() {
        assert!(is_decimal("0.0"));
        assert!(is_decimal("922337203685477.5807"));
        assert!(is_decimal("922337203685477.580"));
        assert!(is_decimal("-922337203685477.5808"))
    }

    #[test]
    fn test_is_decimal_no_decimal_point_is_false() {
        assert!(!is_decimal("0"))
    }

    #[test]
    fn test_is_decimal_integral_potion_not_integer_is_false() {
        assert!(!is_decimal("a.0"))
    }

    #[test]
    fn test_is_decimal_fractional_portion_not_integer_is_false() {
        assert!(!is_decimal("0.a"))
    }

    #[test]
    fn test_is_decimal_fractional_portion_5_digits_is_false() {
        assert!(!is_decimal("0.00000"))
    }

    #[test]
    fn test_is_decimal_inegral_portion_overflows_is_false() {
        assert!(!is_decimal("12345678931234123412.0"))
    }

    #[test]
    fn test_is_decimal_integral_portion_scaling_overflows_is_false() {
        assert!(!is_decimal("922337203685478.0000"))
    }

    #[test]
    fn test_is_decimal_overflows_is_false() {
        assert!(!is_decimal("922337203685477.5808"));
        assert!(!is_decimal("-922337203685477.5809"))
    }
}
