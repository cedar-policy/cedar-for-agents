use cedar_policy::{Authorizer, Context, Entities, EntityUid, PolicySet, Request};
use cedar_policy_agent_builder::CedarAgentPolicyBuilder;
use std::collections::BTreeMap;
use std::str::FromStr;

fn authorize(
    policies: &str,
    entities_json: &str,
    principal: &str,
    action: &str,
    resource: &str,
    context: serde_json::Value,
) -> cedar_policy::Decision {
    let policy_set: PolicySet = policies.parse().expect("policies should parse");
    let entities = Entities::from_json_str(entities_json, None).expect("entities should parse");
    let principal = EntityUid::from_str(principal).expect("principal should parse");
    let action = EntityUid::from_str(action).expect("action should parse");
    let resource = EntityUid::from_str(resource).expect("resource should parse");
    let context = Context::from_json_value(context, None).expect("context should parse");
    let request =
        Request::new(principal, action, resource, context, None).expect("request should build");
    let authorizer = Authorizer::new();
    authorizer
        .is_authorized(&request, &policy_set, &entities)
        .decision()
}

fn entities_to_json(entities: &[cedar_policy_agent_builder::entities::EntityJson]) -> String {
    serde_json::to_string(entities).expect("entities should serialize")
}

#[test]
fn test_admin_allowed_any_action() {
    let result = CedarAgentPolicyBuilder::new()
        .role("admin", &["*"])
        .user("alice", &["admin"])
        .build()
        .unwrap();

    let decision = authorize(
        &result.policies,
        &entities_to_json(&result.entities),
        "Agent::User::\"alice\"",
        "Agent::Action::\"any_tool\"",
        "Agent::Resource::\"default\"",
        serde_json::json!({}),
    );
    assert_eq!(decision, cedar_policy::Decision::Allow);
}

#[test]
fn test_unknown_user_denied() {
    let result = CedarAgentPolicyBuilder::new()
        .role("admin", &["*"])
        .user("alice", &["admin"])
        .build()
        .unwrap();

    let entities_json = entities_to_json(&result.entities);
    let entities_with_bob = {
        let mut ents: Vec<serde_json::Value> = serde_json::from_str(&entities_json).expect("parse");
        ents.push(serde_json::json!({
            "uid": {"type": "Agent::User", "id": "bob"},
            "attrs": {},
            "parents": []
        }));
        serde_json::to_string(&ents).expect("serialize")
    };

    let decision = authorize(
        &result.policies,
        &entities_with_bob,
        "Agent::User::\"bob\"",
        "Agent::Action::\"search\"",
        "Agent::Resource::\"default\"",
        serde_json::json!({}),
    );
    assert_eq!(decision, cedar_policy::Decision::Deny);
}

#[test]
fn test_role_scoped_to_specific_tools() {
    let result = CedarAgentPolicyBuilder::new()
        .role("analyst", &["search", "query"])
        .user("bob", &["analyst"])
        .build()
        .unwrap();

    let entities_json = entities_to_json(&result.entities);

    let allowed = authorize(
        &result.policies,
        &entities_json,
        "Agent::User::\"bob\"",
        "Agent::Action::\"search\"",
        "Agent::Resource::\"default\"",
        serde_json::json!({}),
    );
    assert_eq!(allowed, cedar_policy::Decision::Allow);

    let denied = authorize(
        &result.policies,
        &entities_json,
        "Agent::User::\"bob\"",
        "Agent::Action::\"delete\"",
        "Agent::Resource::\"default\"",
        serde_json::json!({}),
    );
    assert_eq!(denied, cedar_policy::Decision::Deny);
}

#[test]
fn test_rate_limit_denies_when_exceeded() {
    let result = CedarAgentPolicyBuilder::new()
        .role("user", &["send_email"])
        .user("alice", &["user"])
        .rate_limit("send_email", 5)
        .unwrap()
        .build()
        .unwrap();

    let entities_json = entities_to_json(&result.entities);

    let under_limit = authorize(
        &result.policies,
        &entities_json,
        "Agent::User::\"alice\"",
        "Agent::Action::\"send_email\"",
        "Agent::Resource::\"default\"",
        serde_json::json!({"session": {"call_count_send_email": 3}}),
    );
    assert_eq!(under_limit, cedar_policy::Decision::Allow);

    let at_limit = authorize(
        &result.policies,
        &entities_json,
        "Agent::User::\"alice\"",
        "Agent::Action::\"send_email\"",
        "Agent::Resource::\"default\"",
        serde_json::json!({"session": {"call_count_send_email": 5}}),
    );
    assert_eq!(at_limit, cedar_policy::Decision::Deny);
}

#[test]
fn test_time_window_denies_outside_hours() {
    let result = CedarAgentPolicyBuilder::new()
        .role("user", &["deploy"])
        .user("alice", &["user"])
        .time_window("deploy", (9, 17))
        .unwrap()
        .build()
        .unwrap();

    let entities_json = entities_to_json(&result.entities);

    let within_hours = authorize(
        &result.policies,
        &entities_json,
        "Agent::User::\"alice\"",
        "Agent::Action::\"deploy\"",
        "Agent::Resource::\"default\"",
        serde_json::json!({"session": {"hour_utc": 12}}),
    );
    assert_eq!(within_hours, cedar_policy::Decision::Allow);

    let outside_hours = authorize(
        &result.policies,
        &entities_json,
        "Agent::User::\"alice\"",
        "Agent::Action::\"deploy\"",
        "Agent::Resource::\"default\"",
        serde_json::json!({"session": {"hour_utc": 20}}),
    );
    assert_eq!(outside_hours, cedar_policy::Decision::Deny);
}

#[test]
fn test_env_denial_blocks_in_production() {
    let result = CedarAgentPolicyBuilder::new()
        .role("admin", &["*"])
        .user("alice", &["admin"])
        .deny_in_env("production", &["delete"])
        .build()
        .unwrap();

    let entities_json = entities_to_json(&result.entities);

    let in_prod = authorize(
        &result.policies,
        &entities_json,
        "Agent::User::\"alice\"",
        "Agent::Action::\"delete\"",
        "Agent::Resource::\"default\"",
        serde_json::json!({"session": {"environment": "production"}}),
    );
    assert_eq!(in_prod, cedar_policy::Decision::Deny);

    let in_staging = authorize(
        &result.policies,
        &entities_json,
        "Agent::User::\"alice\"",
        "Agent::Action::\"delete\"",
        "Agent::Resource::\"default\"",
        serde_json::json!({"session": {"environment": "staging"}}),
    );
    assert_eq!(in_staging, cedar_policy::Decision::Allow);
}

#[test]
fn test_consent_required_before_action() {
    let result = CedarAgentPolicyBuilder::new()
        .role("user", &["search", "send_email"])
        .user("alice", &["user"])
        .consent_all("send_email")
        .build()
        .unwrap();

    let entities_json = entities_to_json(&result.entities);

    let without_consent = authorize(
        &result.policies,
        &entities_json,
        "Agent::User::\"alice\"",
        "Agent::Action::\"send_email\"",
        "Agent::Resource::\"default\"",
        serde_json::json!({"session": {}}),
    );
    assert_eq!(without_consent, cedar_policy::Decision::Deny);

    let with_consent = authorize(
        &result.policies,
        &entities_json,
        "Agent::User::\"alice\"",
        "Agent::Action::\"send_email\"",
        "Agent::Resource::\"default\"",
        serde_json::json!({"session": {"user_consent": true}}),
    );
    assert_eq!(with_consent, cedar_policy::Decision::Allow);

    let other_tool_no_consent_needed = authorize(
        &result.policies,
        &entities_json,
        "Agent::User::\"alice\"",
        "Agent::Action::\"search\"",
        "Agent::Resource::\"default\"",
        serde_json::json!({}),
    );
    assert_eq!(other_tool_no_consent_needed, cedar_policy::Decision::Allow);
}

#[test]
fn test_restriction_enforces_allowed_values() {
    let result = CedarAgentPolicyBuilder::new()
        .role("analyst", &["query"])
        .user("bob", &["analyst"])
        .restrict(
            "query",
            BTreeMap::from([("database".to_string(), vec![serde_json::json!("analytics")])]),
        )
        .build()
        .unwrap();

    let entities_json = entities_to_json(&result.entities);

    let allowed_db = authorize(
        &result.policies,
        &entities_json,
        "Agent::User::\"bob\"",
        "Agent::Action::\"query\"",
        "Agent::Resource::\"default\"",
        serde_json::json!({"input": {"database": "analytics"}}),
    );
    assert_eq!(allowed_db, cedar_policy::Decision::Allow);

    let disallowed_db = authorize(
        &result.policies,
        &entities_json,
        "Agent::User::\"bob\"",
        "Agent::Action::\"query\"",
        "Agent::Resource::\"default\"",
        serde_json::json!({"input": {"database": "production"}}),
    );
    assert_eq!(disallowed_db, cedar_policy::Decision::Deny);
}

#[test]
fn test_custom_namespace_and_principal_type() {
    let result = CedarAgentPolicyBuilder::new()
        .namespace("MyApp")
        .unwrap()
        .principal("sub", "Agent")
        .unwrap()
        .role("admin", &["*"])
        .user("alice", &["admin"])
        .build()
        .unwrap();

    let entities_json = entities_to_json(&result.entities);

    let decision = authorize(
        &result.policies,
        &entities_json,
        "MyApp::Agent::\"alice\"",
        "MyApp::Action::\"anything\"",
        "MyApp::Resource::\"default\"",
        serde_json::json!({}),
    );
    assert_eq!(decision, cedar_policy::Decision::Allow);
}

#[test]
fn test_multi_role_user_inherits_all_permissions() {
    let result = CedarAgentPolicyBuilder::new()
        .role("reader", &["search"])
        .role("writer", &["create", "update"])
        .user("charlie", &["reader", "writer"])
        .build()
        .unwrap();

    let entities_json = entities_to_json(&result.entities);

    for action in &["search", "create", "update"] {
        let decision = authorize(
            &result.policies,
            &entities_json,
            "Agent::User::\"charlie\"",
            &format!("Agent::Action::\"{action}\""),
            "Agent::Resource::\"default\"",
            serde_json::json!({}),
        );
        assert_eq!(
            decision,
            cedar_policy::Decision::Allow,
            "charlie should be allowed {action}"
        );
    }

    let denied = authorize(
        &result.policies,
        &entities_json,
        "Agent::User::\"charlie\"",
        "Agent::Action::\"delete\"",
        "Agent::Resource::\"default\"",
        serde_json::json!({}),
    );
    assert_eq!(denied, cedar_policy::Decision::Deny);
}

#[test]
fn test_combined_policies_all_interact_correctly() {
    let result = CedarAgentPolicyBuilder::new()
        .namespace("Agent")
        .unwrap()
        .principal("sub", "User")
        .unwrap()
        .role("admin", &["*"])
        .role("analyst", &["search", "query", "send_email"])
        .user("alice", &["admin"])
        .user("bob", &["analyst"])
        .rate_limit("send_email", 3)
        .unwrap()
        .time_window("*", (9, 17))
        .unwrap()
        .consent_all("send_email")
        .deny_in_env("production", &["delete"])
        .restrict(
            "query",
            BTreeMap::from([(
                "database".to_string(),
                vec![
                    serde_json::json!("analytics"),
                    serde_json::json!("reporting"),
                ],
            )]),
        )
        .build()
        .unwrap();

    let entities_json = entities_to_json(&result.entities);

    // Admin can do anything in working hours
    let admin_ok = authorize(
        &result.policies,
        &entities_json,
        "Agent::User::\"alice\"",
        "Agent::Action::\"search\"",
        "Agent::Resource::\"default\"",
        serde_json::json!({"session": {"hour_utc": 10}}),
    );
    assert_eq!(admin_ok, cedar_policy::Decision::Allow);

    // Admin blocked outside hours
    let admin_after_hours = authorize(
        &result.policies,
        &entities_json,
        "Agent::User::\"alice\"",
        "Agent::Action::\"search\"",
        "Agent::Resource::\"default\"",
        serde_json::json!({"session": {"hour_utc": 22}}),
    );
    assert_eq!(admin_after_hours, cedar_policy::Decision::Deny);

    // Admin blocked from delete in production
    let admin_delete_prod = authorize(
        &result.policies,
        &entities_json,
        "Agent::User::\"alice\"",
        "Agent::Action::\"delete\"",
        "Agent::Resource::\"default\"",
        serde_json::json!({"session": {"hour_utc": 10, "environment": "production"}}),
    );
    assert_eq!(admin_delete_prod, cedar_policy::Decision::Deny);

    // Analyst can search in working hours
    let analyst_search = authorize(
        &result.policies,
        &entities_json,
        "Agent::User::\"bob\"",
        "Agent::Action::\"search\"",
        "Agent::Resource::\"default\"",
        serde_json::json!({"session": {"hour_utc": 10}}),
    );
    assert_eq!(analyst_search, cedar_policy::Decision::Allow);

    // Analyst query with allowed database works
    let analyst_query_ok = authorize(
        &result.policies,
        &entities_json,
        "Agent::User::\"bob\"",
        "Agent::Action::\"query\"",
        "Agent::Resource::\"default\"",
        serde_json::json!({"session": {"hour_utc": 10}, "input": {"database": "analytics"}}),
    );
    assert_eq!(analyst_query_ok, cedar_policy::Decision::Allow);

    // Analyst query with disallowed database blocked
    let analyst_query_bad = authorize(
        &result.policies,
        &entities_json,
        "Agent::User::\"bob\"",
        "Agent::Action::\"query\"",
        "Agent::Resource::\"default\"",
        serde_json::json!({"session": {"hour_utc": 10}, "input": {"database": "users"}}),
    );
    assert_eq!(analyst_query_bad, cedar_policy::Decision::Deny);

    // Analyst send_email needs consent AND under rate limit
    let analyst_email_with_consent = authorize(
        &result.policies,
        &entities_json,
        "Agent::User::\"bob\"",
        "Agent::Action::\"send_email\"",
        "Agent::Resource::\"default\"",
        serde_json::json!({"session": {"hour_utc": 10, "user_consent": true, "call_count_send_email": 2}}),
    );
    assert_eq!(analyst_email_with_consent, cedar_policy::Decision::Allow);

    // Analyst send_email over rate limit blocked
    let analyst_email_over_limit = authorize(
        &result.policies,
        &entities_json,
        "Agent::User::\"bob\"",
        "Agent::Action::\"send_email\"",
        "Agent::Resource::\"default\"",
        serde_json::json!({"session": {"hour_utc": 10, "user_consent": true, "call_count_send_email": 3}}),
    );
    assert_eq!(analyst_email_over_limit, cedar_policy::Decision::Deny);
}

#[test]
fn test_consent_all_does_not_grant_to_unauthorized_roles() {
    // Regression test: consent_all must not allow principals without the tool
    // in their role to invoke the tool merely by providing user_consent=true.
    let result = CedarAgentPolicyBuilder::new()
        .role("sender", &["send_email", "search"])
        .role("viewer", &["search"])
        .user("alice", &["sender"])
        .user("bob", &["viewer"])
        .consent_all("send_email")
        .build()
        .unwrap();

    let entities_json = entities_to_json(&result.entities);

    // Alice (sender role, has send_email) with consent → allowed
    let alice_allowed = authorize(
        &result.policies,
        &entities_json,
        "Agent::User::\"alice\"",
        "Agent::Action::\"send_email\"",
        "Agent::Resource::\"default\"",
        serde_json::json!({"session": {"user_consent": true}}),
    );
    assert_eq!(alice_allowed, cedar_policy::Decision::Allow);

    // Bob (viewer role, does NOT have send_email) with consent → must be denied
    let bob_denied = authorize(
        &result.policies,
        &entities_json,
        "Agent::User::\"bob\"",
        "Agent::Action::\"send_email\"",
        "Agent::Resource::\"default\"",
        serde_json::json!({"session": {"user_consent": true}}),
    );
    assert_eq!(bob_denied, cedar_policy::Decision::Deny);
}

#[test]
fn test_consent_all_denies_unknown_principal() {
    // An entity not in any role must be denied even with user_consent=true.
    let result = CedarAgentPolicyBuilder::new()
        .role("user", &["send_email"])
        .user("alice", &["user"])
        .consent_all("send_email")
        .build()
        .unwrap();

    let entities_json = entities_to_json(&result.entities);

    // Add an unknown user entity to the entities
    let entities_with_eve = {
        let mut ents: Vec<serde_json::Value> = serde_json::from_str(&entities_json).expect("parse");
        ents.push(serde_json::json!({
            "uid": {"type": "Agent::User", "id": "eve"},
            "attrs": {},
            "parents": []
        }));
        serde_json::to_string(&ents).expect("serialize")
    };

    let eve_denied = authorize(
        &result.policies,
        &entities_with_eve,
        "Agent::User::\"eve\"",
        "Agent::Action::\"send_email\"",
        "Agent::Resource::\"default\"",
        serde_json::json!({"session": {"user_consent": true}}),
    );
    assert_eq!(eve_denied, cedar_policy::Decision::Deny);
}
