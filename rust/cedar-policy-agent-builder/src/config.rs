//! Configuration types for the Cedar agent policy builder.
//!
//! These types define the declarative input that drives policy generation.
//! They can be deserialized from JSON/YAML or constructed via [`CedarAgentPolicyBuilder`](crate::CedarAgentPolicyBuilder).

use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeMap;

/// Configuration for the principal (user/agent) entity type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrincipalConfig {
    pub key: String,
    #[serde(
        rename = "type",
        default = "default_principal_type",
        deserialize_with = "deserialize_cedar_ident"
    )]
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
    #[serde(
        rename = "type",
        default = "default_resource_type",
        deserialize_with = "deserialize_cedar_ident"
    )]
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
    /// Roles that do not already grant access to the tool are ignored at policy
    /// generation time — consent never adds access that wasn't already configured.
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
    #[serde(
        default = "default_namespace",
        deserialize_with = "deserialize_namespace"
    )]
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

/// Errors returned when a [`CedarAgentConfig`] contains invalid identifier fields.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConfigValidationError {
    /// The `namespace` field is not a valid Cedar identifier.
    #[error("invalid namespace \"{value}\": not a valid Cedar identifier")]
    InvalidNamespace { value: String },
    /// The `principal.type` field is not a valid Cedar identifier.
    #[error("invalid principal type \"{value}\": not a valid Cedar identifier")]
    InvalidPrincipalType { value: String },
    /// The `resource.type` field is not a valid Cedar identifier.
    #[error("invalid resource type \"{value}\": not a valid Cedar identifier")]
    InvalidResourceType { value: String },
}

/// Check whether a string is a valid Cedar identifier (namespace path segment or entity type name).
///
/// Uses the Cedar parser to reject reserved words, invalid characters, and injection attempts.
fn is_valid_cedar_ident(s: &str) -> bool {
    cedar_policy::pst::Id::new(s).is_ok()
}

/// Validate all identifier-like fields in a [`CedarAgentConfig`].
///
/// Returns the first validation error found, or `Ok(())` if all fields are valid.
/// Call this before policy/entity generation when the config was deserialized from
/// an untrusted source (JSON, YAML, API input).
pub fn validate_config(config: &CedarAgentConfig) -> Result<(), ConfigValidationError> {
    if !is_valid_cedar_ident(&config.namespace) {
        return Err(ConfigValidationError::InvalidNamespace {
            value: config.namespace.clone(),
        });
    }
    if !is_valid_cedar_ident(&config.principal.principal_type) {
        return Err(ConfigValidationError::InvalidPrincipalType {
            value: config.principal.principal_type.clone(),
        });
    }
    if let Some(resource) = &config.resource {
        if !is_valid_cedar_ident(&resource.resource_type) {
            return Err(ConfigValidationError::InvalidResourceType {
                value: resource.resource_type.clone(),
            });
        }
    }
    Ok(())
}

/// Serde deserializer that validates a string is a valid Cedar identifier.
///
/// Rejects values that could enable policy injection when interpolated into Cedar text.
fn deserialize_cedar_ident<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    if is_valid_cedar_ident(&s) {
        Ok(s)
    } else {
        Err(serde::de::Error::custom(format!(
            "\"{s}\" is not a valid Cedar identifier"
        )))
    }
}

/// Serde deserializer for namespace with default + validation.
fn deserialize_namespace<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_cedar_ident(deserializer)
}
