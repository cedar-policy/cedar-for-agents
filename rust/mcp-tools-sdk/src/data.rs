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

use smol_str::{SmolStr, ToSmolStr};
use std::collections::HashMap;
use std::path::Path;

use super::deserializer;
use super::err::DeserializationError;
use super::parser::{self, json_value::LocatedValue};

#[derive(Debug, Clone)]
/// A struct representing a JSON encodable `Number`.
pub struct Number(SmolStr);

impl Number {
    /// Get the string representation of this `Number`.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Get this `Number` as a 64-bit integer if possible. Otherwise return None.
    pub fn to_i64(&self) -> Option<i64> {
        self.0.parse().ok()
    }

    /// Get this `Number` as a 64-bit unsigned integer if possible. Otherwise return None.
    pub fn to_u64(&self) -> Option<u64> {
        self.0.parse().ok()
    }

    /// Get this `Number` as a 64-bit float if possible. Otherwise return None.
    pub fn to_f64(&self) -> Option<f64> {
        self.0.parse().ok().filter(|f: &f64| f.is_finite())
    }
}

#[derive(Debug, Clone)]
/// An enum represeting the possible `Value`s a MCP tool argument / result may take.
pub enum Value {
    Null,
    Bool(bool),
    Number(Number),
    String(SmolStr),
    Array(Vec<Value>),
    Map(HashMap<SmolStr, Value>),
}

#[derive(Debug, Clone)]
/// An enum representing the result of validating a MCP tool argument / result in which
/// the `Value` is tagged with the `PropertyType the` `Value` was validated against.
/// For exampel string `Values` can be validated as a String, Enum, Decimal, Datetime, Duration or IpAddr.
pub enum TypedValue {
    Null,
    Bool(bool),
    Integer(i64),
    Float(f64),
    Number(Number),
    String(SmolStr),
    Decimal(SmolStr),
    Datetime(SmolStr),
    Duration(SmolStr),
    IpAddr(SmolStr),
    Enum(SmolStr),
    Array(Vec<TypedValue>),
    Tuple(Vec<TypedValue>),
    Union {
        /// index of which type within union type this validates against
        index: usize,
        value: Box<TypedValue>,
    },
    Object {
        /// properties that were explicitly defined by object's type schema
        properties: HashMap<SmolStr, TypedValue>,
        /// properties that were not explicitly defined by object's type schema
        /// but matched the object's `additionalProperty` type
        additional_properties: HashMap<SmolStr, TypedValue>,
    },
    Ref {
        name: SmolStr,
        val: Box<TypedValue>,
    },
    Unknown(Value),
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
            Value::Number(Number(num_str.to_smolstr()))
        } else if let Some(s) = val.get_smolstr() {
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

impl From<BorrowedValue<'_>> for Value {
    fn from(val: BorrowedValue<'_>) -> Self {
        val.0.into()
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
        self.0.get_numeric_str().map(|s| Number(s.to_smolstr()))
    }

    /// Returns `Some(n)` if this `BorrowedValue` is a number
    /// representable as a 64bit integer; otherwise, returns None.
    pub fn get_i64(&self) -> Option<i64> {
        self.get_number().and_then(|n| n.to_i64())
    }

    /// Returns `Some(n)` if this `BorrowedValue` is a number
    /// representable as a 64bit unsigned integer; otherwise, returns None.
    pub fn get_u64(&self) -> Option<u64> {
        self.get_number().and_then(|n| n.to_u64())
    }

    /// Returns `Some(n)` if this `BorrowedValue` is a number
    /// representable as a 64bit float; otherwise, returns None.
    pub fn get_f64(&self) -> Option<f64> {
        self.get_number().and_then(|n| n.to_f64())
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
pub struct TypedInput {
    pub(crate) name: SmolStr,
    pub(crate) args: HashMap<SmolStr, TypedValue>,
}

impl TypedInput {
    /// Get the name of the requested tool
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get all arguments to the tool
    pub fn get_args(&self) -> impl Iterator<Item = (&str, &TypedValue)> {
        self.args.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// Get an argument to the tool if it exists
    pub fn get_arg(&self, arg: &str) -> Option<&TypedValue> {
        self.args.get(arg)
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

#[derive(Debug, Clone)]
/// A struct representing an MCP `call/tool` response
pub struct TypedOutput {
    pub(crate) results: HashMap<SmolStr, TypedValue>,
}

impl TypedOutput {
    /// Get all returned results from calling an MCP tool
    pub fn get_results(&self) -> impl Iterator<Item = (&str, &TypedValue)> {
        self.results.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// Get a returned result from an MCP tool if it exists
    pub fn get_result(&self, res: &str) -> Option<&TypedValue> {
        self.results.get(res)
    }
}

#[cfg(test)]
mod test {
    use crate::parser::json_parser;

    use super::*;
    use cool_asserts::assert_matches;
    use tempfile::TempDir;

    //------------------- test number converions ----------------------------
    #[test]
    fn test_number_as_str() {
        let num = Number("0".to_smolstr());
        assert!(num.as_str() == "0")
    }

    #[test]
    fn test_number_to_i64_int_zero() {
        let num = Number("0".to_smolstr());
        assert_matches!(num.to_i64(), Some(0))
    }

    #[test]
    fn test_number_to_i64_int_neg() {
        let num = Number("-123".to_smolstr());
        assert_matches!(num.to_i64(), Some(-123))
    }

    #[test]
    fn test_number_to_i64_int_pos() {
        let num = Number("9845".to_smolstr());
        assert_matches!(num.to_i64(), Some(9845))
    }

    #[test]
    fn test_number_to_i64_max_pos_int() {
        let num = Number("9223372036854775807".to_smolstr());
        assert_matches!(num.to_i64(), Some(9223372036854775807))
    }

    #[test]
    fn test_number_to_i64_max_neg_int() {
        let num = Number("-9223372036854775808".to_smolstr());
        assert_matches!(num.to_i64(), Some(-9223372036854775808))
    }

    #[test]
    fn test_number_to_i64_float_zero() {
        let num = Number("0.0".to_smolstr());
        assert!(num.to_i64().is_none())
    }

    #[test]
    fn test_number_to_i64_float_pos() {
        let num = Number("123.0".to_smolstr());
        assert!(num.to_i64().is_none())
    }

    #[test]
    fn test_number_to_i64_float_neg() {
        let num = Number("-8090.0".to_smolstr());
        assert!(num.to_i64().is_none())
    }

    #[test]
    fn test_number_to_i64_large_pos_int() {
        let num = Number("9223372036854775808".to_smolstr());
        assert!(num.to_i64().is_none())
    }

    #[test]
    fn test_number_to_i64_large_neg_int() {
        let num = Number("-9223372036854775809".to_smolstr());
        assert!(num.to_i64().is_none())
    }

    #[test]
    fn test_number_to_i64_pos_exp() {
        let num = Number("1e3".to_smolstr());
        assert!(num.to_i64().is_none())
    }

    #[test]
    fn test_number_to_u64_int_zero() {
        let num = Number("0".to_smolstr());
        assert_matches!(num.to_u64(), Some(0))
    }

    #[test]
    fn test_number_to_u64_int_pos() {
        let num = Number("9845".to_smolstr());
        assert_matches!(num.to_u64(), Some(9845))
    }

    #[test]
    fn test_number_to_u64_max_pos_int() {
        let num = Number("18446744073709551615".to_smolstr());
        assert_matches!(num.to_u64(), Some(18446744073709551615))
    }

    #[test]
    fn test_number_to_u64_int_neg() {
        let num = Number("-123".to_smolstr());
        assert!(num.to_u64().is_none())
    }

    #[test]
    fn test_number_to_u64_float_zero() {
        let num = Number("0.0".to_smolstr());
        assert!(num.to_u64().is_none())
    }

    #[test]
    fn test_number_to_u64_float_pos() {
        let num = Number("123.0".to_smolstr());
        assert!(num.to_u64().is_none())
    }

    #[test]
    fn test_number_to_u64_float_neg() {
        let num = Number("-8090.0".to_smolstr());
        assert!(num.to_u64().is_none())
    }

    #[test]
    fn test_number_to_u64_large_pos_int() {
        let num = Number("18446744073709551616".to_smolstr());
        assert!(num.to_u64().is_none())
    }

    #[test]
    fn test_number_to_u64_pos_exp() {
        let num = Number("1e3".to_smolstr());
        assert!(num.to_u64().is_none())
    }

    #[test]
    fn test_number_to_f64_int_zero() {
        let num = Number("0".to_smolstr());
        assert_matches!(num.to_f64(), Some(0.0))
    }

    #[test]
    fn test_number_to_f64_float_zero() {
        let num = Number("0.00".to_smolstr());
        assert_matches!(num.to_f64(), Some(0.0))
    }

    #[test]
    fn test_number_to_f64_float_neg_zero() {
        let num = Number("-0.0".to_smolstr());
        assert_matches!(num.to_f64(), Some(0.0))
    }

    #[test]
    fn test_number_to_f64_int_neg() {
        let num = Number("-123".to_smolstr());
        assert_matches!(num.to_f64(), Some(-123.0))
    }

    #[test]
    fn test_number_to_f64_int_pos() {
        let num = Number("9845".to_smolstr());
        assert_matches!(num.to_f64(), Some(9845.0))
    }

    #[test]
    fn test_number_to_f64_float_neg() {
        let num = Number("-123.0".to_smolstr());
        assert_matches!(num.to_f64(), Some(-123.0))
    }

    #[test]
    fn test_number_to_f64_float_pos() {
        let num = Number("-8093.01".to_smolstr());
        assert_matches!(num.to_f64(), Some(-8093.01))
    }

    #[test]
    fn test_number_to_f64_float_pos_max() {
        let num = Number("1.7976931348623157e308".to_smolstr());
        assert_matches!(num.to_f64(), Some(1.7976931348623157e308))
    }

    #[test]
    fn test_number_to_f64_float_pos_overflow() {
        let num = Number("1.7976931348623159e308".to_smolstr());
        assert_matches!(num.to_f64(), None)
    }

    #[test]
    fn test_number_to_f64_float_neg_max() {
        let num = Number("-1.7976931348623157e308".to_smolstr());
        assert_matches!(num.to_f64(), Some(-1.7976931348623157e308))
    }

    #[test]
    fn test_number_to_f64_float_neg_overflow() {
        let num = Number("-1.7976931348623159e308".to_smolstr());
        assert!(num.to_f64().is_none())
    }

    #[test]
    fn test_number_to_f64_float_min_normal_pos() {
        let num = Number("2.2250738585072014e-308".to_smolstr());
        assert_matches!(num.to_f64(), Some(2.2250738585072014e-308))
    }

    #[test]
    fn test_number_to_f64_float_min_subnormal_pos() {
        let num = Number("5e-324".to_smolstr());
        assert_matches!(num.to_f64(), Some(5e-324))
    }

    #[test]
    fn test_number_to_f64_float_subnormal_rounds() {
        let num = Number("2.5e-324".to_smolstr());
        assert_matches!(num.to_f64(), Some(5e-324))
    }

    #[test]
    fn test_number_to_f64_float_subnormal_underflows() {
        let num = Number("1e-400".to_smolstr());
        assert_matches!(num.to_f64(), Some(0.0))
    }

    #[test]
    fn test_number_to_f64_max_precision_under_1() {
        let num = Number("0.9999999999999999".to_smolstr());
        assert_matches!(num.to_f64(), Some(0.9999999999999999))
    }

    #[test]
    fn test_number_to_f64_loses_precision_under_1() {
        let num = Number("0.99999999999999999".to_smolstr());
        assert_matches!(num.to_f64(), Some(1.0))
    }

    //------------------- test BorrowedValues ----------------------------
    #[test]
    fn test_borrowed_value_is_null() {
        let mut parser = json_parser::JsonParser::new("null");
        let val = parser.get_value().unwrap();
        let bv = BorrowedValue(&val);

        assert!(bv.is_null());
        assert_matches!(bv.to_owned(), Value::Null)
    }

    #[test]
    fn test_borrowed_value_is_bool_true() {
        let mut parser = json_parser::JsonParser::new("true");
        let val = parser.get_value().unwrap();
        let bv = BorrowedValue(&val);

        assert!(bv.is_bool());
        assert_matches!(bv.get_bool(), Some(true));
        assert_matches!(bv.to_owned(), Value::Bool(true))
    }

    #[test]
    fn test_borrowed_value_is_bool_false() {
        let mut parser = json_parser::JsonParser::new("false");
        let val = parser.get_value().unwrap();
        let bv = BorrowedValue(&val);

        assert!(bv.is_bool());
        assert_matches!(bv.get_bool(), Some(false));
        assert_matches!(bv.to_owned(), Value::Bool(false))
    }

    #[test]
    fn test_borrowed_value_is_number() {
        let mut parser = json_parser::JsonParser::new("0");
        let val = parser.get_value().unwrap();
        let bv = BorrowedValue(&val);

        assert!(bv.is_number());
        assert_matches!(bv.get_number(), Some(Number(..)));
        assert_matches!(bv.get_i64(), Some(0));
        assert_matches!(bv.get_u64(), Some(0));
        assert_matches!(bv.get_f64(), Some(0.0));
        assert_matches!(bv.to_owned(), Value::Number(Number(..)))
    }

    #[test]
    fn test_borrowed_value_is_string() {
        let mut parser = json_parser::JsonParser::new("\"My test string\"");
        let val = parser.get_value().unwrap();
        let bv = BorrowedValue(&val);

        assert!(bv.is_string());
        assert_matches!(bv.get_str(), Some("My test string"));
        assert_matches!(bv.get_string(), Some(v) if v == "My test string");
        assert_matches!(bv.get_smolstr(), Some(v) if v == "My test string");
        assert_matches!(bv.to_owned(), Value::String(v) if v == "My test string")
    }

    #[test]
    fn test_borrowed_value_is_array() {
        let mut parser = json_parser::JsonParser::new("[true, false, 1, 2, 3.0]");
        let val = parser.get_value().unwrap();
        let bv = BorrowedValue(&val);

        assert!(bv.is_array());
        assert_matches!(
            bv.get_array(),
            Some(v)
            if matches!(
                v.as_slice(),
                [a, b, c, d, e]
                if a.get_bool() == Some(true) &&
                   b.get_bool() == Some(false) &&
                   c.get_i64() == Some(1) &&
                   d.get_u64() == Some(2) &&
                   e.get_f64() == Some(3.0)
            )
        );
        assert_matches!(
            bv.to_owned(),
            Value::Array(v)
            if matches!(
                v.as_slice(),
                [
                    Value::Bool(true),
                    Value::Bool(false),
                    Value::Number(..),
                    Value::Number(..),
                    Value::Number(..),
                ]
            )
        );
    }

    #[test]
    fn test_borrowed_value_is_map() {
        let mut parser = json_parser::JsonParser::new("{\"attr\": false}");
        let val = parser.get_value().unwrap();
        let bv = BorrowedValue(&val);

        assert!(bv.is_map());
        assert_matches!(
            bv.get_map(),
            Some(m)
            if m.len() == 1 && matches!(
                m.iter().next(),
                Some((k, v))
                if k == "attr" && v.get_bool() == Some(false)
            )
        );
        assert_matches!(
            bv.to_owned(),
            Value::Map(m)
            if m.len() == 1 && matches!(
                m.iter().next(),
                Some((k, Value::Bool(false)))
                if k == "attr"
            )
        )
    }

    //------------------- test Input ----------------------------
    #[test]
    fn test_simple_input_from_file() {
        let temp_dir = TempDir::new().unwrap();
        let input_file = temp_dir.path().join("input.json");
        std::fs::write(
            &input_file,
            r#"{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "tools/call",
    "params": {
        "tool": "test_tool",
        "args": {
            "arg1": 0
        }
    }
}"#,
        )
        .unwrap();

        // Error reading because forgot to look into tempdir
        assert_matches!(
            Input::from_json_file("input.json"),
            Err(DeserializationError::ReadError(..))
        );
        // Should succeed and match the following assertions
        let input = Input::from_json_file(input_file).unwrap();
        assert_eq!(input.name(), "test_tool");
        assert!(input.get_args().count() == 1);
        assert_matches!(
            input.get_arg("arg1"),
            Some(v)
            if matches!(
                v.to_owned(),
                Value::Number(n)
                if n.to_u64() == Some(0)
            )
        )
    }

    //------------------- test Output ----------------------------
    #[test]
    fn test_simple_output_from_file() {
        let temp_dir = TempDir::new().unwrap();
        let output_file = temp_dir.path().join("output.json");
        std::fs::write(
            &output_file,
            r#"{
    "jsonrpc": "2.0",
    "id": 1,
    "result": {
        "content": [
            {
                "type": "text",
                "text": "{\"value\": 0}"
            }
        ],
        "structuredContent": {
            "value": 0
        }
    }
}"#,
        )
        .unwrap();

        // Error reading because forgot to look into tempdir
        assert_matches!(
            Output::from_json_file("output.json"),
            Err(DeserializationError::ReadError(..))
        );
        // Should succeed and match the following assertions
        let output = Output::from_json_file(output_file).unwrap();
        assert!(output.get_results().count() == 1);
        assert_matches!(
            output.get_result("value"),
            Some(v)
            if matches!(
                v.to_owned(),
                Value::Number(n)
                if n.to_u64() == Some(0)
            )
        )
    }

    #[test]
    fn test_input_not_object_errors() {
        let input = r#""not a valid mcp \"tools/call\" request""#;
        assert_matches!(
            Input::from_json_str(input),
            Err(DeserializationError::UnexpectedType(..))
        )
    }

    #[test]
    fn test_input_params_missing_errors() {
        let input = r#"{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "tools/call"
}"#;
        assert_matches!(
            Input::from_json_str(input),
            Err(DeserializationError::MissingExpectedAttribute(..))
        )
    }

    #[test]
    fn test_input_params_not_object_errors() {
        let input = r#"{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "tools/call",
    "params": false
}"#;
        assert_matches!(
            Input::from_json_str(input),
            Err(DeserializationError::UnexpectedType(..))
        )
    }

    #[test]
    fn test_input_toolname_missing_errors() {
        let input = r#"{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "tools/call",
    "params": {
        "args": {
            "arg1": 0
        }
    }
}"#;
        assert_matches!(
            Input::from_json_str(input),
            Err(DeserializationError::MissingExpectedAttribute(..))
        )
    }

    #[test]
    fn test_input_toolname_not_string_errors() {
        let input = r#"{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "tools/call",
    "params": {
        "tool": false,
        "args": {
            "arg1": 0
        }
    }
}"#;
        assert_matches!(
            Input::from_json_str(input),
            Err(DeserializationError::UnexpectedType(..))
        )
    }

    #[test]
    fn test_input_missing_args_errors() {
        let input = r#"{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "tools/call",
    "params": {
        "tool": "test_tool"
    }
}"#;
        assert_matches!(
            Input::from_json_str(input),
            Err(DeserializationError::MissingExpectedAttribute(..))
        )
    }

    #[test]
    fn test_input_args_not_object_errors() {
        let input = r#"{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "tools/call",
    "params": {
        "tool": "test_tool",
        "args": false
    }
}"#;
        assert_matches!(
            Input::from_json_str(input),
            Err(DeserializationError::UnexpectedType(..))
        )
    }

    #[test]
    fn test_output_not_object_errors() {
        let output = r#""Not a well formed MCP \"tools/call\" output""#;

        assert_matches!(
            Output::from_json_str(output),
            Err(DeserializationError::UnexpectedType(..))
        )
    }

    #[test]
    fn test_output_result_missing_errors() {
        let output = r#"{
    "jsonrpc": "2.0",
    "id": 1
}"#;
        assert_matches!(
            Output::from_json_str(output),
            Err(DeserializationError::MissingExpectedAttribute(..))
        )
    }

    #[test]
    fn test_output_result_not_object_errors() {
        let output = r#"{
    "jsonrpc": "2.0",
    "id": 1,
    "result": false
}"#;
        assert_matches!(
            Output::from_json_str(output),
            Err(DeserializationError::UnexpectedType(..))
        )
    }

    #[test]
    fn test_output_result_content_missing_errors() {
        let output = r#"{
    "jsonrpc": "2.0",
    "id": 1,
    "result": {
    }
}"#;
        assert_matches!(
            Output::from_json_str(output),
            Err(DeserializationError::MissingExpectedAttribute(..))
        )
    }

    #[test]
    fn test_output_result_content_not_object_errors() {
        let output = r#"{
    "jsonrpc": "2.0",
    "id": 1,
    "result": {
        "structuredContent": false
    }
}"#;
        assert_matches!(
            Output::from_json_str(output),
            Err(DeserializationError::UnexpectedType(..))
        )
    }
}
