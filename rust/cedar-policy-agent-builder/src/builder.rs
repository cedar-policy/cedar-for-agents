use crate::config::*;
use crate::BuildResult;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BuilderError {
    #[error("time_window requires hour_start ({start}) < hour_end ({end})")]
    InvalidTimeWindow { start: u8, end: u8 },
    #[error("hour_end ({0}) must be <= 24")]
    HourOutOfRange(u8),
}

#[derive(Debug, Clone)]
#[must_use]
pub struct CedarAgentPolicyBuilder {
    config: CedarAgentConfig,
}

impl Default for CedarAgentPolicyBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl CedarAgentPolicyBuilder {
    pub fn new() -> Self {
        Self {
            config: CedarAgentConfig::default(),
        }
    }

    pub fn namespace(mut self, ns: &str) -> Self {
        self.config.namespace = ns.to_string();
        self
    }

    pub fn principal(mut self, key: &str, principal_type: &str) -> Self {
        self.config.principal = PrincipalConfig {
            key: key.to_string(),
            principal_type: principal_type.to_string(),
        };
        self
    }

    pub fn resource(mut self, resource_type: &str, id: &str) -> Self {
        self.config.resource = Some(ResourceConfig {
            resource_type: resource_type.to_string(),
            id: id.to_string(),
        });
        self
    }

    pub fn role(mut self, name: &str, tools: &[&str]) -> Self {
        self.config.roles.get_or_insert_with(BTreeMap::new).insert(
            name.to_string(),
            tools.iter().map(|t| (*t).to_string()).collect(),
        );
        self
    }

    pub fn user(mut self, id: &str, roles: &[&str]) -> Self {
        self.config.users.get_or_insert_with(BTreeMap::new).insert(
            id.to_string(),
            roles.iter().map(|r| (*r).to_string()).collect(),
        );
        self
    }

    pub fn restrict(
        mut self,
        tool: &str,
        allowed_values: BTreeMap<String, Vec<serde_json::Value>>,
    ) -> Self {
        self.config
            .restrictions
            .get_or_insert_with(BTreeMap::new)
            .insert(tool.to_string(), Restriction { allowed_values });
        self
    }

    pub fn rate_limit(mut self, tool: &str, max: u64) -> Self {
        self.config
            .rate_limits
            .get_or_insert_with(BTreeMap::new)
            .insert(tool.to_string(), max);
        self
    }

    pub fn time_window(mut self, tool: &str, hours: (u8, u8)) -> Result<Self, BuilderError> {
        if hours.0 >= hours.1 {
            return Err(BuilderError::InvalidTimeWindow {
                start: hours.0,
                end: hours.1,
            });
        }
        if hours.1 > 24 {
            return Err(BuilderError::HourOutOfRange(hours.1));
        }
        self.config
            .time_windows
            .get_or_insert_with(BTreeMap::new)
            .insert(
                tool.to_string(),
                TimeWindow {
                    hour_start: hours.0,
                    hour_end: hours.1,
                },
            );
        Ok(self)
    }

    pub fn deny_in_env(mut self, env: &str, tools: &[&str]) -> Self {
        self.config
            .deny_in_env
            .get_or_insert_with(BTreeMap::new)
            .insert(
                env.to_string(),
                tools.iter().map(|t| (*t).to_string()).collect(),
            );
        self
    }

    pub fn consent_all(mut self, tool: &str) -> Self {
        self.config
            .consent
            .get_or_insert_with(BTreeMap::new)
            .insert(tool.to_string(), ConsentScope::AllRoles(true));
        self
    }

    pub fn consent_for_roles(mut self, tool: &str, roles: &[&str]) -> Self {
        self.config
            .consent
            .get_or_insert_with(BTreeMap::new)
            .insert(
                tool.to_string(),
                ConsentScope::SpecificRoles(roles.iter().map(|r| (*r).to_string()).collect()),
            );
        self
    }

    pub fn tool(mut self, definition: McpToolDefinition) -> Self {
        self.config
            .tools
            .get_or_insert_with(Vec::new)
            .push(definition);
        self
    }

    pub fn build(self) -> BuildResult {
        crate::build(&self.config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cedar_policy::PolicySet;

    fn assert_policies_parse(result: &crate::BuildResult) {
        if !result.policies.is_empty() {
            result.policies.parse::<PolicySet>().unwrap_or_else(|e| {
                panic!(
                    "generated policies failed to parse:\n{}\nerror: {e}",
                    result.policies
                )
            });
        }
    }

    #[test]
    fn test_builder_basic() {
        let result = CedarAgentPolicyBuilder::new()
            .role("admin", &["*"])
            .user("alice", &["admin"])
            .build();

        assert!(result
            .policies
            .contains("principal in Agent::Role::\"admin\""));
        assert!(result.entities.iter().any(|e| e.uid.id == "alice"));
        assert!(result.entities.iter().any(|e| e.uid.id == "admin"));
    }

    #[test]
    fn test_builder_with_namespace() {
        let result = CedarAgentPolicyBuilder::new()
            .namespace("MyApp")
            .role("viewer", &["read"])
            .build();

        assert!(result.policies.contains("MyApp::Role::\"viewer\""));
        assert!(result.policies.contains("MyApp::Action::\"read\""));
    }

    #[test]
    fn test_builder_chaining() {
        let result = CedarAgentPolicyBuilder::new()
            .namespace("Agent")
            .principal("sub", "User")
            .role("admin", &["*"])
            .role("analyst", &["search"])
            .user("alice", &["admin"])
            .user("bob", &["analyst"])
            .rate_limit("send_email", 5)
            .time_window("*", (9, 17))
            .unwrap()
            .consent_all("send_email")
            .deny_in_env("production", &["delete"])
            .build();

        assert!(result.policies.contains("Role::\"admin\""));
        assert!(result.policies.contains("Role::\"analyst\""));
        assert!(result.policies.contains("call_count_send_email >= 5"));
        assert!(result.policies.contains("hour_utc < 9"));
        assert!(result.policies.contains("user_consent"));
        assert!(result.policies.contains("\"production\""));
    }

    #[test]
    fn test_builder_default() {
        let builder = CedarAgentPolicyBuilder::default();
        let result = builder.build();
        assert!(result.policies.is_empty());
        assert_eq!(result.entities.len(), 1); // just the default resource
    }

    #[test]
    fn test_all_policy_types_parse_as_valid_cedar() {
        let result = CedarAgentPolicyBuilder::new()
            .namespace("Agent")
            .principal("sub", "User")
            .role("admin", &["*"])
            .role("analyst", &["search", "query"])
            .user("alice", &["admin"])
            .user("bob", &["analyst"])
            .rate_limit("send_email", 5)
            .rate_limit("*", 100)
            .time_window("*", (9, 17))
            .unwrap()
            .consent_all("send_email")
            .consent_for_roles("deploy", &["admin"])
            .deny_in_env("production", &["delete"])
            .restrict(
                "query",
                BTreeMap::from([("db".to_string(), vec![serde_json::json!("analytics")])]),
            )
            .build();

        assert_policies_parse(&result);
    }

    #[test]
    fn test_basic_build_parses() {
        let result = CedarAgentPolicyBuilder::new()
            .role("admin", &["*"])
            .user("alice", &["admin"])
            .build();

        assert_policies_parse(&result);
    }

    #[test]
    fn test_time_window_rejects_inverted() {
        let err = CedarAgentPolicyBuilder::new()
            .time_window("*", (17, 9))
            .unwrap_err();
        assert!(matches!(err, BuilderError::InvalidTimeWindow { .. }));
    }

    #[test]
    fn test_time_window_rejects_over_24() {
        let err = CedarAgentPolicyBuilder::new()
            .time_window("*", (9, 25))
            .unwrap_err();
        assert!(matches!(err, BuilderError::HourOutOfRange(25)));
    }

    #[test]
    fn test_builder_with_tool_definitions() {
        let result = CedarAgentPolicyBuilder::new()
            .role("admin", &["*"])
            .tool(McpToolDefinition {
                name: "search".to_string(),
                description: Some("Search things".to_string()),
                input_schema: serde_json::json!({"type": "object", "properties": {"q": {"type": "string"}}}),
                output_schema: None,
            })
            .build();

        assert!(result.schema.is_some());
        assert!(result.schema_errors.is_empty());
    }

    #[test]
    fn test_builder_resource_custom() {
        let result = CedarAgentPolicyBuilder::new()
            .resource("Gateway", "prod")
            .role("admin", &["*"])
            .build();

        let resource = result.entities.iter().find(|e| e.uid.id == "prod").unwrap();
        assert_eq!(resource.uid.entity_type, "Agent::Gateway");
    }

    #[test]
    fn test_builder_consent_for_roles_in_chain() {
        let result = CedarAgentPolicyBuilder::new()
            .role("admin", &["*"])
            .role("analyst", &["search", "deploy"])
            .consent_for_roles("deploy", &["admin"])
            .build();

        assert!(result.policies.contains("user_consent"));
        assert!(result.policies.contains("Role::\"admin\""));
    }

    #[test]
    fn test_builder_time_window_per_tool() {
        let result = CedarAgentPolicyBuilder::new()
            .role("admin", &["*"])
            .time_window("deploy", (9, 17))
            .unwrap()
            .build();

        assert!(result.policies.contains("Action::\"deploy\""));
        assert!(result.policies.contains("hour_utc"));
    }
}
