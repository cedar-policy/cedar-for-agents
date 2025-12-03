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
use mcp_tools_sdk::data::{BorrowedValue, Input, Output};
use mcp_tools_sdk::description::{self, PropertyType, ServerDescription};
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
        input: Input,
        output: Option<Output>,
    ) -> Result<(Request, Entities), RequestGeneratorError> {
        if self.config.flatten_namespaces {
            return Err(RequestGeneratorError::UnsupportedFlattenedNamespaces);
        }

        self.tools.validate_input(&input)?;
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

        // validate output against the same tool as the input
        if let Some(output) = &output {
            self.tools.validate_output(tool.name(), output)?
        }

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
            // PANIC SAFETY: since `input` validates against `tool`, the argument `name` must exist as an input property
            #[allow(
                clippy::unwrap_used,
                reason = "By validating the input matches the tool description, a matching input property must exist in the tool description"
            )]
            let property = tool
                .inputs()
                .properties()
                .find(|prop| prop.name() == name)
                .unwrap();
            let (expr, new_entities) = self.val_to_cedar(
                arg,
                property.property_type(),
                &inputs_type_defs,
                Some(&input_ns),
                name,
            )?;
            entities = entities.add_entities(
                new_entities.into_iter().map(|e| Arc::from(e)),
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
                    // PANIC SAFETY: since `output` validates against `tool`, the argument `name` must exist as an output property
                    #[allow(
                        clippy::unwrap_used,
                        reason = "By validating the output matches the tool description, a matching output property must exist in the tool description"
                    )]
                    let property = tool
                        .outputs()
                        .properties()
                        .find(|prop| prop.name() == name)
                        .unwrap();
                    let (expr, new_entities) = self.val_to_cedar(
                        res,
                        property.property_type(),
                        &outputputs_type_defs,
                        Some(&output_ns),
                        name,
                    )?;
                    entities = entities.add_entities(
                        new_entities.into_iter().map(|e| Arc::from(e)),
                        None::<&cedar_policy_core::validator::CoreSchema<'_>>,
                        cedar_policy_core::entities::TCComputation::AssumeAlreadyComputed,
                        cedar_policy_core::extensions::Extensions::all_available(),
                    )?;
                    outputs.insert(name.to_smolstr(), expr);
                }
                let outputs = RestrictedExpr::record(outputs)?;
                context.into_iter().chain(
                    vec![
                        ("input".to_smolstr(), inputs),
                        ("output".to_smolstr(), outputs),
                    ]
                    .into_iter(),
                )
            }
            _ => context
                .into_iter()
                .chain(vec![("input".to_smolstr(), inputs)].into_iter()),
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

    /// This function assumes that `val` is validated against the type `ty`
    fn val_to_cedar(
        &self,
        val: BorrowedValue<'_>,
        ty: &PropertyType,
        type_defs: &HashMap<SmolStr, (Option<Name>, description::PropertyTypeDef)>,
        namespace: Option<&Name>,
        ty_name: &str,
    ) -> Result<(RestrictedExpr, Entities), RequestGeneratorError> {
        let mut entities = Entities::new();
        let expr = match ty {
            PropertyType::Null => {
                // PANIC SAFETY: Null should be a valid EntityType name
                #[allow(clippy::unwrap_used, reason = "Null should be a valid EntityType name")]
                let ty: EntityType = "Null".parse().unwrap();
                let ty = ty.qualify_with(self.root_namespace.as_ref());
                let eid = Eid::new("null");
                let euid = EntityUID::from_components(ty, eid, None);
                RestrictedExpr::val(euid)
            }
            PropertyType::Bool => {
                // PANIC SAFETY: By assumption val is of type Bool
                #[allow(clippy::unwrap_used, reason = "By assumption val is of type Bool")]
                let val = val.get_bool().unwrap();
                RestrictedExpr::val(val)
            }
            PropertyType::Integer => {
                // PANIC SAFETY: By assumption val is of type Integer (i64)
                #[allow(
                    clippy::unwrap_used,
                    reason = "By assumption val is of type Integer (i64)"
                )]
                let val = val.get_i64().unwrap();
                RestrictedExpr::val(val)
            }
            PropertyType::Float => {
                if self.config.numbers_as_decimal {
                    // PANIC SAFETY: By assumption val is of type Float (f64)
                    #[allow(
                        clippy::unwrap_used,
                        reason = "By assumption val is of type Float (f64)"
                    )]
                    let val = val.get_f64().unwrap();
                    let val = RestrictedExpr::val(format!("{:.4}", val));
                    // PANIC SAFETY: decimal should be a valid name
                    #[allow(clippy::unwrap_used, reason = "decimal should be a valid name")]
                    let decimal_ctor = "decimal".parse().unwrap();
                    RestrictedExpr::call_extension_fn(decimal_ctor, vec![val])
                } else {
                    // PANIC SAFETY: Float should be a valid EntityType name
                    #[allow(
                        clippy::unwrap_used,
                        reason = "Float should be a valid EntityType name"
                    )]
                    let ty: EntityType = "Float".parse().unwrap();
                    let ty = ty.qualify_with(self.root_namespace.as_ref());
                    // PANIC SAFETY: By assumption val is of type Float <: Number
                    #[allow(
                        clippy::unwrap_used,
                        reason = "By assumption val is of type Float <: Number"
                    )]
                    let val = val.get_number().unwrap();
                    let eid = Eid::new(val.as_str());
                    let euid = EntityUID::from_components(ty, eid, None);
                    RestrictedExpr::val(euid)
                }
            }
            PropertyType::Number => {
                if self.config.numbers_as_decimal {
                    let val = match (val.get_f64(), val.get_i64()) {
                        (Some(f), _) => format!("{:.4}", f),
                        (_, Some(i)) => format!("{}.0", i),
                        _ => {
                            // PANIC SAFETY: By assumption, val is of type Number
                            #[allow(
                                clippy::unwrap_used,
                                reason = "By assumption, val is of type Number"
                            )]
                            return Err(RequestGeneratorError::MalformedDecimalNumber(
                                val.get_number().unwrap().as_str().into(),
                            ));
                        }
                    };
                    let val = RestrictedExpr::val(val);
                    // PANIC SAFETY: decimal should be a valid name
                    #[allow(clippy::unwrap_used, reason = "decimal should be a valid name")]
                    let decimal_ctor = "decimal".parse().unwrap();
                    RestrictedExpr::call_extension_fn(decimal_ctor, vec![val])
                } else {
                    // PANIC SAFETY: Number should be a valid EntityType name
                    #[allow(
                        clippy::unwrap_used,
                        reason = "Number should be a valid EntityType name"
                    )]
                    let ty: EntityType = "Number".parse().unwrap();
                    let ty = ty.qualify_with(self.root_namespace.as_ref());
                    // PANIC SAFETY: By assumption val is of type Number
                    #[allow(clippy::unwrap_used, reason = "By assumption val is of type Number")]
                    let val = val.get_number().unwrap();
                    let eid = Eid::new(val.as_str());
                    let euid = EntityUID::from_components(ty, eid, None);
                    RestrictedExpr::val(euid)
                }
            }
            PropertyType::String => {
                // PANIC SAFETY: By assumption, val is of type String
                #[allow(clippy::unwrap_used, reason = "By assumption, val of type String")]
                let val = val.get_string().unwrap();
                RestrictedExpr::val(val)
            }
            PropertyType::Decimal => {
                // PANIC SAFETY: By assumption, val is of type Decimal <: String
                #[allow(
                    clippy::unwrap_used,
                    reason = "By assumption, val of type Decimal <: String"
                )]
                let val = RestrictedExpr::val(val.get_string().unwrap());
                // PANIC SAFETY: decimal should be a valid name
                #[allow(clippy::unwrap_used, reason = "decimal should be a valid name")]
                let decimal_ctor = "decimal".parse().unwrap();
                RestrictedExpr::call_extension_fn(decimal_ctor, vec![val])
            }
            PropertyType::Datetime => {
                // PANIC SAFETY: By assumption, val is of type Datetime <: String
                #[allow(
                    clippy::unwrap_used,
                    reason = "By assumption, val of type Datetime <: String"
                )]
                let val = RestrictedExpr::val(reformat_datestr(val.get_str().unwrap()));
                // PANIC SAFETY: decimal should be a valid name
                #[allow(clippy::unwrap_used, reason = "datetime should be a valid name")]
                let dt_ctor = "datetime".parse().unwrap();
                RestrictedExpr::call_extension_fn(dt_ctor, vec![val])
            }
            PropertyType::Duration => {
                // PANIC SAFETY: By assumption, val is of type Duration <: String
                #[allow(
                    clippy::unwrap_used,
                    reason = "By assumption, val of type Duration <: String"
                )]
                let val = RestrictedExpr::val(reformat_duration(val.get_str().unwrap()));
                // PANIC SAFETY: decimal should be a valid name
                #[allow(clippy::unwrap_used, reason = "duration should be a valid name")]
                let dur_ctor = "duration".parse().unwrap();
                RestrictedExpr::call_extension_fn(dur_ctor, vec![val])
            }
            PropertyType::IpAddr => {
                // PANIC SAFETY: By assumption, val is of type IpAddr <: String
                #[allow(
                    clippy::unwrap_used,
                    reason = "By assumption, val of type IpAddr <: String"
                )]
                let val = RestrictedExpr::val(reformat_ipaddr(val.get_str().unwrap()));
                // PANIC SAFETY: decimal should be a valid name
                #[allow(clippy::unwrap_used, reason = "ip should be a valid name")]
                let ipaddr_ctor = "ip".parse().unwrap();
                RestrictedExpr::call_extension_fn(ipaddr_ctor, vec![val])
            }
            PropertyType::Enum { .. } => {
                // PANIC SAFETY: By assumption (that we could generate a schema for this tool description), `ty_name` should be a valid EntityType name
                #[allow(
                    clippy::unwrap_used,
                    reason = "By assumption, `ty_name` should be a valid EntityType name"
                )]
                let ty: EntityType = ty_name.parse().unwrap();
                let ty = ty.qualify_with(namespace);
                // PANIC SAFETY: By assumption, val is of type String
                #[allow(clippy::unwrap_used, reason = "By assumption, val of type String")]
                let val = val.get_str().unwrap();
                let eid = Eid::new(val);
                let euid = EntityUID::from_components(ty, eid, None);
                RestrictedExpr::val(euid)
            }
            PropertyType::Array { element_ty } => {
                // PANIC SAFETY: By assumption, val is of type Array<`element_ty`>
                #[allow(clippy::unwrap_used, reason = "By assumption, val is of type Array")]
                let vals = val.get_array().unwrap();
                let mut exprs = Vec::new();
                for val in vals {
                    let (expr, new_entities) =
                        self.val_to_cedar(val, element_ty, type_defs, namespace, ty_name)?;
                    entities = entities.add_entities(
                        new_entities.into_iter().map(|e| Arc::from(e)),
                        None::<&cedar_policy_core::validator::CoreSchema<'_>>,
                        cedar_policy_core::entities::TCComputation::AssumeAlreadyComputed,
                        cedar_policy_core::extensions::Extensions::all_available(),
                    )?;
                    exprs.push(expr);
                }
                RestrictedExpr::set(exprs)
            }
            PropertyType::Tuple { types } => {
                // PANIC SAFETY: By assumption, val is of type Tuple <: Array
                #[allow(
                    clippy::unwrap_used,
                    reason = "By assumption, val is of type Tuple <: Array"
                )]
                let vals = val.get_array().unwrap();
                let mut pairs = HashMap::new();
                for ((i, val), ty) in vals.into_iter().enumerate().zip(types.iter()) {
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
                        self.val_to_cedar(val, ty, type_defs, Some(&sub_namespace), &sub_ty_name)?;
                    entities = entities.add_entities(
                        new_entities.into_iter().map(|e| Arc::from(e)),
                        None::<&cedar_policy_core::validator::CoreSchema<'_>>,
                        cedar_policy_core::entities::TCComputation::AssumeAlreadyComputed,
                        cedar_policy_core::extensions::Extensions::all_available(),
                    )?;
                    pairs.insert(name, expr);
                }
                RestrictedExpr::record(pairs)?
            }
            PropertyType::Union { .. } => return Err(RequestGeneratorError::UnsupportedUnionType),
            PropertyType::Object {
                properties,
                additional_properties,
            } => {
                let properties = properties
                    .iter()
                    .map(|prop| (prop.name(), prop.property_type()))
                    .collect::<HashMap<_, _>>();
                // PANIC SAFETY: By assumption, val is of type Map
                #[allow(clippy::unwrap_used, reason = "By assumption, val is of type Map")]
                let val = val.get_map().unwrap();
                let mut pairs = HashMap::new();
                let mut tags = HashMap::new();
                for (name, val) in val.into_iter() {
                    // PANIC SAFETY: By assumption, (description must pass schema generation) ty_name should be a valid Name
                    #[allow(
                        clippy::unwrap_used,
                        reason = "By assumption, (description must pass schema generation) ty_name should be a valid `Name`"
                    )]
                    let sub_namespace: Name = ty_name.parse().unwrap();
                    let sub_namespace = sub_namespace.qualify_with_name(namespace);
                    let (is_tag, ty) = match properties.get(name.as_str()) {
                        Some(ty) => (false, *ty),
                        None => {
                            // PANIC SAFETY: by assumption val matches the object type (and so any un-named properties must be an additional_property)
                            #[allow(clippy::unwrap_used)]
                            (true, additional_properties.as_ref().unwrap().as_ref())
                        }
                    };
                    let (expr, new_entities) =
                        self.val_to_cedar(val, ty, type_defs, Some(&sub_namespace), name.as_ref())?;
                    entities = entities.add_entities(
                        new_entities.into_iter().map(|e| Arc::from(e)),
                        None::<&cedar_policy_core::validator::CoreSchema<'_>>,
                        cedar_policy_core::entities::TCComputation::AssumeAlreadyComputed,
                        cedar_policy_core::extensions::Extensions::all_available(),
                    )?;
                    if is_tag {
                        tags.insert(name, expr);
                    } else {
                        pairs.insert(name, expr);
                    }
                }
                if tags.is_empty() && self.config.objects_as_records {
                    RestrictedExpr::record(pairs.into_iter())?
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
                    RestrictedExpr::val(euid)
                }
            }
            PropertyType::Ref { name } => {
                // PANIC SAFETY: tool description should be well formed, thus ref must exist in type-defs
                #[allow(
                    clippy::unwrap_used,
                    reason = "tool description should be well formed, thus ref must exist in type-defs"
                )]
                let (ns, ty) = type_defs.get(name).unwrap();
                let (expr, new_entities) = self.val_to_cedar(
                    val,
                    ty.property_type(),
                    type_defs,
                    ns.as_ref(),
                    name.as_str(),
                )?;
                entities = entities.add_entities(
                    new_entities.into_iter().map(|e| Arc::from(e)),
                    None::<&cedar_policy_core::validator::CoreSchema<'_>>,
                    cedar_policy_core::entities::TCComputation::AssumeAlreadyComputed,
                    cedar_policy_core::extensions::Extensions::all_available(),
                )?;
                expr
            }
        };
        Ok((expr, entities))
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
