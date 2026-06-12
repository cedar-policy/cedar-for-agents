# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Initial release of `cedar-agent-policy-builder`
- Fluent builder API: `.role()`, `.restrict()`, `.rateLimit()`, `.timeWindow()`, `.denyToolsInEnv()`, `.consent()`, `.resource()`, `.namespace()`, `.tools()`
- `fromConfig()` function for JSON/object-based configuration
- Cedar schema generation via `@cedar-policy/mcp-schema-generator-wasm` integration
- McpServer resource entity generation (aligns with MCP schema generator conventions)
- Consent-gated policies (permit when `context.session.user_consent == true`)
- Build-time warnings for undeclared tool name references
- Edge case handling: empty `allowedValues` = deny tool, `denyToolsInEnv()` without tools = deny all, `rateLimit(0)` = always deny
