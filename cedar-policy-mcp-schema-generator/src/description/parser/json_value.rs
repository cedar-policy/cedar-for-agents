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

#[derive(Debug, Clone)]
enum ValueKind {
    Null,
    Bool(bool),
    Number,
    String,
    Array(Vec<LocatedValue>),
    Object(LinkedHashMap<LocatedString, LocatedValue>),
}

#[derive(Debug, Clone)]
pub(crate) struct LocatedString {
    loc: Loc,
}

#[derive(Debug, Clone)]
pub(crate) struct LocatedValue {
    kind: ValueKind,
    loc: Loc,
}

impl LocatedString {
    pub(crate) fn new(loc: Loc) -> Self {
        Self { loc }
    }

    pub(crate) fn as_loc(&self) -> &Loc {
        &self.loc
    }

    pub(crate) fn to_loc(self) -> Loc {
        self.loc
    }

    pub(crate) fn as_str(&self) -> &str {
        let start = self.loc.start() + 1;
        let end = self.loc.end() - 1;
        &self.loc.src[start..end]
    }

    #[allow(dead_code, reason = "Added for completeness.")]
    pub(crate) fn to_string(&self) -> String {
        self.as_str().to_string()
    }

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
    pub(crate) fn new_null(loc: Loc) -> Self {
        Self {
            kind: ValueKind::Null,
            loc,
        }
    }

    pub(crate) fn new_bool(b: bool, loc: Loc) -> Self {
        Self {
            kind: ValueKind::Bool(b),
            loc,
        }
    }

    pub(crate) fn new_number(loc: Loc) -> Self {
        Self {
            kind: ValueKind::Number,
            loc,
        }
    }

    pub(crate) fn new_string(loc: Loc) -> Self {
        Self {
            kind: ValueKind::String,
            loc,
        }
    }

    pub(crate) fn new_array(items: Vec<LocatedValue>, loc: Loc) -> Self {
        Self {
            kind: ValueKind::Array(items),
            loc,
        }
    }

    pub(crate) fn new_object(items: LinkedHashMap<LocatedString, LocatedValue>, loc: Loc) -> Self {
        Self {
            kind: ValueKind::Object(items),
            loc,
        }
    }

    pub(crate) fn as_loc(&self) -> &Loc {
        &self.loc
    }

    #[allow(dead_code, reason = "Added for completeness.")]
    pub(crate) fn to_loc(self) -> Loc {
        self.loc
    }

    #[allow(dead_code, reason = "Added for completeness.")]
    pub(crate) fn is_null(&self) -> bool {
        matches!(self.kind, ValueKind::Null)
    }

    #[allow(dead_code, reason = "Added for completeness.")]
    pub(crate) fn is_bool(&self) -> bool {
        matches!(self.kind, ValueKind::Bool(_))
    }

    pub(crate) fn get_bool(&self) -> Option<bool> {
        match self.kind {
            ValueKind::Bool(b) => Some(b),
            _ => None,
        }
    }

    #[allow(dead_code, reason = "Added for completeness.")]
    pub(crate) fn is_number(&self) -> bool {
        matches!(self.kind, ValueKind::Number)
    }

    #[allow(dead_code, reason = "Added for completeness.")]
    pub(crate) fn get_numeric_str(&self) -> Option<&str> {
        match self.kind {
            ValueKind::Number => {
                let start = self.loc.start();
                let end = self.loc.end();
                Some(&self.loc.src[start..end])
            }
            _ => None,
        }
    }

    // Get number functions here

    #[allow(dead_code, reason = "Added for completeness.")]
    pub(crate) fn is_string(&self) -> bool {
        matches!(self.kind, ValueKind::String)
    }

    pub(crate) fn get_str(&self) -> Option<&str> {
        match self.kind {
            ValueKind::String => {
                let start = self.loc.start() + 1;
                let end = self.loc.end() - 1;
                Some(&self.loc.src[start..end])
            }
            _ => None,
        }
    }

    pub(crate) fn get_string(&self) -> Option<String> {
        self.get_str().map(|s| s.to_string())
    }

    pub(crate) fn get_smolstr(&self) -> Option<SmolStr> {
        self.get_str().map(|s| s.into())
    }

    #[allow(dead_code, reason = "Added for completeness.")]
    pub(crate) fn is_array(&self) -> bool {
        matches!(self.kind, ValueKind::Array(_))
    }

    pub(crate) fn get_array(&self) -> Option<&[LocatedValue]> {
        match &self.kind {
            ValueKind::Array(items) => Some(items),
            _ => None,
        }
    }

    pub(crate) fn is_object(&self) -> bool {
        matches!(self.kind, ValueKind::Object(_))
    }

    pub(crate) fn get_object(&self) -> Option<&LinkedHashMap<LocatedString, LocatedValue>> {
        match &self.kind {
            ValueKind::Object(items) => Some(items),
            _ => None,
        }
    }

    pub(crate) fn get(&self, key: impl AsRef<str>) -> Option<&LocatedValue> {
        self.get_object().and_then(|obj| obj.get(key.as_ref()))
    }
}
