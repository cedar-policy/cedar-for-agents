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

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use cedar_policy_core::ast::{
    Context, Eid, Entity, EntityType, EntityUID, InternalName, Name, Request, RestrictedExpr,
};
use cedar_policy_core::entities::Entities;
use cedar_policy_core::validator::ValidatorSchema;

use chrono::{DateTime, NaiveDate, NaiveDateTime, TimeZone, Utc};

use super::identifiers;
use crate::{RequestGeneratorError, SchemaGeneratorConfig};

use mcp_tools_sdk::data::{Input, Output, TypedValue};
use mcp_tools_sdk::description::ServerDescription;
use smol_str::{SmolStr, ToSmolStr};

#[derive(Clone, Debug)]
pub struct RequestGenerator {
    config: SchemaGeneratorConfig,
    tools: ServerDescription,
    root_namespace: Option<Name>,
    schema: ValidatorSchema,
}

impl RequestGenerator {
    pub(crate) fn new(
        config: SchemaGeneratorConfig,
        tools: ServerDescription,
        root_namespace: Option<Name>,
        schema: ValidatorSchema,
    ) -> Self {
        Self {
            config,
            tools,
            root_namespace,
            schema,
        }
    }

    pub fn generate_request(
        &self,
        principal: EntityUID,
        resource: EntityUID,
        context: impl IntoIterator<Item = (SmolStr, RestrictedExpr)>,
        mut entities: Entities,
        input: &Input,
        output: Option<&Output>,
    ) -> Result<(Request, Entities), RequestGeneratorError> {
        let input = self.tools.validate_input(input)?;
        // PANIC SAFETY: `self.tools` must contain a tool named `input.name()` and `input` validates against `tool`
        #[allow(
            clippy::unwrap_used,
            reason = "Validation ensures there is a tool in with the same name that the input validates against"
        )]
        let tool = self
            .tools
            .tool_descriptions()
            .find(|t| t.name() == input.name())
            .unwrap();

        let output = output
            .map(|output| self.tools.validate_output(tool.name(), output))
            .transpose()?;

        let mut type_defs = self
            .tools
            .type_definitions()
            .map(|ty_def| (ty_def.name().to_smolstr(), self.root_namespace.clone()))
            .collect::<HashMap<_, _>>();

        let tool_ns: Name = tool.name().parse()?;
        let tool_ns = tool_ns.qualify_with_name(self.root_namespace.as_ref());

        // collect all of the tool specific type defs
        type_defs.extend(
            tool.type_definitions()
                .map(|ty_def| (ty_def.name().to_smolstr(), Some(tool_ns.clone()))),
        );

        let input_ns = identifiers::INPUT_NAME.qualify_with_name(Some(&tool_ns));

        // Combine server / tool / input specific type defs
        let mut inputs_type_defs = type_defs.clone();
        inputs_type_defs.extend(
            tool.inputs()
                .type_definitions()
                .map(|ty_def| (ty_def.name().to_smolstr(), Some(input_ns.clone()))),
        );

        let mut inputs = HashMap::new();
        for (name, arg) in input.get_args() {
            let (expr, new_entities) =
                self.val_to_cedar(arg, &inputs_type_defs, Some(&input_ns), name)?;
            entities = entities.add_entities(
                new_entities.into_iter().map(Arc::from),
                None::<&cedar_policy_core::validator::CoreSchema<'_>>,
                cedar_policy_core::entities::TCComputation::AssumeAlreadyComputed,
                cedar_policy_core::extensions::Extensions::all_available(),
            )?;
            inputs.insert(name.to_smolstr(), expr);
        }
        let inputs = RestrictedExpr::record(inputs)?;

        let context = match &output {
            Some(output) if self.config.include_outputs => {
                let output_ns = identifiers::OUTPUT_NAME.qualify_with_name(Some(&tool_ns));

                // Combine server / tool / output specific type defs
                let mut outputputs_type_defs = type_defs.clone();
                outputputs_type_defs.extend(
                    tool.outputs()
                        .type_definitions()
                        .map(|ty_def| (ty_def.name().to_smolstr(), Some(output_ns.clone()))),
                );
                let mut outputs = HashMap::new();
                for (name, res) in output.get_results() {
                    let (expr, new_entities) =
                        self.val_to_cedar(res, &outputputs_type_defs, Some(&output_ns), name)?;
                    entities = entities.add_entities(
                        new_entities.into_iter().map(Arc::from),
                        None::<&cedar_policy_core::validator::CoreSchema<'_>>,
                        cedar_policy_core::entities::TCComputation::AssumeAlreadyComputed,
                        cedar_policy_core::extensions::Extensions::all_available(),
                    )?;
                    outputs.insert(name.to_smolstr(), expr);
                }
                let outputs = RestrictedExpr::record(outputs)?;
                context.into_iter().chain(vec![
                    ("input".to_smolstr(), inputs),
                    ("output".to_smolstr(), outputs),
                ])
            }
            _ => context
                .into_iter()
                .chain(vec![("input".to_smolstr(), inputs)]),
        };
        let context = Context::from_pairs(
            context,
            cedar_policy_core::extensions::Extensions::all_available(),
        )?;

        let action_name = Eid::new(tool.name());
        let action = EntityUID::from_components(
            identifiers::ACTION.qualify_with(self.root_namespace.as_ref()),
            action_name,
            None,
        );
        let request = Request::new(
            (principal, None),
            (action, None),
            (resource, None),
            context,
            Some(&self.schema),
            cedar_policy_core::extensions::Extensions::all_available(),
        )?;

        Ok((request, entities))
    }

    fn val_to_cedar(
        &self,
        val: &TypedValue,
        type_defs: &HashMap<SmolStr, Option<Name>>,
        namespace: Option<&Name>,
        ty_name: &str,
    ) -> Result<(RestrictedExpr, Entities), RequestGeneratorError> {
        match val {
            TypedValue::Null => {
                let ty = EntityType::from(Name::from(identifiers::NULL_TYPE.clone()));
                let ty = ty.qualify_with(self.root_namespace.as_ref());
                let eid = Eid::new("null");
                let euid = EntityUID::from_components(ty, eid, None);
                Ok((RestrictedExpr::val(euid), Entities::new()))
            }
            TypedValue::Bool(b) => Ok((RestrictedExpr::val(*b), Entities::new())),
            TypedValue::Integer(i) => Ok((RestrictedExpr::val(*i), Entities::new())),
            TypedValue::Float(f) => {
                if self.config.numbers_as_decimal {
                    let val = RestrictedExpr::val(format!("{:.4}", f));
                    Ok((
                        RestrictedExpr::call_extension_fn(
                            identifiers::DECIMAL_CTOR.clone(),
                            vec![val],
                        ),
                        Entities::new(),
                    ))
                } else {
                    let ty = EntityType::from(Name::from(identifiers::FLOAT_TYPE.clone()));
                    let ty = ty.qualify_with(self.root_namespace.as_ref());
                    let eid = Eid::new(format!("{}", f));
                    let euid = EntityUID::from_components(ty, eid, None);
                    Ok((RestrictedExpr::val(euid), Entities::new()))
                }
            }
            TypedValue::Number(n) => {
                if self.config.numbers_as_decimal {
                    let val = match (n.to_f64(), n.to_i64()) {
                        (Some(f), _) => format!("{:.4}", f),
                        (_, Some(i)) => format!("{}.0", i),
                        _ => {
                            return Err(RequestGeneratorError::MalformedDecimalNumber(
                                n.as_str().into(),
                            ));
                        }
                    };
                    let val = RestrictedExpr::val(val);
                    Ok((
                        RestrictedExpr::call_extension_fn(
                            identifiers::DECIMAL_CTOR.clone(),
                            vec![val],
                        ),
                        Entities::new(),
                    ))
                } else {
                    let ty = EntityType::from(Name::from(identifiers::NUMBER_TYPE.clone()));
                    let ty = ty.qualify_with(self.root_namespace.as_ref());
                    let eid = Eid::new(n.as_str());
                    let euid = EntityUID::from_components(ty, eid, None);
                    Ok((RestrictedExpr::val(euid), Entities::new()))
                }
            }
            TypedValue::String(s) => Ok((RestrictedExpr::val(s.as_str()), Entities::new())),
            TypedValue::Decimal(s) => {
                let val = RestrictedExpr::val(s.as_str());
                Ok((
                    RestrictedExpr::call_extension_fn(identifiers::DECIMAL_CTOR.clone(), vec![val]),
                    Entities::new(),
                ))
            }
            TypedValue::Datetime(s) => {
                let val = RestrictedExpr::val(reformat_datestr(s.as_str()));
                Ok((
                    RestrictedExpr::call_extension_fn(
                        identifiers::DATETIME_CTOR.clone(),
                        vec![val],
                    ),
                    Entities::new(),
                ))
            }
            TypedValue::Duration(s) => {
                let val = RestrictedExpr::val(reformat_duration(s.as_str()));
                Ok((
                    RestrictedExpr::call_extension_fn(
                        identifiers::DURATION_CTOR.clone(),
                        vec![val],
                    ),
                    Entities::new(),
                ))
            }
            TypedValue::IpAddr(s) => {
                let val = RestrictedExpr::val(reformat_ipaddr(s.as_str()));
                Ok((
                    RestrictedExpr::call_extension_fn(identifiers::IPADDR_CTOR.clone(), vec![val]),
                    Entities::new(),
                ))
            }
            TypedValue::Unknown(_) => {
                let ty = EntityType::from(Name::from(identifiers::UNKNOWN_TYPE.clone()));
                let ty = ty.qualify_with(self.root_namespace.as_ref());
                let eid = Eid::new("unknown");
                let euid = EntityUID::from_components(ty, eid, None);
                Ok((RestrictedExpr::val(euid), Entities::new()))
            }
            TypedValue::Enum(s) => {
                let ty: EntityType = ty_name.parse()?;
                let ty = ty.qualify_with(namespace);
                let eid = Eid::new(s.as_str());
                let euid = EntityUID::from_components(ty, eid, None);
                let euid = if self.config.flatten_namespaces {
                    flatten_name(euid)
                } else {
                    euid
                };
                Ok((RestrictedExpr::val(euid), Entities::new()))
            }
            TypedValue::Array(vals) => {
                let mut exprs = Vec::new();
                let mut entities = Entities::new();
                for val in vals {
                    let (expr, new_entities) =
                        self.val_to_cedar(val, type_defs, namespace, ty_name)?;
                    entities = entities.add_entities(
                        new_entities.into_iter().map(Arc::from),
                        None::<&cedar_policy_core::validator::CoreSchema<'_>>,
                        cedar_policy_core::entities::TCComputation::AssumeAlreadyComputed,
                        cedar_policy_core::extensions::Extensions::all_available(),
                    )?;
                    exprs.push(expr);
                }
                Ok((RestrictedExpr::set(exprs), entities))
            }
            TypedValue::Tuple(vals) => {
                let mut pairs = HashMap::new();
                let mut entities = Entities::new();
                for (i, val) in vals.iter().enumerate() {
                    let sub_ty_name = format!("Proj{i}");
                    let name = format!("proj{i}").to_smolstr();
                    let sub_namespace: Name = ty_name.parse()?;
                    let sub_namespace = sub_namespace.qualify_with_name(namespace);
                    let (expr, new_entities) =
                        self.val_to_cedar(val, type_defs, Some(&sub_namespace), &sub_ty_name)?;
                    entities = entities.add_entities(
                        new_entities.into_iter().map(Arc::from),
                        None::<&cedar_policy_core::validator::CoreSchema<'_>>,
                        cedar_policy_core::entities::TCComputation::AssumeAlreadyComputed,
                        cedar_policy_core::extensions::Extensions::all_available(),
                    )?;
                    pairs.insert(name, expr);
                }
                Ok((RestrictedExpr::record(pairs)?, entities))
            }
            TypedValue::Union { index, value } => {
                let sub_ty_name = format!("TypeChoice{}", index);
                let name = format!("typeChoice{}", index).to_smolstr();
                let sub_namespace: Name = ty_name.parse()?;
                let sub_namespace = sub_namespace.qualify_with_name(namespace);
                let (expr, entities) =
                    self.val_to_cedar(value, type_defs, Some(&sub_namespace), &sub_ty_name)?;
                Ok((RestrictedExpr::record([(name, expr)])?, entities))
            }
            TypedValue::Object {
                properties,
                additional_properties,
            } => {
                let sub_namespace: Name = ty_name.parse()?;
                let sub_namespace = sub_namespace.qualify_with_name(namespace);

                let mut entities = Entities::new();
                let into_pairs = |props: &HashMap<SmolStr, TypedValue>,
                                  entities: &mut Entities|
                 -> Result<
                    HashMap<SmolStr, RestrictedExpr>,
                    RequestGeneratorError,
                > {
                    let mut pairs = HashMap::new();
                    for (name, val) in props.iter() {
                        let (expr, new_entities) =
                            self.val_to_cedar(val, type_defs, Some(&sub_namespace), name.as_ref())?;
                        let old_entities = std::mem::replace(entities, Entities::new());
                        *entities = old_entities.add_entities(
                            new_entities.into_iter().map(Arc::from),
                            None::<&cedar_policy_core::validator::CoreSchema<'_>>,
                            cedar_policy_core::entities::TCComputation::AssumeAlreadyComputed,
                            cedar_policy_core::extensions::Extensions::all_available(),
                        )?;
                        pairs.insert(name.clone(), expr);
                    }
                    Ok(pairs)
                };

                let pairs = into_pairs(properties, &mut entities)?;
                let tags = into_pairs(additional_properties, &mut entities)?;

                if tags.is_empty() && self.config.objects_as_records {
                    Ok((RestrictedExpr::record(pairs.into_iter())?, entities))
                } else {
                    let entity_ty: EntityType = ty_name.parse()?;
                    let entity_ty = entity_ty.qualify_with(namespace);
                    let eid = Eid::new("");
                    let euid = EntityUID::from_components(entity_ty, eid, None);
                    let euid = if self.config.flatten_namespaces {
                        flatten_name(euid)
                    } else {
                        euid
                    };
                    let entity = Entity::new(
                        euid.clone(),
                        pairs,
                        HashSet::new(),
                        HashSet::new(),
                        tags,
                        cedar_policy_core::extensions::Extensions::all_available(),
                    )?;
                    entities = entities.add_entities(
                        [Arc::from(entity)],
                        None::<&cedar_policy_core::validator::CoreSchema<'_>>,
                        cedar_policy_core::entities::TCComputation::AssumeAlreadyComputed,
                        cedar_policy_core::extensions::Extensions::all_available(),
                    )?;
                    Ok((RestrictedExpr::val(euid), entities))
                }
            }
            TypedValue::Ref { name, val } => match type_defs.get(name) {
                None => {
                    let ns = match namespace {
                        None => "".into(),
                        Some(name) => format!("{}", name),
                    };
                    Err(RequestGeneratorError::undefined_ref(name.to_string(), ns))
                }
                Some(ns) => self.val_to_cedar(val, type_defs, ns.as_ref(), name.as_str()),
            },
        }
    }
}

// PANIC SAFETY: The input `str` should have been validated as a date-time str, and should parse
#[allow(clippy::unreachable)]
/// This function converts from JSON date or date-time formatted strings to Cedar datetime strings.
/// This conversion uses chrono library to parse and reformat the strings appropriately.
///
/// Note: this function loses sub-millisecond precision
fn reformat_datestr(str: &str) -> String {
    // Try parsing as date only (YYYY-MM-DD)
    if let Ok(date) = NaiveDate::parse_from_str(str, "%Y-%m-%d") {
        return date.format("%Y-%m-%d").to_string();
    }

    // Try parsing as RFC3339 (with timezone)
    if let Ok(dt) = DateTime::parse_from_rfc3339(str) {
        let dt_utc = dt.with_timezone(&Utc);

        // Check if it has subsecond precision
        if dt_utc.timestamp_subsec_millis() > 0 {
            // With milliseconds
            if dt.offset().local_minus_utc() == 0 {
                // UTC with milliseconds: YYYY-MM-DDTHH:MM:SS.sssZ
                return dt_utc.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();
            } else {
                // With timezone offset and milliseconds: YYYY-MM-DDTHH:MM:SS.sss+0100
                return dt.format("%Y-%m-%dT%H:%M:%S%.3f%z").to_string();
            }
        } else {
            // Without milliseconds
            if dt.offset().local_minus_utc() == 0 {
                // UTC: YYYY-MM-DDTHH:MM:SSZ
                return dt_utc.format("%Y-%m-%dT%H:%M:%SZ").to_string();
            } else {
                // With timezone offset: YYYY-MM-DDTHH:MM:SS+0100
                return dt.format("%Y-%m-%dT%H:%M:%S%z").to_string();
            }
        }
    }

    // Try parsing as naive datetime (no timezone)
    if let Ok(ndt) = NaiveDateTime::parse_from_str(str, "%Y-%m-%dT%H:%M:%S%.f") {
        // Convert to UTC (assuming input is UTC)
        let dt_utc = Utc.from_utc_datetime(&ndt);

        // Check if it has subsecond precision
        if dt_utc.timestamp_subsec_millis() > 0 {
            // UTC with milliseconds: YYYY-MM-DDTHH:MM:SS.sssZ
            return dt_utc.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();
        } else {
            // UTC without milliseconds: YYYY-MM-DDTHH:MM:SSZ
            return dt_utc.format("%Y-%m-%dT%H:%M:%SZ").to_string();
        }
    }

    unreachable!("Validated DatetimeStrings should be parsable")
}

/// This function converts from the iso8601 standard for Duration (JSON duration formatted strings) into
/// cedar formatted durations. Unfortunately, iso8601 uses calendar based durations, and Cedar uses fixed
/// time durations. This means that the best we can do for converting iso8601 durations that contain
/// months or year components is to approximate (e.g., 1 year = 365 days and 1 month = 30 days).
fn reformat_duration(str: &str) -> String {
    // PANIC SAFETY: validation ensures that the input `str` will parse as an `iso8601::Duration`
    #[allow(clippy::unwrap_used)]
    let duration = iso8601::duration(str).unwrap();

    match duration {
        iso8601::Duration::YMDHMS {
            year,
            month,
            day,
            hour,
            minute,
            second,
            millisecond,
        } => {
            // APPROXIMATE year & month into number of days
            let n_days = year * 365 + month * 30 + day;
            format!(
                "{}d{}h{}m{}s{}ms",
                n_days, hour, minute, second, millisecond
            )
        }
        iso8601::Duration::Weeks(weeks) => {
            let n_days = 7 * weeks;
            format!("{}d", n_days)
        }
    }
}

/// This function converts from JSON compliant ipv4 and ipv6 formatted strings (which can be parsed by `std::net::IpAddr`)
/// to Cedar compliant IpAddr strings (which requires a stricter formatting, e.g., no leading 0s).
/// This is accomplished by passing through Rust's IpAddr type which allows lax formatting during deserialization and stricter formatting
/// during serialization to string.
fn reformat_ipaddr(str: &str) -> String {
    // PANIC SAFETY: validation ensures that the input `str` will parse as an `IpAddr`
    #[allow(clippy::unwrap_used)]
    str.parse::<std::net::IpAddr>().unwrap().to_string()
}

// PANIC SAFETY: the input EntityUID is valid. Transforming to flatten the entity type name should be safe
#[allow(clippy::unwrap_used)]
fn flatten_name(euid: EntityUID) -> EntityUID {
    let (entity_type, eid) = euid.components();
    let entity_type = entity_type.name().qualify_with(None);
    let mut parts = entity_type.namespace_components().cloned();
    let flattened_namespace = parts
        .next()
        .map(InternalName::from)
        .map(Name::try_from)
        .transpose()
        .unwrap();
    let flattened_basename = parts.map(|id| id.to_string()).collect::<Vec<_>>().join("_");
    let entity_type: EntityType = flattened_basename.parse().unwrap();
    let entity_type = entity_type.qualify_with(flattened_namespace.as_ref());
    EntityUID::from_components(entity_type, eid, None)
}
