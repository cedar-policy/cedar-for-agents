import { describe, it, expect } from 'vitest'
import { CedarAgentPolicyBuilder } from '../src/builder.js'

describe('CedarAgentPolicyBuilder', () => {
  it('supports fluent chaining with schema config in constructor', () => {
    const result = new CedarAgentPolicyBuilder({
      principal: { key: 'user_id', type: 'User' },
    })
      .role('admin', ['*'])
      .role('analyst', ['search', 'query_database'])
      .restrict('query_database', { allowedValues: { database: ['analytics', 'reporting'] } })
      .rateLimit('send_email', 3)
      .timeWindow({ hourStart: 9, hourEnd: 17 })
      .denyToolsInEnv('production', ['delete_record'])
      .build()

    expect(result.policies).toContain('principal.role == "admin"')
    expect(result.policies).toContain('principal.role == "analyst"')
    expect(result.policies).toContain('Action::"query_database"')
    expect(result.policies).toContain('context.session has "call_count" && context.session.call_count >= 3')
    expect(result.policies).toContain('context.session has "hour_utc"')
    expect(result.policies).toContain('context.session has "environment" && context.session.environment == "production"')
    expect(result.entities).toHaveLength(3) // 2 roles + McpServer
  })

  it('defaults principal type to User when no constructor arg', () => {
    const result = new CedarAgentPolicyBuilder()
      .role('viewer', ['read'])
      .build()

    expect(result.policies).toContain('principal is User')
  })

  it('generates McpServer entity even with no roles', () => {
    const result = new CedarAgentPolicyBuilder().build()
    expect(result.policies).toBe('')
    expect(result.entities).toEqual([
      { uid: { type: 'McpServer', id: 'default' }, attrs: {}, parents: [] },
    ])
  })

  it('supports consent method', () => {
    const result = new CedarAgentPolicyBuilder()
      .role('developer', ['search', 'read_file'])
      .consent(['send_email', 'delete_file'])
      .build()

    expect(result.policies).toContain('context.session has "user_consent" && context.session.user_consent == true')
    expect(result.policies).toContain('Action::"send_email"')
    expect(result.policies).toContain('Action::"delete_file"')
    expect(result.policies).toContain('principal.role == "developer"')
  })

  it('supports custom resource entity via constructor', () => {
    const result = new CedarAgentPolicyBuilder({
      resource: { type: 'AgentServer', id: 'my-agent' },
    }).build()

    expect(result.entities).toContainEqual(
      { uid: { type: 'AgentServer', id: 'my-agent' }, attrs: {}, parents: [] },
    )
  })

  it('generates Cedar schema when tools are provided in constructor', () => {
    const result = new CedarAgentPolicyBuilder({
      principal: { key: 'user_id', type: 'User' },
      tools: [
        { name: 'search', inputSchema: { type: 'object', properties: { query: { type: 'string' } }, required: ['query'] } },
      ],
    })
      .role('analyst', ['search'])
      .build()

    expect(result.schema).toBeDefined()
    expect(result.schema).toContain('action "search"')
    expect(result.schema).toContain('searchInput')
    expect(result.schema).toContain('entity User')
    expect(result.schema).toContain('entity McpServer')
  })

  it('does not generate schema when no tools provided', () => {
    const result = new CedarAgentPolicyBuilder()
      .role('admin', ['*'])
      .build()

    expect(result.schema).toBeUndefined()
  })

  it('uses custom namespace from constructor for schema generation', () => {
    const result = new CedarAgentPolicyBuilder({
      namespace: 'MyService',
      tools: [
        { name: 'ping', inputSchema: { type: 'object', properties: {}, required: [] } },
      ],
    }).build()

    expect(result.schema).toContain('namespace MyService')
  })
})
