# cedar-agent-policy-builder

AI agents invoke tools on behalf of users, but today there is no standard way to control *which* user can invoke *which* tool. Developers either hard-code permission checks inside each tool or skip per-tool auth entirely — authorization logic ends up scattered, hard to audit, and impossible to analyze statically.

This package generates [Cedar](https://github.com/cedar-policy/cedar) policies from declarative configuration — roles, argument restrictions, rate limits, time windows, environment rules, and consent gates — so you can express authorization as a standalone artifact without writing Cedar syntax by hand.

Integrates with [`@cedar-policy/mcp-schema-generator-wasm`](https://www.npmjs.com/package/@cedar-policy/mcp-schema-generator-wasm) for schema generation and [`@cedar-policy/cedar-wasm`](https://www.npmjs.com/package/@cedar-policy/cedar-wasm) for policy evaluation.

## Usage

### Builder API

```typescript
import { CedarAgentPolicyBuilder } from 'cedar-agent-policy-builder'
import { allTools } from './tools' // your tool definitions

const { policies, entities, schema } = new CedarAgentPolicyBuilder()
  // Identity: resolve principal from invocationState.user_id as type "User"
  .principal({ key: 'user_id', type: 'User' })
  // Roles: admins can use all tools, analysts can only use search and query_database
  .role('admin', ['*'])
  .role('analyst', ['search', 'query_database'])
  // Restriction: analysts can only query the analytics or reporting databases
  .restrict('query_database', { allowedValues: { database: ['analytics', 'reporting'] } })
  // Rate limit: max 3 send_email calls per session
  .rateLimit('send_email', 3)
  // Time window: tools only allowed between 9am-5pm UTC
  .timeWindow({ hourStart: 9, hourEnd: 17 })
  // Environment denial: deny delete_record in production
  .denyToolsInEnv('production', ['delete_record'])
  // Consent: send_email and delete_file require human approval before executing
  .consent(['send_email', 'delete_file'])
  // Tools: pass tool specs for Cedar schema generation (enables build-time validation)
  .tools(allTools.map(t => t.spec))
  .build()
```

<details>
<summary><strong>Generated output</strong></summary>

The above produces:

**`policies`** — Cedar permit/forbid rules:

```cedar
permit(
  principal is User,
  action,
  resource
) when { principal.role == "admin" };

permit(
  principal is User,
  action == Action::"search",
  resource
) when { principal.role == "analyst" };

permit(
  principal is User,
  action == Action::"query_database",
  resource
) when { principal.role == "analyst" };

forbid(
  principal,
  action == Action::"query_database",
  resource
) when {
  !(context.input has "database" && (context.input.database == "analytics" || context.input.database == "reporting"))
};

forbid(
  principal,
  action == Action::"send_email",
  resource
) when { context.session has "call_count" && context.session.call_count >= 3 };

forbid(
  principal,
  action,
  resource
) when { context.session has "hour_utc" && (context.session.hour_utc < 9 || context.session.hour_utc >= 17) };

forbid(
  principal,
  action == Action::"delete_record",
  resource
) when { context.session has "environment" && context.session.environment == "production" };

permit(
  principal is User,
  action == Action::"send_email",
  resource
) when { context.session has "user_consent" && context.session.user_consent == true };

permit(
  principal is User,
  action == Action::"delete_file",
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
  entity User;

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

### `.tools()` — framework-agnostic

The `.tools()` method accepts any array of `{ name, inputSchema }` objects — the standard MCP tool definition format:

```typescript
// From Strands tool specs (shown above):
.tools(allTools.map(t => t.spec))

// From any MCP server's list_tools response:
.tools(mcpServer.listTools().tools)

// From OpenAI Agents:
.tools(myTools.map(t => ({ name: t.name, inputSchema: t.parameters })))

// Or defined manually:
.tools([
  { name: 'search', inputSchema: { type: 'object', properties: { query: { type: 'string' } }, required: ['query'] } },
])
```

### Schema generation and validation

When `.tools()` is provided, `build()` generates a Cedar schema from the tool input schemas via `@cedar-policy/mcp-schema-generator-wasm`. This schema can be used to validate policies at build time:

```typescript
import { CedarAgentPolicyBuilder } from 'cedar-agent-policy-builder'
import { validate } from '@cedar-policy/cedar-wasm'

const { policies, schema } = new CedarAgentPolicyBuilder()
  .role('analyst', ['search'])
  .tools([{ name: 'search', inputSchema: { type: 'object', properties: { query: { type: 'string' } }, required: ['query'] } }])
  .build()

const result = validate(policies, schema)
// Catches typos in action names, wrong context field references, etc.
```

```
.tools() definitions ──────────┐
                               ├──► @cedar-policy/mcp-schema-generator-wasm ──► Cedar schema
.principal()/.resource() ──────┘                                                     │
                                                                                     ▼
.role()/.restrict()/.consent() ──► Policy generator ──► Cedar policies ──► validate(policies, schema)
                                                                            catches typos/type errors
                                                                            at build time
```

## API Reference

### Builder methods

| Method | Description |
|--------|-------------|
| `.principal({ key, type? })` | Set identity resolution. Default: `{ key: 'user_id', type: 'User' }` |
| `.role(name, tools)` | Grant a role access to tools. `['*']` = all tools. |
| `.restrict(tool, { allowedValues })` | Restrict tool arguments to specific values. Empty `{}` = deny tool entirely. |
| `.rateLimit(tool, max)` | Max calls per session. `0` = always denied. |
| `.timeWindow({ hourStart, hourEnd })` | Allow tools only during UTC hours. `start == end` = deny all. |
| `.denyToolsInEnv(env, tools?)` | Deny tools in an environment. No `tools` arg = deny all tools. |
| `.consent(tools, forRole?)` | Require human consent. No `forRole` = all roles need consent. |
| `.resource({ type, id })` | Custom resource entity. Default: `McpServer::"default"`. |
| `.tools(definitions)` | MCP tool definitions for schema generation. |
| `.namespace(ns)` | Cedar namespace. Default: `"Agent"`. |
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
  consent: { send_email: ['*'], delete_file: ['*'] },
})
```

See `CedarAgentConfig` type for the full shape.

## How it works

The builder generates Cedar policies following the [cedar-for-agents](https://github.com/cedar-policy/cedar-for-agents) MCP schema generator conventions:

- **Actions** are named directly after tools (e.g. `Action::"search"`)
- **Context** is nested: `context.input.*` for tool arguments, `context.session.*` for runtime state
- **Principals** are entity types with a `role` attribute
- **Resource** defaults to `McpServer::"default"`
- **Default-deny** — no permit = denied

All `context.session.*` field accesses include `has` guards for safety (Cedar errors on missing fields rather than returning false).

## License

Apache-2.0
