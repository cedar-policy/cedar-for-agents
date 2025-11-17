/*
 * Copyright Cedar Contributors
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *      https://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

//! This library contains definitions of `ToolDescription`s which is a datatype that
//! represents a Model Context Protocol (MCP) Tool Description. An MCP Tool description is a JSON
//! object that gives the name of the tool, an optional description of the tool, and a description of
//! the input and output parameters of the tool (and any type definitions usded to define these parameters).
//!
//! This library also includes a parser that deserializes an MCP tool description JSON into a `ToolDescription` struct.
//!
//! This library also includes a `ServerDescription` struct that represents a collection of MCP tool descriptions
//! (i.e., the output of `list_tools` from an MCP Server).

pub mod data;
pub mod description;
mod deserializer;
pub mod err;
pub mod parser;
mod validation;
