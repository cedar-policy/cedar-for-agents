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
use super::schema::{annotated_actions, annotated_entity_types, parse_stub, split_stub};
use crate::SchemaGeneratorError;

use cedar_policy_core::ast::{Annotation, AnyId, Name, UnreservedId};
use cedar_policy_core::est::Annotations;
use cedar_policy_core::validator::{
    json_schema::{
        ActionType, AttributesOrContext, EntityType, EntityTypeKind, Fragment, NamespaceDefinition,
        StandardEntityType,
    },
    RawName,
};

use std::collections::BTreeMap;

/// Builds a schema stub by adding a namespace for each MCP server to a schema stub, which
/// may have only its main namespace.
///
/// Each added namespace, `Main::{id}` for main namespace `Main`, contains:
/// - the resource type `Target`, child of the entity types of the main namespace annotated with
///   `@mcp_resource_parent`,
/// - the action `AllActions`, child of the actions of the main namespace annotated with
///   `@mcp_action`.
///
/// The builder only checks the namespaces of the stub; `SchemaGenerator` checks their content.
#[derive(Debug, Clone)]
pub struct StubBuilder {
    stub: Fragment<RawName>,
}

impl StubBuilder {
    /// Create a `StubBuilder` from a Cedar Schema Fragment
    pub fn new(stub: Fragment<RawName>) -> Result<Self, SchemaGeneratorError> {
        split_stub(&stub)?;
        Ok(Self { stub })
    }

    /// Create a `StubBuilder` from a `.cedarschema` string
    pub fn from_cedarschema_str(stub: &str) -> Result<Self, SchemaGeneratorError> {
        Self::new(parse_stub(stub)?)
    }

    /// Add a namespace for server `server`. Its identifier is `server`, where every character
    /// that is not an ASCII letter, digit or `_` is replaced by `_`, with a `_` prefix if it
    /// starts with a digit; e.g., `weather-api` gives `weather_api`. Distinct server names can
    /// give the same identifier, e.g., `weather-api` and `weather_api`.
    pub fn add_server(self, server: &str) -> Result<Self, SchemaGeneratorError> {
        let namespace = namespace_identifier(server);
        self.add_server_with_namespace(server, &namespace)
    }

    /// Add a namespace for server `server`, with identifier `namespace`
    pub fn add_server_with_namespace(
        mut self,
        server: &str,
        namespace: &str,
    ) -> Result<Self, SchemaGeneratorError> {
        let invalid = || SchemaGeneratorError::InvalidServerNamespace {
            server: server.into(),
            namespace: namespace.into(),
        };
        if server.is_empty() {
            return Err(invalid());
        }
        let id: UnreservedId = namespace.parse().map_err(|_| invalid())?;

        let (main, main_ns, servers) = split_stub(&self.stub)?;
        if servers
            .iter()
            .any(|(_, name, _)| name.as_ref().is_some_and(|name| name.val == server))
        {
            return Err(SchemaGeneratorError::DuplicateServerName(server.into()));
        }
        let server_ns = Name::unqualified_name(id).qualify_with_name(Some(main));
        if self.stub.0.contains_key(&Some(server_ns.clone())) {
            return Err(SchemaGeneratorError::ExistingServerNamespace(
                server_ns.to_string(),
            ));
        }

        let parents =
            annotated_entity_types(main_ns, &identifiers::MCP_RESOURCE_PARENT, Some(main));
        let target = EntityType {
            external: false,
            kind: EntityTypeKind::Standard(StandardEntityType {
                member_of_types: parents,
                shape: AttributesOrContext::default(),
                tags: None,
            }),
            annotations: annotation(&identifiers::MCP_RESOURCE, identifiers::TARGET.as_ref()),
            loc: None,
        };
        let actions = annotated_actions(main_ns, Some(main));
        let all_actions = ActionType {
            external: false,
            attributes: None,
            applies_to: None,
            member_of: (!actions.is_empty()).then_some(actions),
            annotations: annotation(&identifiers::MCP_ACTION, identifiers::ALL_ACTIONS),
            loc: None,
        };
        self.stub.0.insert(
            Some(server_ns),
            NamespaceDefinition {
                common_types: BTreeMap::new(),
                entity_types: BTreeMap::from([(identifiers::TARGET.clone(), target)]),
                actions: BTreeMap::from([(identifiers::ALL_ACTIONS.into(), all_actions)]),
                annotations: annotation(&identifiers::MCP_SERVER, server),
            },
        );
        Ok(self)
    }

    /// Get the current schema stub
    pub fn get_stub(&self) -> &Fragment<RawName> {
        &self.stub
    }

    /// Get the current schema stub as a human-readable `.cedarschema` string. Comments of the
    /// input stub are not kept.
    pub fn get_stub_as_str(&self) -> String {
        format!("{}", self.stub)
    }

    /// Get the current schema stub, e.g., to create a `SchemaGenerator`
    pub fn into_stub(self) -> Fragment<RawName> {
        self.stub
    }
}

/// The annotations `@{key}("{val}")`
fn annotation(key: &AnyId, val: &str) -> Annotations {
    Annotations(BTreeMap::from([(
        key.clone(),
        Some(Annotation {
            val: val.into(),
            loc: None,
        }),
    )]))
}

/// The namespace identifier of server `server` used by `StubBuilder::add_server`
fn namespace_identifier(server: &str) -> String {
    let id: String = server
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if id.starts_with(|c: char| c.is_ascii_digit()) {
        format!("_{id}")
    } else {
        id
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{SchemaGenerator, SchemaGeneratorConfig};
    use mcp_tools_sdk::description::ServerDescription;

    const MAIN: &str = r#"namespace Main {
    @mcp_principal("User") entity User;
    @mcp_resource_parent("Gateway") entity Gateway;
    @mcp_action("Mcp") action Mcp;
}"#;

    fn builder(stub: &str) -> StubBuilder {
        StubBuilder::from_cedarschema_str(stub).unwrap()
    }

    fn tool(name: &str) -> ServerDescription {
        ServerDescription::from_json_str(&format!(
            r#"{{"name": "{name}", "inputSchema": {{"type": "object"}}}}"#
        ))
        .unwrap()
    }

    #[test]
    fn adds_server_namespaces() {
        let builder = builder(MAIN)
            .add_server("weather-api")
            .unwrap()
            .add_server_with_namespace("billing", "Billing")
            .unwrap();
        let expected = r#"@mcp_server("billing")
namespace Main::Billing {
  @mcp_resource("Target")
  entity Target in [Main::Gateway];

  @mcp_action("AllActions")
  action "AllActions" in [Main::Action::"Mcp"];
}

namespace Main {
  @mcp_resource_parent("Gateway")
  entity Gateway;

  @mcp_principal("User")
  entity User;

  @mcp_action("Mcp")
  action "Mcp";
}

@mcp_server("weather-api")
namespace Main::weather_api {
  @mcp_resource("Target")
  entity Target in [Main::Gateway];

  @mcp_action("AllActions")
  action "AllActions" in [Main::Action::"Mcp"];
}
"#;
        assert_eq!(builder.get_stub_as_str(), expected);

        // The stub is a valid input of the schema generator
        let mut gen = SchemaGenerator::new(builder.into_stub()).unwrap();
        for server in ["weather-api", "billing"] {
            gen.add_actions_from_named_server_description(server, &tool("t"))
                .unwrap();
        }
        gen.new_request_generator().unwrap();
        // `@mcp_resource_parent` is erased like the other annotations
        assert!(gen.get_schema_as_str().contains("\n  entity Gateway;\n"));
    }

    #[test]
    fn parents_of_target_and_all_actions() {
        let main = r#"namespace Main {
    @mcp_principal("User") entity User;
    @mcp_resource_parent("G1") entity G1;
    @mcp_resource_parent("G2") entity G2;
    @mcp_action("A1") action A1;
    @mcp_action("A2") action A2;
}"#;
        let stub = builder(main).add_server("s").unwrap().get_stub_as_str();
        assert!(stub.contains("entity Target in [Main::G1, Main::G2];"));
        assert!(
            stub.contains(r#"action "AllActions" in [Main::Action::"A1", Main::Action::"A2"];"#)
        );

        let main = r#"namespace Main { @mcp_principal("User") entity User; }"#;
        let stub = builder(main).add_server("s").unwrap().get_stub_as_str();
        assert!(stub.contains("entity Target;"));
        assert!(stub.contains(r#"action "AllActions";"#));
    }

    #[test]
    fn derived_namespace_identifiers() {
        for (server, id) in [
            ("weather", "weather"),
            ("weather-api", "weather_api"),
            ("my.server v2", "my_server_v2"),
            ("1st", "_1st"),
            ("café", "caf_"),
        ] {
            assert_eq!(namespace_identifier(server), id);
        }
    }

    #[test]
    fn adds_to_existing_server_namespaces() {
        let stub = include_str!("../../examples/multi/stub.cedarschema");
        let builder = builder(stub).add_server("maps").unwrap();
        assert!(builder
            .get_stub()
            .0
            .contains_key(&Some("Main::maps".parse().unwrap())));
        assert!(SchemaGenerator::new(builder.into_stub()).is_ok());
    }

    #[test]
    fn errors() {
        let existing = include_str!("../../examples/multi/stub.cedarschema");
        let cases: [(&str, &[(&str, Option<&str>)], &str); 7] = [
            (
                MAIN,
                &[("", None)],
                "`` is not a valid namespace identifier for server ``.",
            ),
            (
                MAIN,
                &[("in", None)],
                "`in` is not a valid namespace identifier for server `in`.",
            ),
            (
                MAIN,
                &[("s", Some("A::B"))],
                "`A::B` is not a valid namespace identifier for server `s`.",
            ),
            (
                MAIN,
                &[("weather-api", None), ("weather_api", None)],
                "Namespace `Main::weather_api` already exists in the schema stub.",
            ),
            (
                MAIN,
                &[("s", None), ("s", Some("Other"))],
                "Server name `s` is used by more than one `@mcp_server` namespace.",
            ),
            (
                existing,
                &[("billing", Some("Billing2"))],
                "Server name `billing` is used by more than one `@mcp_server` namespace.",
            ),
            (
                existing,
                &[("billing2", Some("Billing"))],
                "Namespace `Main::Billing` already exists in the schema stub.",
            ),
        ];
        for (stub, servers, expected) in cases {
            let err = servers
                .iter()
                .try_fold(
                    builder(stub),
                    |builder, (server, namespace)| match namespace {
                        Some(namespace) => builder.add_server_with_namespace(server, namespace),
                        None => builder.add_server(server),
                    },
                )
                .unwrap_err();
            assert_eq!(err.to_string(), expected, "for servers {servers:?}");
        }

        for stub in [
            "entity Global;",
            "namespace A {} namespace B {}",
            r#"@mcp_server("s") namespace A {}"#,
        ] {
            assert!(StubBuilder::from_cedarschema_str(stub).is_err(), "{stub}");
        }
    }

    #[test]
    fn annotations_are_kept_if_not_erased() {
        let stub = builder(MAIN).add_server("s").unwrap().into_stub();
        let gen = SchemaGenerator::new_with_config(
            stub,
            SchemaGeneratorConfig::default().erase_annotations(false),
        )
        .unwrap();
        assert!(gen
            .get_schema_as_str()
            .contains("@mcp_resource_parent(\"Gateway\")"));
    }
}
