import { describe, it, expect } from 'vitest'
import { generatePolicies, escapeCedarString } from '../src/policy-generators.js'
import type { CedarAgentConfig } from '../src/types.js'

describe('escapeCedarString', () => {
  it('escapes backslashes and quotes', () => {
    expect(escapeCedarString('hello "world"')).toBe('hello \\"world\\"')
    expect(escapeCedarString('back\\slash')).toBe('back\\\\slash')
  })

  it('leaves plain strings unchanged', () => {
    expect(escapeCedarString('admin')).toBe('admin')
  })
})

describe('generatePolicies', () => {
  describe('role policies', () => {
    it('generates permit for specific tools', () => {
      const config: CedarAgentConfig = {
        principal: { key: 'user_id', type: 'User' },
        roles: { analyst: ['search', 'query_database'] },
      }
      const policies = generatePolicies(config)
      expect(policies).toContain(
        `permit(\n  principal is Agent::User,\n  action == Agent::Action::"search",\n  resource\n) when { principal.role == "analyst" };`
      )
      expect(policies).toContain(
        `permit(\n  principal is Agent::User,\n  action == Agent::Action::"query_database",\n  resource\n) when { principal.role == "analyst" };`
      )
    })

    it('generates wildcard permit for admin role', () => {
      const config: CedarAgentConfig = {
        principal: { key: 'user_id', type: 'User' },
        roles: { admin: ['*'] },
      }
      const policies = generatePolicies(config)
      expect(policies).toContain(
        `permit(\n  principal is Agent::User,\n  action,\n  resource\n) when { principal.role == "admin" };`
      )
    })

    it('defaults principal type to User', () => {
      const config: CedarAgentConfig = {
        principal: { key: 'user_id' },
        roles: { viewer: ['read'] },
      }
      const policies = generatePolicies(config)
      expect(policies).toContain('principal is Agent::User')
    })

    it('uses custom principal type', () => {
      const config: CedarAgentConfig = {
        principal: { key: 'agent_id', type: 'Agent' },
        roles: { viewer: ['read'] },
      }
      const policies = generatePolicies(config)
      expect(policies).toContain('principal is Agent::Agent')
    })
  })

  describe('restriction policies', () => {
    it('generates forbid with allowed values check', () => {
      const config: CedarAgentConfig = {
        principal: { key: 'user_id' },
        restrictions: {
          query_database: { allowedValues: { database: ['analytics', 'reporting'] } },
        },
      }
      const policies = generatePolicies(config)
      expect(policies).toContain(
        `forbid(\n  principal,\n  action == Agent::Action::"query_database",\n  resource\n) when {\n  !(context.input has "database" && (context.input.database == "analytics" || context.input.database == "reporting"))\n};`
      )
    })

    it('handles single allowed value', () => {
      const config: CedarAgentConfig = {
        principal: { key: 'user_id' },
        restrictions: {
          delete_file: { allowedValues: { path: ['/tmp'] } },
        },
      }
      const policies = generatePolicies(config)
      expect(policies).toContain(
        `forbid(\n  principal,\n  action == Agent::Action::"delete_file",\n  resource\n) when {\n  !(context.input has "path" && (context.input.path == "/tmp"))\n};`
      )
    })
  })

  describe('rate limit policies', () => {
    it('generates forbid with call_count check', () => {
      const config: CedarAgentConfig = {
        principal: { key: 'user_id' },
        rateLimits: { send_email: 3 },
      }
      const policies = generatePolicies(config)
      expect(policies).toContain(
        `forbid(\n  principal,\n  action == Agent::Action::"send_email",\n  resource\n) when { context.session has "call_count" && context.session.call_count >= 3 };`
      )
    })
  })

  describe('time window policies', () => {
    it('generates forbid with hour checks', () => {
      const config: CedarAgentConfig = {
        principal: { key: 'user_id' },
        timeWindow: { hourStart: 9, hourEnd: 17 },
      }
      const policies = generatePolicies(config)
      expect(policies).toContain(
        `forbid(\n  principal,\n  action,\n  resource\n) when { context.session has "hour_utc" && (context.session.hour_utc < 9 || context.session.hour_utc >= 17) };`
      )
    })
  })

  describe('environment denial policies', () => {
    it('generates forbid with environment check', () => {
      const config: CedarAgentConfig = {
        principal: { key: 'user_id' },
        denyInEnv: { production: ['delete_record'] },
      }
      const policies = generatePolicies(config)
      expect(policies).toContain(
        `forbid(\n  principal,\n  action == Agent::Action::"delete_record",\n  resource\n) when { context.session has "environment" && context.session.environment == "production" };`
      )
    })

    it('generates one policy per tool', () => {
      const config: CedarAgentConfig = {
        principal: { key: 'user_id' },
        denyInEnv: { production: ['delete_record', 'drop_table'] },
      }
      const policies = generatePolicies(config)
      expect(policies).toContain('Agent::Action::"delete_record"')
      expect(policies).toContain('Agent::Action::"drop_table"')
    })
  })

  describe('restriction policies - edge cases', () => {
    it('generates one forbid per field when multiple fields constrained', () => {
      const config: CedarAgentConfig = {
        principal: { key: 'user_id' },
        restrictions: {
          query_database: {
            allowedValues: {
              database: ['analytics', 'reporting'],
              schema: ['public'],
            },
          },
        },
      }
      const policies = generatePolicies(config)
      expect(policies).toContain('context.input.database == "analytics" || context.input.database == "reporting"')
      expect(policies).toContain('context.input.schema == "public"')
    })

    it('handles numeric allowed values', () => {
      const config: CedarAgentConfig = {
        principal: { key: 'user_id' },
        restrictions: {
          set_limit: { allowedValues: { limit: [10, 50, 100] } },
        },
      }
      const policies = generatePolicies(config)
      expect(policies).toContain('context.input.limit == 10 || context.input.limit == 50 || context.input.limit == 100')
    })
  })

  describe('role policies - edge cases', () => {
    it('generates nothing for a role with empty tools array', () => {
      const config: CedarAgentConfig = {
        principal: { key: 'user_id' },
        roles: { empty: [] },
      }
      const policies = generatePolicies(config)
      expect(policies).toBe('')
    })

    it('escapes special characters in role and tool names', () => {
      const config: CedarAgentConfig = {
        principal: { key: 'user_id', type: 'User' },
        roles: { 'role"evil': ['tool"inject'] },
      }
      const policies = generatePolicies(config)
      expect(policies).toContain('Agent::Action::"tool\\"inject"')
      expect(policies).toContain('principal.role == "role\\"evil"')
    })
  })

  describe('combined policies', () => {
    it('joins multiple policies with double newline', () => {
      const config: CedarAgentConfig = {
        principal: { key: 'user_id', type: 'User' },
        roles: { admin: ['*'] },
        rateLimits: { send_email: 3 },
      }
      const policies = generatePolicies(config)
      const parts = policies.split('\n\n')
      expect(parts.length).toBe(2)
    })
  })

  it('returns empty string when no policies configured', () => {
    const config: CedarAgentConfig = { principal: { key: 'user_id' } }
    expect(generatePolicies(config)).toBe('')
  })
})
