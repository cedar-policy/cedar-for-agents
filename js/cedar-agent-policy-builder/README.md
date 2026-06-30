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
  .role('developer', ['delete_record'])
  .user('alice', 'admin')
  .user('bob', 'analyst', 'developer')
  .restrict('query_database', { allowedValues: { database: ['analytics', 'reporting'] } })
  .rateLimit('send_email', 3)
  .timeWindow({ hourStart: 9, hourEnd: 17 })
  .denyToolsInEnv('production', ['delete_record'])
  .consent(['send_email', 'delete_file'])
  .build()
```
The policy configuration above references three roles 'admin', 'analyst' and 'developer', and attaches permissions to those roles. A user can have zero or more roles (.e.g. 'alice' is an 'admin' and 'bob' is an 'analyst' and a 'developer'). Declaring users is optional — if you omit `.user()` calls, the generated entities contain only roles and the resource, and you can resolve the principal directly as a `Role` entity (e.g. `{ type: 'Agent::Role', id: 'admin' }`) in your [Strands `principalResolver`](https://strandsagents.com/docs/user-guide/concepts/agents/interventions/cedar-authorization/). This works because Cedar's `in` operator treats an entity as being `in` itself, so `Role::"admin" in Role::"admin"` evaluates to true. When you do declare users, `build()` emits `User` entities with `Role` parents in the entity hierarchy, and your `principalResolver` returns the user identity for a full audit trail.

<details>
<summary><strong>Generated output</strong></summary>

The above produces:

**`policies`** — Cedar permit/forbid rules:

```cedar
permit(
  principal in Agent::Role::"admin",
  action,
  resource
) when { !(action == Agent::Action::"send_email" || action == Agent::Action::"delete_file") };

permit(principal in Agent::Role::"analyst", action == Agent::Action::"search", resource);

permit(principal in Agent::Role::"analyst", action == Agent::Action::"query_database", resource);

permit(principal in Agent::Role::"developer", action == Agent::Action::"delete_record", resource);

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
  principal,
  action == Agent::Action::"send_email",
  resource
) when { context.session has "user_consent" && context.session.user_consent == true };

permit(
  principal,
  action == Agent::Action::"delete_file",
  resource
) when { context.session has "user_consent" && context.session.user_consent == true };
```

**`entities`** — Cedar entity data:

```json
[
  { "uid": { "type": "Agent::Role", "id": "admin" }, "attrs": {}, "parents": [] },
  { "uid": { "type": "Agent::Role", "id": "analyst" }, "attrs": {}, "parents": [] },
  { "uid": { "type": "Agent::Role", "id": "developer" }, "attrs": {}, "parents": [] },
  { "uid": { "type": "Agent::User", "id": "alice" }, "attrs": {}, "parents": [{ "type": "Agent::Role", "id": "admin" }] },
  { "uid": { "type": "Agent::User", "id": "bob" }, "attrs": {}, "parents": [{ "type": "Agent::Role", "id": "analyst" }, { "type": "Agent::Role", "id": "developer" }] },
  { "uid": { "type": "Agent::Resource", "id": "default" }, "attrs": {}, "parents": [] }
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

  entity Resource;

  entity Role;

  entity User in [Role];

  action "search" appliesTo {
    principal: [User],
    resource: [Resource],
    context: { input: searchInput }
  };

  action "query_database" appliesTo {
    principal: [User],
    resource: [Resource],
    context: { input: query_databaseInput }
  };
}
```

</details>

### Role modeling

Roles are modeled using Cedar's entity hierarchy. The generated policies use `principal in Role::"name"`, which is satisfied by:

- A `User` entity whose parents include the role: `User::"alice"` with parent `Role::"admin"`
- The role entity itself: `Role::"admin"` (an entity is always `in` itself)

This gives you two options for how you resolve the principal at request time:

**With users** — full audit trail, the principal is the individual user:

```typescript
const { policies, entities } = new CedarAgentPolicyBuilder(...)
  .role('admin', ['*'])
  .role('analyst', ['search'])
  .user('alice', 'admin')
  .user('bob', 'analyst')
  .build()

// principalResolver returns the user identity
principalResolver: (state) => ({ type: 'Agent::User', id: state.user_id })
```

**Without users** — simpler, the principal is the role directly:

```typescript
const { policies, entities } = new CedarAgentPolicyBuilder(...)
  .role('admin', ['*'])
  .role('analyst', ['search'])
  .build()

// principalResolver returns the role
principalResolver: (state) => ({ type: 'Agent::Role', id: state.role })
```

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

## Integration with Strands Agents

The [Strands Agents SDK](https://strandsagents.com/docs/user-guide/concepts/agents/interventions/cedar-authorization/) provides `CedarAuthorization` as a vended intervention handler. There are two approaches to modeling role-based access:

### Entity hierarchy (recommended)

Use this package to generate policies and entities, then resolve the principal via `principalResolver`. Roles are expressed structurally through Cedar's entity graph:

```typescript
import { CedarAgentPolicyBuilder } from 'cedar-agent-policy-builder'
import { CedarAuthorization } from '@strands-agents/sdk/vended-interventions/cedar'

const { policies, entities } = new CedarAgentPolicyBuilder(...)
  .role('admin', ['*'])
  .role('analyst', ['search'])
  .user('alice', 'admin')
  .user('bob', 'analyst')
  .build()

const cedar = new CedarAuthorization({
  policies,
  entities,
  principalResolver: (state) => {
    if (!state.user_id) return undefined
    return { type: 'Agent::User', id: String(state.user_id) }
  },
})
```

This authorization schema has a few benefits; it allows better static analysis, you can inspect role inheritance, you have a full user audit trail, and no runtime role assertion needed.

### Runtime context (alternative)

Strands also supports passing role via `contextEnricher` into `context.session.role`. In this pattern you write policies manually against `context.session.role`:

```typescript
const cedar = new CedarAuthorization({
  policies: `
    permit(principal, action, resource)
    when { context.session.role == "admin" };
  `,
  principalResolver: (state) => ({ type: 'User', id: String(state.user_id) }),
  contextEnricher: ({ invocationState }) => ({
    role: String(invocationState.role ?? 'none'),
  }),
})
```

This is simpler but role is a pure runtime declaration — no entity graph backing, no static analysis of role membership, and role information must be passed on every request.

## API Reference

### Constructor (schema configuration)

| Option      | Description                                                      |
| ----------- | ---------------------------------------------------------------- |
| `principal` | Identity resolution. Default: `{ key: 'user_id', type: 'User' }` |
| `resource`  | Custom resource entity. Default: `Resource::"default"`.         |
| `tools`     | MCP tool definitions for schema generation.                      |
| `namespace` | Cedar namespace. Default: `"Agent"`.                             |

### Policy methods

| Method                                | Description                                                                       |
| ------------------------------------- | --------------------------------------------------------------------------------- |
| `.role(name, tools)`                  | Grant a role access to tools. `['*']` = all tools.                                |
| `.user(id, ...roles)`                 | Declare a user with one or more roles. Generates a User entity with Role parents. |
| `.restrict(tool, { allowedValues })`  | Restrict tool arguments to specific values. Empty `{}` = deny tool entirely.      |
| `.rateLimit(tool, max)`               | Max calls per session. `0` = always denied.                                       |
| `.timeWindow({ hourStart, hourEnd })` | Allow tools only during UTC hours. `start == end` = deny all.                     |
| `.denyToolsInEnv(env, tools?)`        | Deny tools in an environment. No `tools` arg = deny all tools.                    |
| `.consent(tools, forRole?)`           | Require human consent. No `forRole` = all roles need consent.                     |
| `.build()`                            | Generate `{ policies, entities, schema? }`.                                       |

### `fromConfig(config)`

Same as the builder but from a plain object. Useful for loading authorization config from a JSON file or environment-specific config:

```typescript
import { fromConfig } from 'cedar-agent-policy-builder'

const { policies, entities } = fromConfig({
  principal: { key: 'user_id', type: 'User' },
  roles: { admin: ['*'], analyst: ['search', 'query_database'] },
  users: { alice: ['admin'], bob: ['analyst'] },
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
- **Roles** are Cedar entities; principals are granted access via `principal in Role::"name"`
- **Users** are entities with Role parents in the entity hierarchy
- **Resource** defaults to `Resource::"default"`
- **Default-deny** — no permit = denied

All `context.session.*` field accesses include `has` guards for safety (Cedar errors on missing fields rather than returning false).

## License

Apache-2.0
