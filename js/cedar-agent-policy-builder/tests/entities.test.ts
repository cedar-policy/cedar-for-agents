import { describe, it, expect } from 'vitest'
import { generateEntities } from '../src/entities.js'
import type { CedarAgentConfig } from '../src/types.js'
import type { EntityJson } from '@cedar-policy/cedar-wasm'

describe('generateEntities', () => {
  it('generates McpServer resource entity even with no roles', () => {
    const config: CedarAgentConfig = { principal: { key: 'user_id' } }
    expect(generateEntities(config)).toEqual([
      { uid: { type: 'Agent::McpServer', id: 'default' }, attrs: {}, parents: [] },
    ])
  })

  it('generates Role entities and McpServer resource for each defined role', () => {
    const config: CedarAgentConfig = {
      principal: { key: 'user_id' },
      roles: { admin: ['*'], analyst: ['search'] },
    }
    const entities = generateEntities(config)
    expect(entities).toEqual([
      { uid: { type: 'Agent::Role', id: 'admin' }, attrs: {}, parents: [] },
      { uid: { type: 'Agent::Role', id: 'analyst' }, attrs: {}, parents: [] },
      { uid: { type: 'Agent::McpServer', id: 'default' }, attrs: {}, parents: [] },
    ])
  })

  it('uses custom resource type and id when specified', () => {
    const config: CedarAgentConfig = {
      principal: { key: 'user_id' },
      roles: { viewer: ['read'] },
      resource: { type: 'AgentServer', id: 'my-agent' },
    }
    const entities = generateEntities(config)
    expect(entities).toContainEqual(
      { uid: { type: 'Agent::AgentServer', id: 'my-agent' }, attrs: {}, parents: [] },
    )
  })
})
