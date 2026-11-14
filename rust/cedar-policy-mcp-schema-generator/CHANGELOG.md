# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Adds support for schema stubs with one namespace per MCP server. A namespace annotated with `@mcp_server("name")` is a server namespace; it must be a direct child of the main namespace (the only namespace without `@mcp_server`) and declare exactly one `@mcp_resource` entity type. The tools of a server are generated in its namespace and apply to its resource type. Principals are declared in the main namespace. `@mcp_context` keys of the main namespace apply to every server and can be overridden or extended per server. If a server namespace has no `@mcp_action`, the generator adds an `AllActions` action, child of the main namespace's `@mcp_action` actions, as the parent of the server's tools.
- Adds `SchemaGenerator::add_actions_from_named_server_description` and `SchemaGenerator::add_actions_from_named_server_descriptions` to add the tools of servers to their namespace, and `RequestGenerator::generate_named_request`, `generate_named_request_components` and `generate_named_request_components_from_strings` to generate requests for them.
- Adds the CLI options `generate --server NAME=FILE` (repeatable) and `authorize --server NAME`.
- Adds `StubBuilder`, which adds a server namespace for each server name to a schema stub with a main namespace. Each server namespace has a resource type `Target`, child of the entity types annotated with the new `@mcp_resource_parent` annotation, and an action `AllActions`, child of the `@mcp_action` actions of the main namespace. The namespace identifier is derived from the server name (e.g., `weather-api` gives `weather_api`) or given explicitly. Adds the CLI command `stub` (`stub main.cedarschema --server NAME[=IDENTIFIER]`).

### Changed
- The entities returned by request generation now include the action entities of the schema, so that policies using the action hierarchy (`action in ...`) work. Previously, `action in` conditions never held for generated requests unless the caller added the action entities.
- Adding a tool whose name is the name of an action of the schema stub is now an error (`ConflictingActionName`). Previously, the generated action silently replaced the action of the stub.

## [0.7.0] - 2026-10-22

### Added
- Adds `inline_empty_records` option (CLI flag `--inline-empty-records`) to encode object properties whose shape is an empty record (no `properties` and no `additionalProperties`) inline as the Cedar record type `{}`, instead of emitting a named type declaration and referencing it. An empty record has no attributes, so this does not lose modeling information; it only reduces the size of the generated schema.

### Fixed
- The request generator now encodes an object the same way the schema generator declared it. The encoding decision (inline record, record, or entity-with-tags) is made from the declared schema, not from whether the runtime value happens to carry properties or additional keys. Previously, with `inline_empty_records` or `objects_as_records`, a tool whose parameter declared `additionalProperties` (or only optional properties) was rejected by request validation when the caller supplied an empty object, because the schema declared a named type while the request emitted a record `{}`.

## [0.6.1] - 2026-09-22

### Fixed
- `SchemaGenerator` now stays in a good state even when `add_actions_from_server_description` / `add_action_from_tool_description` fails due to malformed tool descriptions.
- The `authorize` command now reports the correct diagnostic code and help text when the resource argument is missing.

## [0.6.0] - 2026-05-26

### Added
- Adds `deduplicate_entity_types` option to consolidate equivalent enum entity types (same name and variants) and leaf entity types (entities where all attributes are of base type, i.e. no nested entity) into a single definition at the lowest common ancestor namespace.

### Changed
- Translation of JSON numbers to Cedar decimals does not special case integers (represents both `10` and `10.0` as `10.0000`)

### Fixed
- Float entity IDs now use a canonical lossless representation, fixing whole-number floats missing a decimal point and extreme values producing excessively long EIDs.
- In the request generator, property names containing `::` in MCP tool inputs are now rejected. The schema generator already errored on such names; this fix makes the request generator consistent.

## [0.5.0] - 2026-05-12

### Added
- Adds the ability to convert a MCP tool request (and optionally response) to a Cedar request and entity data compliant to the generated Schema.
- Adds support for JSON Schema tuples in MCP tool schemas, which are translated to a record of projections.


### Changed
- Type arrays in MCP tool schemas now result in records of optional fields (encoding union of types), rather than records of projections (encoding tuples).

### Fixed

- Generated JSON format schemas now correctly reference common types using `EntityOrCommon` instead `Entity`.

## [0.4.0] - 2025-12-05

 - Initial release of `cedar-policy-mcp-schema-generator`.
