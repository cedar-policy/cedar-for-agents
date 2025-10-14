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

use crate::err::SchemaGeneratorError;
use crate::description::{
    PropertyType,
    Parameters,
    ServerDescription,
    ToolDescription,
};

use cedar_policy_core::ast::{InternalName, Name, UnreservedId};
use cedar_policy_core::est::Annotations;
use cedar_policy_core::validator::{
    json_schema::{
        ActionType,
        ApplySpec,
        AttributesOrContext,
        CommonType,
        CommonTypeId,
        EntityType,
        EntityTypeKind,
        Fragment,
        NamespaceDefinition,
        RecordType,
        StandardEntityType,
        Type,
        TypeOfAttribute,
        TypeVariant,
    },
    RawName,
};

use nonempty::NonEmpty;

use smol_str::{SmolStr, ToSmolStr};

use std::collections::{BTreeMap, btree_map::Entry};

#[derive(Debug, Clone)]
pub struct SchemaGenerator {
    fragment: Fragment<RawName>,
    namespace: Option<Name>,
    users: Vec<RawName>,
    resources: Vec<RawName>,
    contexts: BTreeMap<SmolStr, RawName>,
}

impl SchemaGenerator {
    pub fn new(schema_stub: Fragment<RawName>) -> Result<Self, SchemaGeneratorError> {
        let (ns, namespace) = match schema_stub.0.iter().next() {
            Some((None, _)) => return Err(SchemaGeneratorError::GlobalNamespaceUsed),
            Some((Some(namespace), ns)) => (ns, namespace.clone()),
            None => return Err(SchemaGeneratorError::WrongNumberOfNamespaces),
        };
        if schema_stub.0.len() > 1 {
            return Err(SchemaGeneratorError::WrongNumberOfNamespaces);
        }

        // PANIC SAFETY: converting "mcp_principal" into an AnyId should not error
        let users = ns
            .entity_types
            .iter()
            .filter_map(|(tyname, ty)| {
                ty.annotations
                    .0
                    .get(&"mcp_principal".parse().unwrap())
                    .map(|_| {
                        RawName::from_name(InternalName::unqualified_name(
                            tyname.clone().into(),
                            None,
                        ))
                    })
            })
            .collect();

        // PANIC SAFETY: converting "mcp_resource" into an AnyId should not error
        let resources = ns
            .entity_types
            .iter()
            .filter_map(|(tyname, ty)| {
                ty.annotations
                    .0
                    .get(&"mcp_resource".parse().unwrap())
                    .map(|_| {
                        RawName::from_name(InternalName::unqualified_name(
                            tyname.clone().into(),
                            None,
                        ))
                    })
            })
            .collect();

        // PANIC SAFETY: converting "mcp_context" into an AnyId should not error
        let contexts = ns
            .entity_types
            .iter()
            .filter_map(|(tyname, ty)| {
                ty.annotations
                    .0
                    .get(&"mcp_context".parse().unwrap())
                    .and_then(|anno| anno.as_ref())
                    .map(|anno| {
                        (anno.val.clone(), RawName::from_name(InternalName::unqualified_name(tyname.clone().into(), None)))
                    })
            }).chain(ns
                .common_types
                .iter()
                .filter_map(|(tyname, ty)| {
                    ty.annotations
                        .0
                        .get(&"mcp_context".parse().unwrap())
                        .and_then(|anno| anno.as_ref())
                        .map(|anno| {
                            (anno.val.clone(), RawName::from_name(InternalName::unqualified_name(tyname.as_ref().clone().into(), None)))
                        })
                })
            )
            .collect();

        Ok(Self {
            fragment: schema_stub,
            namespace: Some(namespace),
            users,
            resources,
            contexts,
        })
    }

    pub fn get_schema(&self) -> &Fragment<RawName> {
        &self.fragment
    }

    pub fn add_action_from_tool_description(&mut self, description: &ToolDescription) -> Result<(), SchemaGeneratorError> {
        // Keep a copy of schema fragment in case we have an error
        let fragment = self.fragment.clone();
        match self.add_action_from_tool_description_inner(description) {
            Ok(_) => Ok(()),
            Err(e) => {
                self.fragment = fragment;
                Err(e)
            }
        }
    }

    pub fn add_actions_from_server_description(&mut self, description: &ServerDescription) -> Result<(), SchemaGeneratorError> {
        // Keep a copy of schema fragment in case we have an error
        let fragment = self.fragment.clone();
        match self.add_actions_from_server_description_inner(description) {
            Ok(_) => Ok(()),
            Err(e) => {
                self.fragment = fragment;
                Err(e)
            }
        }
    }

    pub fn add_actions_from_server_description_inner(&mut self, description: &ServerDescription) -> Result<(), SchemaGeneratorError> {
        // Clone once and reuse to avoid borrow issues
        let namespace = self.namespace.clone();

        // Preemptively add all typedefs as commontypes
        for type_def in description.type_definitions() {
            let ty_name = type_def.name().parse::<UnreservedId>()?;
            let ty = self.cedar_type_from_property_type(
                &namespace,
                ty_name.clone(),
                type_def.property_type(),
            )?;
            self.add_commontype(&namespace, ty, ty_name, true)?;
        }

        for tool_description in description.tool_descriptions() {
            self.add_action_from_tool_description_inner(tool_description)?
        }
        Ok(())
    }

    fn add_action_from_tool_description_inner(
        &mut self,
        description: &ToolDescription
    ) -> Result<(), SchemaGeneratorError> {
        let namespace: Name = description.name().parse()?;
        let namespace = Some(namespace.qualify_with_name(self.namespace.as_ref()));
        self.add_namespace(namespace.clone());

        // Preemptively add all typedefs as commontypes
        for type_def in description.type_definitions() {
            let ty_name = type_def.name().parse::<UnreservedId>()?;
            let ty = self.cedar_type_from_property_type(
                &namespace,
                ty_name.clone(),
                type_def.property_type(),
            )?;
            self.add_commontype(&namespace, ty, ty_name, true)?;
        }

        let input_ns: Name = "Inputs".parse().unwrap();
        let input_ns = Some(input_ns.qualify_with_name(namespace.as_ref()));
        self.add_namespace(input_ns.clone());

        let output_ns: Name = "Outputs".parse().unwrap();
        let output_ns = Some(output_ns.qualify_with_name(namespace.as_ref()));
        self.add_namespace(output_ns.clone());

        let inputs = self.record_from_parameters(description.inputs(), &input_ns)?;
        let outputs = self.record_from_parameters(description.outputs(), &output_ns)?;

        let mut ctx_attrs = self.contexts.iter().map(|(key, ty_name)| {
            (key.clone(),
              TypeOfAttribute {
                ty: Type::Type {
                    ty: TypeVariant::EntityOrCommon { type_name: ty_name.clone() },
                    loc: None
                },
                annotations: Annotations::new(),
                required: true,
              }
            )
        }).collect::<BTreeMap<_,_>>();

        ctx_attrs.insert(
            "inputs".to_smolstr(),
            TypeOfAttribute {
                ty: Type::Type {
                    ty: TypeVariant::Record(inputs),
                    loc: None
                },
                annotations: Annotations::new(),
                required: true,
            }
        );

        ctx_attrs.insert(
            "outputs".to_smolstr(),
            TypeOfAttribute {
                ty: Type::Type {
                    ty: TypeVariant::Record(outputs),
                    loc: None
                },
                annotations: Annotations::new(),
                required: false,
            }
        );

        let action = ActionType {
            attributes: None,
            applies_to: Some(ApplySpec {
                resource_types: self.users.clone(),
                principal_types: self.resources.clone(),
                context: AttributesOrContext(Type::Type {
                    ty: TypeVariant::Record(RecordType {
                        attributes: ctx_attrs,
                        additional_attributes: false,
                    }),
                    loc: None
                })
            }),
            member_of: None,
            annotations: Annotations::new(),
            loc: None,
        };

        self.fragment.0.get_mut(&self.namespace).unwrap().actions.insert(description.name().to_smolstr(), action);


        self.drop_namespace_if_empty(&input_ns);
        self.drop_namespace_if_empty(&output_ns);
        self.drop_namespace_if_empty(&namespace);

        Ok(())
    }

    fn add_namespace(
        &mut self,
        namespace: Option<Name>,
    ) {
        if !self.fragment.0.contains_key(&namespace) {
            let nsdef = NamespaceDefinition {
                common_types: BTreeMap::new(),
                entity_types: BTreeMap::new(),
                actions: BTreeMap::new(),
                annotations: Annotations::new(),
            };
            self.fragment.0.insert(namespace, nsdef);
        }
    }

    fn drop_namespace_if_empty(
        &mut self,
        namespace: &Option<Name>,
    ) {
        if let Some(nsdef) = self.fragment.0.get(&namespace) {
            if nsdef.common_types.len() == 0 && nsdef.entity_types.len() == 0 && nsdef.actions.len() == 0 {
                self.fragment.0.remove(&namespace);
            }
        }
    }

    fn add_commontype(
        &mut self,
        namespace: &Option<Name>,
        ty: Type<RawName>,
        ty_name: UnreservedId,
        error_if_exists: bool,
    ) -> Result<(), SchemaGeneratorError> {
        let ty_name = CommonTypeId::new(ty_name)?;
        // PANIC SAFETY: this function should only be called if namespace belongs to self's fragment
        let nsdef = self.fragment.0.get_mut(namespace).unwrap();

        match nsdef.common_types.entry(ty_name) {
            Entry::Occupied(occ) if error_if_exists => Err(SchemaGeneratorError::conflicting_name(occ.key().to_smolstr())),
            Entry::Occupied(_) => Ok(()),
            Entry::Vacant(vac) => {
                vac.insert(CommonType {
                    ty,
                    annotations: Annotations::new(),
                    loc: None
                });
                Ok(())
            }
        }
    }

    fn add_entitytype(
        &mut self,
        namespace: &Option<Name>,
        ty: EntityType<RawName>,
        ty_name: UnreservedId,
        error_if_exists: bool,
    ) -> Result<(), SchemaGeneratorError> {
        // PANIC SAFETY: this function should only be called if namespace belongs to self's fragment
        let nsdef = self.fragment.0.get_mut(namespace).unwrap();

        match nsdef.entity_types.entry(ty_name) {
            Entry::Occupied(occ) if error_if_exists => Err(SchemaGeneratorError::conflicting_name(occ.key().to_smolstr())),
            Entry::Occupied(_) => Ok(()),
            Entry::Vacant(vac) => {
                vac.insert(ty);
                Ok(())
            }
        }
    }

    fn add_opaque_entity_type(
        &mut self,
        namespace: &Option<Name>,
        ty_name: UnreservedId,
    ) -> Result<(), SchemaGeneratorError> {
        let ty = EntityType {
            kind: EntityTypeKind::Standard(StandardEntityType {
                member_of_types: Vec::new(),
                shape: AttributesOrContext::default(),
                tags: None,
            }),
            annotations: Annotations::new(),
            loc: None,
        };
        self.add_entitytype(namespace, ty, ty_name, false)
    }

    fn record_from_parameters(
        &mut self,
        parameters: &Parameters,
        namespace: &Option<Name>,
    ) -> Result<RecordType<RawName>, SchemaGeneratorError> {

        // Preemptively add all typedefs as commontypes
        for type_def in parameters.type_definitions() {
            let ty_name = type_def.name().parse::<UnreservedId>()?;
            let ty = self.cedar_type_from_property_type(
                &namespace,
                ty_name.clone(),
                type_def.property_type(),
            )?;
            self.add_commontype(&namespace, ty, ty_name, true)?;
        }

        let mut attributes = BTreeMap::new();

        for property in parameters.properties() {
            let attr_name = property.name().to_smolstr();
            let ty_name = property.name().parse()?;

            let ty = self.cedar_type_from_property_type(namespace, ty_name, property.property_type())?;
            let ty = TypeOfAttribute {
                ty,
                annotations: Annotations::new(),
                required: property.is_required(),
            };

            attributes.insert(attr_name, ty);
        }

        Ok(RecordType {
            attributes,
            additional_attributes: false,
        })
    }

    fn cedar_type_from_property_type(
        &mut self,
        namespace: &Option<Name>,
        ty_name: UnreservedId,
        property_type: &PropertyType,
    ) -> Result<Type<RawName>, SchemaGeneratorError> {
        // PANIC SAFETY: by construction namespace should exist

        let variant = match property_type {
            PropertyType::Bool => TypeVariant::Boolean,
            PropertyType::Integer => TypeVariant::Long,
            PropertyType::Float => {
                // PANIC SAFETY: `"Float"` should not be a reserved id
                let name: UnreservedId = "Float".parse().unwrap();
                self.add_opaque_entity_type(&self.namespace.clone(), name.clone())?;
                let name = RawName::new_from_unreserved(name, None);
                let name = RawName::from_name(name.qualify_with_name(self.namespace.as_ref()));
                TypeVariant::Entity { name }
            },
            PropertyType::Number => {
                // PANIC SAFETY: `"Number"` should not be a reserved id
                let name: UnreservedId = "Number".parse().unwrap();
                self.add_opaque_entity_type(&self.namespace.clone(), name.clone())?;
                let name = RawName::new_from_unreserved(name, None);
                let name = RawName::from_name(name.qualify_with_name(self.namespace.as_ref()));
                TypeVariant::Entity { name }
            },
            PropertyType::String => TypeVariant::String,
            PropertyType::Decimal => TypeVariant::Extension { name: "decimal".parse().unwrap() },
            PropertyType::Datetime => TypeVariant::Extension { name: "datetime".parse().unwrap() },
            PropertyType::Duration => TypeVariant::Extension { name: "duration".parse().unwrap() },
            PropertyType::IpAddr => TypeVariant::Extension { name: "ipaddr".parse().unwrap() },
            PropertyType::Null => {
                // PANIC SAFETY: `"Null"` should not be a reserved id
                let name: UnreservedId = "Null".parse().unwrap();
                self.add_opaque_entity_type(&self.namespace.clone(), name.clone())?;
                let name = RawName::new_from_unreserved(name, None);
                let name = RawName::from_name(name.qualify_with_name(self.namespace.as_ref()));
                TypeVariant::Entity { name }
            },
            PropertyType::Enum { variants } => {
                // PANIC SAFETY: variants cannot be empty
                let ty = EntityType {
                    kind: EntityTypeKind::Enum { choices: NonEmpty::from_vec(variants.clone()).unwrap() },
                    annotations: Annotations::new(),
                    loc: None,
                };
                self.add_entitytype(namespace, ty, ty_name.clone(), true)?;
                let name = RawName::new_from_unreserved(ty_name, None);
                let name = RawName::from_name(name.qualify_with_name(namespace.as_ref()));
                TypeVariant::Entity { name }
            },
            PropertyType::Array { element_ty } => {
                let ty = self.cedar_type_from_property_type(
                    namespace,
                    ty_name,
                    element_ty.as_ref()
                )?;
                TypeVariant::Set { element: Box::new(ty) }
            },
            PropertyType::Tuple { types } => {
                let ns: Name = ty_name.into();
                let ns = Some(ns.qualify_with_name(namespace.as_ref()));
                self.add_namespace(ns.clone());
                let attrs = types.iter().enumerate().map(|(i, ptype)| {
                    // PANIC SAFETY: Proj{i} should not fail to parse
                    let proj_tyname: UnreservedId = format!("Proj{i}").as_str().parse().unwrap();
                    let proj = format!("proj{i}").to_smolstr();
                    let ty = self.cedar_type_from_property_type(&ns, proj_tyname, ptype)?;
                    let ty = TypeOfAttribute {
                        ty,
                        annotations: Annotations::new(),
                        required: true
                    };
                    Ok((proj, ty))
                }).collect::<Result<_, SchemaGeneratorError>>()?;
                self.drop_namespace_if_empty(&ns);
                TypeVariant::Record(RecordType {
                    attributes: attrs,
                    additional_attributes: false
                })
            },
            PropertyType::Union { types } => {
                let ns: Name = ty_name.into();
                let ns = Some(ns.qualify_with_name(namespace.as_ref()));
                self.add_namespace(ns.clone());
                let attrs = types.iter().enumerate().map(|(i, ptype)| {
                    // PANIC SAFETY: TypeChoice{i} should not fail to parse
                    let proj_tyname: UnreservedId = format!("TypeChoice{i}").as_str().parse().unwrap();
                    let proj = format!("type_choice{i}").to_smolstr();
                    let ty = self.cedar_type_from_property_type(&ns, proj_tyname, ptype)?;
                    let ty = TypeOfAttribute {
                        ty,
                        annotations: Annotations::new(),
                        required: false
                    };
                    Ok((proj, ty))
                }).collect::<Result<_, SchemaGeneratorError>>()?;
                self.drop_namespace_if_empty(&ns);
                TypeVariant::Record(RecordType {
                    attributes: attrs,
                    additional_attributes: false
                })
            },
            PropertyType::Object { properties, additional_properties } => {
                let ns: Name = ty_name.clone().into();
                let ns = Some(ns.qualify_with_name(namespace.as_ref()));
                self.add_namespace(ns.clone());

                let tag_name = format!("{ty_name}Tag").parse()?;

                let tags = match additional_properties {
                    Some(ptype) => Some(self.cedar_type_from_property_type(
                        &ns,
                        tag_name,
                        ptype.as_ref(),
                    )?),
                    None => None,
                };

                let mut attributes = BTreeMap::new();

                for property in properties {
                    let attr_name = property.name().to_smolstr();
                    let ty_name = property.name().parse()?;

                    let ty = self.cedar_type_from_property_type(namespace, ty_name, property.property_type())?;
                    let ty = TypeOfAttribute {
                        ty,
                        annotations: Annotations::new(),
                        required: property.is_required(),
                    };

                    attributes.insert(attr_name, ty);
                }

                let ty = EntityType {
                    kind: EntityTypeKind::Standard(StandardEntityType {
                        member_of_types: Vec::new(),
                        shape: AttributesOrContext(Type::Type {
                            ty: TypeVariant::Record(RecordType {
                                attributes,
                                additional_attributes: tags.is_some()
                            }),
                            loc: None
                        }),
                        tags: tags,
                    }),
                    annotations: Annotations::new(),
                    loc: None,
                };

                self.add_entitytype(namespace, ty, ty_name.clone(), true)?;

                self.drop_namespace_if_empty(&ns);
                let name = RawName::new_from_unreserved(ty_name, None);
                let name = RawName::from_name(name.qualify_with_name(namespace.as_ref()));
                TypeVariant::Entity { name }
            },
            PropertyType::Ref { name } => {
                let ty_name = CommonTypeId::new(name.parse::<UnreservedId>()?)?;
                let name = self.find_common_types(&ty_name, namespace.clone())?;
                return Ok(Type::CommonTypeRef { type_name: name, loc: None });
            },
        };

        Ok(Type::Type {
            ty: variant,
            loc: None,
        })
    }

    fn find_common_types(
        &self,
        ty_name: &CommonTypeId,
        namespace: Option<Name>,
    ) -> Result<RawName, SchemaGeneratorError> {
        for ns in get_containing_namespaces(namespace.clone()) {
            match self.fragment.0.get(&ns) {
                None => (),
                Some(nsdef) => {
                    if nsdef.common_types.contains_key(ty_name) {
                        if ns == namespace {
                            return Ok(get_refname(None, ty_name))
                        }
                        return Ok(get_refname(ns, ty_name))
                    }
                }
            }
        }
        let ns = match namespace {
            Some(name) => format!("{name}"),
            None => String::new(),
        };
        Err(SchemaGeneratorError::undefined_ref(ty_name.to_string(), ns))
    }
}

fn get_containing_namespaces(namespace: Option<Name>) -> Vec<Option<Name>> {
    let mut ns = Vec::new();

    match namespace {
        Some(name) => {
            let mut name = name;
            while !name.is_unqualified() {
                ns.push(Some(name.clone()));
                let internal_name: InternalName = name.into();
                // PANIC SAFETY: namespace of name should parse
                name = internal_name.namespace().parse().unwrap();
            }
            ns.push(Some(name));
        }
        None => ()
    }

    ns.push(None);

    ns
}

fn get_refname(
    namespace: Option<Name>,
    ty_name: &CommonTypeId,
) -> RawName {
    match namespace {
        Some(name) => format!("{name}::{}", ty_name.to_string()).parse().unwrap(),
        None => ty_name.to_string().parse().unwrap(),
    }
}