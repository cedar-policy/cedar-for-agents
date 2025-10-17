# Cedar Policy MCP Schema Generator

This crate is designed to help authors of Model Context Protocol (MCP) servers and Agents secure thier MCP Servers and Agents using Cedar for fine-grained authorization. You can learn more about Cedar on [github](https://github.com/cedar-policy/cedar) and at the [Cedar docs website](https://docs.cedarpolicy.com/).

> Cedar is a language for writing authorization policies and making authorization decisions based on those policies. When you create an application, you need to ensure that only authorized users can access the application, and can do only what each user is authorized to do.
>
> Using Cedar, you can decouple your business logic from the authorization logic. In your application’s code, you preface requests made to your operations with a call to Cedar’s authorization engine, asking “Is this request authorized?”. Then, the application can either perform the requested operation if the decision is “allow”, or return an error message if the decision is “deny”.

Using this crate, you can automatically generate a [Cedar Schema](https://docs.cedarpolicy.com/schema/schema.html) from a list of MCP Tool Descriptions (i.e., the response to `list_tools` from an MCP server). The generated Schema captures the details of the input MCP Tool descriptions by generating a single Action for reach MCP tool and further encodes all inputs (and optionally all outputs) to the tool as an action specific context that can be used to determine if access to the tool should be allowed.

> A schema is a declaration of the structure of the entity types that you want to support in your application and for which you want Cedar to provide authorization services. After you define a schema, you can ask Cedar to validate your policies against it to ensure that your policies do not contain type errors, such as referencing the entities and their attributes incorrectly.

A schema helps you write better policies for controlling access to your MCP tools by enabling you to perform validation and use Cedar analysis which are capable of finding malformed policies (typos and type errors) as well as find likely bugs (i.e., redundant or impossible policies).

## How to use Schema Generator

The schema generator is very flexible and takes two inputs:
1. A list of MCP Tool Descriptions (A JSON object describing the input (and optionally an output) schema of each tool) and
2. A Cedar Schema Stub that describes the MCP server's users and Resources.

### Input MCP Tool Descriptions

Below is an example MCP Tool description which contains a JSON Schema describing the inputs and outputs to three tools: `check_task_status`, `allocate_time`, and `start_work`.

`mcp_tools.json`
```json
{
    "result": {
        "tools": [
            {
                "name": "check_task_status",
                "description": "Check if a task is ready for work",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "task_id": {"type": "string"}
                    },
                    "required": ["task_id"]
                },
                "outputSchema": {
                    "type": "object",
                    "properties": {
                        "status": {
                            "type": "string",
                            "enum": ["started", "paused", "failed", "completed"]
                        },
                        "priority": {"type": "integer"}
                    },
                    "required": ["status", "priority"]
                }
            },
            {
                "name": "allocate_time",
                "description": "Reserve time slot for task work",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "hours_needed": {"type": "integer"},
                        "priority": {"type": "integer"}
                    },
                    "required": ["hours_needed"]
                },
                "outputSchema": {
                    "type": "object",
                    "properties": {
                        "time_available": {"type": "boolean"},
                        "slot_id": {"type": "string"}
                    },
                    "required": ["time_available", "slot_id"]
                }
            },
            {
                "name": "start_work",
                "description": "Begin working on a task",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "task_id": {"type": "string"},
                        "slot_id": {"type": "string"}
                    },
                    "required": ["task_id", "slot_id"]
                },
                "outputSchema": {
                    "type": "object",
                    "properties": {
                        "work_started": {"type": "boolean"}
                    },
                    "required": ["work_started"]
                }
            }                
        ]
    }
}
```

### Input Cedar Schema stub file

The second input to the Schema generator is a Cedar Schema (stub) file that describes the principal and resource types for each MCP tool. This allows you the flexibility to use any entity type as the MCP user or resource to fit your specific authorization needs. The input stub file requires at least one entity type annotated with the `mcp_principal` annotation and at least one entity type annotated with the `mcp_resource` type. You may specify one or more of each. You may also optionally specify a number of additional shared context variables using the `@mcp_context` annotation.

`input.cedarschema`:
```cedarschema
// Requires exactly one namespace
namespace MyMcpServer {

    @mcp_principal("User")
    entity User {
        id: String, 
        username: String,
        name?: String, 
        givenName?: String,
        // other custom attributes for User
    };

    @mcp_principal("Agent")
    entity Agent {
        agentName: String,
        // other custom attributes for the Agent
    };
    
    // This is a common type that describes generic contextual
    // attributes that are relevant for every tool call.
    // The annotation @mcp_tool_context tells the schema generator to 
    // add a required field "session : CommonContext" to each tool's context.

    @mcp_context("session")
    type CommonContext = {
        currentTimestamp: datetime,
        ipaddr: ipaddr,
    };
  
    // Resource entity i.e. the receiver of the call
    // The McpServer entity should always be the resource entity,
    // Generator requires at least one resource type
    @mcp_resource("McpServer")
    entity McpServer;
}
```

### Calling the Schema Generator

You can call the schema generator in your rust code. Below is a sample program that reads the input schema stub file and mcp tool descriptions and uses the schema generator to create a new Cedar Schema.

```rust
use cedar_policy_core::extensions::Extensions;
use cedar_policy_core::validator::json_schema::Fragment;
use cedar_policy_mcp_schema_generator::{SchemaGenerator, ServerDescription};
use miette::Result;

fn main() -> Result<()> {
    let description = ServerDescription::from_json_file("mcp_tools.json")?;

    let schema_file = std::fs::File::open("input.cedarschema").unwrap();
    let schema = Fragment::from_cedarschema_file(schema_file, Extensions::all_available())?.0;
    let mut generator = SchemaGenerator::new(schema)?;
    generator.add_actions_from_server_description(&description)?;
    println!("{}", generator.get_schema().clone().to_cedarschema()?);
    Ok(())
}
```

### Generated Cedar Schema

The above example program will output the following Cedar Schema that keeps the user input Schema stub along with an action declaration for each input MCP tool description.

```cedarschema
namespace MyMcpServer {
  @mcp_context("session")
  type CommonContext = {
    "currentTimestamp": datetime,
    "ipaddr": ipaddr
  };

  @mcp_principal("Agent")
  entity Agent = {
    "agentName": String
  };

  @mcp_resource("McpServer")
  entity McpServer;

  @mcp_principal("User")
  entity User = {
    "givenName"?: String,
    "id": String,
    "name"?: String,
    "username": String
  };

  action "allocate_time" appliesTo {
    principal: [Agent, User],
    resource: [McpServer],
    context: {
  "inputs": {
    "hours_needed": __cedar::Long,
    "priority"?: __cedar::Long
  },
  "outputs"?: {
    "slot_id": __cedar::String,
    "time_available": __cedar::Bool
  },
  "session": CommonContext
}
  };

  action "check_task_status" appliesTo {
    principal: [Agent, User],
    resource: [McpServer],
    context: {
  "inputs": {
    "task_id": __cedar::String
  },
  "outputs"?: {
    "priority": __cedar::Long,
    "status": MyMcpServer::check_task_status::Outputs::status
  },
  "session": CommonContext
}
  };

  action "start_work" appliesTo {
    principal: [Agent, User],
    resource: [McpServer],
    context: {
  "inputs": {
    "slot_id": __cedar::String,
    "task_id": __cedar::String
  },
  "outputs"?: {
    "work_started": __cedar::Bool
  },
  "session": CommonContext
}
  };
}

namespace MyMcpServer::check_task_status::Outputs {
  entity status enum ["started", "paused", "failed", "completed"];
}
```