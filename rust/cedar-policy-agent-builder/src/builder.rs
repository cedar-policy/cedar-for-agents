use crate::config::*;
use crate::BuildResult;
use std::collections::BTreeMap;

/// Errors returned by [`CedarAgentPolicyBuilder`] methods that validate input.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BuilderError {
    /// The time window has `hour_start >= hour_end`.
    #[error("time_window requires hour_start ({start}) < hour_end ({end})")]
    InvalidTimeWindow { start: u8, end: u8 },
    /// The `hour_end` value exceeds 24.
    #[error("hour_end ({0}) must be <= 24")]
    HourOutOfRange(u8),
    /// The given string is not a valid Cedar identifier (e.g. reserved word, invalid characters).
    #[error("\"{0}\" is not a valid Cedar identifier")]
    InvalidIdentifier(String),
    /// Two tool names produce the same rate-limit counter key after sanitization.
    #[error("rate-limit counter key collision: \"{new_tool}\" and \"{existing_tool}\" both map to counter key \"{key}\"")]
    RateLimitCounterCollision {
        new_tool: String,
        existing_tool: String,
        key: String,
    },
}

fn validate_cedar_ident(s: &str) -> Result<(), BuilderError> {
    cedar_policy::pst::Id::new(s)
        .map(|_| ())
        .map_err(|_| BuilderError::InvalidIdentifier(s.to_string()))
}

/// Fluent builder for generating Cedar policies, entities, and schemas for agent authorization.
///
/// # Example
///
/// ```rust
/// use cedar_policy_agent_builder::CedarAgentPolicyBuilder;
///
/// let result = CedarAgentPolicyBuilder::new()
///     .role("admin", &["*"])
///     .role("analyst", &["search", "query"])
///     .user("alice", &["admin"])
///     .user("bob", &["analyst"])
///     .rate_limit("search", 100).unwrap()
///     .time_window("*", (9, 17)).unwrap()
///     .build();
///
/// assert!(!result.policies.is_empty());
/// ```
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
    /// Create a new builder with default configuration (namespace `"Agent"`, principal type `"User"`).
    pub fn new() -> Self {
        Self {
            config: CedarAgentConfig::default(),
        }
    }

    /// Set the Cedar namespace for all generated entity types and actions.
    ///
    /// Defaults to `"Agent"`. Must be a valid Cedar identifier.
    pub fn namespace(mut self, ns: &str) -> Result<Self, BuilderError> {
        validate_cedar_ident(ns)?;
        self.config.namespace = ns.to_string();
        Ok(self)
    }

    /// Set the principal entity type and the key used to identify principals at runtime.
    ///
    /// `key` is the field name in the authorization context that holds the principal ID
    /// (e.g. `"sub"` for a JWT subject claim). `principal_type` is the Cedar entity type
    /// name (e.g. `"User"`). Must be a valid Cedar identifier.
    pub fn principal(mut self, key: &str, principal_type: &str) -> Result<Self, BuilderError> {
        validate_cedar_ident(principal_type)?;
        self.config.principal = PrincipalConfig {
            key: key.to_string(),
            principal_type: principal_type.to_string(),
        };
        Ok(self)
    }

    /// Set a custom resource entity type and ID.
    ///
    /// Defaults to `Resource::"default"`. The `resource_type` must be a valid Cedar identifier.
    pub fn resource(mut self, resource_type: &str, id: &str) -> Result<Self, BuilderError> {
        validate_cedar_ident(resource_type)?;
        self.config.resource = Some(ResourceConfig {
            resource_type: resource_type.to_string(),
            id: id.to_string(),
        });
        Ok(self)
    }

    /// Define a role with access to the specified tools.
    ///
    /// Use `"*"` as a tool name to grant access to all actions.
    /// Multiple calls add additional roles.
    pub fn role(mut self, name: &str, tools: &[&str]) -> Self {
        self.config.roles.get_or_insert_with(BTreeMap::new).insert(
            name.to_string(),
            tools.iter().map(|t| (*t).to_string()).collect(),
        );
        self
    }

    /// Add a user with membership in the specified roles.
    ///
    /// The user entity will be created with parent edges to each role entity,
    /// enabling Cedar's `principal in Role::"name"` pattern.
    pub fn user(mut self, id: &str, roles: &[&str]) -> Self {
        self.config.users.get_or_insert_with(BTreeMap::new).insert(
            id.to_string(),
            roles.iter().map(|r| (*r).to_string()).collect(),
        );
        self
    }

    /// Restrict a tool's input fields to specific allowed values.
    ///
    /// Generates a `forbid` policy that denies the action unless the input field
    /// matches one of the allowed values. The map keys are field names from
    /// `context.input`, and the values are the permitted values for each field.
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

    /// Add a rate limit for a tool (or `"*"` for all tools).
    ///
    /// Generates a `forbid` policy that denies when the session's call counter
    /// for this tool reaches `max`. The counter is expected in `context.session`
    /// under the key `call_count_<sanitized_tool>`, where non-alphanumeric characters
    /// are replaced with `_`.
    ///
    /// Returns an error if the sanitized counter key collides with a previously
    /// registered rate-limit tool (e.g. `"my-tool"` and `"my.tool"` both map to
    /// `call_count_my_tool`).
    pub fn rate_limit(mut self, tool: &str, max: u64) -> Result<Self, BuilderError> {
        if tool == "*" {
            self.config
                .rate_limits
                .get_or_insert_with(BTreeMap::new)
                .insert(tool.to_string(), max);
            return Ok(self);
        }

        let key = crate::policy::sanitize_counter_key(tool);
        let map = self.config.rate_limits.get_or_insert_with(BTreeMap::new);
        // Check for collision with an existing tool name that maps to the same key.
        for existing_tool in map.keys() {
            if existing_tool == tool || existing_tool == "*" {
                continue;
            }
            if crate::policy::sanitize_counter_key(existing_tool) == key {
                return Err(BuilderError::RateLimitCounterCollision {
                    new_tool: tool.to_string(),
                    existing_tool: existing_tool.clone(),
                    key: format!("call_count_{key}"),
                });
            }
        }
        map.insert(tool.to_string(), max);
        Ok(self)
    }

    /// Restrict a tool (or `"*"` for all tools) to a UTC hour window.
    ///
    /// `hours` is `(start, end)` where `start < end` and `end <= 24`.
    /// Actions are denied when `context.session.hour_utc` is outside this range.
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

    /// Deny specific tools (or `"*"` for all) in a named environment.
    ///
    /// Generates a `forbid` policy conditioned on `context.session.environment == env`.
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

    /// Require explicit user consent before allowing a tool for any role.
    ///
    /// The tool is excluded from role `permit` policies and instead gated behind
    /// a `context.session.user_consent == true` condition, for each role that
    /// grants access to the tool.
    pub fn consent_all(mut self, tool: &str) -> Self {
        self.config
            .consent
            .get_or_insert_with(BTreeMap::new)
            .insert(tool.to_string(), ConsentScope::AllRoles(true));
        self
    }

    /// Require user consent for a tool, but only for specific roles.
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

    /// Register an MCP tool definition for Cedar schema generation.
    ///
    /// When tool definitions are provided, [`BuildResult::schema`] will contain a
    /// Cedar schema with actions derived from the tool's input/output schemas.
    pub fn tool(mut self, definition: McpToolDefinition) -> Self {
        self.config
            .tools
            .get_or_insert_with(Vec::new)
            .push(definition);
        self
    }

    /// Consume the builder and generate Cedar policies, entities, and schema.
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
            .unwrap()
            .role("viewer", &["read"])
            .build();

        assert!(result.policies.contains("MyApp::Role::\"viewer\""));
        assert!(result.policies.contains("MyApp::Action::\"read\""));
    }

    #[test]
    fn test_builder_chaining() {
        let result = CedarAgentPolicyBuilder::new()
            .namespace("Agent")
            .unwrap()
            .principal("sub", "User")
            .unwrap()
            .role("admin", &["*"])
            .role("analyst", &["search"])
            .user("alice", &["admin"])
            .user("bob", &["analyst"])
            .rate_limit("send_email", 5)
            .unwrap()
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
            .unwrap()
            .principal("sub", "User")
            .unwrap()
            .role("admin", &["*"])
            .role("analyst", &["search", "query"])
            .user("alice", &["admin"])
            .user("bob", &["analyst"])
            .rate_limit("send_email", 5)
            .unwrap()
            .rate_limit("*", 100)
            .unwrap()
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
            .unwrap()
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

    #[test]
    fn test_namespace_rejects_reserved_word() {
        let err = CedarAgentPolicyBuilder::new().namespace("if").unwrap_err();
        assert!(matches!(err, BuilderError::InvalidIdentifier(_)));
    }

    #[test]
    fn test_namespace_rejects_invalid_chars() {
        let err = CedarAgentPolicyBuilder::new()
            .namespace("my-ns")
            .unwrap_err();
        assert!(matches!(err, BuilderError::InvalidIdentifier(_)));
    }

    #[test]
    fn test_principal_type_rejects_reserved_word() {
        let err = CedarAgentPolicyBuilder::new()
            .principal("sub", "if")
            .unwrap_err();
        assert!(matches!(err, BuilderError::InvalidIdentifier(_)));
    }

    #[test]
    fn test_resource_type_rejects_invalid() {
        let err = CedarAgentPolicyBuilder::new()
            .resource("123bad", "id")
            .unwrap_err();
        assert!(matches!(err, BuilderError::InvalidIdentifier(_)));
    }

    #[test]
    fn test_rate_limit_rejects_counter_key_collision() {
        // "my-tool" and "my.tool" both sanitize to "my_tool" → collision
        let err = CedarAgentPolicyBuilder::new()
            .rate_limit("my-tool", 5)
            .unwrap()
            .rate_limit("my.tool", 10)
            .unwrap_err();
        assert!(
            matches!(err, BuilderError::RateLimitCounterCollision { .. }),
            "expected RateLimitCounterCollision, got: {err:?}"
        );
    }

    #[test]
    fn test_rate_limit_allows_distinct_counter_keys() {
        // "search" and "deploy" produce different counter keys — no collision
        let result = CedarAgentPolicyBuilder::new()
            .rate_limit("search", 5)
            .unwrap()
            .rate_limit("deploy", 10)
            .unwrap()
            .build();
        assert!(result.policies.contains("call_count_search >= 5"));
        assert!(result.policies.contains("call_count_deploy >= 10"));
    }

    #[test]
    fn test_rate_limit_same_tool_overwrites_without_error() {
        // Setting the same tool twice should just overwrite, not error
        let result = CedarAgentPolicyBuilder::new()
            .rate_limit("search", 5)
            .unwrap()
            .rate_limit("search", 10)
            .unwrap()
            .build();
        assert!(result.policies.contains("call_count_search >= 10"));
        assert!(!result.policies.contains("call_count_search >= 5"));
    }

    #[test]
    fn test_rate_limit_wildcard_does_not_collide() {
        // "*" is the global counter and should not collide with tool-specific entries
        let result = CedarAgentPolicyBuilder::new()
            .rate_limit("*", 100)
            .unwrap()
            .rate_limit("search", 5)
            .unwrap()
            .build();
        assert!(result.policies.contains("call_count >= 100"));
        assert!(result.policies.contains("call_count_search >= 5"));
    }
}
