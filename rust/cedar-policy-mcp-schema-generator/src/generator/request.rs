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
    Context, Eid, Entity, EntityType, EntityUID, Name, Request, RestrictedExpr,
};
use cedar_policy_core::entities::Entities;
use cedar_policy_core::validator::ValidatorSchema;

use crate::{RequestGeneratorError, SchemaGeneratorConfig};
use mcp_tools_sdk::data::{Input, Output, TypedValue};
use mcp_tools_sdk::description::{self, ServerDescription};
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
        if self.config.flatten_namespaces {
            return Err(RequestGeneratorError::UnsupportedFlattenedNamespaces);
        }

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
            .cloned()
            .map(|ty_def| {
                (
                    ty_def.name().to_smolstr(),
                    (self.root_namespace.clone(), ty_def),
                )
            })
            .collect::<HashMap<_, _>>();

        // PANIC SAFETY: By assumption, `tool.name()` should be a valid namespace
        #[allow(
            clippy::unwrap_used,
            reason = "By assumption, `tool.name()` should be a valid namespace"
        )]
        let tool_ns: Name = tool.name().parse().unwrap();
        let tool_ns = tool_ns.qualify_with_name(self.root_namespace.as_ref());

        // collect all of the tool specific type defs
        type_defs.extend(
            tool.type_definitions()
                .cloned()
                .map(|ty_def| (ty_def.name().to_smolstr(), (Some(tool_ns.clone()), ty_def))),
        );

        #[allow(clippy::unwrap_used, reason = "`Input` is a valid Name")]
        // PANIC SAFETY: "Input" is a valid Name
        let input_ns: Name = "Input".parse().unwrap();
        let input_ns = input_ns.qualify_with_name(Some(&tool_ns));

        // Combine server / tool / input specific type defs
        let mut inputs_type_defs = type_defs.clone();
        inputs_type_defs.extend(
            tool.inputs()
                .type_definitions()
                .cloned()
                .map(|ty_def| (ty_def.name().to_smolstr(), (Some(input_ns.clone()), ty_def))),
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
                #[allow(clippy::unwrap_used, reason = "`Output` is a valid Name")]
                // PANIC SAFETY: "Output" is a valid Name
                let output_ns: Name = "Output".parse().unwrap();
                let output_ns = output_ns.qualify_with_name(Some(&tool_ns));

                // Combine server / tool / output specific type defs
                let mut outputputs_type_defs = type_defs.clone();
                outputputs_type_defs.extend(tool.outputs().type_definitions().cloned().map(
                    |ty_def| {
                        (
                            ty_def.name().to_smolstr(),
                            (Some(output_ns.clone()), ty_def),
                        )
                    },
                ));
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

        // PANIC SAFETY: Action should parse as an EntityType
        #[allow(clippy::unwrap_used, reason = "Action should parse as an EntityType")]
        let action_type: EntityType = "Action".parse().unwrap();
        let action_name = Eid::new(tool.name());
        let action = EntityUID::from_components(
            action_type.qualify_with(self.root_namespace.as_ref()),
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
        type_defs: &HashMap<SmolStr, (Option<Name>, description::PropertyTypeDef)>,
        namespace: Option<&Name>,
        ty_name: &str,
    ) -> Result<(RestrictedExpr, Entities), RequestGeneratorError> {
        match val {
            TypedValue::Null => {
                // PANIC SAFETY: Null should be a valid EntityType name
                #[allow(clippy::unwrap_used, reason = "Null should be a valid EntityType name")]
                let ty: EntityType = "Null".parse().unwrap();
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
                    // PANIC SAFETY: decimal should be a valid name
                    #[allow(clippy::unwrap_used, reason = "decimal should be a valid name")]
                    let decimal_ctor = "decimal".parse().unwrap();
                    Ok((
                        RestrictedExpr::call_extension_fn(decimal_ctor, vec![val]),
                        Entities::new(),
                    ))
                } else {
                    // PANIC SAFETY: Float should be a valid EntityType name
                    #[allow(
                        clippy::unwrap_used,
                        reason = "Float should be a valid EntityType name"
                    )]
                    let ty: EntityType = "Float".parse().unwrap();
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
                    // PANIC SAFETY: decimal should be a valid name
                    #[allow(clippy::unwrap_used, reason = "decimal should be a valid name")]
                    let decimal_ctor = "decimal".parse().unwrap();
                    Ok((
                        RestrictedExpr::call_extension_fn(decimal_ctor, vec![val]),
                        Entities::new(),
                    ))
                } else {
                    // PANIC SAFETY: Number should be a valid EntityType name
                    #[allow(
                        clippy::unwrap_used,
                        reason = "Number should be a valid EntityType name"
                    )]
                    let ty: EntityType = "Number".parse().unwrap();
                    let ty = ty.qualify_with(self.root_namespace.as_ref());
                    let eid = Eid::new(n.as_str());
                    let euid = EntityUID::from_components(ty, eid, None);
                    Ok((RestrictedExpr::val(euid), Entities::new()))
                }
            }
            TypedValue::String(s) => Ok((RestrictedExpr::val(s.as_str()), Entities::new())),
            TypedValue::Decimal(s) => {
                let val = RestrictedExpr::val(s.as_str());
                // PANIC SAFETY: decimal should be a valid name
                #[allow(clippy::unwrap_used, reason = "decimal should be a valid name")]
                let decimal_ctor = "decimal".parse().unwrap();
                Ok((
                    RestrictedExpr::call_extension_fn(decimal_ctor, vec![val]),
                    Entities::new(),
                ))
            }
            TypedValue::Datetime(s) => {
                let val = RestrictedExpr::val(reformat_datestr(s.as_str()));
                // PANIC SAFETY: decimal should be a valid name
                #[allow(clippy::unwrap_used, reason = "datetime should be a valid name")]
                let dt_ctor = "datetime".parse().unwrap();
                Ok((
                    RestrictedExpr::call_extension_fn(dt_ctor, vec![val]),
                    Entities::new(),
                ))
            }
            TypedValue::Duration(s) => {
                let val = RestrictedExpr::val(reformat_duration(s.as_str()));
                // PANIC SAFETY: decimal should be a valid name
                #[allow(clippy::unwrap_used, reason = "duration should be a valid name")]
                let dur_ctor = "duration".parse().unwrap();
                Ok((
                    RestrictedExpr::call_extension_fn(dur_ctor, vec![val]),
                    Entities::new(),
                ))
            }
            TypedValue::IpAddr(s) => {
                let val = RestrictedExpr::val(reformat_ipaddr(s.as_str()));
                // PANIC SAFETY: decimal should be a valid name
                #[allow(clippy::unwrap_used, reason = "ip should be a valid name")]
                let ipaddr_ctor = "ip".parse().unwrap();
                Ok((
                    RestrictedExpr::call_extension_fn(ipaddr_ctor, vec![val]),
                    Entities::new(),
                ))
            }
            TypedValue::Unknown(_) => {
                // PANIC SAFETY: Unknown should be a valid EntityType name
                #[allow(
                    clippy::unwrap_used,
                    reason = "Unknown should be a valid EntityType name"
                )]
                let ty: EntityType = "Unknown".parse().unwrap();
                let ty = ty.qualify_with(self.root_namespace.as_ref());
                let eid = Eid::new("unknown");
                let euid = EntityUID::from_components(ty, eid, None);
                Ok((RestrictedExpr::val(euid), Entities::new()))
            }
            TypedValue::Enum(s) => {
                // PANIC SAFETY: By assumption (that we could generate a schema for this tool description), `ty_name` should be a valid EntityType name
                #[allow(
                    clippy::unwrap_used,
                    reason = "By assumption, `ty_name` should be a valid EntityType name"
                )]
                let ty: EntityType = ty_name.parse().unwrap();
                let ty = ty.qualify_with(namespace);
                let eid = Eid::new(s.as_str());
                let euid = EntityUID::from_components(ty, eid, None);
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
                    // PANIC SAFETY: By assumption, (description must pass schema generation) ty_name should be a valid Name
                    #[allow(
                        clippy::unwrap_used,
                        reason = "By assumption, (description must pass schema generation) ty_name should be a valid `Name`"
                    )]
                    let sub_namespace: Name = ty_name.parse().unwrap();
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
                // PANIC SAFETY: By assumption, (description must pass schema generation) ty_name should be a valid Name
                #[allow(
                    clippy::unwrap_used,
                    reason = "By assumption, (description must pass schema generation) ty_name should be a valid `Name`"
                )]
                let sub_namespace: Name = ty_name.parse().unwrap();
                let sub_namespace = sub_namespace.qualify_with_name(namespace);
                let (expr, entities) =
                    self.val_to_cedar(value, type_defs, Some(&sub_namespace), &sub_ty_name)?;
                Ok((RestrictedExpr::record([(name, expr)])?, entities))
            }
            TypedValue::Object {
                properties,
                additional_properties,
            } => {
                // PANIC SAFETY: By assumption, (description must pass schema generation) ty_name should be a valid Name
                #[allow(
                    clippy::unwrap_used,
                    reason = "By assumption, (description must pass schema generation) ty_name should be a valid `Name`"
                )]
                let sub_namespace: Name = ty_name.parse().unwrap();
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
                    // PANIC SAFETY: Schema generator pasing ensures `ty_name` is a valid Cedar `Name`.
                    #[allow(
                        clippy::unwrap_used,
                        reason = "Type description passes schema generation, so `ty_name` should be a valid name"
                    )]
                    let entity_ty: EntityType = ty_name.parse().unwrap();
                    let entity_ty = entity_ty.qualify_with(namespace);
                    let eid = Eid::new("");
                    let euid = EntityUID::from_components(entity_ty, eid, None);
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
            TypedValue::Ref { name, val } => {
                // PANIC SAFETY: tool description should be well formed, thus ref must exist in type-defs
                #[allow(
                    clippy::unwrap_used,
                    reason = "tool description should be well formed, thus ref must exist in type-defs"
                )]
                let (ns, _ty) = type_defs.get(name).unwrap();
                self.val_to_cedar(val, type_defs, ns.as_ref(), name.as_str())
            }
        }
    }
}

fn reformat_datestr(str: &str) -> String {
    str.into()
}

fn reformat_duration(str: &str) -> String {
    str.into()
}

fn reformat_ipaddr(str: &str) -> String {
    str.into()
}
