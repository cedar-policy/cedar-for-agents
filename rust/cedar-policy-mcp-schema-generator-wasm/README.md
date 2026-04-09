# cedar-policy-mcp-schema-generator-wasm

WASM bindings for [cedar-policy-mcp-schema-generator](../cedar-policy-mcp-schema-generator/), exposing `SchemaGenerator` to JavaScript and TypeScript via `wasm-bindgen`.

This enables Node.js and browser environments to generate Cedar authorization schemas from MCP tool descriptions with the **exact same behavior** as the Rust implementation, including correct handling of:

- JSON `number` as `Long` or `Decimal` (configurable)
- `additionalProperties` as Cedar tagged entities
- Namespaced type deduplication for nested objects

## Usage

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
  "numbersAsDecimal": false
}
```

| Option | Default | Description |
|--------|---------|-------------|
| `includeOutputs` | `false` | Include tool output schemas in actions |
| `objectsAsRecords` | `false` | Use records instead of entities for objects without `additionalProperties` |
| `eraseAnnotations` | `true` | Remove `@mcp_*` annotations from output |
| `flattenNamespaces` | `false` | Flatten all types into a single namespace |
| `numbersAsDecimal` | `false` | Encode JSON `number` as Cedar `Decimal` instead of `Long` |

## Building

```bash
# Install wasm-pack
cargo install wasm-pack

# Build for Node.js
wasm-pack build --target nodejs --scope cedar-policy

# Build for browsers
wasm-pack build --target web --scope cedar-policy
```

## Relationship to the Rust Generator

This crate is a thin `wasm-bindgen` wrapper around the existing `cedar-policy-mcp-schema-generator` Rust crate. All schema generation logic, type mapping, and edge case handling is delegated to the Rust implementation. The WASM bindings add no independent logic.

## License

Apache-2.0
