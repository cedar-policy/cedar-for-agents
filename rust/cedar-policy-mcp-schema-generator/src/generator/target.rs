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

//! The namespaces that tools are added to.
//!
//! A schema stub without `@mcp_server` namespaces has a single `ToolTarget`: its only namespace.
//! A schema stub with `@mcp_server` namespaces has one `ToolTarget` per server namespace, and
//! no tools are added to its main namespace.

use std::collections::{BTreeMap, HashMap};

use cedar_policy_core::ast::Name;
use cedar_policy_core::validator::{json_schema::ActionEntityUID, RawName};
use mcp_tools_sdk::description::ServerDescription;
use smol_str::SmolStr;

use super::err::ServerLookupError;
use super::schema::{DeduplicatedEntityType, EntityTypeFingerprint};

/// A namespace that tool actions are generated in, with what these actions apply to.
#[derive(Debug, Clone)]
pub(crate) struct ToolTarget {
    /// The namespace containing the generated actions. Each tool gets a child namespace for its
    /// generated types.
    pub(crate) namespace: Option<Name>,
    /// The resource types of the generated actions, relative to `namespace`.
    pub(crate) resources: Vec<RawName>,
    /// The `@mcp_context` attributes of the generated actions, relative to `namespace`.
    pub(crate) contexts: BTreeMap<SmolStr, RawName>,
    /// The parents of the generated actions, relative to `namespace`.
    pub(crate) actions: Option<Vec<ActionEntityUID<RawName>>>,
    /// The tools added so far.
    pub(crate) tools: ServerDescription,
    /// Resolved deduplication decisions (only when `deduplicate_entity_types` is set).
    /// Maps fingerprint → placement info for entity types that appear in multiple tools.
    pub(crate) resolved_dedup: Option<HashMap<EntityTypeFingerprint, DeduplicatedEntityType>>,
}

impl ToolTarget {
    pub(crate) fn new(
        namespace: Option<Name>,
        resources: Vec<RawName>,
        contexts: BTreeMap<SmolStr, RawName>,
        actions: Vec<ActionEntityUID<RawName>>,
    ) -> Self {
        Self {
            namespace,
            resources,
            contexts,
            actions: if actions.is_empty() {
                None
            } else {
                Some(actions)
            },
            tools: ServerDescription::new(std::iter::empty(), HashMap::new()),
            resolved_dedup: None,
        }
    }
}

/// The `ToolTarget`s of a schema stub.
#[derive(Debug, Clone)]
pub(crate) enum ToolTargets {
    /// The schema stub has no `@mcp_server` namespace: tools go in its only namespace.
    Single(ToolTarget),
    /// The schema stub has `@mcp_server` namespaces: tools go in the namespace of their server,
    /// keyed by server name.
    Multi(BTreeMap<SmolStr, ToolTarget>),
}

impl ToolTargets {
    /// Get the target of the server named `server`, or the single target if `server` is `None`.
    pub(crate) fn get(&self, server: Option<&str>) -> Result<&ToolTarget, ServerLookupError> {
        match (self, server) {
            (Self::Single(target), None) => Ok(target),
            (Self::Single(_), Some(_)) => Err(ServerLookupError::NoServerNamespaces),
            (Self::Multi(_), None) => Err(ServerLookupError::ServerNameRequired),
            (Self::Multi(targets), Some(server)) => targets
                .get(server)
                .ok_or_else(|| ServerLookupError::UnknownServer(server.into())),
        }
    }

    /// Mutable version of `get`.
    pub(crate) fn get_mut(
        &mut self,
        server: Option<&str>,
    ) -> Result<&mut ToolTarget, ServerLookupError> {
        match (self, server) {
            (Self::Single(target), None) => Ok(target),
            (Self::Single(_), Some(_)) => Err(ServerLookupError::NoServerNamespaces),
            (Self::Multi(_), None) => Err(ServerLookupError::ServerNameRequired),
            (Self::Multi(targets), Some(server)) => targets
                .get_mut(server)
                .ok_or_else(|| ServerLookupError::UnknownServer(server.into())),
        }
    }
}
