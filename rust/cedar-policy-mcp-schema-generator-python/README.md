# Cedar MCP Schema Generator - Python Bindings

Python bindings for the [Cedar MCP Schema Generator](../cedar-policy-mcp-schema-generator/),
enabling Python environments to generate Cedar authorization schemas from MCP tool descriptions.

## Installation

Requires Python 3.9+, a Rust toolchain (1.90+), and [maturin](https://www.maturin.rs/):

```bash
pip install maturin
cd rust/cedar-policy-mcp-schema-generator-python
maturin develop
```

## Usage

### Generate a Cedar schema from MCP tools

```python
from cedar_mcp_schema_generator import generate_schema

schema_stub = """
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
"""

tools = [
    {
        "name": "read_file",
        "description": "Read a file from disk",
        "inputSchema": {
            "type": "object",
            "properties": {"path": {"type": "string"}},
            "required": ["path"],
        },
    }
]

result = generate_schema(schema_stub, tools)
if result["isOk"]:
    print(result["schema"])       # Human-readable .cedarschema format
    print(result["schemaJson"])   # JSON format for Cedar evaluation
else:
    print(f"Error: {result['error']}")
```

### Generate a Cedar authorization request

```python
from cedar_mcp_schema_generator import generate_request

result = generate_request(
    schema_stub,
    tools,
    {"params": {"tool": "read_file", "args": {"path": "/etc/hosts"}}},
    principal_type="User",
    principal_id="alice",
    resource_type="McpServer",
    resource_id="my-server",
)

if result["isOk"]:
    print(result["principal"])    # e.g., MyServer::User::"alice"
    print(result["action"])       # e.g., MyServer::Action::"read_file"
    print(result["resource"])     # e.g., MyServer::McpServer::"my-server"
    print(result["entitiesJson"]) # JSON array of entities
```

### Configuration

Both functions accept an optional `config` dict:

```python
result = generate_schema(
    schema_stub,
    tools,
    config={
        "includeOutputs": False,         # Include tool outputs in schema context
        "objectsAsRecords": False,       # Encode objects as Cedar records
        "eraseAnnotations": True,        # Remove mcp annotations from output
        "flattenNamespaces": False,      # Flatten nested namespaces
        "numbersAsDecimal": False,       # Encode numbers as Cedar decimals
        "deduplicateEntityTypes": False, # Deduplicate equivalent enum types
    },
)
```

### Exception-raising variants

For convenience, `_or_raise` variants raise on failure instead of returning error dicts:

```python
from cedar_mcp_schema_generator import generate_schema_or_raise, SchemaGeneratorError

try:
    result = generate_schema_or_raise(schema_stub, tools)
    print(result["schema"])
except SchemaGeneratorError as e:
    print(f"Failed: {e}")
```

## API Reference

### `generate_schema(schema_stub, tools, *, config=None) -> dict`

Returns `{"isOk": bool, "schema": str|None, "schemaJson": str|None, "error": str|None}`.

### `generate_request(schema_stub, tools, input, *, principal_type, principal_id, resource_type, resource_id, config=None) -> dict`

Returns `{"isOk": bool, "principal": str|None, "action": str|None, "resource": str|None, "entitiesJson": str|None, "error": str|None}`.

## License

Apache-2.0
