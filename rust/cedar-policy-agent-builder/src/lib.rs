pub mod builder;
pub mod config;
pub mod entities;
pub mod policy;

pub use builder::{BuilderError, CedarAgentPolicyBuilder};

use cedar_policy::{PolicySet, Schema, ValidationMode, Validator};
use config::CedarAgentConfig;
use entities::{generate_entities, EntityJson};
use policy::generate_policies;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    pub policy_id: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildResult {
    pub policies: String,
    pub entities: Vec<EntityJson>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub schema_errors: Vec<SchemaError>,
}

impl BuildResult {
    pub fn validate(&self) -> ValidationResult {
        let schema_str = match &self.schema {
            Some(s) => s,
            None => {
                return ValidationResult {
                    valid: true,
                    errors: Vec::new(),
                    warnings: Vec::new(),
                }
            }
        };

        let schema: Schema = match schema_str.parse() {
            Ok(s) => s,
            Err(e) => {
                return ValidationResult {
                    valid: false,
                    errors: vec![ValidationError {
                        policy_id: String::new(),
                        message: format!("schema parse error: {e}"),
                        help: None,
                    }],
                    warnings: Vec::new(),
                };
            }
        };

        let policy_set: PolicySet = match self.policies.parse() {
            Ok(p) => p,
            Err(e) => {
                return ValidationResult {
                    valid: false,
                    errors: vec![ValidationError {
                        policy_id: String::new(),
                        message: format!("policy parse error: {e}"),
                        help: None,
                    }],
                    warnings: Vec::new(),
                };
            }
        };

        let validator = Validator::new(schema);
        let result = validator.validate(&policy_set, ValidationMode::default());

        let errors: Vec<ValidationError> = result
            .validation_errors()
            .map(|e| ValidationError {
                policy_id: e.policy_id().to_string(),
                message: e.to_string(),
                help: None,
            })
            .collect();

        let warnings: Vec<ValidationError> = result
            .validation_warnings()
            .map(|w| ValidationError {
                policy_id: w.policy_id().to_string(),
                message: w.to_string(),
                help: None,
            })
            .collect();

        ValidationResult {
            valid: errors.is_empty(),
            errors,
            warnings,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaError {
    pub stage: String,
    pub message: String,
}

impl std::fmt::Display for SchemaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "schema generation failed at {}: {}",
            self.stage, self.message
        )
    }
}

pub fn build(config: &CedarAgentConfig) -> BuildResult {
    let policies = generate_policies(config);
    let entities = generate_entities(config);
    let (schema, schema_errors) = match generate_schema(config) {
        Ok(s) => (s, Vec::new()),
        Err(e) => (None, vec![e]),
    };

    BuildResult {
        policies,
        entities,
        schema,
        schema_errors,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use config::McpToolDefinition;

    #[test]
    fn test_build_with_tools_generates_schema() {
        let config = CedarAgentConfig {
            roles: Some(std::collections::BTreeMap::from([(
                "admin".to_string(),
                vec!["*".to_string()],
            )])),
            tools: Some(vec![McpToolDefinition {
                name: "search".to_string(),
                description: Some("Search for items".to_string()),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "query": { "type": "string" }
                    }
                }),
                output_schema: None,
            }]),
            ..Default::default()
        };
        let result = build(&config);
        assert!(result.schema.is_some());
        assert!(result.schema_errors.is_empty());
        let schema = result.schema.unwrap();
        assert!(schema.contains("search"));
    }

    #[test]
    fn test_build_without_tools_has_no_schema() {
        let config = CedarAgentConfig {
            roles: Some(std::collections::BTreeMap::from([(
                "admin".to_string(),
                vec!["*".to_string()],
            )])),
            ..Default::default()
        };
        let result = build(&config);
        assert!(result.schema.is_none());
        assert!(result.schema_errors.is_empty());
    }

    #[test]
    fn test_schema_errors_surfaced_in_result() {
        let config = CedarAgentConfig {
            tools: Some(vec![McpToolDefinition {
                name: "".to_string(),
                description: None,
                input_schema: serde_json::json!(null),
                output_schema: None,
            }]),
            ..Default::default()
        };
        let result = build(&config);
        assert!(!result.schema_errors.is_empty());
        assert!(result.schema.is_none());
        assert!(!result.schema_errors[0].message.is_empty());
    }

    #[test]
    fn test_build_with_output_schema() {
        let config = CedarAgentConfig {
            tools: Some(vec![McpToolDefinition {
                name: "fetch".to_string(),
                description: Some("Fetch data".to_string()),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": { "url": { "type": "string" } }
                }),
                output_schema: Some(serde_json::json!({
                    "type": "object",
                    "properties": { "body": { "type": "string" } }
                })),
            }]),
            ..Default::default()
        };
        let result = build(&config);
        assert!(result.schema.is_some());
        assert!(result.schema_errors.is_empty());
    }

    #[test]
    fn test_validate_no_schema_returns_valid() {
        let result = BuildResult {
            policies: String::new(),
            entities: Vec::new(),
            schema: None,
            schema_errors: Vec::new(),
        };
        let validation = result.validate();
        assert!(validation.valid);
        assert!(validation.errors.is_empty());
    }

    #[test]
    fn test_validate_invalid_policy_returns_error() {
        let result = BuildResult {
            policies: "this is not valid cedar".to_string(),
            entities: Vec::new(),
            schema: Some("namespace Agent { entity User; }".to_string()),
            schema_errors: Vec::new(),
        };
        let validation = result.validate();
        assert!(!validation.valid);
        assert!(!validation.errors.is_empty());
        assert!(validation.errors[0].message.contains("policy parse error"));
    }

    #[test]
    fn test_validate_invalid_schema_returns_error() {
        let result = BuildResult {
            policies: "permit(principal, action, resource);".to_string(),
            entities: Vec::new(),
            schema: Some("not valid schema syntax {{{{".to_string()),
            schema_errors: Vec::new(),
        };
        let validation = result.validate();
        assert!(!validation.valid);
        assert!(!validation.errors.is_empty());
        assert!(validation.errors[0].message.contains("schema parse error"));
    }

    #[test]
    fn test_build_with_custom_resource() {
        let config = CedarAgentConfig {
            resource: Some(config::ResourceConfig {
                resource_type: "Gateway".to_string(),
                id: "prod".to_string(),
            }),
            tools: Some(vec![McpToolDefinition {
                name: "invoke".to_string(),
                description: None,
                input_schema: serde_json::json!({"type": "object"}),
                output_schema: None,
            }]),
            ..Default::default()
        };
        let result = build(&config);
        assert!(result.schema.is_some());
        assert!(result.schema_errors.is_empty());
        let schema = result.schema.unwrap();
        assert!(schema.contains("Gateway"));
    }

    #[test]
    fn test_build_empty_tools_vec_no_schema() {
        let config = CedarAgentConfig {
            tools: Some(vec![]),
            ..Default::default()
        };
        let result = build(&config);
        assert!(result.schema.is_none());
        assert!(result.schema_errors.is_empty());
    }

    #[test]
    fn test_schema_error_display() {
        let err = SchemaError {
            stage: "test_stage".to_string(),
            message: "something broke".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "schema generation failed at test_stage: something broke"
        );
    }
}

fn generate_schema(config: &CedarAgentConfig) -> Result<Option<String>, SchemaError> {
    let tools = match config.tools.as_ref() {
        Some(t) if !t.is_empty() => t,
        _ => return Ok(None),
    };

    let principal_type = &config.principal.principal_type;
    let resource_type = config
        .resource
        .as_ref()
        .map(|r| r.resource_type.as_str())
        .unwrap_or("Resource");
    let ns = &config.namespace;

    let schema_stub = format!(
        "namespace {ns} {{\n  entity Role;\n\n  @mcp_principal(\"{principal_type}\")\n  entity {principal_type} in [Role];\n\n  @mcp_resource(\"{resource_type}\")\n  entity {resource_type};\n}}"
    );

    let tools_json: Vec<serde_json::Value> = tools
        .iter()
        .map(|t| {
            let mut obj = serde_json::json!({
                "name": t.name,
                "description": t.description.as_deref().unwrap_or(""),
                "inputSchema": t.input_schema,
            });
            if let Some(output) = &t.output_schema {
                if let Some(o) = obj.as_object_mut() {
                    o.insert("outputSchema".to_string(), output.clone());
                }
            }
            obj
        })
        .collect();

    let server_json = serde_json::json!({ "result": { "tools": tools_json } });

    use cedar_policy_core::extensions::Extensions;
    use cedar_policy_core::validator::json_schema::Fragment;
    use cedar_policy_core::validator::RawName;
    use cedar_policy_mcp_schema_generator::{SchemaGenerator, SchemaGeneratorConfig};
    use mcp_tools_sdk::description::ServerDescription;

    let fragment: Fragment<RawName> =
        Fragment::from_cedarschema_str(&schema_stub, Extensions::all_available())
            .map(|(f, _)| f)
            .map_err(|e| SchemaError {
                stage: "parse_schema_stub".to_string(),
                message: format!("{e}"),
            })?;

    let schema_config = SchemaGeneratorConfig::default();
    let mut generator =
        SchemaGenerator::new_with_config(fragment, schema_config).map_err(|e| SchemaError {
            stage: "init_generator".to_string(),
            message: format!("{e}"),
        })?;

    let server_str = server_json.to_string();
    let server_desc = ServerDescription::from_json_str(&server_str).map_err(|e| SchemaError {
        stage: "parse_server_description".to_string(),
        message: format!("{e}"),
    })?;

    generator
        .add_actions_from_server_description(&server_desc)
        .map_err(|e| SchemaError {
            stage: "add_actions".to_string(),
            message: format!("{e}"),
        })?;

    let schema_fragment = generator.get_schema();
    schema_fragment
        .to_cedarschema()
        .map(Some)
        .map_err(|e| SchemaError {
            stage: "serialize_schema".to_string(),
            message: format!("{e}"),
        })
}
