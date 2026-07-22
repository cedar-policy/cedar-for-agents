use crate::config::CedarAgentConfig;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EntityUid {
    #[serde(rename = "type")]
    pub entity_type: String,
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EntityJson {
    pub uid: EntityUid,
    pub attrs: BTreeMap<String, serde_json::Value>,
    pub parents: Vec<EntityUid>,
}

pub fn generate_entities(config: &CedarAgentConfig) -> Vec<EntityJson> {
    let mut entities = Vec::new();
    let ns = &config.namespace;

    if let Some(roles) = &config.roles {
        for role_name in roles.keys() {
            entities.push(EntityJson {
                uid: EntityUid {
                    entity_type: format!("{ns}::Role"),
                    id: role_name.clone(),
                },
                attrs: BTreeMap::new(),
                parents: Vec::new(),
            });
        }
    }

    if let Some(users) = &config.users {
        let principal_type = &config.principal.principal_type;
        for (user_id, roles) in users {
            entities.push(EntityJson {
                uid: EntityUid {
                    entity_type: format!("{ns}::{principal_type}"),
                    id: user_id.clone(),
                },
                attrs: BTreeMap::new(),
                parents: roles
                    .iter()
                    .map(|r| EntityUid {
                        entity_type: format!("{ns}::Role"),
                        id: r.clone(),
                    })
                    .collect(),
            });
        }
    }

    let resource = config.resource.as_ref();
    let resource_type = resource
        .map(|r| r.resource_type.as_str())
        .unwrap_or("Resource");
    let resource_id = resource.map(|r| r.id.as_str()).unwrap_or("default");

    entities.push(EntityJson {
        uid: EntityUid {
            entity_type: format!("{ns}::{resource_type}"),
            id: resource_id.to_string(),
        },
        attrs: BTreeMap::new(),
        parents: Vec::new(),
    });

    entities
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ResourceConfig;
    use std::collections::BTreeMap;

    #[test]
    fn test_entities_with_roles() {
        let config = CedarAgentConfig {
            roles: Some(BTreeMap::from([
                ("admin".to_string(), vec!["*".to_string()]),
                ("analyst".to_string(), vec!["search".to_string()]),
            ])),
            ..Default::default()
        };
        let entities = generate_entities(&config);
        assert_eq!(entities.len(), 3);

        let admin = entities.iter().find(|e| e.uid.id == "admin").unwrap();
        assert_eq!(admin.uid.entity_type, "Agent::Role");

        let analyst = entities.iter().find(|e| e.uid.id == "analyst").unwrap();
        assert_eq!(analyst.uid.entity_type, "Agent::Role");

        let resource = entities.iter().find(|e| e.uid.id == "default").unwrap();
        assert_eq!(resource.uid.entity_type, "Agent::Resource");
    }

    #[test]
    fn test_entities_default_resource() {
        let config = CedarAgentConfig::default();
        let entities = generate_entities(&config);
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].uid.entity_type, "Agent::Resource");
        assert_eq!(entities[0].uid.id, "default");
    }

    #[test]
    fn test_entities_custom_resource() {
        let config = CedarAgentConfig {
            resource: Some(ResourceConfig {
                resource_type: "ApiGateway".to_string(),
                id: "prod".to_string(),
            }),
            ..Default::default()
        };
        let entities = generate_entities(&config);
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].uid.entity_type, "Agent::ApiGateway");
        assert_eq!(entities[0].uid.id, "prod");
    }

    #[test]
    fn test_entities_with_users() {
        let config = CedarAgentConfig {
            roles: Some(BTreeMap::from([
                ("admin".to_string(), vec!["*".to_string()]),
                ("analyst".to_string(), vec!["search".to_string()]),
            ])),
            users: Some(BTreeMap::from([
                ("alice".to_string(), vec!["admin".to_string()]),
                ("bob".to_string(), vec!["analyst".to_string()]),
            ])),
            ..Default::default()
        };
        let entities = generate_entities(&config);
        // 2 roles + 2 users + 1 resource = 5
        assert_eq!(entities.len(), 5);

        let alice = entities.iter().find(|e| e.uid.id == "alice").unwrap();
        assert_eq!(alice.uid.entity_type, "Agent::User");
        assert_eq!(
            alice.parents,
            vec![EntityUid {
                entity_type: "Agent::Role".to_string(),
                id: "admin".to_string()
            }]
        );

        let bob = entities.iter().find(|e| e.uid.id == "bob").unwrap();
        assert_eq!(bob.uid.entity_type, "Agent::User");
        assert_eq!(
            bob.parents,
            vec![EntityUid {
                entity_type: "Agent::Role".to_string(),
                id: "analyst".to_string()
            }]
        );
    }

    #[test]
    fn test_entities_multi_role_user() {
        let config = CedarAgentConfig {
            roles: Some(BTreeMap::from([
                ("admin".to_string(), vec!["*".to_string()]),
                ("developer".to_string(), vec!["deploy".to_string()]),
            ])),
            users: Some(BTreeMap::from([(
                "charlie".to_string(),
                vec!["admin".to_string(), "developer".to_string()],
            )])),
            ..Default::default()
        };
        let entities = generate_entities(&config);
        let charlie = entities.iter().find(|e| e.uid.id == "charlie").unwrap();
        assert_eq!(charlie.parents.len(), 2);
        assert!(charlie.parents.contains(&EntityUid {
            entity_type: "Agent::Role".to_string(),
            id: "admin".to_string()
        }));
        assert!(charlie.parents.contains(&EntityUid {
            entity_type: "Agent::Role".to_string(),
            id: "developer".to_string()
        }));
    }
}
