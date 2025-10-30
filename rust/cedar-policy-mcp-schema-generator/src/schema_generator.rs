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
        ActionEntityUID, ActionType, ApplySpec, AttributesOrContext, CommonType, CommonTypeId,
        EntityType, EntityTypeKind, Fragment, NamespaceDefinition, RecordType, StandardEntityType,
        Type, TypeOfAttribute, TypeVariant,
    },
    RawName,
};

use nonempty::NonEmpty;

use smol_str::{SmolStr, ToSmolStr};

use std::collections::{btree_map::Entry, BTreeMap};

/// A type reserved to configure how the schema generator functions
#[derive(Debug, Clone)]
pub struct SchemaGeneratorConfig {
    include_outputs: bool,
    objects_as_records: bool,
    erase_annotations: bool,
    flatten_namespaces: bool,
}

impl SchemaGeneratorConfig {
    /// Default configuration of Schema Generator
    pub fn default() -> Self {
        Self {
            include_outputs: false,
            objects_as_records: false,
            erase_annotations: true,
            flatten_namespaces: false,
        }
    }

    /// Updates config to set `include_outputs` to `val` (default: false)
    /// if `include_outputs` is set to `true`, then the schema generator
    /// will generate actions for each tool whose context includes both the
    /// input and output parameters of the MCP tool.
    pub fn include_outputs(self, val: bool) -> Self {
        Self {
            include_outputs: val,
            ..self
        }
    }

    /// Updates config to set `objects_as_records` to `val` (default: false)
    /// If `objects_as_records` is set to `false`, then all objects will be
    /// represented as an entity type. If `objects_as_records` is set to `true`,
    /// then any object that does not allow for "additionalProperties" will be
    /// encoded in cedar as a record type. Note, in both settings, objects with
    /// "additionalProperties" will be encoded as an entity type with tags.
    pub fn objects_as_records(self, val: bool) -> Self {
        Self {
            objects_as_records: val,
            ..self
        }
    }

    /// Updates config to set `erase_annotations` to `val` (default: true)
    /// If `erase_annotations` is set to `true`, then all `mcp_principal`,
    /// `mcp_resource`, `mcp_context`, and `mcp_acation` annotations
    /// will be erased in the output schema fragment.
    pub fn erase_annotations(self, val: bool) -> Self {
        Self {
            erase_annotations: val,
            ..self
        }
    }

    /// Updates config to set `flatten_namespaces` to `val` (default: false)
    ///
    /// If `flatten_namespaces` is set to `true` then the fragment returned
    /// by `SchemaGenerator::get_schema` will contain only the input namespace
    ///
    /// This is accomplished by converting every name `Foo::Bar::Baz` to `Foo_Bar_Baz`.
    /// Note, this process may result in a malformed schema if this renaming process
    /// produces conflicting names. For example, if the produced schema (without flattened names)
    /// would contain the names `Foo::Bar_Baz` and `Foo_Bar::Baz`.
    pub fn flatten_namespaces(self, val: bool) -> Self {
        Self {
            flatten_namespaces: val,
            ..self
        }
    }
}

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
    actions: Option<Vec<ActionEntityUID<RawName>>>,
    config: SchemaGeneratorConfig,
}

impl SchemaGenerator {
    /// Create a `SchemaGenerator` from a Cedar Schema Fragment using default configuration
    pub fn new(schema_stub: Fragment<RawName>) -> Result<Self, SchemaGeneratorError> {
        Self::new_with_config(schema_stub, SchemaGeneratorConfig::default())
    }

    /// Create a `SchemaGenerator` from a Cedar Schema Fragment using specified configuration
    pub fn new_with_config(
        schema_stub: Fragment<RawName>,
        config: SchemaGeneratorConfig,
    ) -> Result<Self, SchemaGeneratorError> {
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

        #[allow(clippy::unwrap_used, reason = "`mcp_action` is a valid AnyId")]
        // PANIC SAFETY: converting "mcp_action" into an AnyId should not error
        let actions = ns
            .actions
            .iter()
            .filter_map(|(name, action)| {
                action
                    .annotations
                    .0
                    .get(&"mcp_action".parse().unwrap())
                    .map(|_| ActionEntityUID::new(None, name.clone()))
            })
            .collect::<Vec<_>>();
        let actions = if actions.is_empty() {
            None
        } else {
            Some(actions)
        };

        let fragment = if config.erase_annotations {
            erase_mcp_annotations(schema_stub)
        } else {
            schema_stub
        };

        Ok(Self {
            fragment,
            namespace: Some(namespace),
            users,
            resources,
            contexts,
            actions,
            config,
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
        match self.add_action_from_tool_description_inner(description, BTreeMap::new()) {
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

        // Populate a map from type ref names to fully qualified type name
        // This makes type resolution simpler and will allow for mutually recursive type defs
        let mut common_types = BTreeMap::new();
        for type_def in description.type_definitions() {
            let type_name = CommonTypeId::new(type_def.name().parse()?)?;
            let type_name = get_refname(&namespace, &type_name);
            let ref_name = type_def.name.to_smolstr();
            common_types.insert(ref_name, type_name);
        }

        // Preemptively add all typedefs as commontypes
        for type_def in description.type_definitions() {
            let ty_name = type_def.name().parse::<UnreservedId>()?;
            let ty = self.cedar_type_from_property_type(
                &namespace,
                ty_name.clone(),
                type_def.property_type(),
                &common_types,
            )?;
            self.add_commontype(&namespace, ty, ty_name, true)?;
        }

        for tool_description in description.tool_descriptions() {
            self.add_action_from_tool_description_inner(tool_description, common_types.clone())?
        }
        Ok(())
    }

    fn add_action_from_tool_description_inner(
        &mut self,
        description: &ToolDescription,
        mut common_types: BTreeMap<SmolStr, RawName>,
    ) -> Result<(), SchemaGeneratorError> {
        let namespace: Name = description.name().parse()?;
        let namespace = Some(namespace.qualify_with_name(self.namespace.as_ref()));
        self.add_namespace(namespace.clone());

        // Populate a map from type ref names to fully qualified type name
        // This makes type resolution simpler and will allow for mutually recursive type defs
        for type_def in description.type_definitions() {
            let type_name = CommonTypeId::new(type_def.name().parse()?)?;
            let type_name = get_refname(&namespace, &type_name);
            let ref_name = type_def.name.to_smolstr();
            // Resolution rules are that defs defined closer to use are preferred
            // So we can just overwrite here if a name is redefined
            common_types.insert(ref_name, type_name);
        }

        // Preemptively add all typedefs as commontypes
        for type_def in description.type_definitions() {
            let ty_name = type_def.name().parse::<UnreservedId>()?;
            let ty = self.cedar_type_from_property_type(
                &namespace,
                ty_name.clone(),
                type_def.property_type(),
                &common_types,
            )?;
            self.add_commontype(&namespace, ty, ty_name, true)?;
        }

        // Shared Common (input Context Types)
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

        // Create a `toolnameInput` type to capture inputs to mcp tool
        #[allow(clippy::unwrap_used, reason = "`Input` is a valid Name")]
        // PANIC SAFETY: "Input" is a valid Name
        let input_ns: Name = "Input".parse().unwrap();
        let input_ns = Some(input_ns.qualify_with_name(namespace.as_ref()));

        self.add_namespace(input_ns.clone());
        let inputs =
            self.record_from_parameters(description.inputs(), &input_ns, common_types.clone())?;
        self.drop_namespace_if_empty(&input_ns);

        let input_type = Type::Type {
            ty: TypeVariant::Record(inputs),
            loc: None,
        };
        let tool_input_ty_name: UnreservedId =
            format!("{}Input", description.name()).as_str().parse()?;
        let parent_namespace = self.namespace.clone();
        self.add_commontype(
            &parent_namespace,
            input_type,
            tool_input_ty_name.clone(),
            true,
        )?;

        ctx_attrs.insert(
            "input".to_smolstr(),
            TypeOfAttribute {
                ty: Type::CommonTypeRef {
                    type_name: RawName::new_from_unreserved(tool_input_ty_name, None),
                    loc: None,
                },
                annotations: Annotations::new(),
                required: true,
            },
        );

        if self.config.include_outputs {
            #[allow(clippy::unwrap_used, reason = "`Output` is a valid Name")]
            // PANIC SAFETY: "Outputs" is a valid Name
            let output_ns: Name = "Output".parse().unwrap();
            let output_ns = Some(output_ns.qualify_with_name(namespace.as_ref()));

            self.add_namespace(output_ns.clone());
            let outputs = self.record_from_parameters(
                description.outputs(),
                &output_ns,
                common_types.clone(),
            )?;
            self.drop_namespace_if_empty(&output_ns);

            let output_type = Type::Type {
                ty: TypeVariant::Record(outputs),
                loc: None,
            };
            let tool_output_ty_name: UnreservedId =
                format!("{}Output", description.name()).parse()?;
            self.add_commontype(
                &parent_namespace,
                output_type,
                tool_output_ty_name.clone(),
                true,
            )?;
            ctx_attrs.insert(
                "output".to_smolstr(),
                TypeOfAttribute {
                    ty: Type::CommonTypeRef {
                        type_name: RawName::new_from_unreserved(tool_output_ty_name, None),
                        loc: None,
                    },
                    annotations: Annotations::new(),
                    required: false,
                },
            );
        }

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
            member_of: self.actions.clone(),
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

        self.drop_namespace_if_empty(&namespace);

        Ok(())
    }

    // This function should only be called when name is prefixed by `{self.namespace}::`
    // Converts `{self.namespace}::Foo::Bar::Baz` to `{self.namespace}::Foo_Bar_Baz`.
    fn flatten_internalname(&self, name: InternalName) -> InternalName {
        if self.config.flatten_namespaces {
            let name = name.to_string();
            let name = if self.namespace.is_some() {
                // PANIC SAFETY: by assumption name should include at least one "::"
                #[allow(
                    clippy::unwrap_used,
                    reason = "by assumption name should include at least one \"::\""
                )]
                name.split_once("::").unwrap().1.to_string()
            } else {
                name
            };
            let name = name.replace("::", "_");
            // PANIC SAFETY: name should still parse after converting "::" to "_"
            #[allow(
                clippy::unwrap_used,
                reason = "name should still parse after converting \"::\" to \"_\""
            )]
            let name: InternalName = name.parse().unwrap();
            name.qualify_with_name(self.namespace.as_ref())
        } else {
            name
        }
    }

    // This function should only be called when name is prefixed by `{self.namespace}::`
    // Converts `{self.namespace}::Foo::Bar::Baz` to `{self.namespace}::Foo_Bar_Baz`.
    fn flatten_rawname(&self, name: RawName) -> RawName {
        if self.config.flatten_namespaces {
            RawName::from_name(self.flatten_internalname(name.qualify_with(None)))
        } else {
            name
        }
    }

    // This function should only be called when namespace is prefixed by `{self.namespace}::`
    // If `namespace` is `{self.namespace}::Foo::Bar::Baz` then this function returns the id `Foo_Bar_Baz_id`.
    fn flatten_unreserved_id(&self, id: UnreservedId, namespace: &Option<Name>) -> UnreservedId {
        if self.config.flatten_namespaces {
            let name = Name::unqualified_name(id).qualify_with_name(namespace.as_ref());
            let name = name.qualify_with(None);
            let name = self.flatten_internalname(name);
            // PANIC SAFETY: the basename should be unreserved because the original id used to construct it is unreserved
            #[allow(
                clippy::unwrap_used,
                reason = "the basename should be unreserved because the original id used to construct it is unreserved"
            )]
            UnreservedId::try_from(name.basename().clone()).unwrap()
        } else {
            id
        }
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
        let (namespace, ty_name) = if self.config.flatten_namespaces {
            (
                &self.namespace,
                self.flatten_unreserved_id(ty_name, namespace),
            )
        } else {
            (namespace, ty_name)
        };

        let ty_rawname = RawName::new_from_unreserved(ty_name.clone(), None);
        match ty {
            Type::CommonTypeRef { type_name, loc: _ }
                if unqualify_name(namespace, type_name.clone()) == ty_rawname =>
            {
                return Ok(())
            }
            Type::Type {
                ty: TypeVariant::Entity { name },
                loc: _,
            } if unqualify_name(namespace, name.clone()) == ty_rawname => return Ok(()),
            Type::Type {
                ty: TypeVariant::EntityOrCommon { type_name },
                loc: _,
            } if unqualify_name(namespace, type_name.clone()) == ty_rawname => return Ok(()),
            _ => (),
        }

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
        let (namespace, ty_name) = if self.config.flatten_namespaces {
            (
                &self.namespace,
                self.flatten_unreserved_id(ty_name, namespace),
            )
        } else {
            (namespace, ty_name)
        };

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
        mut common_types: BTreeMap<SmolStr, RawName>,
    ) -> Result<RecordType<RawName>, SchemaGeneratorError> {
        // Populate a map from type ref names to fully qualified type name
        // This makes type resolution simpler and will allow for mutually recursive type defs
        for type_def in parameters.type_definitions() {
            let type_name = CommonTypeId::new(type_def.name().parse()?)?;
            let type_name = get_refname(&namespace, &type_name);
            let ref_name = type_def.name.to_smolstr();
            // Resolution rules are that defs defined closer to use are preferred
            // So we can just overwrite here if a name is redefined
            common_types.insert(ref_name, type_name);
        }

        // Preemptively add all typedefs as commontypes
        for type_def in parameters.type_definitions() {
            let ty_name = type_def.name().parse::<UnreservedId>()?;
            let ty = self.cedar_type_from_property_type(
                namespace,
                ty_name.clone(),
                type_def.property_type(),
                &common_types,
            )?;
            self.add_commontype(namespace, ty, ty_name, true)?;
        }

        let mut attributes = BTreeMap::new();

        for property in parameters.properties() {
            let attr_name = property.name().to_smolstr();
            let ty_name = property.name().parse()?;

            let ty = self.cedar_type_from_property_type(
                namespace,
                ty_name,
                property.property_type(),
                &common_types,
            )?;
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
        common_types: &BTreeMap<SmolStr, RawName>,
    ) -> Result<Type<RawName>, SchemaGeneratorError> {
        // PANIC SAFETY: `Bool` is a valid `RawName`
        #[allow(clippy::unwrap_used, reason = "`Bool` is a valid `RawName`")]
        let bool = TypeVariant::EntityOrCommon {
            type_name: "Bool".parse().unwrap(),
        };
        // PANIC SAFETY: `Long` is a valid `RawName`
        #[allow(clippy::unwrap_used, reason = "`Long` is a valid `RawName`")]
        let long = TypeVariant::EntityOrCommon {
            type_name: "Long".parse().unwrap(),
        };
        // PANIC SAFETY: `String` is a valid `RawName`
        #[allow(clippy::unwrap_used, reason = "`String` is a valid `RawName`")]
        let string = TypeVariant::EntityOrCommon {
            type_name: "String".parse().unwrap(),
        };
        // PANIC SAFETY: `decimal` is a valid `RawName`
        #[allow(clippy::unwrap_used, reason = "`decimal` is a valid `RawName`")]
        let decimal = TypeVariant::EntityOrCommon {
            type_name: "decimal".parse().unwrap(),
        };
        // PANIC SAFETY: `datetime` is a valid `RawName`
        #[allow(clippy::unwrap_used, reason = "`datetime` is a valid `RawName`")]
        let datetime = TypeVariant::EntityOrCommon {
            type_name: "datetime".parse().unwrap(),
        };
        // PANIC SAFETY: `duration` is a valid `RawName`
        #[allow(clippy::unwrap_used, reason = "`duration` is a valid `RawName`")]
        let duration = TypeVariant::EntityOrCommon {
            type_name: "duration".parse().unwrap(),
        };
        // PANIC SAFETY: `ipaddr` is a valid `RawName`
        #[allow(clippy::unwrap_used, reason = "`ipaddr` is a valid `RawName`")]
        let ipaddr = TypeVariant::EntityOrCommon {
            type_name: "ipaddr".parse().unwrap(),
        };

        let variant = match property_type {
            PropertyType::Bool => bool,
            PropertyType::Integer => long,
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
            PropertyType::String => string,
            PropertyType::Decimal => decimal,
            PropertyType::Datetime => datetime,
            PropertyType::Duration => duration,
            PropertyType::IpAddr => ipaddr,
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
                TypeVariant::Entity {
                    name: self.flatten_rawname(name),
                }
            }
            PropertyType::Array { element_ty } => {
                let ty = self.cedar_type_from_property_type(
                    namespace,
                    ty_name,
                    element_ty.as_ref(),
                    common_types,
                )?;
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
                        let ty = self.cedar_type_from_property_type(
                            &ns,
                            proj_tyname,
                            ptype,
                            common_types,
                        )?;
                        let ty = TypeOfAttribute {
                            ty: unqualify_type(namespace, ty),
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
                        let proj = format!("typeChoice{i}").to_smolstr();
                        let ty = self.cedar_type_from_property_type(
                            &ns,
                            proj_tyname,
                            ptype,
                            common_types,
                        )?;
                        let ty = TypeOfAttribute {
                            ty: unqualify_type(namespace, ty),
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
                    Some(ptype) => Some(self.cedar_type_from_property_type(
                        &ns,
                        tag_name,
                        ptype.as_ref(),
                        common_types,
                    )?),
                    None => None,
                };

                let mut attributes = BTreeMap::new();

                for property in properties {
                    let attr_name = property.name().to_smolstr();
                    let ty_name = property.name().parse()?;

                    let ty = self.cedar_type_from_property_type(
                        &ns,
                        ty_name,
                        property.property_type(),
                        common_types,
                    )?;
                    let ty = TypeOfAttribute {
                        ty: unqualify_type(namespace, ty),
                        annotations: Annotations::new(),
                        required: property.is_required(),
                    };

                    attributes.insert(attr_name, ty);
                }

                // Encode as record if possible and allowed
                if self.config.objects_as_records && tags.is_none() {
                    let ty = Type::Type {
                        ty: TypeVariant::Record(RecordType {
                            attributes,
                            additional_attributes: false,
                        }),
                        loc: None,
                    };
                    self.add_commontype(namespace, ty, ty_name.clone(), true)?;
                } else {
                    // otherwise encode as EntityType
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
                }

                self.drop_namespace_if_empty(&ns);
                let name = RawName::new_from_unreserved(ty_name, None);
                let name = RawName::from_name(name.qualify_with_name(namespace.as_ref()));
                TypeVariant::Entity {
                    name: self.flatten_rawname(name),
                }
            }
            PropertyType::Ref { name } => match common_types.get(name) {
                None => {
                    let ns = match namespace {
                        None => "".into(),
                        Some(name) => format!("{}", name),
                    };
                    return Err(SchemaGeneratorError::undefined_ref(name.to_string(), ns));
                }
                Some(name) => {
                    return Ok(Type::Type {
                        ty: TypeVariant::EntityOrCommon {
                            type_name: self.flatten_rawname(name.clone()),
                        },
                        loc: None,
                    })
                }
            },
        };

        Ok(Type::Type {
            ty: variant,
            loc: None,
        })
    }
}

fn get_refname(namespace: &Option<Name>, ty_name: &CommonTypeId) -> RawName {
    RawName::from_name(
        RawName::new_from_unreserved(ty_name.as_ref().clone(), None)
            .qualify_with_name(namespace.as_ref()),
    )
}

// If Type is an entity or common type qualified by namespace, then unquality it;
// otherwise return the original type
fn unqualify_type(namespace: &Option<Name>, ty: Type<RawName>) -> Type<RawName> {
    match ty {
        Type::CommonTypeRef { type_name, loc } => Type::CommonTypeRef {
            type_name: unqualify_name(namespace, type_name),
            loc,
        },
        Type::Type {
            ty: TypeVariant::Entity { name },
            loc,
        } => Type::Type {
            ty: TypeVariant::Entity {
                name: unqualify_name(namespace, name),
            },
            loc,
        },
        Type::Type {
            ty: TypeVariant::EntityOrCommon { type_name },
            loc,
        } => Type::Type {
            ty: TypeVariant::EntityOrCommon {
                type_name: unqualify_name(namespace, type_name),
            },
            loc,
        },
        Type::Type {
            ty:
                TypeVariant::Record(RecordType {
                    attributes,
                    additional_attributes,
                }),
            loc,
        } => {
            let attributes = attributes
                .into_iter()
                .map(|(name, ty)| {
                    (
                        name,
                        TypeOfAttribute {
                            ty: unqualify_type(namespace, ty.ty),
                            annotations: ty.annotations,
                            required: ty.required,
                        },
                    )
                })
                .collect();
            Type::Type {
                ty: TypeVariant::Record(RecordType {
                    attributes,
                    additional_attributes,
                }),
                loc,
            }
        }
        Type::Type {
            ty: TypeVariant::Set { element },
            loc,
        } => {
            let element = unqualify_type(namespace, element.as_ref().clone());
            Type::Type {
                ty: TypeVariant::Set {
                    element: Box::new(element),
                },
                loc,
            }
        }
        ty => ty,
    }
}

// If name is qualified with namespace then return unqualified name
fn unqualify_name(namespace: &Option<Name>, name: RawName) -> RawName {
    match namespace {
        None => name,
        Some(ns) => {
            let internal_name = name.qualify_with(None);
            if internal_name.namespace() == ns.to_string() {
                RawName::from_name(internal_name.basename().clone().into())
            } else {
                name
            }
        }
    }
}

fn erase_mcp_annotations(schema_stub: Fragment<RawName>) -> Fragment<RawName> {
    let ns = schema_stub
        .0
        .into_iter()
        .map(|(name, nsdef)| {
            let common_types = nsdef
                .common_types
                .into_iter()
                .map(|(ty_name, ty)| {
                    let ty = CommonType {
                        annotations: Annotations(
                            ty.annotations
                                .0
                                .into_iter()
                                .filter(|(anno, _)| anno.as_ref() != "mcp_context")
                                .collect(),
                        ),
                        ..ty
                    };
                    (ty_name, ty)
                })
                .collect();
            let entity_types = nsdef
                .entity_types
                .into_iter()
                .map(|(ty_name, ty)| {
                    let ty = EntityType {
                        annotations: Annotations(
                            ty.annotations
                                .0
                                .into_iter()
                                .filter(|(anno, _)| {
                                    anno.as_ref() != "mcp_context"
                                        && anno.as_ref() != "mcp_resource"
                                        && anno.as_ref() != "mcp_principal"
                                })
                                .collect(),
                        ),
                        ..ty
                    };
                    (ty_name, ty)
                })
                .collect();
            let actions = nsdef
                .actions
                .into_iter()
                .map(|(name, act)| {
                    let act = ActionType {
                        annotations: Annotations(
                            act.annotations
                                .0
                                .into_iter()
                                .filter(|(anno, _)| anno.as_ref() != "mcp_action")
                                .collect(),
                        ),
                        ..act
                    };
                    (name, act)
                })
                .collect();
            (
                name,
                NamespaceDefinition {
                    common_types,
                    entity_types,
                    actions,
                    ..nsdef
                },
            )
        })
        .collect();
    Fragment(ns)
}

#[cfg(test)]
mod test {
    use super::*;
    use cedar_policy_core::extensions::Extensions;

    fn test_schema_stub() -> Fragment<RawName> {
        let schema = r#"namespace Test {
    @mcp_principal("User")
    entity user;

    @mcp_resource("McpServer")
    entity resource;

    @mcp_context("foo")
    entity Foo;
}"#;
        Fragment::from_cedarschema_str(schema, Extensions::all_available())
            .expect("Failed to parse schema")
            .0
    }

    #[test]
    fn test_outputs() {
        let schema_stub = test_schema_stub();
        let config = SchemaGeneratorConfig::default().include_outputs(true);

        let tool = r#"{
    "name": "check_task_status",
    "description": "Check if a task is ready for work",
    "inputSchema": {
        "type": "object",
        "properties": {
            "task_id": {"type": "string"}
        },
        "required": ["task_id"]
    },
    "outputSchema": {
        "type": "object",
        "properties": {
            "status": {
                "type": "string",
                "enum": ["started", "paused", "failed", "completed"]
            },
            "priority": {"type": "integer"}
        },
        "required": ["status", "priority"]
    }
}"#;
        let tool = ToolDescription::from_json_str(tool).expect("Failed to parse tool description");

        let mut schema_generator = SchemaGenerator::new_with_config(schema_stub, config)
            .expect("Failed to create schema generator");
        schema_generator
            .add_action_from_tool_description(&tool)
            .expect("Failed to add tool description");

        let schema = schema_generator.get_schema();

        assert!(schema.0.iter().count() == 2, "Expected only two namespaces");

        let root_namespace = Some("Test".parse::<Name>().unwrap());
        let output_namespace = Some("Test::check_task_status::Output".parse::<Name>().unwrap());

        let root_nsdef = schema
            .0
            .get(&root_namespace)
            .expect("Expected namespace Test to exist");
        let output_nsdef = schema
            .0
            .get(&output_namespace)
            .expect("Expected namespace Test::check_status::Output to exist");

        assert!(root_nsdef.actions.contains_key("check_task_status"));
        assert!(output_nsdef.actions.is_empty());
        assert!(output_nsdef.common_types.is_empty());
        assert!(output_nsdef.entity_types.iter().count() == 1);
        assert!(output_nsdef
            .entity_types
            .contains_key(&"status".parse().unwrap()))
    }

    #[test]
    fn test_objects_as_records() {
        let schema_stub = test_schema_stub();
        let config = SchemaGeneratorConfig::default().objects_as_records(true);

        let tool = r#"{
    "name": "test_tool",
    "description": "A tool for testing purposes",
    "parameters": {
        "type": "object",
        "properties": {
            "test_dt": {"type": "string", "format": "date-time"},
            "test_obj": {
                "type": "object",
                "properties": {
                    "test1": {"type": "float"},
                    "test2": {"type": "string"}
                }
            },
            "test_obj2": { "type": "object", "additionalProperties": {"type": "string"} }
        },
        "required": ["test_dt"]
    }
}"#;

        let tool = ToolDescription::from_json_str(tool).expect("Failed to parse tool description");

        let mut schema_generator = SchemaGenerator::new_with_config(schema_stub, config)
            .expect("Failed to create schema generator");
        schema_generator
            .add_action_from_tool_description(&tool)
            .expect("Failed to add tool description");

        let schema = schema_generator.get_schema();

        let root_namespace = Some("Test".parse::<Name>().unwrap());
        let input_namespace = Some("Test::test_tool::Input".parse::<Name>().unwrap());

        let root_nsdef = schema
            .0
            .get(&root_namespace)
            .expect("Expected namespace Test to exist");
        let input_nsdef = schema
            .0
            .get(&input_namespace)
            .expect("Expected namespace Test::test_tool::Input to exist");

        assert!(root_nsdef.actions.contains_key("test_tool"));
        assert!(input_nsdef.actions.is_empty());

        assert!(input_nsdef.common_types.iter().count() == 1);
        assert!(input_nsdef
            .common_types
            .contains_key(&CommonTypeId::unchecked("test_obj".parse().unwrap())));

        assert!(input_nsdef.entity_types.iter().count() == 1);
        assert!(input_nsdef
            .entity_types
            .contains_key(&"test_obj2".parse().unwrap()));
    }

    #[test]
    fn test_dont_erase_annotations() {
        let schema_stub = test_schema_stub();
        let config = SchemaGeneratorConfig::default().erase_annotations(false);

        let tool = r#"{
    "name": "test_tool",
    "description": "A tool for testing purposes",
    "parameters": {
        "type": "object",
        "properties": {
            "test_dt": {"type": [{"type": "string", "format": "date-time"}, {"type": "string", "format": "date"}]}
        },
        "required": ["test_dt"]
    }
}"#;

        let tool = ToolDescription::from_json_str(tool).expect("Failed to parse tool description");

        let mut schema_generator = SchemaGenerator::new_with_config(schema_stub, config)
            .expect("Failed to create schema generator");
        schema_generator
            .add_action_from_tool_description(&tool)
            .expect("Failed to add tool description");

        let schema = schema_generator.get_schema();

        let root_namespace = Some("Test".parse::<Name>().unwrap());

        assert!(schema.0.iter().count() == 1);
        let root_nsdef = schema
            .0
            .get(&root_namespace)
            .expect("Expected namespace Test to exist");

        assert!(root_nsdef.actions.contains_key("test_tool"));
        assert!(root_nsdef.common_types.iter().count() == 1);

        assert!(root_nsdef.entity_types.iter().count() == 3);
        assert!(root_nsdef
            .entity_types
            .get(&"user".parse().unwrap())
            .unwrap()
            .annotations
            .0
            .contains_key(&"mcp_principal".parse().unwrap()));
        assert!(root_nsdef
            .entity_types
            .get(&"resource".parse().unwrap())
            .unwrap()
            .annotations
            .0
            .contains_key(&"mcp_resource".parse().unwrap()));
        assert!(root_nsdef
            .entity_types
            .get(&"Foo".parse().unwrap())
            .unwrap()
            .annotations
            .0
            .contains_key(&"mcp_context".parse().unwrap()));
    }
}
