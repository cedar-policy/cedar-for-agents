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

use super::identifiers;
use super::target::{ToolTarget, ToolTargets};
use crate::{RequestGenerator, SchemaGeneratorError, ServerLookupError};

use cedar_policy_core::ast::{Annotation, AnyId, Eid, Id, InternalName, Name, UnreservedId};
use cedar_policy_core::est::Annotations;
use cedar_policy_core::validator::{
    json_schema::{
        ActionEntityUID, ActionType, ApplySpec, AttributesOrContext, CommonType, CommonTypeId,
        EntityType, EntityTypeKind, Fragment, NamespaceDefinition, RecordType, StandardEntityType,
        Type, TypeOfAttribute, TypeVariant,
    },
    RawName,
};
use mcp_tools_sdk::description::{
    Parameters, Property, PropertyType, ServerDescription, ToolDescription,
};

use nonempty::NonEmpty;

use smol_str::{SmolStr, ToSmolStr};

use std::collections::{btree_map::Entry, BTreeMap, HashMap, HashSet};

/// A type reserved to configure how the schema generator functions
#[derive(Debug, Clone)]
pub struct SchemaGeneratorConfig {
    pub(crate) include_outputs: bool,
    pub(crate) objects_as_records: bool,
    pub(crate) erase_annotations: bool,
    pub(crate) flatten_namespaces: bool,
    pub(crate) numbers_as_decimal: bool,
    pub(crate) deduplicate_entity_types: bool,
    pub(crate) inline_empty_records: bool,
}

impl SchemaGeneratorConfig {
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

    /// Updates config to set `encode_numbers_as_decimal` to `val` (default: false)
    ///
    /// If `encode_numbers_as_decimal` is set to `true`, then every parameter of type
    /// `"number"` or `"float"` in an input `ToolDescription` to
    /// `add_action_from_tool_description` and `add_actions_from_server_description`
    /// will be encoded as a Cedar `decimal` in the output Cedar Schema.
    ///
    /// Otherwise, each `"number"` typed parameter will be encoded as an opaque `number`
    /// entity type that can only be compared for equality to other numbers. Similarly,
    /// each `"float"` typed parameter will be encoded as an opaque `float` entity type.
    ///
    /// Note: Representing `"number"` and `"float"` type parameters as `decimals` results
    /// in a loss of precision as `decimal`s only have four decimal places of precision.
    /// This may result in unsound authorization policies. For example `x < y` is true for
    /// `x = 2` and `y = 2.00004`. However, when converted to decimals, `x < y` evaluates to
    /// false as `x == y == 2.0000`. Additionally, numbers & floats have a significantly larger
    /// range than decimals. Decimals are limited between [-922337203685477.5808, 922337203685477.5807].
    pub fn encode_numbers_as_decimal(self, val: bool) -> Self {
        Self {
            numbers_as_decimal: val,
            ..self
        }
    }

    /// Updates config to set `deduplicate_entity_types` to `val` (default: false)
    ///
    /// If `deduplicate_entity_types` is set to `true`, then entity types with
    /// equivalent definitions across multiple tools will be consolidated into
    /// a single entity type placed in the lowest common ancestor namespace.
    ///
    /// Currently supports enum entity types (matched by name + variant values)
    /// and structural entity types where all attributes are of base type.
    pub fn deduplicate_entity_types(self, val: bool) -> Self {
        Self {
            deduplicate_entity_types: val,
            ..self
        }
    }

    /// Updates config to set `inline_empty_records` to `val` (default: false)
    ///
    /// If `inline_empty_records` is set to `true`, then any object property whose
    /// shape is an empty record---no `properties` and no `additionalProperties`---is
    /// encoded inline as the Cedar record type `{}` at its use site, instead of
    /// emitting a named entity (or common) type declaration and referencing it.
    ///
    /// An empty record carries no attributes, so inlining does not lose any modeling
    /// information.
    pub fn inline_empty_records(self, val: bool) -> Self {
        Self {
            inline_empty_records: val,
            ..self
        }
    }
}

impl Default for SchemaGeneratorConfig {
    fn default() -> Self {
        Self {
            include_outputs: false,
            objects_as_records: false,
            erase_annotations: true,
            flatten_namespaces: false,
            numbers_as_decimal: false,
            deduplicate_entity_types: false,
            inline_empty_records: false,
        }
    }
}

/// Returns `true` if the `PropertyType` is a primitive (leaf) type.
fn is_primitive(pt: &PropertyType) -> bool {
    matches!(
        pt,
        PropertyType::Bool
            | PropertyType::Integer
            | PropertyType::Float
            | PropertyType::Number
            | PropertyType::String
            | PropertyType::Decimal
            | PropertyType::Datetime
            | PropertyType::Duration
            | PropertyType::IpAddr
            | PropertyType::Null
            | PropertyType::Unknown
    )
}

// Returns `true` if the record is a "leaf" record, i.e. all its properties are of
// primitive type and it doesn't have any `additionalProperties`
fn is_leaf_record(p: &PropertyType) -> bool {
    match p {
        PropertyType::Object {
            properties,
            additional_properties,
        } => {
            additional_properties.is_none()
                && !properties.is_empty()
                && properties.iter().all(|p| is_primitive(p.property_type()))
        }
        _ => false,
    }
}

/// A fingerprint that uniquely identifies an entity type's definition.
/// Two entity types are considered equivalent (and thus deduplication candidates)
/// if and only if they produce the same fingerprint.
///
/// Designed as an enum to support future extension to other entity type kinds.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum EntityTypeFingerprint {
    /// Fingerprint for enum entity types.
    /// The order of variants in enums matters, i.e. ['foo', 'bar'] and
    /// ['bar', 'foo'] are two different entity types.
    /// Enum fingerprints are matched by name + variant values (in original order).
    Enum {
        base_name: UnreservedId,
        variants: Vec<Eid>,
    },
    /// Fingerprint for leaf record entity types: objects whose properties are all primitive.
    /// The order of attributes in objects does not matter.
    /// Record fingerprints are matched by name + sorted list of (property_name, type, required).
    LeafRecord {
        base_name: UnreservedId,
        /// Sorted by property name for deterministic comparison.
        fields: Vec<(SmolStr, PropertyType, bool)>,
    },
}

impl EntityTypeFingerprint {
    pub(crate) fn base_name(&self) -> &UnreservedId {
        match self {
            Self::Enum { base_name, .. } | Self::LeafRecord { base_name, .. } => base_name,
        }
    }

    pub(crate) fn new_leaf_record(
        base_name: UnreservedId,
        props: &[Property],
    ) -> EntityTypeFingerprint {
        let mut fields: Vec<(SmolStr, PropertyType, bool)> = props
            .iter()
            .map(|prop| {
                (
                    prop.name().to_smolstr(),
                    prop.property_type().clone(),
                    prop.is_required(),
                )
            })
            .collect();
        fields.sort_by(|a, b| a.0.cmp(&b.0));
        EntityTypeFingerprint::LeafRecord { base_name, fields }
    }
}

/// The resolved placement for a deduplicated entity type.
#[derive(Debug, Clone)]
pub(crate) struct DeduplicatedEntityType {
    /// The LCA namespace where the shared entity type will be placed
    pub(crate) lca_namespace: Option<Name>,
    /// The original namespaces where this entity type appeared before dedup
    pub(crate) source_namespaces: Vec<Option<Name>>,
}

/// Tracks all entity type occurrences and computes deduplication decisions.
#[derive(Debug, Clone, Default)]
struct DeduplicationMap {
    /// Maps each unique fingerprint to the namespaces where it was seen
    occurrences: HashMap<EntityTypeFingerprint, Vec<Option<Name>>>,
}

impl DeduplicationMap {
    fn record(&mut self, fingerprint: EntityTypeFingerprint, namespace: Option<Name>) {
        self.occurrences
            .entry(fingerprint)
            .or_default()
            .push(namespace);
    }

    /// Resolve all duplicates: returns a map from fingerprint to placement info
    /// for entity types that appear in more than one namespace.
    fn resolve_duplicates(&self) -> HashMap<EntityTypeFingerprint, DeduplicatedEntityType> {
        self.occurrences
            .iter()
            .filter(|(_, namespaces)| namespaces.len() > 1)
            .map(|(fp, namespaces)| {
                let lca = compute_lca(namespaces);
                (
                    fp.clone(),
                    DeduplicatedEntityType {
                        lca_namespace: lca,
                        source_namespaces: namespaces.clone(),
                    },
                )
            })
            .collect()
    }
}

/// Compute the lowest common ancestor namespace of a set of namespaces.
///
/// Namespaces are hierarchical (e.g., `MyMcpServer::tool_a::Input`).
/// The LCA is the longest common prefix of all namespace paths.
///
/// Examples:
/// - LCA of `A::B::C` and `A::B::D` is `A::B`
/// - LCA of `A::B::C` and `A::D::E` is `A`
/// - LCA of `A::B` and `A::B` is `A::B`
///
/// For the root namespace case (`None`), the LCA is `None` (global namespace).
fn compute_lca(namespaces: &[Option<Name>]) -> Option<Name> {
    // Empty list → None
    if namespaces.is_empty() {
        return None;
    }

    // Collect all Some values; if any namespace is None (global), the LCA is None
    let names: Vec<&Name> = namespaces
        .iter()
        .map(|ns| ns.as_ref())
        .collect::<Option<Vec<&Name>>>()?;

    // Get path segments (namespace components + basename) as Vec<&Id> for each name
    let segment_lists: Vec<Vec<&Id>> = names
        .iter()
        .map(|name| {
            let internal: &InternalName = name.as_ref();
            internal
                .namespace_components()
                .chain(std::iter::once(internal.basename()))
                .collect()
        })
        .collect();

    let first = segment_lists.first()?;
    let mut prefix_len = 0;

    for (i, segment) in first.iter().enumerate() {
        if segment_lists
            .iter()
            .all(|segs: &Vec<&Id>| segs.get(i) == Some(segment))
        {
            prefix_len = i + 1;
        } else {
            break;
        }
    }

    if prefix_len == 0 {
        return None;
    }

    let prefix = first.get(..prefix_len)?;
    let basename = (*prefix.last()?).clone();
    let path = prefix.get(..prefix.len() - 1)?.iter().map(|&id| id.clone());
    Name::try_from(InternalName::new(basename, path, None)).ok()
}

/// A type that allows constructing a Cedar Schema (Fragment)
/// from an input Cedar Schema Stub that defines the Cedar Type of
/// MCP principals, MCP Resources, and common MCP Contects.
///
/// The Generator can then be populated with a number of tool / server
/// descriptions to auto-generate Cedar actions corresponding one-to-one
/// with each tool description.
///
/// If the schema stub has namespaces annotated with `@mcp_server("name")`, then
/// the tools of each server are added to the namespace of that server, using the
/// methods taking a server name (e.g., `add_actions_from_named_server_description`).
#[derive(Debug, Clone)]
pub struct SchemaGenerator {
    fragment: Fragment<RawName>,
    /// The main namespace of the schema stub, i.e., its only namespace without `@mcp_server`.
    namespace: Option<Name>,
    users: Vec<RawName>,
    config: SchemaGeneratorConfig,
    targets: ToolTargets,
}

impl SchemaGenerator {
    /// Create a `SchemaGenerator` from a Cedar Schema Fragment using default configuration
    pub fn new(schema_stub: Fragment<RawName>) -> Result<Self, SchemaGeneratorError> {
        Self::new_with_config(schema_stub, SchemaGeneratorConfig::default())
    }

    /// Create a `SchemaGenerator` from a `.cedarschema` string using default configuration.
    ///
    /// This is a convenience method that parses the schema stub from a string,
    /// avoiding the need for callers to depend on `cedar-policy-core` directly.
    pub fn from_cedarschema_str(schema_stub: &str) -> Result<Self, SchemaGeneratorError> {
        Self::from_cedarschema_str_with_config(schema_stub, SchemaGeneratorConfig::default())
    }

    /// Create a `SchemaGenerator` from a `.cedarschema` string using specified configuration.
    ///
    /// This is a convenience method that parses the schema stub from a string,
    /// avoiding the need for callers to depend on `cedar-policy-core` directly.
    pub fn from_cedarschema_str_with_config(
        schema_stub: &str,
        config: SchemaGeneratorConfig,
    ) -> Result<Self, SchemaGeneratorError> {
        Self::new_with_config(parse_stub(schema_stub)?, config)
    }

    /// Create a `SchemaGenerator` from a Cedar Schema Fragment using specified configuration
    pub fn new_with_config(
        schema_stub: Fragment<RawName>,
        config: SchemaGeneratorConfig,
    ) -> Result<Self, SchemaGeneratorError> {
        let (namespace, ns, servers) = split_stub(&schema_stub)?;

        // With `@mcp_server` namespaces, actions are generated in the server namespaces,
        // so the names they use from the main namespace must be qualified.
        let main_qualifier = (!servers.is_empty()).then_some(namespace);
        let users = annotated_entity_types(ns, &identifiers::MCP_PRINCIPAL, main_qualifier);
        if users.is_empty() {
            return Err(SchemaGeneratorError::NoPrincipalTypes);
        }
        let resources = annotated_entity_types(ns, &identifiers::MCP_RESOURCE, None);
        let contexts = annotated_contexts(ns, main_qualifier);
        let actions = annotated_actions(ns, main_qualifier);

        // Server namespaces that get a generated `AllActions` action
        let mut all_actions_namespaces = Vec::new();
        let targets = if servers.is_empty() {
            if resources.is_empty() {
                return Err(SchemaGeneratorError::NoResourceTypes);
            }
            ToolTargets::Single(ToolTarget::new(
                Some(namespace.clone()),
                resources,
                contexts,
                actions.clone(),
            ))
        } else {
            if config.flatten_namespaces {
                return Err(SchemaGeneratorError::FlattenWithServerNamespaces);
            }
            if !resources.is_empty() {
                return Err(SchemaGeneratorError::ResourceInMainNamespace(
                    namespace.to_string(),
                ));
            }
            let mut targets = BTreeMap::new();
            for (server_ns, server, ns) in servers {
                let server_name = server
                    .as_ref()
                    .map(|anno| anno.val.clone())
                    .filter(|name| !name.is_empty())
                    .ok_or_else(|| {
                        SchemaGeneratorError::MissingServerName(server_ns.to_string())
                    })?;
                // Otherwise, a tool `Y` of server `Main::X` would generate the namespace of
                // server `Main::X::Y`
                if server_ns.as_ref().namespace() != namespace.to_string() {
                    return Err(SchemaGeneratorError::ServerNamespaceNotChildOfMain {
                        namespace: server_ns.to_string(),
                        main: namespace.to_string(),
                    });
                }
                if !annotated_entity_types(ns, &identifiers::MCP_PRINCIPAL, None).is_empty() {
                    return Err(SchemaGeneratorError::PrincipalInServerNamespace(
                        server_ns.to_string(),
                    ));
                }
                let resources = annotated_entity_types(ns, &identifiers::MCP_RESOURCE, None);
                if resources.len() != 1 {
                    return Err(SchemaGeneratorError::WrongNumberOfServerResources {
                        namespace: server_ns.to_string(),
                        count: resources.len(),
                    });
                }
                // Server contexts override main contexts with the same key
                let mut server_contexts = contexts.clone();
                server_contexts.extend(annotated_contexts(ns, None));
                if let Some(key) = server_contexts
                    .keys()
                    .find(|key| *key == identifiers::INPUT_KEY || *key == identifiers::OUTPUT_KEY)
                {
                    return Err(SchemaGeneratorError::ReservedContextKey(key.clone()));
                }
                // The parents of these actions are the ones written in the stub
                let mut server_actions = annotated_actions(ns, None);
                if server_actions.is_empty() {
                    all_actions_namespaces.push(server_ns.clone());
                    server_actions
                        .push(ActionEntityUID::new(None, identifiers::ALL_ACTIONS.into()));
                }
                let target = ToolTarget::new(
                    Some(server_ns.clone()),
                    resources,
                    server_contexts,
                    server_actions,
                );
                if targets.insert(server_name.clone(), target).is_some() {
                    return Err(SchemaGeneratorError::DuplicateServerName(server_name));
                }
            }
            ToolTargets::Multi(targets)
        };
        let namespace = namespace.clone();

        let mut fragment = if config.erase_annotations {
            erase_mcp_annotations(schema_stub)
        } else {
            schema_stub
        };
        for server_ns in all_actions_namespaces {
            add_all_actions(&mut fragment, &server_ns, &actions)?;
        }

        Ok(Self {
            fragment,
            namespace: Some(namespace),
            users,
            config,
            targets,
        })
    }

    /// Get the current Cedar Schema
    pub fn get_schema(&self) -> &Fragment<RawName> {
        &self.fragment
    }

    /// Get the current Cedar Schema as a human-readable `.cedarschema` string.
    pub fn get_schema_as_str(&self) -> String {
        format!("{}", self.fragment)
    }

    /// Get a `RequestGenerator` that will convert MCP tool Input/Ouptut
    /// requests that validate against a tool added to this `SchemaGenerator`
    /// to Cedar Authorization Requests that validate against the current Schema.
    pub fn new_request_generator(&self) -> Result<RequestGenerator, SchemaGeneratorError> {
        let schema =
            cedar_policy_core::validator::ValidatorSchema::try_from(self.fragment.clone())?;
        Ok(RequestGenerator::new(
            self.config.clone(),
            self.namespace.clone(),
            schema,
            self.targets.clone(),
        ))
    }

    /// Add a new action to the generated Cedar Schema
    /// that corresponds to the input `ToolDescription`
    ///
    /// Only for schema stubs without `@mcp_server` namespaces.
    pub fn add_action_from_tool_description(
        &mut self,
        description: &ToolDescription,
    ) -> Result<(), SchemaGeneratorError> {
        self.transaction(|generator| {
            generator
                .target_generator(None)?
                .add_action_from_tool_description(description)
        })
    }

    /// Add a new action to the generated Cedar Schema
    /// for each tool description within the `ServerDescription`
    ///
    /// Only for schema stubs without `@mcp_server` namespaces.
    pub fn add_actions_from_server_description(
        &mut self,
        description: &ServerDescription,
    ) -> Result<(), SchemaGeneratorError> {
        self.add_actions_from_optionally_named_server_description(None, description)
    }

    /// Add a new action to the generated Cedar Schema for each tool description within
    /// the `ServerDescription`, in the namespace annotated with `@mcp_server("{server}")`.
    /// The tools of a server can only be added once.
    ///
    /// Only for schema stubs with `@mcp_server` namespaces.
    pub fn add_actions_from_named_server_description(
        &mut self,
        server: &str,
        description: &ServerDescription,
    ) -> Result<(), SchemaGeneratorError> {
        self.add_actions_from_optionally_named_server_description(Some(server), description)
    }

    /// `add_actions_from_named_server_description` if `server` is given, and
    /// `add_actions_from_server_description` otherwise.
    pub(crate) fn add_actions_from_optionally_named_server_description(
        &mut self,
        server: Option<&str>,
        description: &ServerDescription,
    ) -> Result<(), SchemaGeneratorError> {
        self.transaction(|generator| {
            generator
                .target_generator(server)?
                .add_actions_from_server_description(description)
        })
    }

    /// Call `add_actions_from_named_server_description` for each server name and
    /// `ServerDescription` of `servers`. If one of them fails, then no actions are added.
    ///
    /// Only for schema stubs with `@mcp_server` namespaces.
    pub fn add_actions_from_named_server_descriptions(
        &mut self,
        servers: &BTreeMap<String, ServerDescription>,
    ) -> Result<(), SchemaGeneratorError> {
        self.transaction(|generator| {
            servers.iter().try_for_each(|(server, description)| {
                generator
                    .target_generator(Some(server))?
                    .add_actions_from_server_description(description)
            })
        })
    }

    /// Run `f`, and restore the generator to its current state if `f` fails.
    fn transaction(
        &mut self,
        f: impl FnOnce(&mut Self) -> Result<(), SchemaGeneratorError>,
    ) -> Result<(), SchemaGeneratorError> {
        let snapshot = self.clone();
        let result = f(self);
        if result.is_err() {
            *self = snapshot;
        }
        result
    }

    /// Get a `TargetGenerator` adding tools to the namespace of `server`, or to the only
    /// namespace of the schema stub if `server` is `None`.
    fn target_generator(
        &mut self,
        server: Option<&str>,
    ) -> Result<TargetGenerator<'_>, ServerLookupError> {
        Ok(TargetGenerator {
            target: self.targets.get_mut(server)?,
            fragment: &mut self.fragment,
            config: &self.config,
            main_namespace: &self.namespace,
            users: &self.users,
        })
    }
}

/// Adds the actions of tools to one `ToolTarget` of a `SchemaGenerator`. It borrows the fields of
/// the `SchemaGenerator` separately, so that the target and the fragment can both be mutated.
#[derive(Debug)]
struct TargetGenerator<'a> {
    target: &'a mut ToolTarget,
    fragment: &'a mut Fragment<RawName>,
    config: &'a SchemaGeneratorConfig,
    /// The main namespace of the schema stub. It contains the opaque entity types
    /// (`Float`, `Number`, `Null`, `Unknown`) shared by all targets.
    main_namespace: &'a Option<Name>,
    /// The principal types, qualified if `target` is not the main namespace.
    users: &'a [RawName],
}

impl TargetGenerator<'_> {
    /// Check if a fingerprint matches an existing entity type definition.
    fn fingerprint_matches_entity(
        fingerprint: &EntityTypeFingerprint,
        entity: &EntityType<RawName>,
    ) -> bool {
        match fingerprint {
            EntityTypeFingerprint::Enum {
                base_name: _,
                variants,
            } => match &entity.kind {
                EntityTypeKind::Enum { choices } => {
                    let existing: Vec<&Eid> = choices.iter().collect();
                    let candidate: Vec<&Eid> = variants.iter().collect();
                    existing == candidate
                }
                _ => false,
            },
            EntityTypeFingerprint::LeafRecord {
                base_name: _,
                fields,
            } => match &entity.kind {
                EntityTypeKind::Standard(std_entity) => {
                    Self::record_matches_fields(&std_entity.shape.0, fields)
                }
                _ => false,
            },
        }
    }

    /// Check if a record type matches the expected leaf record fields.
    fn record_matches_fields(ty: &Type<RawName>, fields: &[(SmolStr, PropertyType, bool)]) -> bool {
        let Type::Type {
            ty: TypeVariant::Record(record),
            ..
        } = ty
        else {
            return false;
        };
        if record.attributes.len() != fields.len() {
            return false;
        }
        let mut existing: Vec<(&SmolStr, &TypeOfAttribute<RawName>)> =
            record.attributes.iter().collect();
        existing.sort_by_key(|(name, _)| *name);
        existing
            .iter()
            .zip(fields.iter())
            .all(|((name, attr), (f_name, f_type, f_required))| {
                *name == f_name
                    && attr.required == *f_required
                    && Self::type_matches_primitive(&attr.ty, f_type)
            })
    }

    /// Check if a Cedar type corresponds to the given primitive property type.
    fn type_matches_primitive(ty: &Type<RawName>, prim: &PropertyType) -> bool {
        let Type::Type { ty: variant, .. } = ty else {
            return false;
        };
        match (variant, prim) {
            (TypeVariant::EntityOrCommon { type_name }, prim) => {
                let expected = match prim {
                    PropertyType::Bool => &identifiers::BOOL_TYPE,
                    PropertyType::Integer => &identifiers::LONG_TYPE,
                    PropertyType::String => &identifiers::STRING_TYPE,
                    PropertyType::Decimal => &identifiers::DECIMAL_TYPE,
                    PropertyType::Datetime => &identifiers::DATETIME_TYPE,
                    PropertyType::Duration => &identifiers::DURATION_TYPE,
                    PropertyType::IpAddr => &identifiers::IPADDR_TYPE,
                    _ => return false,
                };
                type_name == &**expected
            }
            (TypeVariant::Entity { name }, PropertyType::Float) => {
                name == &RawName::from_name(
                    RawName::new_from_unreserved(identifiers::FLOAT_TYPE.clone(), None)
                        .qualify_with_name(None),
                )
            }
            (TypeVariant::Entity { name }, PropertyType::Number) => {
                name == &RawName::from_name(
                    RawName::new_from_unreserved(identifiers::NUMBER_TYPE.clone(), None)
                        .qualify_with_name(None),
                )
            }
            (TypeVariant::Entity { name }, PropertyType::Null) => {
                name == &RawName::from_name(
                    RawName::new_from_unreserved(identifiers::NULL_TYPE.clone(), None)
                        .qualify_with_name(None),
                )
            }
            (TypeVariant::Entity { name }, PropertyType::Unknown) => {
                name == &RawName::from_name(
                    RawName::new_from_unreserved(identifiers::UNKNOWN_TYPE.clone(), None)
                        .qualify_with_name(None),
                )
            }
            _ => false,
        }
    }

    /// Look up whether a given fingerprint was deduplicated.
    /// Returns the LCA namespace if so.
    fn get_dedup_namespace(&self, fingerprint: &EntityTypeFingerprint) -> Option<&Option<Name>> {
        self.target
            .resolved_dedup
            .as_ref()
            .and_then(|map| map.get(fingerprint))
            .map(|d| &d.lca_namespace)
    }

    /// Recursively scan parameters for enum properties and record their fingerprints.
    /// Recurses into nested objects to find enums at any depth.
    #[expect(
        clippy::ref_option,
        reason = "Consistent with the rest of the codebase's namespace parameter style."
    )]
    fn collect_enum_fingerprints(
        parameters: &Parameters,
        namespace: &Option<Name>,
        dedup_map: &mut DeduplicationMap,
    ) {
        for property in parameters.properties() {
            Self::collect_enum_fingerprints_from_property_type(
                property.name(),
                property.property_type(),
                namespace,
                dedup_map,
            );
        }
        // Also scan type definitions within parameters
        for type_def in parameters.type_definitions() {
            Self::collect_enum_fingerprints_from_property_type(
                type_def.name(),
                type_def.property_type(),
                namespace,
                dedup_map,
            );
        }
    }

    /// Recursively scan a single property type for enum occurrences.
    /// For objects, computes the child namespace and recurses into nested properties.
    #[expect(
        clippy::ref_option,
        reason = "Consistent with the rest of the codebase's namespace parameter style."
    )]
    fn collect_enum_fingerprints_from_property_type(
        name: &str,
        property_type: &PropertyType,
        namespace: &Option<Name>,
        dedup_map: &mut DeduplicationMap,
    ) {
        match property_type {
            PropertyType::Enum { variants } => {
                if !variants.is_empty() {
                    if let Ok(base_name) = name.parse::<UnreservedId>() {
                        let fingerprint = EntityTypeFingerprint::Enum {
                            base_name,
                            variants: variants.clone().into_iter().map(Eid::new).collect(),
                        };
                        dedup_map.record(fingerprint, namespace.clone());
                    }
                }
            }
            PropertyType::Object {
                properties,
                additional_properties,
            } => {
                if let Ok(obj_name) = name.parse::<UnreservedId>() {
                    let child_ns: Name = obj_name.into();
                    let child_ns = Some(child_ns.qualify_with_name(namespace.as_ref()));
                    for prop in properties {
                        Self::collect_enum_fingerprints_from_property_type(
                            prop.name(),
                            prop.property_type(),
                            &child_ns,
                            dedup_map,
                        );
                    }
                    if let Some(additional) = additional_properties {
                        let tag_name = format!("{name}Tag");
                        Self::collect_enum_fingerprints_from_property_type(
                            &tag_name,
                            additional.as_ref(),
                            &child_ns,
                            dedup_map,
                        );
                    }

                    if is_leaf_record(property_type) {
                        if let Ok(base_name) = name.parse::<UnreservedId>() {
                            let fingerprint =
                                EntityTypeFingerprint::new_leaf_record(base_name, properties);
                            dedup_map.record(fingerprint, namespace.clone());
                        }
                    }
                }
            }
            PropertyType::Array { element_ty } => {
                Self::collect_enum_fingerprints_from_property_type(
                    name,
                    element_ty.as_ref(),
                    namespace,
                    dedup_map,
                );
            }
            PropertyType::Union { types } => {
                if let Ok(union_name) = name.parse::<UnreservedId>() {
                    let child_ns: Name = union_name.into();
                    let child_ns = Some(child_ns.qualify_with_name(namespace.as_ref()));
                    for (i, ty) in types.iter().enumerate() {
                        let variant_name = format!("TypeChoice{i}");
                        Self::collect_enum_fingerprints_from_property_type(
                            &variant_name,
                            ty,
                            &child_ns,
                            dedup_map,
                        );
                    }
                }
            }
            PropertyType::Tuple { types } => {
                if let Ok(tuple_name) = name.parse::<UnreservedId>() {
                    let child_ns: Name = tuple_name.into();
                    let child_ns = Some(child_ns.qualify_with_name(namespace.as_ref()));
                    for (i, ty) in types.iter().enumerate() {
                        let proj_name = format!("Proj{i}");
                        Self::collect_enum_fingerprints_from_property_type(
                            &proj_name, ty, &child_ns, dedup_map,
                        );
                    }
                }
            }
            _ => {}
        }
    }

    fn add_action_from_tool_description(
        &mut self,
        description: &ToolDescription,
    ) -> Result<(), SchemaGeneratorError> {
        if self.target.tools.tool_descriptions().count() != 0 {
            return Err(SchemaGeneratorError::ServerDescriptionMerge);
        }
        self.target.tools =
            ServerDescription::new(vec![description.clone()].into_iter(), HashMap::new());
        self.add_action_from_tool_description_inner(description, BTreeMap::new())
    }

    fn add_actions_from_server_description(
        &mut self,
        description: &ServerDescription,
    ) -> Result<(), SchemaGeneratorError> {
        if self.target.tools.tool_descriptions().count() != 0 {
            return Err(SchemaGeneratorError::ServerDescriptionMerge);
        }
        self.target.tools = description.clone();

        // Clone once and reuse to avoid borrow issues
        let namespace = self.target.namespace.clone();

        // Populate a map from type ref names to fully qualified type name
        // This makes type resolution simpler and will allow for mutually recursive type defs
        let mut common_types = BTreeMap::new();
        for type_def in description.type_definitions() {
            let type_name = CommonTypeId::new(type_def.name().parse()?)?;
            let type_name = get_refname(&namespace, &type_name);
            let ref_name = type_def.name().to_smolstr();
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

        self.deduplicate_entities(description)?;

        for tool_description in description.tool_descriptions() {
            self.add_action_from_tool_description_inner(tool_description, common_types.clone())?
        }
        Ok(())
    }

    /// Scans all tool descriptions for equivalent enum entity types and places
    /// shared definitions in the lowest common ancestor namespace.
    /// Must be called before individual tool actions are processed.
    fn deduplicate_entities(
        &mut self,
        description: &ServerDescription,
    ) -> Result<(), SchemaGeneratorError> {
        if !self.config.deduplicate_entity_types {
            return Ok(());
        }

        let mut dedup_map = DeduplicationMap::default();

        for tool_description in description.tool_descriptions() {
            let tool_ns: Name = tool_description.name().parse()?;
            let tool_ns = tool_ns.qualify_with_name(self.target.namespace.as_ref());
            let input_ns = Some(identifiers::INPUT_NAME.qualify_with_name(Some(&tool_ns)));

            Self::collect_enum_fingerprints(tool_description.inputs(), &input_ns, &mut dedup_map);

            if self.config.include_outputs {
                let output_ns = Some(identifiers::OUTPUT_NAME.qualify_with_name(Some(&tool_ns)));
                Self::collect_enum_fingerprints(
                    tool_description.outputs(),
                    &output_ns,
                    &mut dedup_map,
                );
            }

            for type_def in tool_description.type_definitions() {
                if let PropertyType::Enum { variants } = type_def.property_type() {
                    if !variants.is_empty() {
                        if let Ok(base_name) = type_def.name().parse::<UnreservedId>() {
                            let fingerprint = EntityTypeFingerprint::Enum {
                                base_name,
                                variants: variants.clone().into_iter().map(Eid::new).collect(),
                            };
                            dedup_map.record(fingerprint, Some(tool_ns.clone()));
                        }
                    }
                }
            }
        }

        let mut resolved = dedup_map.resolve_duplicates();

        // LeafRecord dedup only applies when objects are encoded as entity types
        if self.config.objects_as_records {
            resolved.retain(|fp, _| !matches!(fp, EntityTypeFingerprint::LeafRecord { .. }));
        }

        // Determine which fingerprints to skip:
        // - Same base_name targeting the same LCA (different variants conflict)
        // - Base_name already exists in the LCA namespace
        let mut skipped = HashSet::<&EntityTypeFingerprint>::new();

        // Group by (base_name, lca_namespace) to detect same-name conflicts
        let mut lca_groups: HashMap<(&UnreservedId, &Option<Name>), Vec<&EntityTypeFingerprint>> =
            HashMap::new();
        for (fp, info) in &resolved {
            lca_groups
                .entry((fp.base_name(), &info.lca_namespace))
                .or_default()
                .push(fp);
        }
        for fps in lca_groups.values() {
            if fps.len() > 1 {
                skipped.extend(fps.iter());
            }
        }

        // Skip fingerprints whose base_name collides with a *different* type in the LCA.
        // If the LCA already has an identical enum (same name + same variants), we reuse it.
        let mut reused = HashSet::<&EntityTypeFingerprint>::new();
        for (fp, info) in &resolved {
            if skipped.contains(fp) {
                continue;
            }
            let base_name = fp.base_name();
            if let Some(nsdef) = self.fragment.0.get(&info.lca_namespace) {
                if nsdef.common_types.keys().any(|k| k.as_ref() == base_name) {
                    skipped.insert(fp);
                } else if let Some(existing_entity) = nsdef.entity_types.get(base_name) {
                    if Self::fingerprint_matches_entity(fp, existing_entity) {
                        reused.insert(fp);
                    } else {
                        skipped.insert(fp);
                    }
                }
            }
        }

        // Place non-skipped entity types in their LCA namespace.
        // Reused types already exist — record them in `placed` without re-inserting.
        let mut placed = HashMap::new();
        for (fingerprint, dedup_info) in &resolved {
            if skipped.contains(fingerprint) {
                continue;
            }
            let lca_ns = &dedup_info.lca_namespace;

            if !reused.contains(fingerprint) {
                self.add_namespace(lca_ns.clone());

                match fingerprint {
                    EntityTypeFingerprint::Enum {
                        base_name,
                        variants,
                    } => {
                        #[expect(
                            clippy::unwrap_used,
                            reason = "Variants are non-empty by construction from PropertyType::Enum"
                        )]
                        let choices = NonEmpty::from_slice(variants).unwrap();
                        let ty = EntityType {
                            external: false,
                            kind: EntityTypeKind::Enum { choices },
                            annotations: Annotations::new(),
                            loc: None,
                        };
                        self.add_entitytype(lca_ns, ty, base_name.clone(), true)?;
                    }
                    EntityTypeFingerprint::LeafRecord { base_name, fields } => {
                        let empty_common_types = BTreeMap::new();
                        let attributes = fields
                            .iter()
                            .map(|(name, prop_type, required)| {
                                let ty_name: UnreservedId =
                                    name.parse().map_err(SchemaGeneratorError::from)?;
                                let ty = self.cedar_type_from_property_type(
                                    lca_ns,
                                    ty_name,
                                    prop_type,
                                    &empty_common_types,
                                )?;
                                Ok((
                                    name.clone(),
                                    TypeOfAttribute {
                                        ty,
                                        annotations: Annotations::new(),
                                        required: *required,
                                    },
                                ))
                            })
                            .collect::<Result<_, SchemaGeneratorError>>()?;
                        let ty = EntityType {
                            external: false,
                            kind: EntityTypeKind::Standard(StandardEntityType {
                                member_of_types: Vec::new(),
                                shape: AttributesOrContext(Type::Type {
                                    ty: TypeVariant::Record(RecordType {
                                        attributes,
                                        additional_attributes: false,
                                    }),
                                    loc: None,
                                }),
                                tags: None,
                            }),
                            annotations: Annotations::new(),
                            loc: None,
                        };
                        self.add_entitytype(lca_ns, ty, base_name.clone(), true)?;
                    }
                }
            }
            placed.insert(fingerprint.clone(), dedup_info.clone());
        }

        self.target.resolved_dedup = Some(placed);
        Ok(())
    }

    fn add_action_from_tool_description_inner(
        &mut self,
        description: &ToolDescription,
        mut common_types: BTreeMap<SmolStr, RawName>,
    ) -> Result<(), SchemaGeneratorError> {
        let namespace: Name = description.name().parse()?;
        let namespace = Some(namespace.qualify_with_name(self.target.namespace.as_ref()));
        self.add_namespace(namespace.clone());

        // Populate a map from type ref names to fully qualified type name
        // This makes type resolution simpler and will allow for mutually recursive type defs
        for type_def in description.type_definitions() {
            let type_name = CommonTypeId::new(type_def.name().parse()?)?;
            let type_name = get_refname(&namespace, &type_name);
            let ref_name = type_def.name().to_smolstr();
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
            .target
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
        let input_ns = Some(identifiers::INPUT_NAME.qualify_with_name(namespace.as_ref()));

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
        let parent_namespace = self.target.namespace.clone();
        self.add_commontype(
            &parent_namespace,
            input_type,
            tool_input_ty_name.clone(),
            true,
        )?;

        ctx_attrs.insert(
            identifiers::INPUT_KEY.to_smolstr(),
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
            let output_ns = Some(identifiers::OUTPUT_NAME.qualify_with_name(namespace.as_ref()));

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
                identifiers::OUTPUT_KEY.to_smolstr(),
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
            external: false,
            attributes: None,
            applies_to: Some(ApplySpec {
                resource_types: self.target.resources.clone(),
                principal_types: self.users.to_vec(),
                context: AttributesOrContext(Type::Type {
                    ty: TypeVariant::Record(RecordType {
                        attributes: ctx_attrs,
                        additional_attributes: false,
                    }),
                    loc: None,
                }),
            }),
            member_of: self.target.actions.clone(),
            annotations: Annotations::new(),
            loc: None,
        };

        #[expect(clippy::unwrap_used, reason = "Namespace exists by construction.")]
        let actions = &mut self
            .fragment
            .0
            .get_mut(&self.target.namespace)
            .unwrap()
            .actions;
        match actions.entry(description.name().to_smolstr()) {
            Entry::Occupied(occ) => {
                return Err(SchemaGeneratorError::ConflictingActionName {
                    action: occ.key().clone(),
                    namespace: self
                        .target
                        .namespace
                        .as_ref()
                        .map(ToString::to_string)
                        .unwrap_or_default(),
                })
            }
            Entry::Vacant(vac) => {
                vac.insert(action);
            }
        }

        self.drop_namespace_if_empty(&namespace);

        Ok(())
    }

    // This function should only be called when name is prefixed by `{self.main_namespace}::`
    // Converts `{self.main_namespace}::Foo::Bar::Baz` to `{self.main_namespace}::Foo_Bar_Baz`.
    fn flatten_internalname(&self, name: InternalName) -> InternalName {
        if self.config.flatten_namespaces {
            let name = name.to_string();
            let name = if self.main_namespace.is_some() {
                #[expect(
                    clippy::unwrap_used,
                    reason = "By assumption name should include at least one \"::\"."
                )]
                name.split_once("::").unwrap().1.to_string()
            } else {
                name
            };
            let name = name.replace("::", "_");
            #[expect(
                clippy::unwrap_used,
                reason = "The `name` should still parse after converting \"::\" to \"_\"."
            )]
            let name: InternalName = name.parse().unwrap();
            name.qualify_with_name(self.main_namespace.as_ref())
        } else {
            name
        }
    }

    // This function should only be called when name is prefixed by `{self.main_namespace}::`
    // Converts `{self.main_namespace}::Foo::Bar::Baz` to `{self.main_namespace}::Foo_Bar_Baz`.
    fn flatten_rawname(&self, name: RawName) -> RawName {
        if self.config.flatten_namespaces {
            RawName::from_name(self.flatten_internalname(name.qualify_with(None)))
        } else {
            name
        }
    }

    #[expect(
        clippy::ref_option,
        reason = "This follows a decision made by cedar-policy-core which we are using."
    )]
    // This function should only be called when namespace is prefixed by `{self.main_namespace}::`
    // If `namespace` is `{self.main_namespace}::Foo::Bar::Baz` then this function returns the id `Foo_Bar_Baz_id`.
    fn flatten_unreserved_id(&self, id: UnreservedId, namespace: &Option<Name>) -> UnreservedId {
        if self.config.flatten_namespaces {
            let name = Name::unqualified_name(id).qualify_with_name(namespace.as_ref());
            let name = name.qualify_with(None);
            let name = self.flatten_internalname(name);
            #[expect(
                clippy::unwrap_used,
                reason = "The basename should be unreserved because the original id used to construct it is unreserved."
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

    #[expect(
        clippy::ref_option,
        reason = "More ergonomic for indexing into fragment."
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

    #[expect(
        clippy::ref_option,
        reason = "More ergnomic for indexing into fragment."
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
                self.main_namespace,
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
        #[expect(
            clippy::unwrap_used,
            reason = "This function is only called on namespaces appearing in fragment."
        )]
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

    #[expect(
        clippy::ref_option,
        reason = "More ergnomic for indexing into fragment."
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
                self.main_namespace,
                self.flatten_unreserved_id(ty_name, namespace),
            )
        } else {
            (namespace, ty_name)
        };

        #[expect(
            clippy::unwrap_used,
            reason = "This function is only called on namespaces appearing in fragment."
        )]
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

    #[expect(
        clippy::ref_option,
        reason = "More ergnomic for indexing into fragment."
    )]
    fn add_opaque_entity_type(
        &mut self,
        namespace: &Option<Name>,
        ty_name: UnreservedId,
    ) -> Result<(), SchemaGeneratorError> {
        let ty = EntityType {
            external: false,
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

    #[expect(
        clippy::ref_option,
        reason = "More ergnomic for indexing into fragment."
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
            let type_name = get_refname(namespace, &type_name);
            let ref_name = type_def.name().to_smolstr();
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

    #[expect(
        clippy::ref_option,
        reason = "More ergnomic for indexing into fragment."
    )]
    fn cedar_type_from_property_type(
        &mut self,
        namespace: &Option<Name>,
        ty_name: UnreservedId,
        property_type: &PropertyType,
        common_types: &BTreeMap<SmolStr, RawName>,
    ) -> Result<Type<RawName>, SchemaGeneratorError> {
        let bool = TypeVariant::EntityOrCommon {
            type_name: identifiers::BOOL_TYPE.clone(),
        };
        let long = TypeVariant::EntityOrCommon {
            type_name: identifiers::LONG_TYPE.clone(),
        };
        let string = TypeVariant::EntityOrCommon {
            type_name: identifiers::STRING_TYPE.clone(),
        };
        let decimal = TypeVariant::EntityOrCommon {
            type_name: identifiers::DECIMAL_TYPE.clone(),
        };
        let datetime = TypeVariant::EntityOrCommon {
            type_name: identifiers::DATETIME_TYPE.clone(),
        };
        let duration = TypeVariant::EntityOrCommon {
            type_name: identifiers::DURATION_TYPE.clone(),
        };
        let ipaddr = TypeVariant::EntityOrCommon {
            type_name: identifiers::IPADDR_TYPE.clone(),
        };

        let variant = match property_type {
            PropertyType::Bool => bool,
            PropertyType::Integer => long,
            PropertyType::Float => {
                if self.config.numbers_as_decimal {
                    decimal
                } else {
                    self.add_opaque_entity_type(
                        &self.main_namespace.clone(),
                        identifiers::FLOAT_TYPE.clone(),
                    )?;
                    let name = RawName::new_from_unreserved(identifiers::FLOAT_TYPE.clone(), None);
                    let name =
                        RawName::from_name(name.qualify_with_name(self.main_namespace.as_ref()));
                    TypeVariant::Entity { name }
                }
            }
            PropertyType::Number => {
                if self.config.numbers_as_decimal {
                    decimal
                } else {
                    self.add_opaque_entity_type(
                        &self.main_namespace.clone(),
                        identifiers::NUMBER_TYPE.clone(),
                    )?;
                    let name = RawName::new_from_unreserved(identifiers::NUMBER_TYPE.clone(), None);
                    let name =
                        RawName::from_name(name.qualify_with_name(self.main_namespace.as_ref()));
                    TypeVariant::Entity { name }
                }
            }
            PropertyType::String => string,
            PropertyType::Decimal => decimal,
            PropertyType::Datetime => datetime,
            PropertyType::Duration => duration,
            PropertyType::IpAddr => ipaddr,
            PropertyType::Null => {
                self.add_opaque_entity_type(
                    &self.main_namespace.clone(),
                    identifiers::NULL_TYPE.clone(),
                )?;
                let name = RawName::new_from_unreserved(identifiers::NULL_TYPE.clone(), None);
                let name = RawName::from_name(name.qualify_with_name(self.main_namespace.as_ref()));
                TypeVariant::Entity { name }
            }
            PropertyType::Unknown => {
                self.add_opaque_entity_type(
                    &self.main_namespace.clone(),
                    identifiers::UNKNOWN_TYPE.clone(),
                )?;
                let name = RawName::new_from_unreserved(identifiers::UNKNOWN_TYPE.clone(), None);
                let name = RawName::from_name(name.qualify_with_name(self.main_namespace.as_ref()));
                TypeVariant::Entity { name }
            }
            PropertyType::Enum { variants } => {
                let choices = NonEmpty::from_slice(variants)
                    .ok_or_else(|| SchemaGeneratorError::empty_enum_choice(ty_name.to_string()))?
                    .map(Eid::new);

                // Check if this enum was deduplicated (placed in LCA namespace during Pass 1)
                let fingerprint = EntityTypeFingerprint::Enum {
                    base_name: ty_name.clone(),
                    variants: variants.clone().into_iter().map(Eid::new).collect(),
                };
                if let Some(lca_ns) = self.get_dedup_namespace(&fingerprint) {
                    // Reference the shared type in the LCA namespace (already placed in Pass 1)
                    let name = RawName::new_from_unreserved(ty_name, None);
                    let name = RawName::from_name(name.qualify_with_name(lca_ns.as_ref()));
                    TypeVariant::Entity {
                        name: self.flatten_rawname(name),
                    }
                } else {
                    // Original behavior: place locally
                    let ty = EntityType {
                        external: false,
                        kind: EntityTypeKind::Enum { choices },
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
                        #[expect(
                            clippy::unwrap_used,
                            reason = "The string `Proj{i}` is a valid UnreservedId."
                        )]
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
                        #[expect(
                            clippy::unwrap_used,
                            reason = "The string `TypeChoice{i}` is a valid UnreservedId."
                        )]
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
                // Return early and just inline an empty record (no tags, no attributes) if the option is set
                if self.config.inline_empty_records
                    && properties.is_empty()
                    && additional_properties.is_none()
                {
                    return Ok(Type::Type {
                        ty: TypeVariant::Record(RecordType {
                            attributes: BTreeMap::new(),
                            additional_attributes: false,
                        }),
                        loc: None,
                    });
                }

                // Check if this is a leaf record and it was deduplicated (placed in LCA namespace during Pass 1)
                if !self.config.objects_as_records && is_leaf_record(property_type) {
                    let fingerprint =
                        EntityTypeFingerprint::new_leaf_record(ty_name.clone(), properties);
                    if let Some(lca_ns) = self.get_dedup_namespace(&fingerprint) {
                        let name = RawName::new_from_unreserved(ty_name, None);
                        let name = RawName::from_name(name.qualify_with_name(lca_ns.as_ref()));
                        return Ok(Type::Type {
                            ty: TypeVariant::Entity {
                                name: self.flatten_rawname(name),
                            },
                            loc: None,
                        });
                    }
                }

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

                let qualified_ty_name = RawName::from_name(
                    RawName::new_from_unreserved(ty_name.clone(), None)
                        .qualify_with_name(namespace.as_ref()),
                );
                // Encode as record if possible and allowed
                let type_reference = if self.config.objects_as_records && tags.is_none() {
                    let ty = Type::Type {
                        ty: TypeVariant::Record(RecordType {
                            attributes,
                            additional_attributes: false,
                        }),
                        loc: None,
                    };
                    self.add_commontype(namespace, ty, ty_name, true)?;
                    TypeVariant::EntityOrCommon {
                        type_name: self.flatten_rawname(qualified_ty_name),
                    }
                } else {
                    // otherwise encode as EntityType
                    let ty = EntityType {
                        external: false,
                        kind: EntityTypeKind::Standard(StandardEntityType {
                            member_of_types: Vec::new(),
                            shape: AttributesOrContext(Type::Type {
                                ty: TypeVariant::Record(RecordType {
                                    attributes,
                                    additional_attributes: false,
                                }),
                                loc: None,
                            }),
                            tags,
                        }),
                        annotations: Annotations::new(),
                        loc: None,
                    };

                    self.add_entitytype(namespace, ty, ty_name, true)?;
                    TypeVariant::Entity {
                        name: self.flatten_rawname(qualified_ty_name),
                    }
                };

                self.drop_namespace_if_empty(&ns);
                type_reference
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

#[expect(
    clippy::ref_option,
    reason = "This follows a decision made by cedar-policy-core which we are using."
)]
fn get_refname(namespace: &Option<Name>, ty_name: &CommonTypeId) -> RawName {
    RawName::from_name(
        RawName::new_from_unreserved(ty_name.as_ref().clone(), None)
            .qualify_with_name(namespace.as_ref()),
    )
}

#[expect(
    clippy::ref_option,
    reason = "This follows a decision made by cedar-policy-core which we are using."
)]
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

#[expect(
    clippy::ref_option,
    reason = "This follows a decision made by cedar-policy-core which we are using."
)]
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
        .map(|(name, mut nsdef)| {
            nsdef
                .annotations
                .0
                .retain(|anno, _| anno.as_ref() != "mcp_server");
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
                                        && anno.as_ref() != "mcp_resource_parent"
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

/// Parse a `.cedarschema` schema stub
pub(super) fn parse_stub(stub: &str) -> Result<Fragment<RawName>, SchemaGeneratorError> {
    let extensions = cedar_policy_core::extensions::Extensions::all_available();
    let (fragment, _warnings) = Fragment::<RawName>::from_cedarschema_str(stub, extensions)
        .map_err(|e| SchemaGeneratorError::SchemaParseError(e.to_string()))?;
    Ok(fragment)
}

/// A server namespace of a schema stub: its name, its `@mcp_server` annotation, and its definition
pub(super) type ServerNamespace<'a> = (
    &'a Name,
    &'a Option<Annotation>,
    &'a NamespaceDefinition<RawName>,
);

/// Split the namespaces of `stub` into the main namespace, i.e., the only namespace without
/// `@mcp_server`, and the server namespaces
pub(super) fn split_stub(
    stub: &Fragment<RawName>,
) -> Result<
    (
        &Name,
        &NamespaceDefinition<RawName>,
        Vec<ServerNamespace<'_>>,
    ),
    SchemaGeneratorError,
> {
    let mut main = None;
    let mut servers = Vec::new();
    for (name, ns) in &stub.0 {
        let Some(name) = name else {
            return Err(SchemaGeneratorError::GlobalNamespaceUsed);
        };
        match ns.annotations.0.get(&*identifiers::MCP_SERVER) {
            Some(server) => servers.push((name, server, ns)),
            None if main.is_none() => main = Some((name, ns)),
            None => return Err(SchemaGeneratorError::WrongNumberOfNamespaces),
        }
    }
    let (name, ns) = main.ok_or(SchemaGeneratorError::WrongNumberOfNamespaces)?;
    Ok((name, ns, servers))
}

/// Qualify `id` with `namespace`, if any
pub(super) fn qualified_rawname(id: UnreservedId, namespace: Option<&Name>) -> RawName {
    RawName::from_name(RawName::new_from_unreserved(id, None).qualify_with_name(namespace))
}

/// The entity types of `ns` with annotation `annotation`, qualified with `qualifier`
pub(super) fn annotated_entity_types(
    ns: &NamespaceDefinition<RawName>,
    annotation: &AnyId,
    qualifier: Option<&Name>,
) -> Vec<RawName> {
    ns.entity_types
        .iter()
        .filter(|(_, ty)| ty.annotations.0.contains_key(annotation))
        .map(|(tyname, _)| qualified_rawname(tyname.clone(), qualifier))
        .collect()
}

/// The context keys given by the `@mcp_context` annotations of the entity and common types
/// of `ns`, mapped to these types qualified with `qualifier`
fn annotated_contexts(
    ns: &NamespaceDefinition<RawName>,
    qualifier: Option<&Name>,
) -> BTreeMap<SmolStr, RawName> {
    let context_key = |annotations: &Annotations| {
        annotations
            .0
            .get(&*identifiers::MCP_CONTEXT)
            .and_then(|anno| anno.as_ref())
            .map(|anno| anno.val.clone())
    };
    ns.entity_types
        .iter()
        .filter_map(|(tyname, ty)| {
            Some((
                context_key(&ty.annotations)?,
                qualified_rawname(tyname.clone(), qualifier),
            ))
        })
        .chain(ns.common_types.iter().filter_map(|(tyname, ty)| {
            Some((
                context_key(&ty.annotations)?,
                qualified_rawname(tyname.as_ref().clone(), qualifier),
            ))
        }))
        .collect()
}

/// The actions of `ns` with an `@mcp_action` annotation, with their type qualified with `qualifier`
pub(super) fn annotated_actions(
    ns: &NamespaceDefinition<RawName>,
    qualifier: Option<&Name>,
) -> Vec<ActionEntityUID<RawName>> {
    let ty = qualifier
        .map(|qualifier| qualified_rawname(identifiers::ACTION_ID.clone(), Some(qualifier)));
    ns.actions
        .iter()
        .filter(|(_, action)| action.annotations.0.contains_key(&*identifiers::MCP_ACTION))
        .map(|(name, _)| ActionEntityUID::new(ty.clone(), name.clone()))
        .collect()
}

/// Add the action `AllActions`, with parents `parents`, to `namespace`
fn add_all_actions(
    fragment: &mut Fragment<RawName>,
    namespace: &Name,
    parents: &[ActionEntityUID<RawName>],
) -> Result<(), SchemaGeneratorError> {
    let action = ActionType {
        external: false,
        attributes: None,
        applies_to: None,
        member_of: (!parents.is_empty()).then(|| parents.to_vec()),
        annotations: Annotations::new(),
        loc: None,
    };
    #[expect(
        clippy::unwrap_used,
        reason = "This function is only called on namespaces appearing in fragment."
    )]
    let nsdef = fragment.0.get_mut(&Some(namespace.clone())).unwrap();
    match nsdef.actions.entry(identifiers::ALL_ACTIONS.into()) {
        Entry::Occupied(_) => Err(SchemaGeneratorError::AllActionsInStub(
            namespace.to_string(),
        )),
        Entry::Vacant(vac) => {
            vac.insert(action);
            Ok(())
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use cedar_policy_core::extensions::Extensions;
    use cool_asserts::assert_matches;
    use mcp_tools_sdk::description::Property;

    use std::collections::HashMap;

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
    fn test_default() {
        let schema_stub = test_schema_stub();

        let tool = r#"{
    "name": "check_task_status",
    "description": "Check if a task is ready for work",
    "inputSchema": {
        "type": "object",
        "properties": {
            "task_id": {"type": "string"},
            "sub_tasks": { "type": "array" }
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

        let mut schema_generator =
            SchemaGenerator::new(schema_stub).expect("Failed to create schema generator");
        schema_generator
            .add_action_from_tool_description(&tool)
            .expect("Failed to add tool description");

        let schema = schema_generator.get_schema();

        assert!(schema.0.iter().count() == 1, "Expected only two namespaces");

        let root_namespace = Some("Test".parse::<Name>().unwrap());

        let root_nsdef = schema
            .0
            .get(&root_namespace)
            .expect("Expected namespace Test to exist");

        assert!(root_nsdef.actions.contains_key("check_task_status"));
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
        // Check that common type references is not encoded as `Entity`
        assert_matches!(
            root_nsdef.common_types.get(&CommonTypeId::unchecked("test_toolInput".parse().unwrap())),
            Some(
                CommonType { ty: Type::Type{ ty: TypeVariant::Record(test_tool_rty), ..}, .. }
            ) => {
                assert_matches!(
                    test_tool_rty.attributes.get("test_obj"),
                    Some(TypeOfAttribute { ty: Type::Type { ty: TypeVariant::EntityOrCommon { .. }, .. }, .. })
                );
            }
        );

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

    #[test]
    fn test_encode_numbers_as_decimals() {
        let schema_stub = test_schema_stub();
        let config = SchemaGeneratorConfig::default().encode_numbers_as_decimal(true);

        let tool = r#"{
    "name": "test_tool",
    "description": "A tool for testing purposes",
    "parameters": {
        "type": "object",
        "properties": {
            "test_number": {"type": "number"},
            "test_float": {"type": "float"}
        },
        "required": ["test_number"]
    }
}"#;

        let tool = ToolDescription::from_json_str(tool).expect("Failed to parse tool description");

        let mut schema_generator = SchemaGenerator::new_with_config(schema_stub, config)
            .expect("Failed to create schema generator");
        schema_generator
            .add_action_from_tool_description(&tool)
            .expect("Failed to add tool description");

        let schema = schema_generator.get_schema();

        assert!(schema.0.iter().count() == 1);
        let root_namespace = Some("Test".parse::<Name>().unwrap());
        let root_nsdef = schema
            .0
            .get(&root_namespace)
            .expect("Expected namespace Test to exist");

        assert!(root_nsdef.actions.contains_key("test_tool"));
        // Only `test_toolInput` type is added
        assert!(root_nsdef.common_types.iter().count() == 1);
        assert!(root_nsdef.entity_types.iter().count() == 3);

        let test_tool_input_type = root_nsdef.common_types.iter().next().unwrap().1;
        // assert that both `test_number` and `test_float` are encoded as `decimal`.
        assert_matches!(
            test_tool_input_type.ty,
            Type::Type {
                ty: TypeVariant::Record(
                    RecordType {
                        ref attributes, ..
                    }
                ), ..
            }
            if matches!(
                attributes.get("test_number"),
                Some(TypeOfAttribute {
                    ty: Type::Type {
                        ty: TypeVariant::EntityOrCommon {
                            ref type_name
                        },
                        ..
                    },
                    ..
                })
                if *type_name == "decimal".parse().unwrap()
            ) && matches!(
                attributes.get("test_float"),
                Some(TypeOfAttribute {
                    ty: Type::Type {
                        ty: TypeVariant::EntityOrCommon {
                            ref type_name
                        },
                        ..
                    },
                    ..
                })
                if *type_name == "decimal".parse().unwrap()
            )
        );
    }

    #[test]
    fn test_global_namespace_used_error() {
        let schema = r#"@mcp_principal("User")
    entity user;
"#;
        let schema_stub = Fragment::from_cedarschema_str(schema, Extensions::all_available())
            .expect("Failed to parse schema")
            .0;

        assert_matches!(
            SchemaGenerator::new(schema_stub),
            Err(SchemaGeneratorError::GlobalNamespaceUsed)
        );
    }

    #[test]
    fn test_multiple_namespaces_used_error() {
        let schema = r#"namespace Test {
}

namespace Test2 {
}"#;

        let schema_stub = Fragment::from_cedarschema_str(schema, Extensions::all_available())
            .expect("Failed to parse schema")
            .0;

        assert_matches!(
            SchemaGenerator::new(schema_stub),
            Err(SchemaGeneratorError::WrongNumberOfNamespaces)
        );
    }

    #[test]
    fn test_no_namespaces_error() {
        let schema = r#""#;
        let schema_stub = Fragment::from_cedarschema_str(schema, Extensions::all_available())
            .expect("Failed to parse schema")
            .0;

        assert_matches!(
            SchemaGenerator::new(schema_stub),
            Err(SchemaGeneratorError::WrongNumberOfNamespaces)
        );
    }

    #[test]
    fn test_no_principal_declared_error() {
        let schema = r#"namespace Test {
    // forgot to annotate mcp_principal
    entity user;

    @mcp_resource("McpServer")
    entity resource;

    @mcp_context("foo")
    entity Foo;
}"#;

        let schema_stub = Fragment::from_cedarschema_str(schema, Extensions::all_available())
            .expect("Failed to parse schema")
            .0;

        assert_matches!(
            SchemaGenerator::new(schema_stub),
            Err(SchemaGeneratorError::NoPrincipalTypes)
        );
    }

    #[test]
    fn test_no_resource_declared_error() {
        let schema = r#"namespace Test {
    @mcp_principal("User")
    entity user;

    // forgot to annotate mcp_resource
    entity resource;

    @mcp_context("foo")
    entity Foo;
}"#;

        let schema_stub = Fragment::from_cedarschema_str(schema, Extensions::all_available())
            .expect("Failed to parse schema")
            .0;

        assert_matches!(
            SchemaGenerator::new(schema_stub),
            Err(SchemaGeneratorError::NoResourceTypes)
        );
    }

    #[test]
    fn test_adding_tool_action_errors() {
        let schema = r#"namespace Test {
    @mcp_principal("User")
    entity user;

    @mcp_resource("McpServer")
    entity resource;

    @mcp_context("foo")
    entity Foo;

    type test_toolInput = {};
}"#;

        let schema_stub = Fragment::from_cedarschema_str(schema, Extensions::all_available())
            .expect("Failed to parse schema")
            .0;

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
        let config = SchemaGeneratorConfig::default().erase_annotations(false);
        let mut schema_generator = SchemaGenerator::new_with_config(schema_stub.clone(), config)
            .expect("Failed to create schema generator");

        assert_matches!(
            schema_generator.add_action_from_tool_description(&tool),
            Err(SchemaGeneratorError::ConflictingSchemaNameError(..))
        );
        assert_eq!(&schema_stub, schema_generator.get_schema());
    }

    #[test]
    fn test_adding_server_actions_errors() {
        let schema = r#"namespace Test {
    @mcp_principal("User")
    entity user;

    @mcp_resource("McpServer")
    entity resource;

    @mcp_context("foo")
    entity Foo;

    type test_toolInput = {};
}"#;

        let schema_stub = Fragment::from_cedarschema_str(schema, Extensions::all_available())
            .expect("Failed to parse schema")
            .0;

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

        let tool =
            ServerDescription::from_json_str(tool).expect("Failed to parse tool description");
        let config = SchemaGeneratorConfig::default().erase_annotations(false);
        let mut schema_generator = SchemaGenerator::new_with_config(schema_stub.clone(), config)
            .expect("Failed to create schema generator");

        assert_matches!(
            schema_generator.add_actions_from_server_description(&tool),
            Err(SchemaGeneratorError::ConflictingSchemaNameError(..))
        );
        assert_eq!(&schema_stub, schema_generator.get_schema());
    }

    #[test]
    fn test_undefined_ref_error() {
        let schema_stub = test_schema_stub();

        let tool = r##"{
    "name": "test_tool",
    "description": "a test tool",
    "inputSchema": {
        "type": "object",
        "properties": {
            "test_attr": {
                "$ref": "#/$defs/undefined_ref"
            }
        },
        "required": []
    }
}"##;

        let tool =
            ServerDescription::from_json_str(tool).expect("Failed to parse tool description");
        let config = SchemaGeneratorConfig::default().erase_annotations(false);
        let mut schema_generator = SchemaGenerator::new_with_config(schema_stub.clone(), config)
            .expect("Failed to create schema generator");

        assert_matches!(
            schema_generator.add_actions_from_server_description(&tool),
            Err(SchemaGeneratorError::UndefinedReferenceType(..))
        );
        assert_eq!(&schema_stub, schema_generator.get_schema());
    }

    #[test]
    fn test_empty_enum_error() {
        let schema_stub = test_schema_stub();

        // An empty enum doesn't parse with our deserializer. Need to construct
        let tool = ToolDescription::new(
            "test_tool".to_smolstr(),
            Parameters::new(
                vec![Property::new(
                    "empty_enum".to_smolstr(),
                    true,
                    PropertyType::Enum {
                        variants: Vec::new(),
                    },
                    None,
                )],
                HashMap::new(),
            ),
            Parameters::new(Vec::new(), HashMap::new()),
            HashMap::new(),
            None,
        );

        let config = SchemaGeneratorConfig::default().erase_annotations(false);
        let mut schema_generator = SchemaGenerator::new_with_config(schema_stub.clone(), config)
            .expect("Failed to create schema generator");

        assert_matches!(
            schema_generator.add_action_from_tool_description(&tool),
            Err(SchemaGeneratorError::EmptyEnumChoice(..))
        );
        assert_eq!(&schema_stub, schema_generator.get_schema());
    }

    // ── Tests for from_cedarschema_str and get_schema_as_str ──

    #[test]
    fn test_from_cedarschema_str_basic() {
        let stub = r#"
            namespace TestServer {
                @mcp_principal
                entity User;
                @mcp_resource
                entity McpServer;
                action "call_tool" appliesTo {
                    principal: [User],
                    resource: [McpServer]
                };
            }
        "#;
        let generator = SchemaGenerator::from_cedarschema_str(stub);
        assert!(generator.is_ok(), "Should parse valid cedarschema string");
    }

    #[test]
    fn test_from_cedarschema_str_with_config() {
        let stub = r#"
            namespace TestServer {
                @mcp_principal
                entity User;
                @mcp_resource
                entity McpServer;
                action "call_tool" appliesTo {
                    principal: [User],
                    resource: [McpServer]
                };
            }
        "#;
        let config = SchemaGeneratorConfig::default().include_outputs(true);
        let generator = SchemaGenerator::from_cedarschema_str_with_config(stub, config);
        assert!(
            generator.is_ok(),
            "Should parse valid cedarschema string with config"
        );
    }

    #[test]
    fn test_from_cedarschema_str_invalid_input() {
        let result = SchemaGenerator::from_cedarschema_str("not valid cedar schema");
        assert!(result.is_err(), "Should fail on invalid cedarschema");
        let err = result.unwrap_err();
        assert!(
            matches!(err, SchemaGeneratorError::SchemaParseError(_)),
            "Error should be SchemaParseError, got: {err:?}"
        );
    }

    #[test]
    fn test_from_cedarschema_str_matches_fragment_constructor() {
        // Verify that from_cedarschema_str produces the same generator
        // as manually parsing the fragment and calling new().
        let stub = r#"
            namespace TestServer {
                @mcp_principal
                entity User;
                @mcp_resource
                entity McpServer;
                action "call_tool" appliesTo {
                    principal: [User],
                    resource: [McpServer]
                };
            }
        "#;

        let gen_str = SchemaGenerator::from_cedarschema_str(stub).expect("from_cedarschema_str");

        let extensions = Extensions::all_available();
        let (fragment, _) =
            Fragment::<RawName>::from_cedarschema_str(stub, extensions).expect("parse fragment");
        let gen_frag = SchemaGenerator::new(fragment).expect("new from fragment");

        // Both should produce identical schema output
        assert_eq!(gen_str.get_schema_as_str(), gen_frag.get_schema_as_str());
    }

    #[test]
    fn test_get_schema_as_str_contains_namespace() {
        let stub = r#"
            namespace MyNamespace {
                @mcp_principal
                entity User;
                @mcp_resource
                entity McpServer;
                action "call_tool" appliesTo {
                    principal: [User],
                    resource: [McpServer]
                };
            }
        "#;
        let generator = SchemaGenerator::from_cedarschema_str(stub).expect("parse");
        let output = generator.get_schema_as_str();
        assert!(
            output.contains("MyNamespace"),
            "get_schema_as_str should contain the namespace name"
        );
    }

    #[test]
    fn test_get_schema_as_str_matches_display() {
        // get_schema_as_str should produce the same output as Display
        let stub = r#"
            namespace TestServer {
                @mcp_principal
                entity User;
                @mcp_resource
                entity McpServer;
                action "call_tool" appliesTo {
                    principal: [User],
                    resource: [McpServer]
                };
            }
        "#;
        let generator = SchemaGenerator::from_cedarschema_str(stub).expect("parse");
        let str_output = generator.get_schema_as_str();
        let display_output = format!("{}", generator.get_schema());
        assert_eq!(
            str_output, display_output,
            "get_schema_as_str and Display should produce identical output"
        );
    }

    #[test]
    fn test_is_primitive() {
        assert!(is_primitive(&PropertyType::Bool));
        assert!(is_primitive(&PropertyType::Integer));
        assert!(is_primitive(&PropertyType::Float));
        assert!(is_primitive(&PropertyType::Number));
        assert!(is_primitive(&PropertyType::String));
        assert!(is_primitive(&PropertyType::Decimal));
        assert!(is_primitive(&PropertyType::Datetime));
        assert!(is_primitive(&PropertyType::Duration));
        assert!(is_primitive(&PropertyType::IpAddr));
        assert!(is_primitive(&PropertyType::Null));
        assert!(is_primitive(&PropertyType::Unknown));

        assert!(!is_primitive(&PropertyType::Enum {
            variants: vec!["a".into()]
        }));
        assert!(!is_primitive(&PropertyType::Array {
            element_ty: Box::new(PropertyType::String)
        }));
        assert!(!is_primitive(&PropertyType::Object {
            properties: vec![],
            additional_properties: None
        }));
        assert!(!is_primitive(&PropertyType::Ref { name: "Foo".into() }));
        assert!(!is_primitive(&PropertyType::Tuple {
            types: vec![PropertyType::String]
        }));
        assert!(!is_primitive(&PropertyType::Union {
            types: vec![PropertyType::String]
        }));
    }

    #[test]
    fn test_leaf_record_fingerprint_equality() {
        let fp1 = EntityTypeFingerprint::LeafRecord {
            base_name: "config".parse().unwrap(),
            fields: vec![
                ("host".into(), PropertyType::String, true),
                ("port".into(), PropertyType::Integer, true),
            ],
        };
        let fp2 = EntityTypeFingerprint::LeafRecord {
            base_name: "config".parse().unwrap(),
            fields: vec![
                ("host".into(), PropertyType::String, true),
                ("port".into(), PropertyType::Integer, true),
            ],
        };
        assert_eq!(fp1, fp2);
    }

    #[test]
    fn test_leaf_record_fingerprint_different_name() {
        let fp1 = EntityTypeFingerprint::LeafRecord {
            base_name: "config".parse().unwrap(),
            fields: vec![("host".into(), PropertyType::String, true)],
        };
        let fp2 = EntityTypeFingerprint::LeafRecord {
            base_name: "settings".parse().unwrap(),
            fields: vec![("host".into(), PropertyType::String, true)],
        };
        assert_ne!(fp1, fp2);
    }

    #[test]
    fn test_leaf_record_fingerprint_different_field_names() {
        let fp1 = EntityTypeFingerprint::LeafRecord {
            base_name: "config".parse().unwrap(),
            fields: vec![("host".into(), PropertyType::String, true)],
        };
        let fp2 = EntityTypeFingerprint::LeafRecord {
            base_name: "config".parse().unwrap(),
            fields: vec![("url".into(), PropertyType::String, true)],
        };
        assert_ne!(fp1, fp2);
    }

    #[test]
    fn test_leaf_record_fingerprint_different_field_types() {
        let fp1 = EntityTypeFingerprint::LeafRecord {
            base_name: "config".parse().unwrap(),
            fields: vec![("port".into(), PropertyType::Integer, true)],
        };
        let fp2 = EntityTypeFingerprint::LeafRecord {
            base_name: "config".parse().unwrap(),
            fields: vec![("port".into(), PropertyType::String, true)],
        };
        assert_ne!(fp1, fp2);
    }

    #[test]
    fn test_leaf_record_fingerprint_different_required() {
        let fp1 = EntityTypeFingerprint::LeafRecord {
            base_name: "config".parse().unwrap(),
            fields: vec![("host".into(), PropertyType::String, true)],
        };
        let fp2 = EntityTypeFingerprint::LeafRecord {
            base_name: "config".parse().unwrap(),
            fields: vec![("host".into(), PropertyType::String, false)],
        };
        assert_ne!(fp1, fp2);
    }

    #[test]
    fn test_leaf_record_fingerprint_not_equal_to_enum() {
        let fp_record = EntityTypeFingerprint::LeafRecord {
            base_name: "status".parse().unwrap(),
            fields: vec![("code".into(), PropertyType::Integer, true)],
        };
        let fp_enum = EntityTypeFingerprint::Enum {
            base_name: "status".parse().unwrap(),
            variants: vec![Eid::new("active")],
        };
        assert_ne!(fp_record, fp_enum);
    }

    #[test]
    fn test_leaf_record_fingerprint_hash_consistency() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let fp1 = EntityTypeFingerprint::LeafRecord {
            base_name: "config".parse().unwrap(),
            fields: vec![
                ("host".into(), PropertyType::String, true),
                ("port".into(), PropertyType::Integer, false),
            ],
        };
        let fp2 = fp1.clone();

        let mut h1 = DefaultHasher::new();
        let mut h2 = DefaultHasher::new();
        fp1.hash(&mut h1);
        fp2.hash(&mut h2);
        assert_eq!(h1.finish(), h2.finish());
    }

    #[test]
    fn test_dedup_leaf_record_schema_generation() {
        let schema_stub = test_schema_stub();
        let config = SchemaGeneratorConfig::default().deduplicate_entity_types(true);

        let tools_json = r#"{
            "result": {
                "tools": [
                    {
                        "name": "tool_a",
                        "description": "Tool A",
                        "inputSchema": {
                            "json": {
                                "type": "object",
                                "properties": {
                                    "opts": {
                                        "type": "object",
                                        "properties": {
                                            "verbose": { "type": "boolean" },
                                            "limit": { "type": "integer" }
                                        },
                                        "required": ["verbose"]
                                    }
                                },
                                "required": ["opts"]
                            }
                        }
                    },
                    {
                        "name": "tool_b",
                        "description": "Tool B",
                        "inputSchema": {
                            "json": {
                                "type": "object",
                                "properties": {
                                    "opts": {
                                        "type": "object",
                                        "properties": {
                                            "verbose": { "type": "boolean" },
                                            "limit": { "type": "integer" }
                                        },
                                        "required": ["verbose"]
                                    }
                                },
                                "required": ["opts"]
                            }
                        }
                    }
                ]
            }
        }"#;

        let description =
            ServerDescription::from_json_str(tools_json).expect("Failed to parse tools JSON");
        let mut generator = SchemaGenerator::new_with_config(schema_stub, config)
            .expect("Failed to create schema generator");
        generator
            .add_actions_from_server_description(&description)
            .expect("Failed to add server description");

        let schema = generator.get_schema();
        let root_ns = Some("Test".parse::<Name>().unwrap());
        let root_nsdef = schema.0.get(&root_ns).expect("Expected namespace Test");

        // The deduplicated entity type should be in the root namespace
        assert!(
            root_nsdef
                .entity_types
                .contains_key(&"opts".parse().unwrap()),
            "Expected deduplicated 'opts' entity type in root namespace"
        );

        // Tool-local namespaces should NOT have their own 'opts'
        let tool_a_input_ns: Option<Name> = Some("Test::tool_a::Input".parse().unwrap());
        let tool_a_nsdef = schema.0.get(&tool_a_input_ns);
        assert!(
            tool_a_nsdef.is_none()
                || !tool_a_nsdef
                    .unwrap()
                    .entity_types
                    .contains_key(&"opts".parse().unwrap()),
            "tool_a::Input should not have local 'opts'"
        );
    }

    #[test]
    fn test_dedup_leaf_record_not_triggered_for_non_leaf() {
        // An object with a nested object property should NOT be fingerprinted as a leaf record
        let schema_stub = test_schema_stub();
        let config = SchemaGeneratorConfig::default().deduplicate_entity_types(true);

        let tools_json = r#"{
            "result": {
                "tools": [
                    {
                        "name": "tool_a",
                        "description": "Tool A",
                        "inputSchema": {
                            "json": {
                                "type": "object",
                                "properties": {
                                    "nested": {
                                        "type": "object",
                                        "properties": {
                                            "inner": {
                                                "type": "object",
                                                "properties": {
                                                    "val": { "type": "string" }
                                                }
                                            }
                                        }
                                    }
                                },
                                "required": ["nested"]
                            }
                        }
                    },
                    {
                        "name": "tool_b",
                        "description": "Tool B",
                        "inputSchema": {
                            "json": {
                                "type": "object",
                                "properties": {
                                    "nested": {
                                        "type": "object",
                                        "properties": {
                                            "inner": {
                                                "type": "object",
                                                "properties": {
                                                    "val": { "type": "string" }
                                                }
                                            }
                                        }
                                    }
                                },
                                "required": ["nested"]
                            }
                        }
                    }
                ]
            }
        }"#;

        let description =
            ServerDescription::from_json_str(tools_json).expect("Failed to parse tools JSON");
        let mut generator = SchemaGenerator::new_with_config(schema_stub, config)
            .expect("Failed to create schema generator");
        generator
            .add_actions_from_server_description(&description)
            .expect("Failed to add server description");

        let schema = generator.get_schema();
        let root_ns = Some("Test".parse::<Name>().unwrap());
        let root_nsdef = schema.0.get(&root_ns).expect("Expected namespace Test");

        // Non-leaf objects should NOT be deduplicated to root
        assert!(
            !root_nsdef
                .entity_types
                .contains_key(&"nested".parse().unwrap()),
            "Non-leaf 'nested' should not be deduplicated to root namespace"
        );
    }

    #[test]
    fn test_dedup_leaf_record_skipped_with_objects_as_records() {
        // With objects_as_records, leaf records become common types, not entity types.
        // Dedup should not apply.
        let schema_stub = test_schema_stub();
        let config = SchemaGeneratorConfig::default()
            .deduplicate_entity_types(true)
            .objects_as_records(true);

        let tools_json = r#"{
            "result": {
                "tools": [
                    {
                        "name": "tool_a",
                        "description": "Tool A",
                        "inputSchema": {
                            "json": {
                                "type": "object",
                                "properties": {
                                    "meta": {
                                        "type": "object",
                                        "properties": {
                                            "name": { "type": "string" }
                                        },
                                        "required": ["name"]
                                    }
                                },
                                "required": ["meta"]
                            }
                        }
                    },
                    {
                        "name": "tool_b",
                        "description": "Tool B",
                        "inputSchema": {
                            "json": {
                                "type": "object",
                                "properties": {
                                    "meta": {
                                        "type": "object",
                                        "properties": {
                                            "name": { "type": "string" }
                                        },
                                        "required": ["name"]
                                    }
                                },
                                "required": ["meta"]
                            }
                        }
                    }
                ]
            }
        }"#;

        let description =
            ServerDescription::from_json_str(tools_json).expect("Failed to parse tools JSON");
        let mut generator = SchemaGenerator::new_with_config(schema_stub, config)
            .expect("Failed to create schema generator");
        generator
            .add_actions_from_server_description(&description)
            .expect("Failed to add server description");

        let schema = generator.get_schema();
        let root_ns = Some("Test".parse::<Name>().unwrap());
        let root_nsdef = schema.0.get(&root_ns).expect("Expected namespace Test");

        // Should NOT have a deduplicated entity type in root
        assert!(
            !root_nsdef
                .entity_types
                .contains_key(&"meta".parse().unwrap()),
            "'meta' should not be deduplicated as entity type when objects_as_records is true"
        );
        // Should have common types in tool-local namespaces instead
        let tool_a_input_ns: Option<Name> = Some("Test::tool_a::Input".parse().unwrap());
        let tool_a_nsdef = schema
            .0
            .get(&tool_a_input_ns)
            .expect("Expected tool_a::Input namespace");
        assert!(
            tool_a_nsdef
                .common_types
                .contains_key(&CommonTypeId::new("meta".parse().unwrap()).unwrap()),
            "tool_a::Input should have local 'meta' common type"
        );
    }

    /// Enum types nested inside Union and Tuple properties are deduplicated
    /// across tools when they share the same name and variants.
    #[test]
    fn test_dedup_enum_inside_union_and_tuple() {
        let schema_stub = test_schema_stub();
        let config = SchemaGeneratorConfig::default().deduplicate_entity_types(true);

        let tools_json = r#"{
            "result": {
                "tools": [
                    {
                        "name": "tool_a",
                        "description": "Tool A",
                        "inputSchema": {
                            "json": {
                                "type": "object",
                                "properties": {
                                    "choice": {
                                        "anyOf": [
                                            { "type": "string", "enum": ["on", "off"] },
                                            { "type": "integer" }
                                        ]
                                    },
                                    "pair": {
                                        "type": "array",
                                        "prefixItems": [
                                            { "type": "string", "enum": ["on", "off"] },
                                            { "type": "integer" }
                                        ],
                                        "items": false
                                    }
                                },
                                "required": ["choice", "pair"]
                            }
                        }
                    },
                    {
                        "name": "tool_b",
                        "description": "Tool B",
                        "inputSchema": {
                            "json": {
                                "type": "object",
                                "properties": {
                                    "choice": {
                                        "anyOf": [
                                            { "type": "string", "enum": ["on", "off"] },
                                            { "type": "integer" }
                                        ]
                                    },
                                    "pair": {
                                        "type": "array",
                                        "prefixItems": [
                                            { "type": "string", "enum": ["on", "off"] },
                                            { "type": "integer" }
                                        ],
                                        "items": false
                                    }
                                },
                                "required": ["choice", "pair"]
                            }
                        }
                    }
                ]
            }
        }"#;

        let description =
            ServerDescription::from_json_str(tools_json).expect("Failed to parse tools JSON");
        let mut generator = SchemaGenerator::new_with_config(schema_stub, config)
            .expect("Failed to create schema generator");
        generator
            .add_actions_from_server_description(&description)
            .expect("Failed to add server description");

        let schema = generator.get_schema();
        let root_ns = Some("Test".parse::<Name>().unwrap());
        let root_nsdef = schema.0.get(&root_ns).expect("Expected namespace Test");

        // The enum "on"/"off" appears identically in both tools (inside union and tuple),
        // so dedup should hoist shared entity types to the root namespace.
        assert!(
            root_nsdef
                .entity_types
                .contains_key(&"TypeChoice0".parse().unwrap()),
            "Expected deduplicated enum from union in root namespace"
        );
        assert!(
            root_nsdef
                .entity_types
                .contains_key(&"Proj0".parse().unwrap()),
            "Expected deduplicated enum from tuple in root namespace"
        );
    }

    fn bad_tool() -> ToolDescription {
        let json = r#"{
    "name": "bad_tool",
    "description": "Tool with invalid property name",
    "inputSchema": {
        "type": "object",
        "properties": { "foo bar": {"type": "string"} }
    }
}"#;
        ToolDescription::from_json_str(json).expect("Failed to parse tool description")
    }

    fn good_tool() -> ToolDescription {
        let json = r#"{
    "name": "good_tool",
    "description": "Valid tool",
    "inputSchema": {
        "type": "object",
        "properties": { "task_id": {"type": "string"} }
    }
}"#;
        ToolDescription::from_json_str(json).expect("Failed to parse tool description")
    }

    #[test]
    fn test_failed_add_action_does_not_lock_generator() {
        let mut schema_gen =
            SchemaGenerator::new(test_schema_stub()).expect("Failed to create schema generator");

        assert!(schema_gen
            .add_action_from_tool_description(&bad_tool())
            .is_err());

        assert_eq!(
            schema_gen
                .targets
                .get(None)
                .unwrap()
                .tools
                .tool_descriptions()
                .count(),
            0,
            "Failed tool should not remain in generator state"
        );

        schema_gen
            .add_action_from_tool_description(&good_tool())
            .expect("Generator should accept a valid tool after a failed attempt");
    }

    #[test]
    fn test_failed_add_actions_from_server_does_not_lock_generator() {
        let bad_server = ServerDescription::new(vec![bad_tool()].into_iter(), HashMap::new());

        let mut gen =
            SchemaGenerator::new(test_schema_stub()).expect("Failed to create schema generator");

        assert!(gen
            .add_actions_from_server_description(&bad_server)
            .is_err());

        gen.add_action_from_tool_description(&good_tool())
            .expect("Generator should accept a valid tool after a failed server description");
    }
}

#[cfg(test)]
mod coverage_tests {
    use super::*;

    #[test]
    fn test_schema_parse_error_display_format() {
        // Exercises the SchemaParseError Display impl from err.rs
        let result = SchemaGenerator::from_cedarschema_str("this is not valid cedar");
        assert!(result.is_err());
        let err = result.unwrap_err();
        let err_msg = format!("{err}");
        assert!(
            err_msg.contains("Failed to parse Cedar schema"),
            "Error message should contain expected prefix, got: {err_msg}"
        );
    }

    #[test]
    fn test_from_cedarschema_str_with_config_error_path() {
        // Exercises the error path of from_cedarschema_str_with_config
        let config = SchemaGeneratorConfig::default().encode_numbers_as_decimal(true);
        let result = SchemaGenerator::from_cedarschema_str_with_config("invalid schema", config);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SchemaGeneratorError::SchemaParseError(_)
        ));
    }

    #[test]
    fn test_get_schema_as_str_content_validation() {
        // Validates the content of get_schema_as_str more thoroughly
        let stub = r#"
            namespace TestServer {
                @mcp_principal
                entity User;
                @mcp_resource
                entity McpServer;
                action "call_tool" appliesTo {
                    principal: [User],
                    resource: [McpServer]
                };
            }
        "#;

        let gen = SchemaGenerator::from_cedarschema_str(stub).expect("parse");
        let output = gen.get_schema_as_str();
        assert!(output.contains("TestServer"), "Should contain namespace");
        assert!(output.contains("User"), "Should contain User entity");
        assert!(
            output.contains("McpServer"),
            "Should contain McpServer entity"
        );
        assert!(
            output.contains("call_tool"),
            "Should contain call_tool action"
        );
        // Verify it matches Display for the same fragment
        assert_eq!(output, format!("{}", gen.get_schema()));
    }
}

#[cfg(test)]
mod multi_namespace_tests {
    use super::*;
    use cool_asserts::assert_matches;

    const MAIN: &str = r#"@mcp_principal("User") entity User;
        @mcp_context("session") type Session = { id: String };
        @mcp_action("Mcp") action Mcp;"#;
    const TARGET: &str = r#"@mcp_resource("Target") entity Target;"#;
    const S1: &str = r#"@mcp_server("s1")"#;

    /// A stub with main namespace `Main` with body `main`, and the server namespaces
    /// `(annotation, namespace, body)` of `servers`
    fn stub(main: &str, servers: &[(&str, &str, &str)]) -> String {
        servers.iter().fold(
            format!("namespace Main {{ {main} }}\n"),
            |stub, (annotation, namespace, body)| {
                format!("{stub}{annotation} namespace {namespace} {{ {body} }}\n")
            },
        )
    }

    /// A stub with server `s1` in namespace `Main::S1` with body `server`
    fn one_server(main: &str, server: &str) -> String {
        stub(main, &[(S1, "Main::S1", server)])
    }

    fn generator(stub: &str) -> SchemaGenerator {
        SchemaGenerator::from_cedarschema_str(stub).unwrap()
    }

    fn tool(name: &str) -> ServerDescription {
        ServerDescription::from_json_str(&format!(
            r#"{{"name": "{name}", "inputSchema": {{"type": "object"}}}}"#
        ))
        .unwrap()
    }

    fn actions<'a>(
        gen: &'a SchemaGenerator,
        namespace: &str,
    ) -> &'a BTreeMap<SmolStr, ActionType<RawName>> {
        &gen.get_schema().0[&Some(namespace.parse().unwrap())].actions
    }

    #[test]
    fn server_namespace_gets_actions() {
        let mut gen = generator(&one_server(MAIN, TARGET));
        gen.add_actions_from_named_server_description("s1", &tool("t"))
            .unwrap();
        let expected = r#"namespace Main {
  type Session = {
    id: String
  };

  entity User;

  action "Mcp";
}

namespace Main::S1 {
  type tInput = {  };

  entity Target;

  action "AllActions" in [Main::Action::"Mcp"];

  action "t" in [Action::"AllActions"] appliesTo {
    principal: [Main::User],
    resource: [Target],
    context: {
      input: tInput,
      session: Main::Session
    }
  };
}
"#;
        assert_eq!(gen.get_schema().to_cedarschema().unwrap(), expected);
        gen.new_request_generator().unwrap();
    }

    #[test]
    fn main_namespace_with_path() {
        let stub = r#"namespace A::B { @mcp_principal("User") entity User; }
            @mcp_server("s1") namespace A::B::S1 { @mcp_resource("Target") entity Target; }"#;
        let mut gen = generator(stub);
        gen.add_actions_from_named_server_description("s1", &tool("t"))
            .unwrap();
        let applies_to = actions(&gen, "A::B::S1")["t"].applies_to.as_ref().unwrap();
        assert_eq!(
            applies_to.principal_types,
            vec!["A::B::User".parse().unwrap()]
        );
        gen.new_request_generator().unwrap();
    }

    #[test]
    fn json_stub() {
        let stub = r#"{
            "Main": {
                "entityTypes": { "User": { "annotations": { "mcp_principal": "User" } } },
                "actions": {}
            },
            "Main::S1": {
                "annotations": { "mcp_server": "s1" },
                "entityTypes": { "Target": { "annotations": { "mcp_resource": "Target" } } },
                "actions": {}
            }
        }"#;
        let mut gen = SchemaGenerator::new(Fragment::from_json_str(stub).unwrap()).unwrap();
        gen.add_actions_from_named_server_description("s1", &tool("t"))
            .unwrap();
        assert!(actions(&gen, "Main::S1").contains_key("t"));
    }

    #[test]
    fn all_actions_without_main_action_has_no_parent() {
        let main = MAIN.replace(r#"@mcp_action("Mcp")"#, "");
        let gen = generator(&one_server(&main, TARGET));
        assert_matches!(
            actions(&gen, "Main::S1").get(identifiers::ALL_ACTIONS),
            Some(ActionType {
                member_of: None,
                applies_to: None,
                ..
            })
        );
    }

    #[test]
    fn server_action_replaces_all_actions() {
        let server = format!(r#"{TARGET} @mcp_action("Read") action Read;"#);
        let mut gen = generator(&one_server(MAIN, &server));
        gen.add_actions_from_named_server_description("s1", &tool("t"))
            .unwrap();
        let actions = actions(&gen, "Main::S1");
        assert!(!actions.contains_key(identifiers::ALL_ACTIONS));
        // The generator does not add parents to `Read`
        assert_matches!(
            actions.get("Read"),
            Some(ActionType {
                member_of: None,
                ..
            })
        );
        assert_eq!(
            actions["t"].member_of,
            Some(vec![ActionEntityUID::new(None, "Read".into())])
        );
    }

    #[test]
    fn server_context_overrides_and_extends_main_context() {
        let server = format!(
            r#"{TARGET} @mcp_context("session") entity Session; @mcp_context("region") type Region = String;"#
        );
        let mut gen = generator(&one_server(MAIN, &server));
        gen.add_actions_from_named_server_description("s1", &tool("t"))
            .unwrap();
        assert!(gen.get_schema_as_str().contains(
            "    context: {\n      input: tInput,\n      region: Region,\n      session: Session\n    }"
        ));
        gen.new_request_generator().unwrap();
    }

    #[test]
    fn same_tool_name_in_two_servers() {
        let stub = stub(
            MAIN,
            &[
                (S1, "Main::S1", TARGET),
                (r#"@mcp_server("s2")"#, "Main::S2", TARGET),
            ],
        );
        let mut gen = generator(&stub);
        gen.add_actions_from_named_server_descriptions(&BTreeMap::from([
            ("s1".to_string(), tool("t")),
            ("s2".to_string(), tool("t")),
        ]))
        .unwrap();
        assert!(actions(&gen, "Main::S1").contains_key("t"));
        assert!(actions(&gen, "Main::S2").contains_key("t"));
        gen.new_request_generator().unwrap();
    }

    #[test]
    fn name_conflicts() {
        let currency = ServerDescription::from_json_str(
            r##"{"result": {
                "tools": [{"name": "t", "inputSchema": {"type": "object", "properties": {"c": {"$ref": "#/$defs/Currency"}}}}],
                "$defs": {"Currency": {"type": "string", "enum": ["USD"]}}
            }}"##,
        )
        .unwrap();
        let conflicting_type =
            "Conflicting type definitions between MCP Tool Description and input Cedar Schema Stub File.";
        let cases = [
            (
                one_server(MAIN, TARGET),
                tool("AllActions"),
                r#"The action of tool `AllActions` conflicts with the action `Main::S1::Action::"AllActions"`."#,
            ),
            (
                one_server(MAIN, &format!(r#"{TARGET} @mcp_action("t") action t;"#)),
                tool("t"),
                r#"The action of tool `t` conflicts with the action `Main::S1::Action::"t"`."#,
            ),
            (
                one_server(MAIN, &format!("{TARGET} type tInput = {{}};")),
                tool("t"),
                conflicting_type,
            ),
            (
                one_server(MAIN, &format!("{TARGET} entity Currency;")),
                currency,
                conflicting_type,
            ),
        ];
        for (stub, tools, expected) in cases {
            let err = generator(&stub)
                .add_actions_from_named_server_description("s1", &tools)
                .unwrap_err();
            assert_eq!(err.to_string(), expected, "for stub:\n{stub}");
        }

        let mut gen = generator(&format!(
            r#"namespace Main {{ {MAIN} @mcp_resource("Server") entity Server; }}"#
        ));
        assert_eq!(
            gen.add_actions_from_server_description(&tool("Mcp"))
                .unwrap_err()
                .to_string(),
            r#"The action of tool `Mcp` conflicts with the action `Main::Action::"Mcp"`."#
        );
    }

    #[test]
    fn server_annotation_is_erased_unless_kept() {
        let stub = one_server(MAIN, TARGET);
        let server_ns = Some("Main::S1".parse().unwrap());
        assert!(generator(&stub).get_schema().0[&server_ns]
            .annotations
            .is_empty());
        let gen = SchemaGenerator::from_cedarschema_str_with_config(
            &stub,
            SchemaGeneratorConfig::default().erase_annotations(false),
        )
        .unwrap();
        assert!(gen.get_schema().0[&server_ns]
            .annotations
            .0
            .contains_key(&*identifiers::MCP_SERVER));
    }

    #[test]
    fn wrong_api_or_unknown_server() {
        let mut gen = generator(&one_server(MAIN, TARGET));
        assert_matches!(
            gen.add_actions_from_server_description(&tool("t")),
            Err(SchemaGeneratorError::ServerLookup(
                ServerLookupError::ServerNameRequired
            ))
        );
        assert_matches!(
            gen.add_action_from_tool_description(tool("t").tool_descriptions().next().unwrap()),
            Err(SchemaGeneratorError::ServerLookup(
                ServerLookupError::ServerNameRequired
            ))
        );
        assert_matches!(
            gen.add_actions_from_named_server_description("s2", &tool("t")),
            Err(SchemaGeneratorError::ServerLookup(ServerLookupError::UnknownServer(name))) if name == "s2"
        );

        let mut gen = generator(&format!("namespace Main {{ {MAIN} {TARGET} }}"));
        assert_matches!(
            gen.add_actions_from_named_server_description("s1", &tool("t")),
            Err(SchemaGeneratorError::ServerLookup(
                ServerLookupError::NoServerNamespaces
            ))
        );
    }

    #[test]
    fn stub_errors() {
        let two_mains = format!("{}namespace Other {{}}", one_server(MAIN, TARGET));
        let resource_in_main =
            one_server(&format!(r#"{MAIN} @mcp_resource("R") entity R;"#), TARGET);
        let cases = [
            (two_mains, "Expected schema with a single namespace not annotated with `@mcp_server`"),
            (
                format!("{S1} namespace Main::S1 {{ {TARGET} }}"),
                "Expected schema with a single namespace not annotated with `@mcp_server`",
            ),
            (
                stub(MAIN, &[("@mcp_server", "Main::S1", TARGET)]),
                "Namespace `Main::S1` has an `@mcp_server` annotation without a server name.",
            ),
            (
                stub(MAIN, &[(r#"@mcp_server("")"#, "Main::S1", TARGET)]),
                "Namespace `Main::S1` has an `@mcp_server` annotation without a server name.",
            ),
            (
                stub(MAIN, &[(S1, "Main::S1", TARGET), (S1, "Main::S2", TARGET)]),
                "Server name `s1` is used by more than one `@mcp_server` namespace.",
            ),
            (
                stub(MAIN, &[(S1, "Main::S1::T", TARGET)]),
                "Server namespace `Main::S1::T` is not a direct child of the main namespace `Main`.",
            ),
            (
                stub(MAIN, &[(S1, "Other::S1", TARGET)]),
                "Server namespace `Other::S1` is not a direct child of the main namespace `Main`.",
            ),
            (
                stub(MAIN, &[(S1, "S1", TARGET)]),
                "Server namespace `S1` is not a direct child of the main namespace `Main`.",
            ),
            (
                one_server(MAIN, &format!(r#"{TARGET} @mcp_principal("U") entity U;"#)),
                "Server namespace `Main::S1` declares an `@mcp_principal` entity type.",
            ),
            (
                resource_in_main,
                "The main namespace `Main` declares an `@mcp_resource` entity type, but the schema stub has `@mcp_server` namespaces.",
            ),
            (
                one_server(MAIN, "entity Target;"),
                "Server namespace `Main::S1` declares 0 `@mcp_resource` entity types.",
            ),
            (
                one_server(MAIN, &format!(r#"{TARGET} @mcp_resource("T2") entity T2;"#)),
                "Server namespace `Main::S1` declares 2 `@mcp_resource` entity types.",
            ),
            (
                one_server(&MAIN.replace(r#""session""#, r#""input""#), TARGET),
                "`@mcp_context(\"input\")` uses a context key reserved by the schema generator.",
            ),
            (
                one_server(MAIN, &format!(r#"{TARGET} @mcp_context("output") entity O;"#)),
                "`@mcp_context(\"output\")` uses a context key reserved by the schema generator.",
            ),
            (
                one_server(&MAIN.replace(r#"@mcp_principal("User")"#, ""), TARGET),
                "No MCP Principal Entity types specified.",
            ),
            (
                one_server(MAIN, &format!("{TARGET} action AllActions;")),
                "Server namespace `Main::S1` declares an action `AllActions`, but no `@mcp_action`.",
            ),
        ];
        for (stub, expected) in cases {
            let err = SchemaGenerator::from_cedarschema_str(&stub).unwrap_err();
            assert_eq!(err.to_string(), expected, "for stub:\n{stub}");
        }

        assert_matches!(
            SchemaGenerator::from_cedarschema_str_with_config(
                &one_server(MAIN, TARGET),
                SchemaGeneratorConfig::default().flatten_namespaces(true),
            ),
            Err(SchemaGeneratorError::FlattenWithServerNamespaces)
        );
    }

    #[test]
    fn reserved_context_keys_allowed_without_server_namespaces() {
        generator(&format!(
            r#"namespace Main {{ {MAIN} {TARGET} @mcp_context("input") entity Foo; }}"#
        ));
    }
}
