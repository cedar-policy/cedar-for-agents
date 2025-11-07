# MCP Tools SDK

This crate contains an SDK for parsing and manipulating Model Context Protocol (MCP) tool descriptions. Specifically, the SDK is designed in a way in which any MCP tool description can be parsed into a generic `ToolDescription` struct which allows for meta-mcp tools to process the tool descriptions without needing to know the exact structure of each concrete mcp tool.

## Example Usage

`mcp_tool.json`
```json
{
    "name": "MyCoolTool",
    "description": "A cool tool made by me!",
    "inputSchema": {
        "type": "object",
        "properties": {
            "cool_attr": { "type": "string" }
        },
        "required": ["cool_attr"]
    }
}
```


```rust
use mcp_tools_sdk::description::ToolDescription;

fn main() {
    let tool = ToolDescription::from_json_file("mcp_tool.json").expect("Tool description should have parsed.");
    println!("{}: {}", tool.name(), tool.description().unwrap_or(""))
}
```