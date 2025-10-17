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

use crate::description::{Parameters, PropertyType, ServerDescription, ToolDescription};
use crate::err::SchemaGeneratorError;

use cedar_policy_core::ast::{InternalName, Name, UnreservedId};
use cedar_policy_core::est::Annotations;
use cedar_policy_core::validator::{
    json_schema::{
        ActionType, ApplySpec, AttributesOrContext, CommonType, CommonTypeId, EntityType,
        EntityTypeKind, Fragment, NamespaceDefinition, RecordType, StandardEntityType, Type,
        TypeOfAttribute, TypeVariant,
    },
    RawName,
};

use nonempty::NonEmpty;

use smol_str::{SmolStr, ToSmolStr};

use std::collections::{btree_map::Entry, BTreeMap};

/// A type that allows constructing a Cedar Schema (Fragment)
/// from an input Cedar Schema Stub that defines the Cedar Type of
/// MCP principals, MCP Resources, and common MCP Contects.
///
/// The Generator can then be populated with a number of tool / server
/// descriptions to auto-generate Cedar actions corresponding one-to-one
/// with each tool description.
#[derive(Debug, Clone)]
pub struct SchemaGenerator {
    fragment: Fragment<RawName>,
    namespace: Option<Name>,
    users: Vec<RawName>,
    resources: Vec<RawName>,
    contexts: BTreeMap<SmolStr, RawName>,
}

impl SchemaGenerator {
    /// Create a `SchemaGenerator` from a Cedar Schema Fragment
    pub fn new(schema_stub: Fragment<RawName>) -> Result<Self, SchemaGeneratorError> {
        let (ns, namespace) = match schema_stub.0.iter().next() {
            Some((None, _)) => return Err(SchemaGeneratorError::GlobalNamespaceUsed),
            Some((Some(namespace), ns)) => (ns, namespace.clone()),
            None => return Err(SchemaGeneratorError::WrongNumberOfNamespaces),
        };
        if schema_stub.0.len() > 1 {
            return Err(SchemaGeneratorError::WrongNumberOfNamespaces);
        }

        #[allow(clippy::unwrap_used, reason = "`mcp_principal` is a valid AnyId")]
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

        #[allow(clippy::unwrap_used, reason = "`mcp_resource` is a valid AnyId")]
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

        #[allow(clippy::unwrap_used, reason = "`mcp_context` is a valid AnyId")]
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
                        (
                            anno.val.clone(),
                            RawName::from_name(InternalName::unqualified_name(
                                tyname.clone().into(),
                                None,
                            )),
                        )
                    })
            })
            .chain(ns.common_types.iter().filter_map(|(tyname, ty)| {
                ty.annotations
                    .0
                    .get(&"mcp_context".parse().unwrap())
                    .and_then(|anno| anno.as_ref())
                    .map(|anno| {
                        (
                            anno.val.clone(),
                            RawName::from_name(InternalName::unqualified_name(
                                tyname.as_ref().clone().into(),
                                None,
                            )),
                        )
                    })
            }))
            .collect();

        Ok(Self {
            fragment: schema_stub,
            namespace: Some(namespace),
            users,
            resources,
            contexts,
        })
    }

    /// Get the current Cedar Schema
    pub fn get_schema(&self) -> &Fragment<RawName> {
        &self.fragment
    }

    /// Add a new action to the generated Cedar Schema
    /// that corresponds to the input `ToolDescription`
    pub fn add_action_from_tool_description(
        &mut self,
        description: &ToolDescription,
    ) -> Result<(), SchemaGeneratorError> {
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

    /// Add a new action to the generated Cedar Schema
    /// for each tool description within the `ServerDescription`
    pub fn add_actions_from_server_description(
        &mut self,
        description: &ServerDescription,
    ) -> Result<(), SchemaGeneratorError> {
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

    fn add_actions_from_server_description_inner(
        &mut self,
        description: &ServerDescription,
    ) -> Result<(), SchemaGeneratorError> {
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
        description: &ToolDescription,
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

        #[allow(clippy::unwrap_used, reason = "`Inputs` is a valid Name")]
        // PANIC SAFETY: "Inputs" is a valid Name
        let input_ns: Name = "Inputs".parse().unwrap();
        let input_ns = Some(input_ns.qualify_with_name(namespace.as_ref()));
        self.add_namespace(input_ns.clone());

        #[allow(clippy::unwrap_used, reason = "`Outputs` is a valid Name")]
        // PANIC SAFETY: "Outputs" is a valid Name
        let output_ns: Name = "Outputs".parse().unwrap();
        let output_ns = Some(output_ns.qualify_with_name(namespace.as_ref()));
        self.add_namespace(output_ns.clone());

        let inputs = self.record_from_parameters(description.inputs(), &input_ns)?;
        let outputs = self.record_from_parameters(description.outputs(), &output_ns)?;

        let mut ctx_attrs = self
            .contexts
            .iter()
            .map(|(key, ty_name)| {
                (
                    key.clone(),
                    TypeOfAttribute {
                        ty: Type::Type {
                            ty: TypeVariant::EntityOrCommon {
                                type_name: ty_name.clone(),
                            },
                            loc: None,
                        },
                        annotations: Annotations::new(),
                        required: true,
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();

        ctx_attrs.insert(
            "inputs".to_smolstr(),
            TypeOfAttribute {
                ty: Type::Type {
                    ty: TypeVariant::Record(inputs),
                    loc: None,
                },
                annotations: Annotations::new(),
                required: true,
            },
        );

        ctx_attrs.insert(
            "outputs".to_smolstr(),
            TypeOfAttribute {
                ty: Type::Type {
                    ty: TypeVariant::Record(outputs),
                    loc: None,
                },
                annotations: Annotations::new(),
                required: false,
            },
        );

        let action = ActionType {
            attributes: None,
            applies_to: Some(ApplySpec {
                resource_types: self.resources.clone(),
                principal_types: self.users.clone(),
                context: AttributesOrContext(Type::Type {
                    ty: TypeVariant::Record(RecordType {
                        attributes: ctx_attrs,
                        additional_attributes: false,
                    }),
                    loc: None,
                }),
            }),
            member_of: None,
            annotations: Annotations::new(),
            loc: None,
        };

        #[allow(clippy::unwrap_used, reason = "Namespace exists by construction")]
        // PANIC SAFETY: Constructor ensures self.namespace does belong to the fragment
        self.fragment
            .0
            .get_mut(&self.namespace)
            .unwrap()
            .actions
            .insert(description.name().to_smolstr(), action);

        self.drop_namespace_if_empty(&input_ns);
        self.drop_namespace_if_empty(&output_ns);
        self.drop_namespace_if_empty(&namespace);

        Ok(())
    }

    fn add_namespace(&mut self, namespace: Option<Name>) {
        self.fragment
            .0
            .entry(namespace)
            .or_insert_with(|| NamespaceDefinition {
                common_types: BTreeMap::new(),
                entity_types: BTreeMap::new(),
                actions: BTreeMap::new(),
                annotations: Annotations::new(),
            });
    }

    #[allow(
        clippy::ref_option,
        reason = "More ergonomic for indexing into fragment"
    )]
    fn drop_namespace_if_empty(&mut self, namespace: &Option<Name>) {
        if let Some(nsdef) = self.fragment.0.get(namespace) {
            if nsdef.common_types.is_empty()
                && nsdef.entity_types.is_empty()
                && nsdef.actions.is_empty()
            {
                self.fragment.0.remove(namespace);
            }
        }
    }

    #[allow(
        clippy::ref_option,
        reason = "More ergnomic for indexing into fragment"
    )]
    fn add_commontype(
        &mut self,
        namespace: &Option<Name>,
        ty: Type<RawName>,
        ty_name: UnreservedId,
        error_if_exists: bool,
    ) -> Result<(), SchemaGeneratorError> {
        let ty_name = CommonTypeId::new(ty_name)?;
        #[allow(
            clippy::unwrap_used,
            reason = "This function is only called on namespaces appearing in fragment"
        )]
        // PANIC SAFETY: this function should only be called if namespace belongs to self's fragment
        let nsdef = self.fragment.0.get_mut(namespace).unwrap();

        match nsdef.common_types.entry(ty_name) {
            Entry::Occupied(occ) if error_if_exists => Err(SchemaGeneratorError::conflicting_name(
                occ.key().to_smolstr(),
            )),
            Entry::Occupied(_) => Ok(()),
            Entry::Vacant(vac) => {
                vac.insert(CommonType {
                    ty,
                    annotations: Annotations::new(),
                    loc: None,
                });
                Ok(())
            }
        }
    }

    #[allow(
        clippy::ref_option,
        reason = "More ergnomic for indexing into fragment"
    )]
    fn add_entitytype(
        &mut self,
        namespace: &Option<Name>,
        ty: EntityType<RawName>,
        ty_name: UnreservedId,
        error_if_exists: bool,
    ) -> Result<(), SchemaGeneratorError> {
        #[allow(
            clippy::unwrap_used,
            reason = "This function is only called on namespaces appearing in fragment"
        )]
        // PANIC SAFETY: this function should only be called if namespace belongs to self's fragment
        let nsdef = self.fragment.0.get_mut(namespace).unwrap();

        match nsdef.entity_types.entry(ty_name) {
            Entry::Occupied(occ) if error_if_exists => Err(SchemaGeneratorError::conflicting_name(
                occ.key().to_smolstr(),
            )),
            Entry::Occupied(_) => Ok(()),
            Entry::Vacant(vac) => {
                vac.insert(ty);
                Ok(())
            }
        }
    }

    #[allow(
        clippy::ref_option,
        reason = "More ergnomic for indexing into fragment"
    )]
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

    #[allow(
        clippy::ref_option,
        reason = "More ergnomic for indexing into fragment"
    )]
    fn record_from_parameters(
        &mut self,
        parameters: &Parameters,
        namespace: &Option<Name>,
    ) -> Result<RecordType<RawName>, SchemaGeneratorError> {
        // Preemptively add all typedefs as commontypes
        for type_def in parameters.type_definitions() {
            let ty_name = type_def.name().parse::<UnreservedId>()?;
            let ty = self.cedar_type_from_property_type(
                namespace,
                ty_name.clone(),
                type_def.property_type(),
            )?;
            self.add_commontype(namespace, ty, ty_name, true)?;
        }

        let mut attributes = BTreeMap::new();

        for property in parameters.properties() {
            let attr_name = property.name().to_smolstr();
            let ty_name = property.name().parse()?;

            let ty =
                self.cedar_type_from_property_type(namespace, ty_name, property.property_type())?;
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

    #[allow(
        clippy::ref_option,
        reason = "More ergnomic for indexing into fragment"
    )]
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
                #[allow(clippy::unwrap_used, reason = "`Float` is a valid UnreservedId")]
                // PANIC SAFETY: `"Float"` should not be a reserved id
                let name: UnreservedId = "Float".parse().unwrap();
                self.add_opaque_entity_type(&self.namespace.clone(), name.clone())?;
                let name = RawName::new_from_unreserved(name, None);
                let name = RawName::from_name(name.qualify_with_name(self.namespace.as_ref()));
                TypeVariant::Entity { name }
            }
            PropertyType::Number => {
                #[allow(clippy::unwrap_used, reason = "`Number` is a valid UnreservedId")]
                // PANIC SAFETY: `"Number"` should not be a reserved id
                let name: UnreservedId = "Number".parse().unwrap();
                self.add_opaque_entity_type(&self.namespace.clone(), name.clone())?;
                let name = RawName::new_from_unreserved(name, None);
                let name = RawName::from_name(name.qualify_with_name(self.namespace.as_ref()));
                TypeVariant::Entity { name }
            }
            PropertyType::String => TypeVariant::String,
            PropertyType::Decimal => {
                #[allow(clippy::unwrap_used, reason = "`decimal` is a valid UnreservedId")]
                // PANIC SAFETY: `decimal` is a valid UnreservedId
                TypeVariant::Extension {
                    name: "decimal".parse().unwrap(),
                }
            }
            PropertyType::Datetime => {
                #[allow(clippy::unwrap_used, reason = "`datetime` is a valid UnreservedId")]
                // PANIC SAFETY: `datetime` is a valid UnreservedId
                TypeVariant::Extension {
                    name: "datetime".parse().unwrap(),
                }
            }
            PropertyType::Duration => {
                #[allow(clippy::unwrap_used, reason = "`duration` is a valid UnreservedId")]
                // PANIC SAFETY: `duration` is a valid UnreservedId
                TypeVariant::Extension {
                    name: "duration".parse().unwrap(),
                }
            }
            PropertyType::IpAddr => {
                #[allow(clippy::unwrap_used, reason = "`ipaddr` is a valid UnreservedId")]
                // PANIC SAFETY: `ipaddr` is a valid UnreservedId
                TypeVariant::Extension {
                    name: "ipaddr".parse().unwrap(),
                }
            }
            PropertyType::Null => {
                #[allow(clippy::unwrap_used, reason = "`Null` is a valid UnreservedId")]
                // PANIC SAFETY: `"Null"` should not be a reserved id
                let name: UnreservedId = "Null".parse().unwrap();
                self.add_opaque_entity_type(&self.namespace.clone(), name.clone())?;
                let name = RawName::new_from_unreserved(name, None);
                let name = RawName::from_name(name.qualify_with_name(self.namespace.as_ref()));
                TypeVariant::Entity { name }
            }
            PropertyType::Enum { variants } => {
                if variants.is_empty() {
                    return Err(SchemaGeneratorError::empty_enum_choice(format!(
                        "{ty_name}"
                    )));
                }
                #[allow(clippy::unwrap_used, reason = "variants are not empty")]
                // PANIC SAFETY: variants cannot be empty
                let ty = EntityType {
                    kind: EntityTypeKind::Enum {
                        choices: NonEmpty::from_vec(variants.clone()).unwrap(),
                    },
                    annotations: Annotations::new(),
                    loc: None,
                };
                self.add_entitytype(namespace, ty, ty_name.clone(), true)?;
                let name = RawName::new_from_unreserved(ty_name, None);
                let name = RawName::from_name(name.qualify_with_name(namespace.as_ref()));
                TypeVariant::Entity { name }
            }
            PropertyType::Array { element_ty } => {
                let ty =
                    self.cedar_type_from_property_type(namespace, ty_name, element_ty.as_ref())?;
                TypeVariant::Set {
                    element: Box::new(ty),
                }
            }
            PropertyType::Tuple { types } => {
                let ns: Name = ty_name.into();
                let ns = Some(ns.qualify_with_name(namespace.as_ref()));
                self.add_namespace(ns.clone());
                let attrs = types
                    .iter()
                    .enumerate()
                    .map(|(i, ptype)| {
                        #[allow(clippy::unwrap_used, reason = "`Proj{i}` is a valid UnreservedId")]
                        // PANIC SAFETY: Proj{i} should not fail to parse
                        let proj_tyname: UnreservedId =
                            format!("Proj{i}").as_str().parse().unwrap();
                        let proj = format!("proj{i}").to_smolstr();
                        let ty = self.cedar_type_from_property_type(&ns, proj_tyname, ptype)?;
                        let ty = TypeOfAttribute {
                            ty,
                            annotations: Annotations::new(),
                            required: true,
                        };
                        Ok((proj, ty))
                    })
                    .collect::<Result<_, SchemaGeneratorError>>()?;
                self.drop_namespace_if_empty(&ns);
                TypeVariant::Record(RecordType {
                    attributes: attrs,
                    additional_attributes: false,
                })
            }
            PropertyType::Union { types } => {
                let ns: Name = ty_name.into();
                let ns = Some(ns.qualify_with_name(namespace.as_ref()));
                self.add_namespace(ns.clone());
                let attrs = types
                    .iter()
                    .enumerate()
                    .map(|(i, ptype)| {
                        #[allow(
                            clippy::unwrap_used,
                            reason = "`TypeChoice{i}` is a valid UnreservedId"
                        )]
                        // PANIC SAFETY: TypeChoice{i} should not fail to parse
                        let proj_tyname: UnreservedId =
                            format!("TypeChoice{i}").as_str().parse().unwrap();
                        let proj = format!("type_choice{i}").to_smolstr();
                        let ty = self.cedar_type_from_property_type(&ns, proj_tyname, ptype)?;
                        let ty = TypeOfAttribute {
                            ty,
                            annotations: Annotations::new(),
                            required: false,
                        };
                        Ok((proj, ty))
                    })
                    .collect::<Result<_, SchemaGeneratorError>>()?;
                self.drop_namespace_if_empty(&ns);
                TypeVariant::Record(RecordType {
                    attributes: attrs,
                    additional_attributes: false,
                })
            }
            PropertyType::Object {
                properties,
                additional_properties,
            } => {
                let ns: Name = ty_name.clone().into();
                let ns = Some(ns.qualify_with_name(namespace.as_ref()));
                self.add_namespace(ns.clone());

                let tag_name = format!("{ty_name}Tag").parse()?;

                let tags = match additional_properties {
                    Some(ptype) => {
                        Some(self.cedar_type_from_property_type(&ns, tag_name, ptype.as_ref())?)
                    }
                    None => None,
                };

                let mut attributes = BTreeMap::new();

                for property in properties {
                    let attr_name = property.name().to_smolstr();
                    let ty_name = property.name().parse()?;

                    let ty =
                        self.cedar_type_from_property_type(&ns, ty_name, property.property_type())?;
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
                                additional_attributes: tags.is_some(),
                            }),
                            loc: None,
                        }),
                        tags,
                    }),
                    annotations: Annotations::new(),
                    loc: None,
                };

                self.add_entitytype(namespace, ty, ty_name.clone(), true)?;

                self.drop_namespace_if_empty(&ns);
                let name = RawName::new_from_unreserved(ty_name, None);
                let name = RawName::from_name(name.qualify_with_name(namespace.as_ref()));
                TypeVariant::Entity { name }
            }
            PropertyType::Ref { name } => {
                let ty_name = CommonTypeId::new(name.parse::<UnreservedId>()?)?;
                let name = self.find_common_types(&ty_name, namespace.clone())?;
                return Ok(Type::CommonTypeRef {
                    type_name: name,
                    loc: None,
                });
            }
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
                            return Ok(get_refname(None, ty_name));
                        }
                        return Ok(get_refname(ns, ty_name));
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

    #[allow(clippy::unwrap_used, reason = "Subnames of a name should parse")]
    if let Some(name) = namespace {
        let mut name = name;
        while !name.is_unqualified() {
            ns.push(Some(name.clone()));
            let internal_name: InternalName = name.into();
            // PANIC SAFETY: namespace of name should parse
            name = internal_name.namespace().parse().unwrap();
        }
        ns.push(Some(name));
    }

    ns.push(None);

    ns
}

fn get_refname(namespace: Option<Name>, ty_name: &CommonTypeId) -> RawName {
    match namespace {
        Some(name) => {
            #[allow(
                clippy::unwrap_used,
                reason = "Parsing the combined namespace and type_name should still parse"
            )]
            // PANIC SAFETY: Combining a namespace & CommonTypeId should parese as a RawName
            format!("{name}::{}", ty_name).parse().unwrap()
        }
        None => {
            #[allow(
                clippy::unwrap_used,
                reason = "Parsing a valid typename's string representation should parse"
            )]
            // PANIC SAFETY: Parsing a valid typename's string representation should parse
            ty_name.to_string().parse().unwrap()
        }
    }
}
