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

use linked_hash_map::LinkedHashMap;
use smol_str::SmolStr;

use std::borrow::Borrow;
use std::hash::{Hash, Hasher};

use super::loc::Loc;

// The kind of a Located (JSON) Value
#[derive(Debug, Clone)]
pub(crate) enum ValueKind {
    Null,
    Bool(bool),
    Number,
    String,
    Array(Vec<LocatedValue>),
    Object(LinkedHashMap<LocatedString, LocatedValue>),
}

/// A String Literal represented by its location in the input String
#[derive(Debug, Clone)]
pub(crate) struct LocatedString {
    loc: Loc,
}

/// A Located (JSON) Value that combines the JSON Type of the value and the value's location within the input string
#[derive(Debug, Clone)]
pub(crate) struct LocatedValue {
    kind: ValueKind,
    loc: Loc,
}

impl LocatedString {
    /// Create a new `LocatedString`
    pub(crate) fn new(loc: Loc) -> Self {
        Self { loc }
    }

    /// Get a reference to the location of the `LocatedString` within the input string
    pub(crate) fn as_loc(&self) -> &Loc {
        &self.loc
    }

    /// Unwrap the `LocatedString` and retrieve the location of the string within the input string.
    pub(crate) fn into_loc(self) -> Loc {
        self.loc
    }

    /// Get the `&str` matching the contents of the `LocatedString`
    pub(crate) fn as_str(&self) -> &str {
        let start = self.loc.start() + 1;
        let end = self.loc.end() - 1;
        #[expect(
            clippy::string_slice,
            reason = "By construction the indexes are guaranteed to satisfy 0 <= start <= end < self.loc.src.len()."
        )]
        &self.loc.src[start..end]
    }

    /// Create a `String` matching the contents of the `LocatedString`
    #[expect(dead_code, reason = "Added for completeness.")]
    #[expect(
        clippy::inherent_to_string,
        reason = "Not provided as a proxy for display."
    )]
    pub(crate) fn to_string(&self) -> String {
        self.as_str().to_string()
    }

    /// Create a `SmolStr` matching the contents of the `LocatedString`
    pub(crate) fn to_smolstr(&self) -> SmolStr {
        self.as_str().into()
    }
}

// Allow LocatedStr to be the Key of a HashMap
// Make hash and eq consistent with &str representation
impl Hash for LocatedString {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_str().hash(state)
    }
}

impl PartialEq for LocatedString {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl Eq for LocatedString {}

// Allow Searching Located Object map using &str as the key
impl Borrow<str> for LocatedString {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl LocatedValue {
    /// Create a new `LocatedValue` of kind Null
    pub(crate) fn new_null(loc: Loc) -> Self {
        Self {
            kind: ValueKind::Null,
            loc,
        }
    }

    /// Create a new `LocatedValue` of kind Bool
    pub(crate) fn new_bool(b: bool, loc: Loc) -> Self {
        Self {
            kind: ValueKind::Bool(b),
            loc,
        }
    }

    /// Create a new `LocatedValue` of kind Number
    pub(crate) fn new_number(loc: Loc) -> Self {
        Self {
            kind: ValueKind::Number,
            loc,
        }
    }

    /// Create a new `LocatedValue` of kind String
    pub(crate) fn new_string(loc: Loc) -> Self {
        Self {
            kind: ValueKind::String,
            loc,
        }
    }

    /// Create a new `LocatedValue` of kind Array
    pub(crate) fn new_array(items: Vec<LocatedValue>, loc: Loc) -> Self {
        Self {
            kind: ValueKind::Array(items),
            loc,
        }
    }

    /// Create a new `LocatedValue` of kind Object
    pub(crate) fn new_object(items: LinkedHashMap<LocatedString, LocatedValue>, loc: Loc) -> Self {
        Self {
            kind: ValueKind::Object(items),
            loc,
        }
    }

    /// Retrieve the kind of the `LocatedValue`
    #[expect(dead_code, reason = "Added for completeness.")]
    pub(crate) fn as_kind(&self) -> &ValueKind {
        &self.kind
    }

    /// Unwrap the `LocatedValue` to get its underlying `ValueKind`
    #[expect(dead_code, reason = "Added for completeness.")]
    pub(crate) fn into_kind(self) -> ValueKind {
        self.kind
    }

    /// Retrieve the location of the `LocatedValue`
    pub(crate) fn as_loc(&self) -> &Loc {
        &self.loc
    }

    /// Unwrap the `LocatedValue` to get its underlying Location
    #[expect(dead_code, reason = "Added for completeness.")]
    pub(crate) fn into_loc(self) -> Loc {
        self.loc
    }

    /// Returns if this `LocatedValue` is of kind Null
    pub(crate) fn is_null(&self) -> bool {
        matches!(self.kind, ValueKind::Null)
    }

    /// Returns if this `LocatedValue` is of kind Bool
    pub(crate) fn is_bool(&self) -> bool {
        matches!(self.kind, ValueKind::Bool(_))
    }

    /// Returns Some(b) if this `LocatedValue` represents
    /// the boolean value b. Otherwise, returns None
    pub(crate) fn get_bool(&self) -> Option<bool> {
        match self.kind {
            ValueKind::Bool(b) => Some(b),
            _ => None,
        }
    }

    /// Returns if this `LocatedValue` is of kind Number
    pub(crate) fn is_number(&self) -> bool {
        matches!(self.kind, ValueKind::Number)
    }

    /// Returns Some(str) if this `LocatedValue` represents
    /// a Number, where str is a `&str` representing the numeric litral.
    /// Otherwise, returns None
    pub(crate) fn get_numeric_str(&self) -> Option<&str> {
        match self.kind {
            ValueKind::Number => {
                let start = self.loc.start();
                let end = self.loc.end();
                #[expect(
                    clippy::string_slice,
                    reason = "By construction the indexes are guaranteed to satisfy 0 <= start <= end < self.loc.src.len()."
                )]
                Some(&self.loc.src[start..end])
            }
            _ => None,
        }
    }

    /// Returns if this `LocatedValue` is of kind String
    pub(crate) fn is_string(&self) -> bool {
        matches!(self.kind, ValueKind::String)
    }

    /// Returns Some(str) if this `Located` value is of kind String
    /// where str is the string literal corresponding to this LocatedValue.
    /// Otherwise, return None if not of kind String.
    pub(crate) fn get_str(&self) -> Option<&str> {
        match self.kind {
            ValueKind::String => {
                let start = self.loc.start() + 1;
                let end = self.loc.end() - 1;
                #[expect(
                    clippy::string_slice,
                    reason = "By construction the indexes are guaranteed to satisfy 0 <= start <= end < self.loc.src.len()."
                )]
                Some(&self.loc.src[start..end])
            }
            _ => None,
        }
    }

    /// Returns Some(String) if this `Located` value is of kind String
    /// where String is the string literal corresponding to this LocatedValue.
    /// Otherwise, return None if not of kind String.
    pub(crate) fn get_string(&self) -> Option<String> {
        self.get_str().map(|s| s.to_string())
    }

    /// Returns Some(SmolStr) if this `Located` value is of kind String
    /// where SmolStr is the string literal corresponding to this LocatedValue.
    /// Otherwise, return None if not of kind String.
    pub(crate) fn get_smolstr(&self) -> Option<SmolStr> {
        self.get_str().map(|s| s.into())
    }

    /// Returns if this `LocatedValue` is of kind Array
    pub(crate) fn is_array(&self) -> bool {
        matches!(self.kind, ValueKind::Array(_))
    }

    /// Returns Some(values) where values is an array of `LocatedValue`s
    /// if this `LocatedValue` is of kind Array. Otherwise, returns None
    pub(crate) fn get_array(&self) -> Option<&[LocatedValue]> {
        match &self.kind {
            ValueKind::Array(items) => Some(items),
            _ => None,
        }
    }

    /// Returns if this `LocatedValue` is of kind Object
    pub(crate) fn is_object(&self) -> bool {
        matches!(self.kind, ValueKind::Object(_))
    }

    /// Returns Some(key_value_map) where key_value_map is a mapping from `LocatedString`s to
    /// `LocatedValue`s if this `LocatedValue` is of kind Object. Otherwise, returns None
    pub(crate) fn get_object(&self) -> Option<&LinkedHashMap<LocatedString, LocatedValue>> {
        match &self.kind {
            ValueKind::Object(items) => Some(items),
            _ => None,
        }
    }

    /// returns Some(value) if this `LocatedValue` is of kind Object and the
    /// (key, value) mapping appears within the Object
    pub(crate) fn get(&self, key: impl AsRef<str>) -> Option<&LocatedValue> {
        self.get_object().and_then(|obj| obj.get(key.as_ref()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cool_asserts::assert_matches;

    fn new_loc(str: &str) -> Loc {
        Loc::new((0, str.len()), std::sync::Arc::from(str))
    }

    #[test]
    fn test_isbool() {
        assert!(LocatedValue::new_bool(true, new_loc("true")).is_bool());
        assert!(LocatedValue::new_bool(false, new_loc("false")).is_bool());
        assert!(!LocatedValue::new_null(new_loc("null")).is_bool());
        assert!(!LocatedValue::new_number(new_loc("0.1")).is_bool());
        assert!(!LocatedValue::new_string(new_loc("my cool str")).is_bool());
        assert!(!LocatedValue::new_array(Vec::new(), new_loc("[]")).is_bool());
        assert!(!LocatedValue::new_object(LinkedHashMap::new(), new_loc("{}")).is_bool());
    }

    #[test]
    fn test_getbool() {
        assert_matches!(
            LocatedValue::new_bool(true, new_loc("true")).get_bool(),
            Some(true)
        );
        assert_matches!(
            LocatedValue::new_bool(false, new_loc("false")).get_bool(),
            Some(false)
        );
        assert_matches!(LocatedValue::new_null(new_loc("null")).get_bool(), None);
        assert_matches!(LocatedValue::new_number(new_loc("0.1")).get_bool(), None);
        assert_matches!(
            LocatedValue::new_string(new_loc("my cool str")).get_bool(),
            None
        );
        assert_matches!(
            LocatedValue::new_array(Vec::new(), new_loc("[]")).get_bool(),
            None
        );
        assert_matches!(
            LocatedValue::new_object(LinkedHashMap::new(), new_loc("{}")).get_bool(),
            None
        );
    }

    #[test]
    fn test_isnull() {
        assert!(!LocatedValue::new_bool(true, new_loc("true")).is_null());
        assert!(!LocatedValue::new_bool(false, new_loc("false")).is_null());
        assert!(LocatedValue::new_null(new_loc("null")).is_null());
        assert!(!LocatedValue::new_number(new_loc("0.1")).is_null());
        assert!(!LocatedValue::new_string(new_loc("my cool str")).is_null());
        assert!(!LocatedValue::new_array(Vec::new(), new_loc("[]")).is_null());
        assert!(!LocatedValue::new_object(LinkedHashMap::new(), new_loc("{}")).is_null());
    }

    #[test]
    fn test_isnumber() {
        assert!(!LocatedValue::new_bool(true, new_loc("true")).is_number());
        assert!(!LocatedValue::new_bool(false, new_loc("false")).is_number());
        assert!(!LocatedValue::new_null(new_loc("null")).is_number());
        assert!(LocatedValue::new_number(new_loc("0.1")).is_number());
        assert!(!LocatedValue::new_string(new_loc("my cool str")).is_number());
        assert!(!LocatedValue::new_array(Vec::new(), new_loc("[]")).is_number());
        assert!(!LocatedValue::new_object(LinkedHashMap::new(), new_loc("{}")).is_number());
    }

    #[test]
    fn test_get_numeric_str() {
        assert_matches!(
            LocatedValue::new_bool(true, new_loc("true")).get_numeric_str(),
            None
        );
        assert_matches!(
            LocatedValue::new_bool(false, new_loc("false")).get_numeric_str(),
            None
        );
        assert_matches!(
            LocatedValue::new_null(new_loc("null")).get_numeric_str(),
            None
        );
        assert_matches!(
            LocatedValue::new_number(new_loc("0.1")).get_numeric_str(),
            Some(..)
        );
        assert_matches!(
            LocatedValue::new_string(new_loc("my cool str")).get_numeric_str(),
            None
        );
        assert_matches!(
            LocatedValue::new_array(Vec::new(), new_loc("[]")).get_numeric_str(),
            None
        );
        assert_matches!(
            LocatedValue::new_object(LinkedHashMap::new(), new_loc("{}")).get_numeric_str(),
            None
        );
    }

    #[test]
    fn test_isstring() {
        assert!(!LocatedValue::new_bool(true, new_loc("true")).is_string());
        assert!(!LocatedValue::new_bool(false, new_loc("false")).is_string());
        assert!(!LocatedValue::new_null(new_loc("null")).is_string());
        assert!(!LocatedValue::new_number(new_loc("0.1")).is_string());
        assert!(LocatedValue::new_string(new_loc("my cool str")).is_string());
        assert!(!LocatedValue::new_array(Vec::new(), new_loc("[]")).is_string());
        assert!(!LocatedValue::new_object(LinkedHashMap::new(), new_loc("{}")).is_string());
    }

    #[test]
    fn test_get_str() {
        assert_matches!(
            LocatedValue::new_bool(true, new_loc("true")).get_str(),
            None
        );
        assert_matches!(
            LocatedValue::new_bool(false, new_loc("false")).get_str(),
            None
        );
        assert_matches!(LocatedValue::new_null(new_loc("null")).get_str(), None);
        assert_matches!(LocatedValue::new_number(new_loc("0.1")).get_str(), None);
        assert_matches!(
            LocatedValue::new_string(new_loc("my cool str")).get_str(),
            Some(..)
        );
        assert_matches!(
            LocatedValue::new_array(Vec::new(), new_loc("[]")).get_str(),
            None
        );
        assert_matches!(
            LocatedValue::new_object(LinkedHashMap::new(), new_loc("{}")).get_str(),
            None
        );
    }

    #[test]
    fn test_get_string() {
        assert_matches!(
            LocatedValue::new_bool(true, new_loc("true")).get_string(),
            None
        );
        assert_matches!(
            LocatedValue::new_bool(false, new_loc("false")).get_string(),
            None
        );
        assert_matches!(LocatedValue::new_null(new_loc("null")).get_string(), None);
        assert_matches!(LocatedValue::new_number(new_loc("0.1")).get_string(), None);
        assert_matches!(
            LocatedValue::new_string(new_loc("my cool str")).get_string(),
            Some(..)
        );
        assert_matches!(
            LocatedValue::new_array(Vec::new(), new_loc("[]")).get_string(),
            None
        );
        assert_matches!(
            LocatedValue::new_object(LinkedHashMap::new(), new_loc("{}")).get_string(),
            None
        );
    }

    #[test]
    fn test_get_smolstr() {
        assert_matches!(
            LocatedValue::new_bool(true, new_loc("true")).get_smolstr(),
            None
        );
        assert_matches!(
            LocatedValue::new_bool(false, new_loc("false")).get_smolstr(),
            None
        );
        assert_matches!(LocatedValue::new_null(new_loc("null")).get_smolstr(), None);
        assert_matches!(LocatedValue::new_number(new_loc("0.1")).get_smolstr(), None);
        assert_matches!(
            LocatedValue::new_string(new_loc("my cool str")).get_smolstr(),
            Some(..)
        );
        assert_matches!(
            LocatedValue::new_array(Vec::new(), new_loc("[]")).get_smolstr(),
            None
        );
        assert_matches!(
            LocatedValue::new_object(LinkedHashMap::new(), new_loc("{}")).get_smolstr(),
            None
        );
    }

    #[test]
    fn test_isarray() {
        assert!(!LocatedValue::new_bool(true, new_loc("true")).is_array());
        assert!(!LocatedValue::new_bool(false, new_loc("false")).is_array());
        assert!(!LocatedValue::new_null(new_loc("null")).is_array());
        assert!(!LocatedValue::new_number(new_loc("0.1")).is_array());
        assert!(!LocatedValue::new_string(new_loc("my cool str")).is_array());
        assert!(LocatedValue::new_array(Vec::new(), new_loc("[]")).is_array());
        assert!(!LocatedValue::new_object(LinkedHashMap::new(), new_loc("{}")).is_array());
    }

    #[test]
    fn test_get_array() {
        assert_matches!(
            LocatedValue::new_bool(true, new_loc("true")).get_array(),
            None
        );
        assert_matches!(
            LocatedValue::new_bool(false, new_loc("false")).get_array(),
            None
        );
        assert_matches!(LocatedValue::new_null(new_loc("null")).get_array(), None);
        assert_matches!(LocatedValue::new_number(new_loc("0.1")).get_array(), None);
        assert_matches!(
            LocatedValue::new_string(new_loc("my cool str")).get_array(),
            None
        );
        assert_matches!(
            LocatedValue::new_array(Vec::new(), new_loc("[]")).get_array(),
            Some(..)
        );
        assert_matches!(
            LocatedValue::new_object(LinkedHashMap::new(), new_loc("{}")).get_array(),
            None
        );
    }

    #[test]
    fn test_isobject() {
        assert!(!LocatedValue::new_bool(true, new_loc("true")).is_object());
        assert!(!LocatedValue::new_bool(false, new_loc("false")).is_object());
        assert!(!LocatedValue::new_null(new_loc("null")).is_object());
        assert!(!LocatedValue::new_number(new_loc("0.1")).is_object());
        assert!(!LocatedValue::new_string(new_loc("my cool str")).is_object());
        assert!(!LocatedValue::new_array(Vec::new(), new_loc("[]")).is_object());
        assert!(LocatedValue::new_object(LinkedHashMap::new(), new_loc("{}")).is_object());
    }

    #[test]
    fn test_get_object() {
        assert_matches!(
            LocatedValue::new_bool(true, new_loc("true")).get_object(),
            None
        );
        assert_matches!(
            LocatedValue::new_bool(false, new_loc("false")).get_object(),
            None
        );
        assert_matches!(LocatedValue::new_null(new_loc("null")).get_object(), None);
        assert_matches!(LocatedValue::new_number(new_loc("0.1")).get_object(), None);
        assert_matches!(
            LocatedValue::new_string(new_loc("my cool str")).get_object(),
            None
        );
        assert_matches!(
            LocatedValue::new_array(Vec::new(), new_loc("[]")).get_object(),
            None
        );
        assert_matches!(
            LocatedValue::new_object(LinkedHashMap::new(), new_loc("{}")).get_object(),
            Some(..)
        );
    }
}
