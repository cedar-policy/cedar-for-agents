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

use smol_str::SmolStr;
use std::collections::HashMap;
use std::path::Path;

use super::deserializer;
use super::err::DeserializationError;
use super::parser::{self, json_value::LocatedValue};

#[derive(Debug, Clone)]
/// A struct representing a JSON encodable `Number`.
pub struct Number(String);

impl Number {
    /// Get the string representation of this `Number`.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Get this `Number` as a 64-bit integer if possible. Otherwise return None.
    pub fn as_i64(&self) -> Option<i64> {
        self.0.parse().ok()
    }

    /// Get this `Number` as a 64-bit unsigned integer if possible. Otherwise return None.
    pub fn as_u64(&self) -> Option<u64> {
        self.0.parse().ok()
    }

    /// Get this `Number` as a 64-bit float if possible. Otherwise return None.
    pub fn as_f64(&self) -> Option<f64> {
        self.0.parse().ok()
    }
}

#[derive(Debug, Clone)]
/// An enum represeting the possible `Value`s a MCP tool argument / result may take.
pub enum Value {
    Null,
    Bool(bool),
    Number(Number),
    String(String),
    Array(Vec<Value>),
    Map(HashMap<SmolStr, Value>),
}

#[doc(hidden)]
impl From<&LocatedValue> for Value {
    // PANIC SAFETY: A Located Value should be one-to-one with Value
    #[allow(
        clippy::unreachable,
        reason = "A Located Value should be one-to-one with Value"
    )]
    fn from(val: &LocatedValue) -> Value {
        if val.is_null() {
            Value::Null
        } else if let Some(b) = val.get_bool() {
            Value::Bool(b)
        } else if let Some(num_str) = val.get_numeric_str() {
            Value::Number(Number(num_str.to_string()))
        } else if let Some(s) = val.get_string() {
            Value::String(s)
        } else if let Some(arr) = val.get_array() {
            Value::Array(arr.iter().map(Value::from).collect())
        } else if let Some(map) = val.get_object() {
            Value::Map(
                map.iter()
                    .map(|(k, v)| (k.to_smolstr(), Value::from(v)))
                    .collect(),
            )
        } else {
            unreachable!("A Located Value should be one-to-one with Value")
        }
    }
}

#[derive(Debug, Clone)]
/// A struct containing a borrowed `Value`.
pub struct BorrowedValue<'a>(&'a LocatedValue);

impl BorrowedValue<'_> {
    /// Convert this `BarrowedValue` to an owned `Value`.
    pub fn to_owned(&self) -> Value {
        self.0.into()
    }

    /// Returns if this `BorrowedValue` is a null `Value`.
    pub fn is_null(&self) -> bool {
        self.0.is_null()
    }

    /// Returns if this `BorrowedValue` is a boolean `Value`.
    pub fn is_bool(&self) -> bool {
        self.0.is_bool()
    }

    /// Returns `Some(b)` if this `BorrowedValue` is a borrowed
    /// boolean value; otherwise, returns None.
    pub fn get_bool(&self) -> Option<bool> {
        self.0.get_bool()
    }

    /// Returns if this `BorrowedValue` is a number `Value`.
    pub fn is_number(&self) -> bool {
        self.0.is_number()
    }

    /// Returns `Some(n)` if this `BorrowedValue` is a number;
    /// otherwise, returns None.
    pub fn get_number(&self) -> Option<Number> {
        self.0.get_numeric_str().map(|s| Number(s.to_string()))
    }

    /// Returns `Some(n)` if this `BorrowedValue` is a number
    /// representable as a 64bit integer; otherwise, returns None.
    pub fn get_i64(&self) -> Option<i64> {
        self.0.get_numeric_str().and_then(|s| s.parse().ok())
    }

    /// Returns `Some(n)` if this `BorrowedValue` is a number
    /// representable as a 64bit unsigned integer; otherwise, returns None.
    pub fn get_u64(&self) -> Option<u64> {
        self.0.get_numeric_str().and_then(|s| s.parse().ok())
    }

    /// Returns `Some(n)` if this `BorrowedValue` is a number
    /// representable as a 64bit float; otherwise, returns None.
    pub fn get_f64(&self) -> Option<f64> {
        self.0.get_numeric_str().and_then(|s| s.parse().ok())
    }

    /// Returns if this `BorrowedValue` is a string `Value`
    pub fn is_string(&self) -> bool {
        self.0.is_string()
    }

    /// Returns `Some(s)` if this `BorrowedValue` is a string `Value`;
    /// otherwise, return None.
    pub fn get_str(&self) -> Option<&str> {
        self.0.get_str()
    }

    /// Returns `Some(s)` if this `BorrowedValue` is a string `Value`;
    /// otherwise, return None.
    pub fn get_string(&self) -> Option<String> {
        self.0.get_string()
    }

    /// Returns `Some(s)` if this `BorrowedValue` is a string `Value`;
    /// otherwise, return None.
    pub fn get_smolstr(&self) -> Option<SmolStr> {
        self.0.get_smolstr()
    }

    /// Returns if this `BorrowedValue` is a array `Value`.
    pub fn is_array(&self) -> bool {
        self.0.is_array()
    }

    /// Returns `Some(v)` if this `BorrowedValue` is an
    /// array `Value`; otherwise, return None.
    pub fn get_array(&self) -> Option<Vec<BorrowedValue<'_>>> {
        self.0
            .get_array()
            .map(|vals| vals.iter().map(BorrowedValue).collect())
    }

    /// Returns if this `BorrowedValue` is a map `Value`
    pub fn is_map(&self) -> bool {
        self.0.is_object()
    }

    /// Returns `Some(m)` if this `BorrowedValue` is a
    /// map `Value`; otherwise, return None.
    pub fn get_map(&self) -> Option<HashMap<SmolStr, BorrowedValue<'_>>> {
        self.0.get_object().map(|kvs| {
            kvs.iter()
                .map(|(k, v)| (k.to_smolstr(), BorrowedValue(v)))
                .collect()
        })
    }
}

#[derive(Debug, Clone)]
/// A struct representing an MCP "call/tool" request
pub struct Input {
    pub(crate) name: SmolStr,
    pub(crate) args: HashMap<SmolStr, LocatedValue>,
}

impl Input {
    /// Get the name of the requested tool
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get all arguments to the tool
    pub fn get_args(&self) -> impl Iterator<Item = (&str, BorrowedValue<'_>)> {
        self.args
            .iter()
            .map(|(k, v)| (k.as_str(), BorrowedValue(v)))
    }

    /// Get an argument to the tool if it exists
    pub fn get_arg(&self, arg: &str) -> Option<BorrowedValue<'_>> {
        self.args.get(arg).map(BorrowedValue)
    }

    /// Deserialize an MCP `tools/call` json request into an `Input`
    pub fn from_json_str(json_str: &str) -> Result<Self, DeserializationError> {
        let mut parser = parser::json_parser::JsonParser::new(json_str);
        deserializer::mcp_tool_input_from_json_value(&parser.get_value()?)
    }

    /// Deserialize an MCP `tools/call` json request into an `Input`
    pub fn from_json_file<P: AsRef<Path>>(json_file: P) -> Result<Self, DeserializationError> {
        let contents = std::fs::read_to_string(json_file.as_ref()).map_err(|e| {
            DeserializationError::read_error(json_file.as_ref().into(), format!("{e}"))
        })?;
        Self::from_json_str(&contents)
    }
}

#[derive(Debug, Clone)]
/// A struct representing an MCP `call/tool` response
pub struct Output {
    pub(crate) results: HashMap<SmolStr, LocatedValue>,
}

impl Output {
    /// Get all returned results from calling an MCP tool
    pub fn get_results(&self) -> impl Iterator<Item = (&str, BorrowedValue<'_>)> {
        self.results
            .iter()
            .map(|(k, v)| (k.as_str(), BorrowedValue(v)))
    }

    /// Get a returned result from an MCP tool if it exists
    pub fn get_result(&self, res: &str) -> Option<BorrowedValue<'_>> {
        self.results.get(res).map(BorrowedValue)
    }

    /// Deserialize an MCP `tools/call` json response into an `Output`
    pub fn from_json_str(json_str: &str) -> Result<Self, DeserializationError> {
        let mut parser = parser::json_parser::JsonParser::new(json_str);
        deserializer::mcp_tool_output_from_json_value(&parser.get_value()?)
    }

    /// Deserialize an MCP `tools/call` json response into an `Output`
    pub fn from_json_file<P: AsRef<Path>>(json_file: P) -> Result<Self, DeserializationError> {
        let contents = std::fs::read_to_string(json_file.as_ref()).map_err(|e| {
            DeserializationError::read_error(json_file.as_ref().into(), format!("{e}"))
        })?;
        Self::from_json_str(&contents)
    }
}
