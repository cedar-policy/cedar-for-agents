import type { EntityJson } from '@cedar-policy/cedar-wasm'
import type { CedarAgentConfig } from './types.js'

export function generateEntities(config: CedarAgentConfig): EntityJson[] {
  const entities: EntityJson[] = []
  const ns = config.namespace ?? 'Agent'

  if (config.roles) {
    for (const roleName of Object.keys(config.roles)) {
      entities.push({ uid: { type: `${ns}::Role`, id: roleName }, attrs: {}, parents: [] })
    }
  }

  if (config.users) {
    const principalType = config.principal.type ?? 'User'
    for (const [userId, roles] of Object.entries(config.users)) {
      entities.push({
        uid: { type: `${ns}::${principalType}`, id: userId },
        attrs: {},
        parents: roles.map((r) => ({ type: `${ns}::Role`, id: r })),
      })
    }
  }

  const resource = config.resource ?? { type: 'McpServer', id: 'default' }
  entities.push({ uid: { type: `${ns}::${resource.type}`, id: resource.id }, attrs: {}, parents: [] })

  return entities
}
