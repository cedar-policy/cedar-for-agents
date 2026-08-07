//! Configuration types for the Cedar agent policy builder.
//!
//! These types define the declarative input that drives policy generation.
//! They can be deserialized from JSON/YAML or constructed via [`CedarAgentPolicyBuilder`](crate::CedarAgentPolicyBuilder).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Configuration for the principal (user/agent) entity type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrincipalConfig {
    pub key: String,
    #[serde(rename = "type", default = "default_principal_type")]
    pub principal_type: String,
}

fn default_principal_type() -> String {
    "User".to_string()
}

impl Default for PrincipalConfig {
    fn default() -> Self {
        Self {
            key: "user_id".to_string(),
            principal_type: default_principal_type(),
        }
    }
}

/// Configuration for the resource entity (the thing being accessed).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceConfig {
    #[serde(rename = "type", default = "default_resource_type")]
    pub resource_type: String,
    #[serde(default = "default_resource_id")]
    pub id: String,
}

fn default_resource_type() -> String {
    "Resource".to_string()
}

fn default_resource_id() -> String {
    "default".to_string()
}

impl Default for ResourceConfig {
    fn default() -> Self {
        Self {
            resource_type: default_resource_type(),
            id: default_resource_id(),
        }
    }
}

/// Whether consent is required for all roles or specific ones.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ConsentScope {
    /// `true` = all roles require consent; `false` = no consent required.
    AllRoles(bool),
    /// Only the listed roles require consent for this tool.
    SpecificRoles(Vec<String>),
}

/// Input field restrictions for a tool action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Restriction {
    /// Map of field name to allowed values. The action is denied unless the field matches.
    #[serde(rename = "allowedValues")]
    pub allowed_values: BTreeMap<String, Vec<serde_json::Value>>,
}

/// A UTC hour window during which an action is permitted.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeWindow {
    /// Start hour (inclusive), 0-23.
    #[serde(rename = "hourStart")]
    pub hour_start: u8,
    /// End hour (exclusive), 1-24.
    #[serde(rename = "hourEnd")]
    pub hour_end: u8,
}

/// An MCP tool definition used for Cedar schema generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolDefinition {
    /// Tool name (becomes a Cedar action).
    pub name: String,
    /// Human-readable description.
    #[serde(default)]
    pub description: Option<String>,
    /// JSON Schema for the tool's input parameters.
    #[serde(rename = "inputSchema")]
    pub input_schema: serde_json::Value,
    /// Optional JSON Schema for the tool's output.
    #[serde(rename = "outputSchema", default)]
    pub output_schema: Option<serde_json::Value>,
}

/// Top-level configuration for Cedar agent policy generation.
///
/// This is the serializable form of the builder's internal state. It can be
/// deserialized directly from JSON/YAML for config-file-driven workflows.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CedarAgentConfig {
    /// Principal (user/agent) entity configuration.
    #[serde(default)]
    pub principal: PrincipalConfig,
    /// Role definitions: role name → list of permitted tool names (`"*"` = all).
    #[serde(default)]
    pub roles: Option<BTreeMap<String, Vec<String>>>,
    /// User definitions: user ID → list of role names.
    #[serde(default)]
    pub users: Option<BTreeMap<String, Vec<String>>>,
    /// Per-tool input field restrictions.
    #[serde(default)]
    pub restrictions: Option<BTreeMap<String, Restriction>>,
    /// Per-tool (or global `"*"`) invocation count limits.
    ///
    /// Each key is a tool name (or `"*"` for all tools). The generated `forbid`
    /// policy references a session counter attribute named `call_count_<key>` where
    /// non-alphanumeric characters are replaced with `_`. Tool names that collide
    /// after this sanitization (e.g. `"my-tool"` and `"my.tool"`) will produce an
    /// error from [`CedarAgentPolicyBuilder::rate_limit`](crate::CedarAgentPolicyBuilder::rate_limit).
    #[serde(rename = "rateLimits", default)]
    pub rate_limits: Option<BTreeMap<String, u64>>,
    /// Per-tool (or global `"*"`) allowed UTC hour windows.
    #[serde(rename = "timeWindows", default)]
    pub time_windows: Option<BTreeMap<String, TimeWindow>>,
    /// Tools denied in specific environments.
    #[serde(rename = "denyInEnv", default)]
    pub deny_in_env: Option<BTreeMap<String, Vec<String>>>,
    /// Tools requiring user consent.
    #[serde(default)]
    pub consent: Option<BTreeMap<String, ConsentScope>>,
    /// Custom resource entity configuration.
    #[serde(default)]
    pub resource: Option<ResourceConfig>,
    /// MCP tool definitions for schema generation.
    #[serde(default)]
    pub tools: Option<Vec<McpToolDefinition>>,
    /// Cedar namespace (default: `"Agent"`).
    #[serde(default = "default_namespace")]
    pub namespace: String,
}

fn default_namespace() -> String {
    "Agent".to_string()
}

impl Default for CedarAgentConfig {
    fn default() -> Self {
        Self {
            principal: PrincipalConfig::default(),
            roles: None,
            users: None,
            restrictions: None,
            rate_limits: None,
            time_windows: None,
            deny_in_env: None,
            consent: None,
            resource: None,
            tools: None,
            namespace: default_namespace(),
        }
    }
}
