import { describe, it, expect } from 'vitest'
import { fromConfig } from '../src/builder.js'

describe('fromConfig', () => {
  it('produces expected output for the full example from the brief', () => {
    const { policies, entities } = fromConfig({
      principal: { key: 'user_id', type: 'User' },
      roles: { admin: ['*'], analyst: ['search', 'query_database'] },
      restrictions: { query_database: { allowedValues: { database: ['analytics', 'reporting'] } } },
      rateLimits: { send_email: 3 },
      timeWindow: { hourStart: 9, hourEnd: 17 },
      denyInEnv: { production: ['delete_record'] },
    })

    expect(policies).toContain(
      `permit(principal in Agent::Role::"admin", action, resource);`
    )
    expect(policies).toContain(
      `permit(principal in Agent::Role::"analyst", action == Agent::Action::"search", resource);`
    )
    expect(policies).toContain(
      `permit(principal in Agent::Role::"analyst", action == Agent::Action::"query_database", resource);`
    )
    expect(policies).toContain(
      `forbid(\n  principal,\n  action == Agent::Action::"query_database",\n  resource\n) when {\n  !(context.input has "database" && (context.input.database == "analytics" || context.input.database == "reporting"))\n};`
    )
    expect(policies).toContain(
      `forbid(\n  principal,\n  action == Agent::Action::"send_email",\n  resource\n) when { context.session has "call_count" && context.session.call_count >= 3 };`
    )
    expect(policies).toContain(
      `forbid(\n  principal,\n  action,\n  resource\n) when { context.session has "hour_utc" && (context.session.hour_utc < 9 || context.session.hour_utc >= 17) };`
    )
    expect(policies).toContain(
      `forbid(\n  principal,\n  action == Agent::Action::"delete_record",\n  resource\n) when { context.session has "environment" && context.session.environment == "production" };`
    )

    expect(entities).toEqual([
      { uid: { type: 'Agent::Role', id: 'admin' }, attrs: {}, parents: [] },
      { uid: { type: 'Agent::Role', id: 'analyst' }, attrs: {}, parents: [] },
      { uid: { type: 'Agent::McpServer', id: 'default' }, attrs: {}, parents: [] },
    ])
  })
})
