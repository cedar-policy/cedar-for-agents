/**
 * Integration examples: demonstrates how the builder's .user() API generates
 * entity hierarchy that Cedar evaluates via the `in` operator.
 *
 * These tests use the builder's own output (policies + entities) end-to-end,
 * serving as executable documentation of the two principal-resolution patterns.
 */
import { describe, it, expect } from 'vitest'
import { isAuthorized } from '@cedar-policy/cedar-wasm/nodejs'
import { CedarAgentPolicyBuilder, fromConfig } from '../src/index.js'

function authorize(opts: {
  policies: string
  entities: Array<{ uid: { type: string; id: string }; attrs: Record<string, unknown>; parents: Array<{ type: string; id: string }> }>
  principal: { type: string; id: string }
  action: string
  context?: { input?: Record<string, unknown>; session?: Record<string, unknown> }
}) {
  const result = isAuthorized({
    principal: opts.principal,
    action: { type: 'Agent::Action', id: opts.action },
    resource: { type: 'Agent::McpServer', id: 'default' },
    context: {
      input: opts.context?.input ?? {},
      session: { hour_utc: 12, call_count: 0, ...opts.context?.session },
    },
    policies: { staticPolicies: opts.policies },
    entities: opts.entities as any,
  })
  if (result.type === 'failure') return { decision: 'error' as const, errors: result.errors }
  return { decision: result.response.decision }
}

describe('integration: entity hierarchy with .user()', () => {
  const { policies, entities } = new CedarAgentPolicyBuilder({
    principal: { key: 'user_id', type: 'User' },
  })
    .role('admin', ['*'])
    .role('analyst', ['search', 'query_database'])
    .role('developer', ['deploy'])
    .user('alice', 'admin')
    .user('bob', 'analyst')
    .user('charlie', 'analyst', 'developer') // multi-role
    .build()

  it('user inherits permissions from their role', () => {
    expect(authorize({ policies, entities, principal: { type: 'Agent::User', id: 'alice' }, action: 'anything' }).decision).toBe('allow')
    expect(authorize({ policies, entities, principal: { type: 'Agent::User', id: 'bob' }, action: 'search' }).decision).toBe('allow')
  })

  it('user cannot access tools outside their role', () => {
    expect(authorize({ policies, entities, principal: { type: 'Agent::User', id: 'bob' }, action: 'deploy' }).decision).toBe('deny')
  })

  it('multi-role user inherits from all assigned roles', () => {
    expect(authorize({ policies, entities, principal: { type: 'Agent::User', id: 'charlie' }, action: 'search' }).decision).toBe('allow')
    expect(authorize({ policies, entities, principal: { type: 'Agent::User', id: 'charlie' }, action: 'deploy' }).decision).toBe('allow')
    expect(authorize({ policies, entities, principal: { type: 'Agent::User', id: 'charlie' }, action: 'anything' }).decision).toBe('deny')
  })

  it('undeclared user is denied', () => {
    expect(authorize({ policies, entities, principal: { type: 'Agent::User', id: 'mallory' }, action: 'search' }).decision).toBe('deny')
  })
})

describe('integration: role as principal (no users declared)', () => {
  const { policies, entities } = new CedarAgentPolicyBuilder()
    .role('admin', ['*'])
    .role('viewer', ['search'])
    .build()

  it('role entity satisfies principal in Role (self-membership)', () => {
    expect(authorize({ policies, entities, principal: { type: 'Agent::Role', id: 'admin' }, action: 'deploy' }).decision).toBe('allow')
    expect(authorize({ policies, entities, principal: { type: 'Agent::Role', id: 'viewer' }, action: 'search' }).decision).toBe('allow')
  })

  it('role is still constrained to its tools', () => {
    expect(authorize({ policies, entities, principal: { type: 'Agent::Role', id: 'viewer' }, action: 'deploy' }).decision).toBe('deny')
  })

  it('unknown role is denied', () => {
    expect(authorize({ policies, entities, principal: { type: 'Agent::Role', id: 'hacker' }, action: 'search' }).decision).toBe('deny')
  })
})

describe('integration: fromConfig with users', () => {
  const { policies, entities } = fromConfig({
    principal: { key: 'user_id', type: 'User' },
    roles: { admin: ['*'], viewer: ['search'] },
    users: { alice: ['admin'], bob: ['viewer'] },
  })

  it('generates correct entity hierarchy', () => {
    expect(entities).toContainEqual({
      uid: { type: 'Agent::User', id: 'alice' },
      attrs: {},
      parents: [{ type: 'Agent::Role', id: 'admin' }],
    })
    expect(entities).toContainEqual({
      uid: { type: 'Agent::User', id: 'bob' },
      attrs: {},
      parents: [{ type: 'Agent::Role', id: 'viewer' }],
    })
  })

  it('evaluates correctly through entity hierarchy', () => {
    expect(authorize({ policies, entities, principal: { type: 'Agent::User', id: 'alice' }, action: 'deploy' }).decision).toBe('allow')
    expect(authorize({ policies, entities, principal: { type: 'Agent::User', id: 'bob' }, action: 'deploy' }).decision).toBe('deny')
  })
})
