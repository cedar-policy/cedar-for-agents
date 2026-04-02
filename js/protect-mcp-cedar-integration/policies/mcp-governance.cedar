// Cedar policies for MCP tool governance
// These policies use the auto-generated schema from protect-mcp's
// generateCedarSchema() to enforce typed, attribute-aware access control.
//
// Usage:
//   npx protect-mcp --cedar ./policies --enforce -- node mcp-server.js
//
// Every allow/deny decision produces an Ed25519-signed receipt.

// ── Read operations: allow within workspace ──

@id("allow-workspace-reads")
permit(
  principal,
  action == Action::"MCP::Tool::call",
  resource == Tool::"read_file"
);

// ── Write operations: require identified agent ──

@id("block-anonymous-writes")
forbid(
  principal,
  action == Action::"MCP::Tool::call",
  resource == Tool::"write_file"
)
when {
  context.tier == "unknown"
};

// ── Shell execution: blocked entirely ──
// Prevents prompt injection attacks (CVE-2025-6514 "Clinejection")

@id("block-shell-execution")
forbid(
  principal,
  action == Action::"MCP::Tool::call",
  resource == Tool::"execute_command"
);

@id("block-bash")
forbid(
  principal,
  action == Action::"MCP::Tool::call",
  resource == Tool::"bash"
);

// ── Web search: allowed for all agents ──

@id("allow-web-search")
permit(
  principal,
  action == Action::"MCP::Tool::call",
  resource == Tool::"search_web"
);

// ── Status checks: allowed (read-only, no side effects) ──

@id("allow-status")
permit(
  principal,
  action == Action::"MCP::Tool::call",
  resource == Tool::"get_status"
);
