use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ConsentScope {
    AllRoles(bool),
    SpecificRoles(Vec<String>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Restriction {
    #[serde(rename = "allowedValues")]
    pub allowed_values: BTreeMap<String, Vec<serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeWindow {
    #[serde(rename = "hourStart")]
    pub hour_start: u8,
    #[serde(rename = "hourEnd")]
    pub hour_end: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolDefinition {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(rename = "inputSchema")]
    pub input_schema: serde_json::Value,
    #[serde(rename = "outputSchema", default)]
    pub output_schema: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CedarAgentConfig {
    #[serde(default)]
    pub principal: PrincipalConfig,
    #[serde(default)]
    pub roles: Option<BTreeMap<String, Vec<String>>>,
    #[serde(default)]
    pub users: Option<BTreeMap<String, Vec<String>>>,
    #[serde(default)]
    pub restrictions: Option<BTreeMap<String, Restriction>>,
    #[serde(rename = "rateLimits", default)]
    pub rate_limits: Option<BTreeMap<String, u64>>,
    #[serde(rename = "timeWindows", default)]
    pub time_windows: Option<BTreeMap<String, TimeWindow>>,
    #[serde(rename = "denyInEnv", default)]
    pub deny_in_env: Option<BTreeMap<String, Vec<String>>>,
    #[serde(default)]
    pub consent: Option<BTreeMap<String, ConsentScope>>,
    #[serde(default)]
    pub resource: Option<ResourceConfig>,
    #[serde(default)]
    pub tools: Option<Vec<McpToolDefinition>>,
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
