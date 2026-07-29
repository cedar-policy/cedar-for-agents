# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - Coming soon

### Added
- Fluent `CedarAgentPolicyBuilder` API for generating Cedar policies, entities, and schemas from declarative configuration.
- Role-based access control with `permit` policies scoped to role membership.
- Rate limiting via `forbid` policies conditioned on session call counters.
- UTC time window restrictions for tool access.
- Environment-based denial policies (e.g., deny destructive tools in production).
- User consent gates requiring explicit approval before tool execution.
- Input field restrictions limiting tool parameters to allowed values.
- Cedar schema generation from MCP tool definitions.
- Policy validation against generated schemas.
- Custom namespace, principal type, and resource entity configuration.
