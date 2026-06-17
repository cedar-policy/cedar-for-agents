# cedar-agent-policy-builder

AI agents invoke tools on behalf of users, but today there is no standard way to control *which* user can invoke *which* tool. Developers either hard-code permission checks inside each tool or skip per-tool auth entirely — authorization logic ends up scattered, hard to audit, and impossible to analyze statically.

This package generates [Cedar](https://github.com/cedar-policy/cedar) policies from declarative configuration — roles, argument restrictions, rate limits, time windows, environment rules, and consent gates — so you can express authorization as a standalone artifact without writing Cedar syntax by hand.

Integrates with [`@cedar-policy/mcp-schema-generator-wasm`](https://www.npmjs.com/package/@cedar-policy/mcp-schema-generator-wasm) for schema generation and [`@cedar-policy/cedar-wasm`](https://www.npmjs.com/package/@cedar-policy/cedar-wasm) for policy evaluation.

## Usage

### Builder API

```typescript
import { CedarAgentPolicyBuilder } from 'cedar-agent-policy-builder'
import { allTools } from './tools' // your tool definitions

const { policies, entities, schema } = new CedarAgentPolicyBuilder({
  // Schema configuration — defines types and structure
  principal: { key: 'user_id', type: 'User' },
  tools: allTools.map(t => t.spec), // MCP tool definitions for schema generation
})
  // Policy configuration — defines who can do what
  .role('admin', ['*'])
  .role('analyst', ['search', 'query_database'])
  .restrict('query_database', { allowedValues: { database: ['analytics', 'reporting'] } })
  .rateLimit('send_email', 3)
  .timeWindow({ hourStart: 9, hourEnd: 17 })
  .denyToolsInEnv('production', ['delete_record'])
  .consent(['send_email', 'delete_file'])
  .build()
```

<details>
<summary><strong>Generated output</strong></summary>

The above produces:

**`policies`** — Cedar permit/forbid rules:

```cedar
permit(
  principal is Agent::User,
  action,
  resource
) when { principal.role == "admin" };

permit(
  principal is Agent::User,
  action == Agent::Action::"search",
  resource
) when { principal.role == "analyst" };

permit(
  principal is Agent::User,
  action == Agent::Action::"query_database",
  resource
) when { principal.role == "analyst" };

forbid(
  principal,
  action == Agent::Action::"query_database",
  resource
) when {
  !(context.input has "database" && (context.input.database == "analytics" || context.input.database == "reporting"))
};

forbid(
  principal,
  action == Agent::Action::"send_email",
  resource
) when { context.session has "call_count" && context.session.call_count >= 3 };

forbid(
  principal,
  action,
  resource
) when { context.session has "hour_utc" && (context.session.hour_utc < 9 || context.session.hour_utc >= 17) };

forbid(
  principal,
  action == Agent::Action::"delete_record",
  resource
) when { context.session has "environment" && context.session.environment == "production" };

permit(
  principal is Agent::User,
  action == Agent::Action::"send_email",
  resource
) when { context.session has "user_consent" && context.session.user_consent == true };

permit(
  principal is Agent::User,
  action == Agent::Action::"delete_file",
  resource
) when { context.session has "user_consent" && context.session.user_consent == true };
```

**`entities`** — Cedar entity data:

```json
[
  { "uid": { "type": "Role", "id": "admin" }, "attrs": {}, "parents": [] },
  { "uid": { "type": "Role", "id": "analyst" }, "attrs": {}, "parents": [] },
  { "uid": { "type": "McpServer", "id": "default" }, "attrs": {}, "parents": [] }
]
```

**`schema`** — Cedar schema (generated from `.tools()` via the MCP schema generator):

```cedarschema
namespace Agent {
  type searchInput = {
    query: String
  };

  type query_databaseInput = {
    database: String,
    query: String
  };

  entity McpServer;

  entity User = {
    role: String,
  };

  action "search" appliesTo {
    principal: [User],
    resource: [McpServer],
    context: { input: searchInput }
  };

  action "query_database" appliesTo {
    principal: [User],
    resource: [McpServer],
    context: { input: query_databaseInput }
  };
}
```

</details>

### `tools` — framework-agnostic

The `tools` constructor option accepts any array of `{ name, inputSchema }` objects — the standard MCP tool definition format:

```typescript
// From Strands tool specs:
new CedarAgentPolicyBuilder({ tools: allTools.map(t => t.spec) })

// From any MCP server's list_tools response:
new CedarAgentPolicyBuilder({ tools: mcpServer.listTools().tools })

// From OpenAI Agents:
new CedarAgentPolicyBuilder({ tools: myTools.map(t => ({ name: t.name, inputSchema: t.parameters })) })

// Or defined manually:
new CedarAgentPolicyBuilder({ tools: [{ name: 'search', inputSchema: { type: 'object', properties: { query: { type: 'string' } }, required: ['query'] } }] })
```

### Schema generation and validation

When `tools` is provided, `build()` generates a Cedar schema from the tool input schemas via `@cedar-policy/mcp-schema-generator-wasm`. This schema can be used to validate policies at build time:

```typescript
import { CedarAgentPolicyBuilder } from 'cedar-agent-policy-builder'
import { validate } from '@cedar-policy/cedar-wasm'

const { policies, schema } = new CedarAgentPolicyBuilder({
  tools: [{ name: 'search', inputSchema: { type: 'object', properties: { query: { type: 'string' } }, required: ['query'] } }],
})
  .role('analyst', ['search'])
  .build()

const result = validate(policies, schema)
// Catches typos in action names, wrong context field references, etc.
```

```
constructor { tools } ─────────┐
                               ├──► @cedar-policy/mcp-schema-generator-wasm ──► Cedar schema
constructor { principal } ─────┘                                                     │
                                                                                     ▼
.role()/.restrict()/.consent() ──► Policy generator ──► Cedar policies ──► validate(policies, schema)
                                                                            catches typos/type errors
                                                                            at build time
```

## API Reference

### Constructor (schema configuration)

| Option | Description |
|--------|-------------|
| `principal` | Identity resolution. Default: `{ key: 'user_id', type: 'User' }` |
| `resource` | Custom resource entity. Default: `McpServer::"default"`. |
| `tools` | MCP tool definitions for schema generation. |
| `namespace` | Cedar namespace. Default: `"Agent"`. |

### Policy methods

| Method | Description |
|--------|-------------|
| `.role(name, tools)` | Grant a role access to tools. `['*']` = all tools. |
| `.restrict(tool, { allowedValues })` | Restrict tool arguments to specific values. Empty `{}` = deny tool entirely. |
| `.rateLimit(tool, max)` | Max calls per session. `0` = always denied. |
| `.timeWindow({ hourStart, hourEnd })` | Allow tools only during UTC hours. `start == end` = deny all. |
| `.denyToolsInEnv(env, tools?)` | Deny tools in an environment. No `tools` arg = deny all tools. |
| `.consent(tools, forRole?)` | Require human consent. No `forRole` = all roles need consent. |
| `.build()` | Generate `{ policies, entities, schema? }`. |

### `fromConfig(config)`

Same as the builder but from a plain object. Useful for loading authorization config from a JSON file or environment-specific config:

```typescript
import { fromConfig } from 'cedar-agent-policy-builder'

const { policies, entities } = fromConfig({
  principal: { key: 'user_id', type: 'User' },
  roles: { admin: ['*'], analyst: ['search', 'query_database'] },
  restrictions: { query_database: { allowedValues: { database: ['analytics', 'reporting'] } } },
  rateLimits: { send_email: 3 },
  timeWindow: { hourStart: 9, hourEnd: 17 },
  denyInEnv: { production: ['delete_record'] },
  consent: { send_email: true, delete_file: true },
})
```

See `CedarAgentConfig` type for the full shape.

## How it works

The builder generates Cedar policies following the [cedar-for-agents](https://github.com/cedar-policy/cedar-for-agents) MCP schema generator conventions:

- **Actions** are named directly after tools (e.g. `Agent::Action::"search"`)
- **Context** is nested: `context.input.*` for tool arguments, `context.session.*` for runtime state
- **Principals** are entity types with a `role` attribute
- **Resource** defaults to `McpServer::"default"`
- **Default-deny** — no permit = denied

All `context.session.*` field accesses include `has` guards for safety (Cedar errors on missing fields rather than returning false).

## License

Apache-2.0
