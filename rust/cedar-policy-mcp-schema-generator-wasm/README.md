# cedar-policy-mcp-schema-generator-wasm

WASM bindings for [cedar-policy-mcp-schema-generator](../cedar-policy-mcp-schema-generator/), exposing `SchemaGenerator` to JavaScript and TypeScript via `wasm-bindgen`.

This enables Node.js and browser environments to generate Cedar authorization schemas from MCP tool descriptions with the **exact same behavior** as the Rust implementation, including correct handling of:

- JSON `number` as `Long` or `Decimal` (configurable)
- `additionalProperties` as Cedar tagged entities
- Namespaced type deduplication for nested objects

## Usage

Install the npm package:

```bash
npm install @cedar-policy/mcp-schema-generator-wasm
```

```javascript
const { generateSchema } = require('@cedar-policy/mcp-schema-generator-wasm');

const stub = `
namespace MyServer {
    @mcp_principal
    entity User;
    @mcp_resource
    entity McpServer;
    action "call_tool" appliesTo {
        principal: [User],
        resource: [McpServer]
    };
}
`;

const tools = JSON.stringify([
  {
    name: 'read_file',
    description: 'Read a file from disk',
    inputSchema: {
      type: 'object',
      properties: { path: { type: 'string' } },
      required: ['path'],
    },
  },
]);

const result = JSON.parse(generateSchema(stub, tools));

if (result.isOk) {
  console.log(result.schema);     // Human-readable .cedarschema
  console.log(result.schemaJson); // JSON for Cedar WASM isAuthorized()
} else {
  console.error(result.error);
}
```

## API

### `generateSchema(schemaStub, toolsJson, configJson?)`

| Parameter | Type | Description |
|-----------|------|-------------|
| `schemaStub` | `string` | Cedar schema stub with `@mcp_principal` and `@mcp_resource` annotations |
| `toolsJson` | `string` | MCP tool descriptions as JSON (the `tools` array from `tools/list`) |
| `configJson` | `string?` | Optional configuration as JSON |

**Returns:** JSON string with fields:

| Field | Type | Description |
|-------|------|-------------|
| `schema` | `string \| null` | Generated Cedar schema as `.cedarschema` text |
| `schemaJson` | `string \| null` | Generated schema as JSON (for `isAuthorized()`) |
| `error` | `string \| null` | Error message if generation failed |
| `isOk` | `boolean` | Whether generation succeeded |

### Configuration

```json
{
  "includeOutputs": false,
  "objectsAsRecords": false,
  "eraseAnnotations": true,
  "flattenNamespaces": false,
  "numbersAsDecimal": false,
  "deduplicateEntityTypes": false
}
```

| Option | Default | Description |
|--------|---------|-------------|
| `includeOutputs` | `false` | Include tool output schemas in actions |
| `objectsAsRecords` | `false` | Use records instead of entities for objects without `additionalProperties` |
| `eraseAnnotations` | `true` | Remove `@mcp_*` annotations from output |
| `flattenNamespaces` | `false` | Flatten all types into a single namespace |
| `numbersAsDecimal` | `false` | Encode JSON `number` as Cedar `Decimal` instead of `Long` |
| `deduplicateEntityTypes` | `false` | Consolidate identical entity types across tools into the lowest common ancestor namespace |

### `generateRequest(schemaStub, toolsJson, inputJson, principalType, principalId, resourceType, resourceId, configJson?)`

Generates Cedar authorization request components from an MCP tool call.

| Parameter | Type | Description |
|-----------|------|-------------|
| `schemaStub` | `string` | Cedar schema stub (same as `generateSchema`) |
| `toolsJson` | `string` | MCP tool descriptions as JSON |
| `inputJson` | `string` | MCP tool call input (`{"params": {"tool": "...", "args": {...}}}`) |
| `principalType` | `string` | Cedar entity type for the principal (e.g., `"User"`) |
| `principalId` | `string` | Principal identifier (e.g., `"alice"`) |
| `resourceType` | `string` | Cedar entity type for the resource (e.g., `"McpServer"`) |
| `resourceId` | `string` | Resource identifier (e.g., `"my-server"`) |
| `configJson` | `string?` | Optional configuration as JSON |

**Returns:** JSON string with fields:

| Field | Type | Description |
|-------|------|-------------|
| `principal` | `string \| null` | Principal EntityUID (e.g., `MyServer::User::"alice"`) |
| `action` | `string \| null` | Action EntityUID (e.g., `MyServer::Action::"read_file"`) |
| `resource` | `string \| null` | Resource EntityUID (e.g., `MyServer::McpServer::"my-server"`) |
| `entitiesJson` | `string \| null` | Entities as a JSON array string |
| `error` | `string \| null` | Error message if generation failed |
| `isOk` | `boolean` | Whether generation succeeded |

**Example:**

```javascript
const { generateRequest } = require('@cedar-policy/mcp-schema-generator-wasm');

const stub = `
namespace MyServer {
    @mcp_principal
    entity User;
    @mcp_resource
    entity McpServer;
    action "call_tool" appliesTo {
        principal: [User],
        resource: [McpServer]
    };
}
`;

const tools = JSON.stringify([
  {
    name: 'read_file',
    description: 'Read a file from disk',
    inputSchema: {
      type: 'object',
      properties: { path: { type: 'string' } },
      required: ['path'],
    },
  },
]);

const input = JSON.stringify({
  params: { tool: 'read_file', args: { path: '/etc/config.yaml' } },
});

const result = JSON.parse(
  generateRequest(stub, tools, input, 'User', 'alice', 'McpServer', 'my-server')
);

if (result.isOk) {
  console.log(result.principal);    // MyServer::User::"alice"
  console.log(result.action);       // MyServer::Action::"read_file"
  console.log(result.resource);     // MyServer::McpServer::"my-server"
  console.log(result.entitiesJson); // JSON array of entities for isAuthorized()
} else {
  console.error(result.error);
}
```

## Building

```bash
# Install wasm-pack
cargo install wasm-pack

# Build for Node.js
wasm-pack build --target nodejs --scope cedar-policy

# Build for browsers
wasm-pack build --target web --scope cedar-policy
```

## npm Release

The npm package name is:

```text
@cedar-policy/mcp-schema-generator-wasm
```

The Rust crate name remains `cedar-policy-mcp-schema-generator-wasm`. During
release, the generated `wasm-pack` package metadata is normalized so the npm
package uses the shorter name above while preserving the `Cargo.toml` version.

Releases are performed manually through the
`Publish MCP schema generator WASM to npm` GitHub Actions workflow. The workflow:

1. validates that it is triggered from `main`;
2. validates a tag of the form
   `cedar-policy-mcp-schema-generator-wasm-v<MAJOR>.<MINOR>.<PATCH>`;
3. checks that the tag version matches this crate's `Cargo.toml`;
4. runs `wasm-pack build --target nodejs --scope cedar-policy`;
5. normalizes the generated npm package metadata;
6. runs `npm pack --dry-run`; and
7. computes the correct npm dist-tag; and
8. publishes with `npm publish --access public --tag <tag> --provenance`.

The workflow expects npm authentication to use npm trusted publishing through
the repository's release environment. It does not require an `NPM_TOKEN` secret.

## Relationship to the Rust Generator

This crate is a thin `wasm-bindgen` wrapper around the existing `cedar-policy-mcp-schema-generator` Rust crate. All schema generation logic, type mapping, and edge case handling is delegated to the Rust implementation. The WASM bindings add no independent logic.

## License

Apache-2.0
