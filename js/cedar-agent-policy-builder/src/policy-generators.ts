import type { CedarAgentConfig } from './types.js'

export function escapeCedarString(s: string): string {
  return s.replace(/\\/g, '\\\\').replace(/"/g, '\\"')
}

function getConsentToolsForRole(config: CedarAgentConfig, roleName: string): Set<string> {
  if (!config.consent) return new Set()
  const tools = new Set<string>()
  for (const [tool, roles] of Object.entries(config.consent)) {
    // Global consent ('*') applies to all roles; role-specific consent only applies to that role
    if (roles.includes('*') || roles.includes(roleName)) {
      tools.add(tool)
    }
  }
  return tools
}

function generateRolePolicies(config: CedarAgentConfig): string[] {
  if (!config.roles) return []
  const principalType = config.principal.type ?? 'User'
  const policies: string[] = []

  for (const [roleName, tools] of Object.entries(config.roles)) {
    const consentToolsForRole = getConsentToolsForRole(config, roleName)

    if (tools.includes('*')) {
      if (consentToolsForRole.size > 0) {
        // Wildcard with consent exclusions: permit all actions EXCEPT consent-gated tools for this role
        const exclusions = [...consentToolsForRole]
          .map((t) => `action == Action::"${escapeCedarString(t)}"`)
          .join(' || ')
        policies.push(
          `permit(\n  principal is ${principalType},\n  action,\n  resource\n) when { principal.role == "${escapeCedarString(roleName)}" && !(${exclusions}) };`
        )
      } else {
        policies.push(
          `permit(\n  principal is ${principalType},\n  action,\n  resource\n) when { principal.role == "${escapeCedarString(roleName)}" };`
        )
      }
    } else {
      // Filter out consent-gated tools for this role — they get their own consent permits
      const filteredTools = tools.filter((t) => !consentToolsForRole.has(t))
      for (const tool of filteredTools) {
        policies.push(
          `permit(\n  principal is ${principalType},\n  action == Action::"${escapeCedarString(tool)}",\n  resource\n) when { principal.role == "${escapeCedarString(roleName)}" };`
        )
      }
    }
  }

  return policies
}

function generateRestrictionPolicies(config: CedarAgentConfig): string[] {
  if (!config.restrictions) return []
  const policies: string[] = []

  for (const [tool, restriction] of Object.entries(config.restrictions)) {
    const fields = Object.entries(restriction.allowedValues)
    if (fields.length === 0) {
      // Empty allowedValues = no values are allowed, deny the tool entirely
      policies.push(
        `forbid(\n  principal,\n  action == Action::"${escapeCedarString(tool)}",\n  resource\n);`
      )
      continue
    }
    for (const [field, allowedValues] of fields) {
      const valueChecks = allowedValues
        .map((v) => `context.input.${field} == ${JSON.stringify(v)}`)
        .join(' || ')
      policies.push(
        `forbid(\n  principal,\n  action == Action::"${escapeCedarString(tool)}",\n  resource\n) when {\n  !(context.input has "${escapeCedarString(field)}" && (${valueChecks}))\n};`
      )
    }
  }

  return policies
}

function generateRateLimitPolicies(config: CedarAgentConfig): string[] {
  if (!config.rateLimits) return []
  const policies: string[] = []

  for (const [tool, max] of Object.entries(config.rateLimits)) {
    policies.push(
      `forbid(\n  principal,\n  action == Action::"${escapeCedarString(tool)}",\n  resource\n) when { context.session has "call_count" && context.session.call_count >= ${max} };`
    )
  }

  return policies
}

function generateTimeWindowPolicy(config: CedarAgentConfig): string[] {
  if (!config.timeWindow) return []
  const { hourStart, hourEnd } = config.timeWindow
  return [
    `forbid(\n  principal,\n  action,\n  resource\n) when { context.session has "hour_utc" && (context.session.hour_utc < ${hourStart} || context.session.hour_utc >= ${hourEnd}) };`,
  ]
}

function generateEnvDenialPolicies(config: CedarAgentConfig): string[] {
  if (!config.denyInEnv) return []
  const policies: string[] = []

  for (const [env, tools] of Object.entries(config.denyInEnv)) {
    if (tools.includes('*')) {
      // Deny all tools in this environment
      policies.push(
        `forbid(\n  principal,\n  action,\n  resource\n) when { context.session has "environment" && context.session.environment == "${escapeCedarString(env)}" };`
      )
    } else {
      for (const tool of tools) {
        policies.push(
          `forbid(\n  principal,\n  action == Action::"${escapeCedarString(tool)}",\n  resource\n) when { context.session has "environment" && context.session.environment == "${escapeCedarString(env)}" };`
        )
      }
    }
  }

  return policies
}

function generateConsentPolicies(config: CedarAgentConfig): string[] {
  if (!config.consent) return []
  const principalType = config.principal.type ?? 'User'
  const policies: string[] = []

  for (const [tool, roles] of Object.entries(config.consent)) {
    if (roles.includes('*')) {
      // All roles need consent — no role check in the policy
      policies.push(
        `permit(\n  principal is ${principalType},\n  action == Action::"${escapeCedarString(tool)}",\n  resource\n) when { context.session has "user_consent" && context.session.user_consent == true };`
      )
    } else {
      for (const role of roles) {
        policies.push(
          `permit(\n  principal is ${principalType},\n  action == Action::"${escapeCedarString(tool)}",\n  resource\n) when { principal.role == "${escapeCedarString(role)}" && context.session has "user_consent" && context.session.user_consent == true };`
        )
      }
    }
  }

  return policies
}

export function generatePolicies(config: CedarAgentConfig): string {
  const allPolicies = [
    ...generateRolePolicies(config),
    ...generateRestrictionPolicies(config),
    ...generateRateLimitPolicies(config),
    ...generateTimeWindowPolicy(config),
    ...generateEnvDenialPolicies(config),
    ...generateConsentPolicies(config),
  ]
  return allPolicies.join('\n\n')
}
