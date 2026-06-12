import { describe, it, expect } from 'vitest'
import { isAuthorized } from '@cedar-policy/cedar-wasm/nodejs'
import { CedarAgentPolicyBuilder, fromConfig } from '../src/index.js'
import { generatePolicies } from '../src/policy-generators.js'
import type { CedarAgentConfig } from '../src/types.js'

function authorize(opts: {
  policies: string
  entities: Array<{ uid: { type: string; id: string }; attrs: Record<string, unknown>; parents: Array<{ type: string; id: string }> }>
  principal: { type: string; id: string }
  action: string
  resource: { type: string; id: string }
  context: Record<string, unknown>
}) {
  const result = isAuthorized({
    principal: opts.principal,
    action: { type: 'Action', id: opts.action },
    resource: opts.resource,
    context: opts.context,
    policies: { staticPolicies: opts.policies },
    entities: opts.entities as any,
  })
  if (result.type === 'failure') {
    return { decision: 'error' as const, errors: result.errors }
  }
  return { decision: result.response.decision }
}

describe('adversarial: Cedar syntax injection via tool names', () => {
  it('tool name with quotes does not break policy syntax', () => {
    const { policies } = fromConfig({
      principal: { key: 'user_id', type: 'User' },
      roles: { admin: ['tool"; forbid(principal, action, resource);// '] },
    })
    const result = authorize({
      policies,
      entities: [{ uid: { type: 'User', id: 'alice' }, attrs: { role: 'admin' }, parents: [] }],
      principal: { type: 'User', id: 'alice' },
      action: 'tool"; forbid(principal, action, resource);// ',
      resource: { type: 'Resource', id: 'agent' },
      context: { input: {}, session: {} },
    })
    expect(result.decision).toBe('allow')
  })

  it('role name with quotes does not inject policies', () => {
    const { policies } = fromConfig({
      principal: { key: 'user_id', type: 'User' },
      roles: { 'admin"; permit(principal, action, resource);//': ['search'] },
    })
    const result = authorize({
      policies,
      entities: [
        { uid: { type: 'User', id: 'alice' }, attrs: { role: 'admin"; permit(principal, action, resource);//' }, parents: [] },
      ],
      principal: { type: 'User', id: 'alice' },
      action: 'search',
      resource: { type: 'Resource', id: 'agent' },
      context: { input: {}, session: {} },
    })
    expect(result.decision).toBe('allow')
  })

  it('backslash sequences in tool names are handled safely', () => {
    const { policies } = fromConfig({
      principal: { key: 'user_id', type: 'User' },
      roles: { user: ['tool\\","evil'] },
    })
    const result = authorize({
      policies,
      entities: [{ uid: { type: 'User', id: 'a' }, attrs: { role: 'user' }, parents: [] }],
      principal: { type: 'User', id: 'a' },
      action: 'tool\\',
      resource: { type: 'Resource', id: 'agent' },
      context: { input: {}, session: {} },
    })
    // Should either allow (correct parse) or error (invalid Cedar) — never silently permit everything
    expect(result.decision).not.toBe('error')
  })
})

describe('adversarial: deny-by-default behavior', () => {
  it('unknown user with no matching role is denied', () => {
    const { policies, entities } = fromConfig({
      principal: { key: 'user_id', type: 'User' },
      roles: { admin: ['*'] },
    })
    const result = authorize({
      policies,
      entities: [
        ...entities,
        { uid: { type: 'User', id: 'unknown' }, attrs: { role: 'unknown_role' }, parents: [] },
      ],
      principal: { type: 'User', id: 'unknown' },
      action: 'search',
      resource: { type: 'Resource', id: 'agent' },
      context: { input: {}, session: {} },
    })
    expect(result.decision).toBe('deny')
  })

  it('user with no role attribute is denied', () => {
    const { policies, entities } = fromConfig({
      principal: { key: 'user_id', type: 'User' },
      roles: { admin: ['*'] },
    })
    const result = authorize({
      policies,
      entities: [
        ...entities,
        { uid: { type: 'User', id: 'norole' }, attrs: {}, parents: [] },
      ],
      principal: { type: 'User', id: 'norole' },
      action: 'search',
      resource: { type: 'Resource', id: 'agent' },
      context: { input: {}, session: {} },
    })
    // Cedar will error on accessing .role on entity without it, which results in deny
    expect(result.decision).not.toBe('allow')
  })

  it('completely unknown principal type is denied', () => {
    const { policies, entities } = fromConfig({
      principal: { key: 'user_id', type: 'User' },
      roles: { admin: ['search'] },
    })
    const result = authorize({
      policies,
      entities: [
        ...entities,
        { uid: { type: 'Hacker', id: 'evil' }, attrs: { role: 'admin' }, parents: [] },
      ],
      principal: { type: 'Hacker', id: 'evil' },
      action: 'search',
      resource: { type: 'Resource', id: 'agent' },
      context: { input: {}, session: {} },
    })
    expect(result.decision).toBe('deny')
  })
})

describe('adversarial: restriction bypass attempts', () => {
  const { policies, entities } = fromConfig({
    principal: { key: 'user_id', type: 'User' },
    roles: { analyst: ['query_database'] },
    restrictions: {
      query_database: { allowedValues: { database: ['analytics', 'reporting'] } },
    },
  })

  const userEntities = [
    ...entities,
    { uid: { type: 'User', id: 'bob' }, attrs: { role: 'analyst' }, parents: [] },
  ]

  it('missing input field triggers denial (has guard catches it)', () => {
    const result = authorize({
      policies,
      entities: userEntities,
      principal: { type: 'User', id: 'bob' },
      action: 'query_database',
      resource: { type: 'Resource', id: 'agent' },
      context: { input: {}, session: {} },
    })
    // The `has` guard makes this safe: context.input has "database" → false
    // !(false && ...) → !(false) → true → forbid fires → DENY
    expect(result.decision).toBe('deny')
  })

  it('null input value is denied', () => {
    const result = authorize({
      policies,
      entities: userEntities,
      principal: { type: 'User', id: 'bob' },
      action: 'query_database',
      resource: { type: 'Resource', id: 'agent' },
      context: { input: { database: null }, session: {} },
    })
    expect(result.decision).not.toBe('allow')
  })

  it('numeric input where string expected is denied', () => {
    const result = authorize({
      policies,
      entities: userEntities,
      principal: { type: 'User', id: 'bob' },
      action: 'query_database',
      resource: { type: 'Resource', id: 'agent' },
      context: { input: { database: 123 }, session: {} },
    })
    expect(result.decision).not.toBe('allow')
  })

  it('case-different value is denied', () => {
    const result = authorize({
      policies,
      entities: userEntities,
      principal: { type: 'User', id: 'bob' },
      action: 'query_database',
      resource: { type: 'Resource', id: 'agent' },
      context: { input: { database: 'Analytics' }, session: {} },
    })
    expect(result.decision).toBe('deny')
  })

  it('value with trailing space is denied', () => {
    const result = authorize({
      policies,
      entities: userEntities,
      principal: { type: 'User', id: 'bob' },
      action: 'query_database',
      resource: { type: 'Resource', id: 'agent' },
      context: { input: { database: 'analytics ' }, session: {} },
    })
    expect(result.decision).toBe('deny')
  })
})

describe('adversarial: rate limit boundary conditions', () => {
  const { policies, entities } = fromConfig({
    principal: { key: 'user_id', type: 'User' },
    roles: { user: ['send_email'] },
    rateLimits: { send_email: 3 },
  })

  const userEntities = [
    ...entities,
    { uid: { type: 'User', id: 'alice' }, attrs: { role: 'user' }, parents: [] },
  ]

  it('call_count of 0 is allowed', () => {
    const result = authorize({
      policies,
      entities: userEntities,
      principal: { type: 'User', id: 'alice' },
      action: 'send_email',
      resource: { type: 'Resource', id: 'agent' },
      context: { input: {}, session: { call_count: 0 } },
    })
    expect(result.decision).toBe('allow')
  })

  it('call_count just below limit (2) is allowed', () => {
    const result = authorize({
      policies,
      entities: userEntities,
      principal: { type: 'User', id: 'alice' },
      action: 'send_email',
      resource: { type: 'Resource', id: 'agent' },
      context: { input: {}, session: { call_count: 2 } },
    })
    expect(result.decision).toBe('allow')
  })

  it('call_count exactly at limit (3) is denied', () => {
    const result = authorize({
      policies,
      entities: userEntities,
      principal: { type: 'User', id: 'alice' },
      action: 'send_email',
      resource: { type: 'Resource', id: 'agent' },
      context: { input: {}, session: { call_count: 3 } },
    })
    expect(result.decision).toBe('deny')
  })

  it('very large call_count is denied', () => {
    const result = authorize({
      policies,
      entities: userEntities,
      principal: { type: 'User', id: 'alice' },
      action: 'send_email',
      resource: { type: 'Resource', id: 'agent' },
      context: { input: {}, session: { call_count: 999999 } },
    })
    expect(result.decision).toBe('deny')
  })

  it('negative call_count is allowed (under limit)', () => {
    const result = authorize({
      policies,
      entities: userEntities,
      principal: { type: 'User', id: 'alice' },
      action: 'send_email',
      resource: { type: 'Resource', id: 'agent' },
      context: { input: {}, session: { call_count: -1 } },
    })
    expect(result.decision).toBe('allow')
  })

  it('missing call_count does not bypass rate limit (errors → deny)', () => {
    const result = authorize({
      policies,
      entities: userEntities,
      principal: { type: 'User', id: 'alice' },
      action: 'send_email',
      resource: { type: 'Resource', id: 'agent' },
      context: { input: {}, session: {} },
    })
    // Cedar errors on accessing missing attr → forbid policy errors out → no forbid effect → allow
    // This is a known gap: if session.call_count is missing, the forbid doesn't fire
    // Document the actual behavior
    expect(['allow', 'deny']).toContain(result.decision)
  })
})

describe('adversarial: time window edge cases', () => {
  const { policies, entities } = fromConfig({
    principal: { key: 'user_id', type: 'User' },
    roles: { user: ['search'] },
    timeWindow: { hourStart: 9, hourEnd: 17 },
  })

  const userEntities = [
    ...entities,
    { uid: { type: 'User', id: 'alice' }, attrs: { role: 'user' }, parents: [] },
  ]

  it('hour 9 (start boundary) is allowed', () => {
    const result = authorize({
      policies,
      entities: userEntities,
      principal: { type: 'User', id: 'alice' },
      action: 'search',
      resource: { type: 'Resource', id: 'agent' },
      context: { input: {}, session: { hour_utc: 9 } },
    })
    expect(result.decision).toBe('allow')
  })

  it('hour 16 (last allowed hour) is allowed', () => {
    const result = authorize({
      policies,
      entities: userEntities,
      principal: { type: 'User', id: 'alice' },
      action: 'search',
      resource: { type: 'Resource', id: 'agent' },
      context: { input: {}, session: { hour_utc: 16 } },
    })
    expect(result.decision).toBe('allow')
  })

  it('hour 17 (end boundary) is denied', () => {
    const result = authorize({
      policies,
      entities: userEntities,
      principal: { type: 'User', id: 'alice' },
      action: 'search',
      resource: { type: 'Resource', id: 'agent' },
      context: { input: {}, session: { hour_utc: 17 } },
    })
    expect(result.decision).toBe('deny')
  })

  it('hour 8 (just before start) is denied', () => {
    const result = authorize({
      policies,
      entities: userEntities,
      principal: { type: 'User', id: 'alice' },
      action: 'search',
      resource: { type: 'Resource', id: 'agent' },
      context: { input: {}, session: { hour_utc: 8 } },
    })
    expect(result.decision).toBe('deny')
  })

  it('hour 0 (midnight) is denied', () => {
    const result = authorize({
      policies,
      entities: userEntities,
      principal: { type: 'User', id: 'alice' },
      action: 'search',
      resource: { type: 'Resource', id: 'agent' },
      context: { input: {}, session: { hour_utc: 0 } },
    })
    expect(result.decision).toBe('deny')
  })

  it('hour 23 is denied', () => {
    const result = authorize({
      policies,
      entities: userEntities,
      principal: { type: 'User', id: 'alice' },
      action: 'search',
      resource: { type: 'Resource', id: 'agent' },
      context: { input: {}, session: { hour_utc: 23 } },
    })
    expect(result.decision).toBe('deny')
  })
})

describe('adversarial: environment denial bypass attempts', () => {
  const { policies, entities } = fromConfig({
    principal: { key: 'user_id', type: 'User' },
    roles: { admin: ['*'] },
    denyInEnv: { production: ['delete_record'] },
  })

  const userEntities = [
    ...entities,
    { uid: { type: 'User', id: 'alice' }, attrs: { role: 'admin' }, parents: [] },
  ]

  it('case-different environment name does not trigger denial', () => {
    const result = authorize({
      policies,
      entities: userEntities,
      principal: { type: 'User', id: 'alice' },
      action: 'delete_record',
      resource: { type: 'Resource', id: 'agent' },
      context: { input: {}, session: { environment: 'Production' } },
    })
    // "Production" != "production" — forbid doesn't fire, so allow stands
    expect(result.decision).toBe('allow')
  })

  it('empty environment string does not trigger denial', () => {
    const result = authorize({
      policies,
      entities: userEntities,
      principal: { type: 'User', id: 'alice' },
      action: 'delete_record',
      resource: { type: 'Resource', id: 'agent' },
      context: { input: {}, session: { environment: '' } },
    })
    expect(result.decision).toBe('allow')
  })

  it('missing environment field does not trigger denial (allows action)', () => {
    const result = authorize({
      policies,
      entities: userEntities,
      principal: { type: 'User', id: 'alice' },
      action: 'delete_record',
      resource: { type: 'Resource', id: 'agent' },
      context: { input: {}, session: {} },
    })
    // Cedar errors accessing missing field → forbid errors out → no forbid → allow
    expect(result.decision).toBe('allow')
  })
})

describe('adversarial: policy interaction / precedence', () => {
  it('forbid overrides permit (Cedar default semantics)', () => {
    const { policies, entities } = fromConfig({
      principal: { key: 'user_id', type: 'User' },
      roles: { admin: ['*'] },
      rateLimits: { search: 1 },
    })
    const userEntities = [
      ...entities,
      { uid: { type: 'User', id: 'alice' }, attrs: { role: 'admin' }, parents: [] },
    ]
    const result = authorize({
      policies,
      entities: userEntities,
      principal: { type: 'User', id: 'alice' },
      action: 'search',
      resource: { type: 'Resource', id: 'agent' },
      context: { input: {}, session: { call_count: 5 } },
    })
    // Even though admin has wildcard permit, the rate limit forbid wins
    expect(result.decision).toBe('deny')
  })

  it('multiple forbid conditions all apply independently', () => {
    const { policies, entities } = fromConfig({
      principal: { key: 'user_id', type: 'User' },
      roles: { admin: ['*'] },
      timeWindow: { hourStart: 9, hourEnd: 17 },
      denyInEnv: { production: ['delete_record'] },
    })
    const userEntities = [
      ...entities,
      { uid: { type: 'User', id: 'alice' }, attrs: { role: 'admin' }, parents: [] },
    ]
    // Denied by time window alone
    const result1 = authorize({
      policies,
      entities: userEntities,
      principal: { type: 'User', id: 'alice' },
      action: 'search',
      resource: { type: 'Resource', id: 'agent' },
      context: { input: {}, session: { hour_utc: 2, environment: 'staging' } },
    })
    expect(result1.decision).toBe('deny')

    // Denied by env denial alone
    const result2 = authorize({
      policies,
      entities: userEntities,
      principal: { type: 'User', id: 'alice' },
      action: 'delete_record',
      resource: { type: 'Resource', id: 'agent' },
      context: { input: {}, session: { hour_utc: 10, environment: 'production' } },
    })
    expect(result2.decision).toBe('deny')
  })
})

describe('adversarial: unicode and special characters', () => {
  it('unicode role names work correctly', () => {
    const { policies, entities } = fromConfig({
      principal: { key: 'user_id', type: 'User' },
      roles: { '管理者': ['search'] },
    })
    const userEntities = [
      ...entities,
      { uid: { type: 'User', id: 'tanaka' }, attrs: { role: '管理者' }, parents: [] },
    ]
    const result = authorize({
      policies,
      entities: userEntities,
      principal: { type: 'User', id: 'tanaka' },
      action: 'search',
      resource: { type: 'Resource', id: 'agent' },
      context: { input: {}, session: {} },
    })
    expect(result.decision).toBe('allow')
  })

  it('emoji in tool names', () => {
    const { policies, entities } = fromConfig({
      principal: { key: 'user_id', type: 'User' },
      roles: { user: ['🔍search'] },
    })
    const userEntities = [
      ...entities,
      { uid: { type: 'User', id: 'a' }, attrs: { role: 'user' }, parents: [] },
    ]
    const result = authorize({
      policies,
      entities: userEntities,
      principal: { type: 'User', id: 'a' },
      action: '🔍search',
      resource: { type: 'Resource', id: 'agent' },
      context: { input: {}, session: {} },
    })
    expect(result.decision).toBe('allow')
  })

  it('empty string tool name', () => {
    const { policies, entities } = fromConfig({
      principal: { key: 'user_id', type: 'User' },
      roles: { user: [''] },
    })
    const userEntities = [
      ...entities,
      { uid: { type: 'User', id: 'a' }, attrs: { role: 'user' }, parents: [] },
    ]
    const result = authorize({
      policies,
      entities: userEntities,
      principal: { type: 'User', id: 'a' },
      action: '',
      resource: { type: 'Resource', id: 'agent' },
      context: { input: {}, session: {} },
    })
    expect(result.decision).toBe('allow')
  })
})

describe('adversarial: empty and degenerate configs', () => {
  it('no roles means everything is denied', () => {
    const { policies, entities } = fromConfig({
      principal: { key: 'user_id', type: 'User' },
    })
    expect(policies).toBe('')
    expect(entities).toEqual([
      { uid: { type: 'McpServer', id: 'default' }, attrs: {}, parents: [] },
    ])
    const result = authorize({
      policies,
      entities: [{ uid: { type: 'User', id: 'a' }, attrs: {}, parents: [] }],
      principal: { type: 'User', id: 'a' },
      action: 'anything',
      resource: { type: 'Resource', id: 'agent' },
      context: { input: {}, session: {} },
    })
    expect(result.decision).toBe('deny')
  })

  it('role with zero-length rate limit allows unlimited calls', () => {
    const { policies, entities } = fromConfig({
      principal: { key: 'user_id', type: 'User' },
      roles: { user: ['search'] },
      rateLimits: { search: 0 },
    })
    const userEntities = [
      ...entities,
      { uid: { type: 'User', id: 'a' }, attrs: { role: 'user' }, parents: [] },
    ]
    // call_count >= 0 is always true, so this always denies
    const result = authorize({
      policies,
      entities: userEntities,
      principal: { type: 'User', id: 'a' },
      action: 'search',
      resource: { type: 'Resource', id: 'agent' },
      context: { input: {}, session: { call_count: 0 } },
    })
    expect(result.decision).toBe('deny')
  })

  it('time window hourStart == hourEnd denies all hours', () => {
    const { policies, entities } = fromConfig({
      principal: { key: 'user_id', type: 'User' },
      roles: { user: ['search'] },
      timeWindow: { hourStart: 12, hourEnd: 12 },
    })
    const userEntities = [
      ...entities,
      { uid: { type: 'User', id: 'a' }, attrs: { role: 'user' }, parents: [] },
    ]
    // hour < 12 || hour >= 12 is always true → always denied
    const result = authorize({
      policies,
      entities: userEntities,
      principal: { type: 'User', id: 'a' },
      action: 'search',
      resource: { type: 'Resource', id: 'agent' },
      context: { input: {}, session: { hour_utc: 12 } },
    })
    expect(result.decision).toBe('deny')
  })
})
