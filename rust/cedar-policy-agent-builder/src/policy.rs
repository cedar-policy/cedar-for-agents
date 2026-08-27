use crate::config::{CedarAgentConfig, ConsentScope};
use cedar_policy::EntityId;
use std::collections::BTreeSet;

fn action_ref(ns: &str, name: &str) -> String {
    let eid = EntityId::new(name);
    format!("action == {ns}::Action::\"{}\"", eid.escaped())
}

// Tools like "my-tool" and "my_tool" will collide to the same counter key.
pub(crate) fn sanitize_counter_key(tool: &str) -> String {
    tool.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

fn normalize_consent_roles(scope: &ConsentScope) -> Vec<&str> {
    match scope {
        ConsentScope::AllRoles(true) => vec!["*"],
        ConsentScope::AllRoles(false) => vec![],
        ConsentScope::SpecificRoles(roles) => roles.iter().map(|s| s.as_str()).collect(),
    }
}

/// Get the roles that grant access to the given `tool`.
/// A role grants access to a tool if either it grants access to any tool via `"*"` or it
/// grants access to that specific tool.
fn get_roles_granting_tool<'a>(config: &'a CedarAgentConfig, tool: &str) -> Vec<&'a str> {
    let roles = match &config.roles {
        Some(r) => r,
        None => return Vec::new(),
    };
    roles
        .iter()
        .filter(|(_, tools)| tools.iter().any(|t| t == "*" || t == tool))
        .map(|(role_name, _)| role_name.as_str())
        .collect()
}

fn get_consent_tools_for_role(config: &CedarAgentConfig, role_name: &str) -> BTreeSet<String> {
    let mut tools = BTreeSet::new();
    if let Some(consent) = &config.consent {
        for (tool, scope) in consent {
            let roles = normalize_consent_roles(scope);
            if roles.contains(&"*") || roles.contains(&role_name) {
                tools.insert(tool.clone());
            }
        }
    }
    tools
}

/// A [`PolicyGenerator`] generates the policy text from a `config`, keeping track of additional
/// state as it generates those policies.
// We may want to use this to store additional state when doing policy generation.
#[derive(Debug)]
pub struct PolicyGenerator<'a> {
    config: &'a CedarAgentConfig,
    policies_text: Vec<String>,
}

impl<'a> PolicyGenerator<'a> {
    fn generate_role_policies(&mut self) {
        let roles = match &self.config.roles {
            Some(r) => r,
            None => return,
        };
        let ns = &self.config.namespace;
        let mut policies = Vec::new();

        for (role_name, tools) in roles {
            let consent_tools = get_consent_tools_for_role(self.config, role_name);
            let role_ref = format!("{ns}::Role::\"{}\"", EntityId::new(role_name).escaped());

            if tools.contains(&"*".to_string()) {
                if consent_tools.is_empty() {
                    policies.push(format!(
                        "permit(principal in {role_ref}, action, resource);"
                    ));
                } else {
                    let exclusions: Vec<String> =
                        consent_tools.iter().map(|t| action_ref(ns, t)).collect();
                    policies.push(format!(
                    "permit(\n  principal in {role_ref},\n  action,\n  resource\n) when {{ !({}) }};",
                    exclusions.join(" || ")
                ));
                }
            } else {
                let filtered: Vec<&String> = tools
                    .iter()
                    .filter(|t| !consent_tools.contains(t.as_str()))
                    .collect();
                for tool in filtered {
                    policies.push(format!(
                        "permit(principal in {role_ref}, {}, resource);",
                        action_ref(ns, tool)
                    ));
                }
            }
        }

        self.policies_text.append(&mut policies);
    }

    fn generate_restriction_policies(&mut self) {
        let restrictions = match &self.config.restrictions {
            Some(r) => r,
            None => return,
        };
        let ns = &self.config.namespace;
        let mut policies = Vec::new();

        for (tool, restriction) in restrictions {
            let action_clause = action_ref(ns, tool);
            if restriction.allowed_values.is_empty() {
                policies.push(format!(
                    "forbid(\n  principal,\n  {action_clause},\n  resource\n);"
                ));
                continue;
            }
            for (field, allowed_values) in &restriction.allowed_values {
                let value_checks: Vec<String> = allowed_values
                    .iter()
                    .map(|v| {
                        let formatted = format_cedar_value(v);
                        format!(
                            "context.input[\"{}\"] == {formatted}",
                            EntityId::new(field).escaped()
                        )
                    })
                    .collect();
                policies.push(format!(
                "forbid(\n  principal,\n  {action_clause},\n  resource\n) when {{\n  !(context.input has \"{}\" && ({}))\n}};",
                EntityId::new(field).escaped(),
                value_checks.join(" || ")
            ));
            }
        }

        self.policies_text.append(&mut policies);
    }

    fn generate_rate_limit_policies(&mut self) {
        let rate_limits = match &self.config.rate_limits {
            Some(r) => r,
            None => return,
        };
        let ns = &self.config.namespace;
        let mut policies = Vec::new();

        for (tool, max) in rate_limits {
            if tool == "*" {
                // forbid if call count not provided, or call count is too large
                policies.push(format!(
                    "forbid(\n  principal,\n  action,\n  resource\n) when {{ !(context.session has \"call_count\") || context.session.call_count >= {max} }};",
                ));
            } else {
                let action_clause = action_ref(ns, tool);
                let counter_key = format!("call_count_{}", sanitize_counter_key(tool));
                // forbid if call count not provided for this tool, or call count is too large
                policies.push(format!(
                "forbid(\n  principal,\n  {action_clause},\n  resource\n) when {{ !(context.session has \"{}\") || context.session.{} >= {max} }};",
                counter_key,
                counter_key
                ));
            }
        }

        self.policies_text.append(&mut policies);
    }

    fn generate_time_window_policies(&mut self) {
        let time_windows = match &self.config.time_windows {
            Some(t) => t,
            None => return,
        };
        let ns = &self.config.namespace;
        let mut policies = Vec::new();

        for (tool, tw) in time_windows {
            let action_clause = if tool == "*" {
                "action".to_string()
            } else {
                action_ref(ns, tool)
            };
            // forbid if hour_utc not provided, or outside of specified window
            policies.push(format!(
                "forbid(\n  principal,\n  {action_clause},\n  resource\n) when {{ !(context.session has \"hour_utc\") || (context.session.hour_utc < {} || context.session.hour_utc >= {}) }};",
                tw.hour_start, tw.hour_end));
        }
        self.policies_text.append(&mut policies);
    }

    fn generate_env_denial_policies(&mut self) {
        let deny_in_env = match &self.config.deny_in_env {
            Some(d) => d,
            None => return,
        };
        let ns = &self.config.namespace;
        let mut policies = Vec::new();

        for (env, tools) in deny_in_env {
            if tools.contains(&"*".to_string()) {
                // forbid if environment not provided, or environment is the denied one
                policies.push(format!(
                "forbid(\n  principal,\n  action,\n  resource\n) when {{ !(context.session has \"environment\") || context.session.environment == \"{}\" }};",
                EntityId::new(env).escaped()
            ));
            } else {
                for tool in tools {
                    // forbid if environment not provided for this action, or environment is the denied one
                    policies.push(format!(
                    "forbid(\n  principal,\n  {},\n  resource\n) when {{ !(context.session has \"environment\") || context.session.environment == \"{}\" }};",
                    action_ref(ns, tool),
                    EntityId::new(env).escaped()
                ));
                }
            }

            self.policies_text.append(&mut policies);
        }
    }

    fn generate_consent_policies(&mut self) {
        let consent = match &self.config.consent {
            Some(c) => c,
            None => return,
        };
        let ns = &self.config.namespace;
        let mut policies = Vec::new();

        for (tool, scope) in consent {
            let action_clause = action_ref(ns, tool);
            let roles = normalize_consent_roles(scope);
            let applicable_roles = get_roles_granting_tool(self.config, tool);
            if roles.contains(&"*") {
                // Collect all roles that actually grant access to this tool
                // (either explicitly or via wildcard "*").
                for role in &applicable_roles {
                    let role_ref = format!("{ns}::Role::\"{}\"", EntityId::new(role).escaped());
                    policies.push(format!(
                    "permit(\n  principal in {role_ref},\n  {action_clause},\n  resource\n) when {{ context.session has user_consent && context.session.user_consent }};",
                ));
                }
            } else {
                for role in &roles {
                    if !applicable_roles.contains(role) {
                        continue;
                    }
                    let role_ref = format!("{ns}::Role::\"{}\"", EntityId::new(role).escaped());
                    policies.push(format!(
                    "permit(\n  principal in {role_ref},\n  {action_clause},\n  resource\n) when {{ context.session has user_consent && context.session.user_consent }};",
                ));
                }
            }
        }
        self.policies_text.append(&mut policies);
    }

    pub fn generate_policies(config: &CedarAgentConfig) -> String {
        let mut generator = PolicyGenerator {
            config,
            policies_text: Vec::new(),
        };
        generator.generate_role_policies();
        generator.generate_restriction_policies();
        generator.generate_rate_limit_policies();
        generator.generate_time_window_policies();
        generator.generate_env_denial_policies();
        generator.generate_consent_policies();

        generator.policies_text.join("\n\n")
    }
}

fn format_cedar_value(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => format!("\"{}\"", EntityId::new(s).escaped()),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        _ => format!("\"{}\"", EntityId::new(v.to_string()).escaped()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{PrincipalConfig, Restriction};
    use cedar_policy::PolicySet;
    use std::collections::BTreeMap;

    fn assert_valid_cedar(policies: &str) {
        if !policies.is_empty() {
            policies.parse::<PolicySet>().unwrap_or_else(|e| {
                panic!("generated policies are not valid Cedar:\n{policies}\nerror: {e}")
            });
        }
    }

    #[test]
    fn test_role_specific_tools() {
        let config = CedarAgentConfig {
            principal: PrincipalConfig {
                key: "user_id".to_string(),
                principal_type: "User".to_string(),
            },
            roles: Some(BTreeMap::from([(
                "analyst".to_string(),
                vec!["search".to_string(), "query_database".to_string()],
            )])),
            ..Default::default()
        };
        let policies = PolicyGenerator::generate_policies(&config);
        assert_valid_cedar(&policies);
        assert!(policies.contains(
            "permit(principal in Agent::Role::\"analyst\", action == Agent::Action::\"search\", resource);"
        ));
        assert!(policies.contains(
            "permit(principal in Agent::Role::\"analyst\", action == Agent::Action::\"query_database\", resource);"
        ));
    }

    #[test]
    fn test_wildcard_role() {
        let config = CedarAgentConfig {
            roles: Some(BTreeMap::from([(
                "admin".to_string(),
                vec!["*".to_string()],
            )])),
            ..Default::default()
        };
        let policies = PolicyGenerator::generate_policies(&config);
        assert_valid_cedar(&policies);
        assert!(policies.contains("permit(principal in Agent::Role::\"admin\", action, resource);"));
    }

    #[test]
    fn test_restriction_policy() {
        let config = CedarAgentConfig {
            restrictions: Some(BTreeMap::from([(
                "query_database".to_string(),
                Restriction {
                    allowed_values: BTreeMap::from([(
                        "database".to_string(),
                        vec![
                            serde_json::Value::String("analytics".to_string()),
                            serde_json::Value::String("reporting".to_string()),
                        ],
                    )]),
                },
            )])),
            ..Default::default()
        };
        let policies = PolicyGenerator::generate_policies(&config);
        assert_valid_cedar(&policies);
        assert!(policies.contains(
            "forbid(\n  principal,\n  action == Agent::Action::\"query_database\",\n  resource\n) when {\n  !(context.input has \"database\" && (context.input[\"database\"] == \"analytics\" || context.input[\"database\"] == \"reporting\"))\n};"
        ));
    }

    #[test]
    fn test_rate_limit_per_tool() {
        let config = CedarAgentConfig {
            rate_limits: Some(BTreeMap::from([("send_email".to_string(), 3)])),
            ..Default::default()
        };
        let policies = PolicyGenerator::generate_policies(&config);
        assert_valid_cedar(&policies);
        assert!(policies.contains(
            "forbid(\n  principal,\n  action == Agent::Action::\"send_email\",\n  resource\n) when { !(context.session has \"call_count_send_email\") || context.session.call_count_send_email >= 3 };"
        ));
    }

    #[test]
    fn test_rate_limit_global() {
        let config = CedarAgentConfig {
            rate_limits: Some(BTreeMap::from([("*".to_string(), 100)])),
            ..Default::default()
        };
        let policies = PolicyGenerator::generate_policies(&config);
        assert_valid_cedar(&policies);
        assert!(policies.contains(
            "forbid(\n  principal,\n  action,\n  resource\n) when { !(context.session has \"call_count\") || context.session.call_count >= 100 };"
        ));
    }

    #[test]
    fn test_time_window_global() {
        let config = CedarAgentConfig {
            time_windows: Some(BTreeMap::from([(
                "*".to_string(),
                crate::config::TimeWindow {
                    hour_start: 9,
                    hour_end: 17,
                },
            )])),
            ..Default::default()
        };
        let policies = PolicyGenerator::generate_policies(&config);
        assert_valid_cedar(&policies);
        assert!(policies.contains(
            "forbid(\n  principal,\n  action,\n  resource\n) when { !(context.session has \"hour_utc\") || (context.session.hour_utc < 9 || context.session.hour_utc >= 17) };"
        ));
    }

    #[test]
    fn test_time_window_tool_scoped() {
        let config = CedarAgentConfig {
            time_windows: Some(BTreeMap::from([(
                "deploy".to_string(),
                crate::config::TimeWindow {
                    hour_start: 9,
                    hour_end: 17,
                },
            )])),
            ..Default::default()
        };
        let policies = PolicyGenerator::generate_policies(&config);
        assert_valid_cedar(&policies);
        assert!(policies.contains(
            "forbid(\n  principal,\n  action == Agent::Action::\"deploy\",\n  resource\n) when { !(context.session has \"hour_utc\") || (context.session.hour_utc < 9 || context.session.hour_utc >= 17) };"
        ));
    }

    #[test]
    fn test_env_denial_policy() {
        let config = CedarAgentConfig {
            deny_in_env: Some(BTreeMap::from([(
                "production".to_string(),
                vec!["delete_record".to_string()],
            )])),
            ..Default::default()
        };
        let policies = PolicyGenerator::generate_policies(&config);
        assert_valid_cedar(&policies);
        assert!(policies.contains(
            "forbid(\n  principal,\n  action == Agent::Action::\"delete_record\",\n  resource\n) when { !(context.session has \"environment\") || context.session.environment == \"production\" };"
        ));
    }

    #[test]
    fn test_consent_all_roles() {
        let config = CedarAgentConfig {
            roles: Some(BTreeMap::from([
                (
                    "analyst".to_string(),
                    vec!["search".to_string(), "send_email".to_string()],
                ),
                ("admin".to_string(), vec!["*".to_string()]),
            ])),
            consent: Some(BTreeMap::from([(
                "send_email".to_string(),
                ConsentScope::AllRoles(true),
            )])),
            ..Default::default()
        };
        let policies = PolicyGenerator::generate_policies(&config);
        assert_valid_cedar(&policies);
        // The consent permit must be scoped to roles that grant the tool
        assert!(policies.contains(
            "permit(\n  principal in Agent::Role::\"analyst\",\n  action == Agent::Action::\"send_email\",\n  resource\n) when { context.session has user_consent && context.session.user_consent };"
        ));
        assert!(policies.contains(
            "permit(\n  principal in Agent::Role::\"admin\",\n  action == Agent::Action::\"send_email\",\n  resource\n) when { context.session has user_consent && context.session.user_consent };"
        ));
        // Must NOT have an unconstrained principal permit
        assert!(!policies.contains(
            "permit(\n  principal,\n  action == Agent::Action::\"send_email\",\n  resource\n)"
        ));
    }

    #[test]
    fn test_consent_specific_role() {
        let config = CedarAgentConfig {
            roles: Some(BTreeMap::from([(
                "analyst".to_string(),
                vec!["send_email".to_string()],
            )])),
            consent: Some(BTreeMap::from([(
                "send_email".to_string(),
                ConsentScope::SpecificRoles(vec!["analyst".to_string()]),
            )])),
            ..Default::default()
        };
        let policies = PolicyGenerator::generate_policies(&config);
        assert_valid_cedar(&policies);
        assert!(policies.contains(
            "permit(\n  principal in Agent::Role::\"analyst\",\n  action == Agent::Action::\"send_email\",\n  resource\n) when { context.session has user_consent && context.session.user_consent };"
        ));
    }

    #[test]
    fn test_consent_excludes_from_role_permit() {
        let config = CedarAgentConfig {
            roles: Some(BTreeMap::from([(
                "analyst".to_string(),
                vec!["search".to_string(), "send_email".to_string()],
            )])),
            consent: Some(BTreeMap::from([(
                "send_email".to_string(),
                ConsentScope::AllRoles(true),
            )])),
            ..Default::default()
        };
        let policies = PolicyGenerator::generate_policies(&config);
        assert_valid_cedar(&policies);
        // send_email should NOT appear in the unconditional role permit
        assert!(!policies.contains(
            "principal in Agent::Role::\"analyst\", action == Agent::Action::\"send_email\", resource);"
        ));
        // search should still be there
        assert!(policies.contains(
            "permit(principal in Agent::Role::\"analyst\", action == Agent::Action::\"search\", resource);"
        ));
        // consent permit for send_email should be scoped to analyst
        assert!(policies.contains(
            "permit(\n  principal in Agent::Role::\"analyst\",\n  action == Agent::Action::\"send_email\",\n  resource\n) when { context.session has user_consent && context.session.user_consent };"
        ));
    }

    #[test]
    fn test_wildcard_with_consent_exclusion() {
        let config = CedarAgentConfig {
            roles: Some(BTreeMap::from([(
                "admin".to_string(),
                vec!["*".to_string()],
            )])),
            consent: Some(BTreeMap::from([(
                "send_email".to_string(),
                ConsentScope::AllRoles(true),
            )])),
            ..Default::default()
        };
        let policies = PolicyGenerator::generate_policies(&config);
        assert_valid_cedar(&policies);
        assert!(policies.contains("!(action == Agent::Action::\"send_email\")"));
        // Consent permit must be scoped to admin role (which grants * tools)
        assert!(policies.contains(
            "permit(\n  principal in Agent::Role::\"admin\",\n  action == Agent::Action::\"send_email\",\n  resource\n) when { context.session has user_consent && context.session.user_consent };"
        ));
    }

    #[test]
    fn test_empty_config_produces_no_policies() {
        let config = CedarAgentConfig::default();
        let policies = PolicyGenerator::generate_policies(&config);
        assert!(policies.is_empty());
    }

    #[test]
    fn test_empty_role_tools() {
        let config = CedarAgentConfig {
            roles: Some(BTreeMap::from([("empty".to_string(), vec![])])),
            ..Default::default()
        };
        let policies = PolicyGenerator::generate_policies(&config);
        assert!(policies.is_empty());
    }

    #[test]
    fn test_escapes_special_chars() {
        let config = CedarAgentConfig {
            roles: Some(BTreeMap::from([(
                "role\"evil".to_string(),
                vec!["tool\"inject".to_string()],
            )])),
            ..Default::default()
        };
        let policies = PolicyGenerator::generate_policies(&config);
        assert_valid_cedar(&policies);
        assert!(policies.contains("Agent::Action::\"tool\\\"inject\""));
        assert!(policies.contains("Agent::Role::\"role\\\"evil\""));
    }

    #[test]
    fn test_sanitize_counter_key() {
        assert_eq!(sanitize_counter_key("send_email"), "send_email");
        assert_eq!(sanitize_counter_key("my.tool-v2"), "my_tool_v2");
        assert_eq!(sanitize_counter_key("ns::action"), "ns__action");
        assert_eq!(sanitize_counter_key("tool with spaces"), "tool_with_spaces");
    }

    #[test]
    fn test_rate_limit_special_chars_in_tool_name() {
        let config = CedarAgentConfig {
            rate_limits: Some(BTreeMap::from([("my.tool-v2".to_string(), 10)])),
            ..Default::default()
        };
        let policies = PolicyGenerator::generate_policies(&config);
        assert_valid_cedar(&policies);
        assert!(policies.contains("call_count_my_tool_v2"));
        assert!(!policies.contains("call_count_my.tool-v2"));
    }

    #[test]
    fn test_format_cedar_value_types() {
        assert_eq!(format_cedar_value(&serde_json::json!("hello")), "\"hello\"");
        assert_eq!(format_cedar_value(&serde_json::json!(42)), "42");
        assert_eq!(format_cedar_value(&serde_json::json!(true)), "true");
        assert_eq!(format_cedar_value(&serde_json::json!(false)), "false");
        assert_eq!(format_cedar_value(&serde_json::json!([1, 2])), "\"[1,2]\"");
    }

    #[test]
    fn test_restriction_empty_allowed_values() {
        let config = CedarAgentConfig {
            restrictions: Some(BTreeMap::from([(
                "dangerous_tool".to_string(),
                Restriction {
                    allowed_values: BTreeMap::new(),
                },
            )])),
            ..Default::default()
        };
        let policies = PolicyGenerator::generate_policies(&config);
        assert_valid_cedar(&policies);
        assert!(policies.contains(
            "forbid(\n  principal,\n  action == Agent::Action::\"dangerous_tool\",\n  resource\n);"
        ));
    }

    #[test]
    fn test_env_denial_wildcard_tools() {
        let config = CedarAgentConfig {
            deny_in_env: Some(BTreeMap::from([(
                "staging".to_string(),
                vec!["*".to_string()],
            )])),
            ..Default::default()
        };
        let policies = PolicyGenerator::generate_policies(&config);
        assert_valid_cedar(&policies);
        assert!(policies.contains("action,"));
        assert!(policies.contains("\"staging\""));
    }

    #[test]
    fn test_consent_false_produces_no_policies() {
        let config = CedarAgentConfig {
            consent: Some(BTreeMap::from([(
                "send_email".to_string(),
                ConsentScope::AllRoles(false),
            )])),
            ..Default::default()
        };
        let policies = PolicyGenerator::generate_policies(&config);
        assert!(policies.is_empty());
    }

    #[test]
    fn test_consent_all_roles_no_roles_defined_produces_no_policies() {
        // If no roles are configured, AllRoles(true) should produce no consent permits
        // rather than an unconstrained permit that any principal can satisfy.
        let config = CedarAgentConfig {
            consent: Some(BTreeMap::from([(
                "send_email".to_string(),
                ConsentScope::AllRoles(true),
            )])),
            ..Default::default()
        };
        let policies = PolicyGenerator::generate_policies(&config);
        assert!(policies.is_empty());
    }

    #[test]
    fn test_consent_all_roles_only_grants_to_applicable_roles() {
        // Only roles that actually list the tool should get consent access.
        let config = CedarAgentConfig {
            roles: Some(BTreeMap::from([
                (
                    "analyst".to_string(),
                    vec!["search".to_string(), "send_email".to_string()],
                ),
                ("viewer".to_string(), vec!["search".to_string()]),
            ])),
            consent: Some(BTreeMap::from([(
                "send_email".to_string(),
                ConsentScope::AllRoles(true),
            )])),
            ..Default::default()
        };
        let policies = PolicyGenerator::generate_policies(&config);
        assert_valid_cedar(&policies);
        // analyst has send_email → should get consent permit
        assert!(policies.contains(
            "permit(\n  principal in Agent::Role::\"analyst\",\n  action == Agent::Action::\"send_email\",\n  resource\n) when { context.session has user_consent && context.session.user_consent };"
        ));
        // viewer does NOT have send_email → must NOT get consent permit for send_email
        assert!(!policies.contains(
            "permit(\n  principal in Agent::Role::\"viewer\",\n  action == Agent::Action::\"send_email\",\n  resource\n) when { context.session has user_consent && context.session.user_consent };"
        ));
        // No unconstrained permit
        assert!(!policies.contains(
            "permit(\n  principal,\n  action == Agent::Action::\"send_email\",\n  resource\n)"
        ));
    }

    #[test]
    fn test_restriction_with_number_value() {
        let config = CedarAgentConfig {
            restrictions: Some(BTreeMap::from([(
                "query".to_string(),
                Restriction {
                    allowed_values: BTreeMap::from([(
                        "limit".to_string(),
                        vec![serde_json::json!(100), serde_json::json!(500)],
                    )]),
                },
            )])),
            ..Default::default()
        };
        let policies = PolicyGenerator::generate_policies(&config);
        assert_valid_cedar(&policies);
        assert!(policies.contains("context.input[\"limit\"] == 100"));
        assert!(policies.contains("context.input[\"limit\"] == 500"));
    }
}
