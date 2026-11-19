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

#![expect(
    clippy::unwrap_used,
    reason = "All parsed identifiers are constants which we know are valid."
)]

use cedar_policy_core::ast::{AnyId, EntityType, Name, UnreservedId};
use cedar_policy_core::validator::RawName;
use cedar_policy_core::FromNormalizedStr;
use std::fmt::Write;
use std::sync::LazyLock;

/// Encodes an arbitrary strings into a valid Cedar identifier. Encoding (and decoding)
/// is injective.
/// Encodings returns:
/// - the input string when that string is already a normal unreserved Cedar identifier that
///   does not start with `CDRG26__` (which we reserve as an encoding prefix), or
/// - a string starting with `CDRG26__` followed by the name where ASCII letters and digits are
///   kept and every other character, `_` included, is replaced by is hex encoding between
///   underscores. For example, `get-weather` gives `CDRG26__get_2d_weather`, `@odata.type` gives
///   `CDRG26___40_odata_2e_type`, and the empty name gives `CDRG26__`.
///
/// The derivation is injective:
/// - A derived name of the second case always starts with `CDRG26__`, and a name returned
///   unchanged by the first case never does (it is excluded), so the two cases have disjoint
///   ranges.
/// - Within the second case, every `_` in the output starts an escape `_{hex}_` (literal `_` is
///   escaped too), so the output decodes back to the input unambiguously.
pub fn encode_id(name: &str) -> String {
    if !name.starts_with(ESCAPE_PREFIX) && UnreservedId::from_normalized_str(name).is_ok() {
        return name.to_string();
    }
    let mut id = String::with_capacity(name.len() + ESCAPE_PREFIX.len());
    id.push_str(ESCAPE_PREFIX);
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            id.push(c);
        } else {
            // Writing to a `String` cannot fail.
            let _ = write!(id, "_{:x}_", u32::from(c));
        }
    }
    id
}

/// Prefix of a name derived by escaping
const ESCAPE_PREFIX: &str = "CDRG26__";

/// `encode_id`, as an `UnreservedId`. `encode_id` always produces a normalized unreserved
/// identifier, so this never fails.
#[expect(
    clippy::expect_used,
    reason = "encode_id always returns a normalized unreserved identifier"
)]
pub(super) fn encoded_id(name: &str) -> UnreservedId {
    UnreservedId::from_normalized_str(&encode_id(name))
        .expect("encode_id always produces a valid unreserved identifier")
}

pub(super) static MCP_PRINCIPAL: LazyLock<AnyId> =
    LazyLock::new(|| "mcp_principal".parse().unwrap());
pub(super) static MCP_RESOURCE: LazyLock<AnyId> = LazyLock::new(|| "mcp_resource".parse().unwrap());
pub(super) static MCP_CONTEXT: LazyLock<AnyId> = LazyLock::new(|| "mcp_context".parse().unwrap());
pub(super) static MCP_ACTION: LazyLock<AnyId> = LazyLock::new(|| "mcp_action".parse().unwrap());

// Namespace names
pub(super) static INPUT_NAME: LazyLock<Name> = LazyLock::new(|| "Input".parse().unwrap());
pub(super) static OUTPUT_NAME: LazyLock<Name> = LazyLock::new(|| "Output".parse().unwrap());

// Cedar built-in and extension types
pub(super) static BOOL_TYPE: LazyLock<RawName> = LazyLock::new(|| "Bool".parse().unwrap());
pub(super) static LONG_TYPE: LazyLock<RawName> = LazyLock::new(|| "Long".parse().unwrap());
pub(super) static STRING_TYPE: LazyLock<RawName> = LazyLock::new(|| "String".parse().unwrap());
pub(super) static DECIMAL_TYPE: LazyLock<RawName> = LazyLock::new(|| "decimal".parse().unwrap());
pub(super) static DATETIME_TYPE: LazyLock<RawName> = LazyLock::new(|| "datetime".parse().unwrap());
pub(super) static DURATION_TYPE: LazyLock<RawName> = LazyLock::new(|| "duration".parse().unwrap());
pub(super) static IPADDR_TYPE: LazyLock<RawName> = LazyLock::new(|| "ipaddr".parse().unwrap());
pub(super) static ACTION: LazyLock<EntityType> = LazyLock::new(|| "Action".parse().unwrap());

// Cedar extension functions
pub(super) static DECIMAL_CTOR: LazyLock<Name> = LazyLock::new(|| "decimal".parse().unwrap());
pub(super) static DATETIME_CTOR: LazyLock<Name> = LazyLock::new(|| "datetime".parse().unwrap());
pub(super) static DURATION_CTOR: LazyLock<Name> = LazyLock::new(|| "duration".parse().unwrap());
pub(super) static IPADDR_CTOR: LazyLock<Name> = LazyLock::new(|| "ip".parse().unwrap());

// Special entity type names
pub(super) static FLOAT_TYPE: LazyLock<UnreservedId> = LazyLock::new(|| "Float".parse().unwrap());
pub(super) static NUMBER_TYPE: LazyLock<UnreservedId> = LazyLock::new(|| "Number".parse().unwrap());
pub(super) static NULL_TYPE: LazyLock<UnreservedId> = LazyLock::new(|| "Null".parse().unwrap());
pub(super) static UNKNOWN_TYPE: LazyLock<UnreservedId> =
    LazyLock::new(|| "Unknown".parse().unwrap());

#[cfg(test)]
mod test {
    use super::*;

    // Forces evaluation of lazy locks, so that we'll see any parse errors
    // regardless of whether the code that uses the identifier is covered by
    // other tests.
    #[test]
    fn identifiers_are_valid() {
        let _ = *MCP_PRINCIPAL;
        let _ = *MCP_RESOURCE;
        let _ = *MCP_CONTEXT;
        let _ = *MCP_ACTION;
        let _ = *INPUT_NAME;
        let _ = *OUTPUT_NAME;
        let _ = *BOOL_TYPE;
        let _ = *LONG_TYPE;
        let _ = *STRING_TYPE;
        let _ = *DECIMAL_TYPE;
        let _ = *DATETIME_TYPE;
        let _ = *DURATION_TYPE;
        let _ = *IPADDR_TYPE;
        let _ = *ACTION;
        let _ = *DECIMAL_CTOR;
        let _ = *DATETIME_CTOR;
        let _ = *DURATION_CTOR;
        let _ = *IPADDR_CTOR;
        let _ = *FLOAT_TYPE;
        let _ = *NUMBER_TYPE;
        let _ = *NULL_TYPE;
        let _ = *UNKNOWN_TYPE;
    }

    #[test]
    fn derive_identifier_keeps_valid_identifiers() {
        for name in ["foo", "get_weather", "Foo123", "_private", "a_b_c", "CDR"] {
            assert_eq!(encode_id(name), name, "{name}");
        }
    }

    #[test]
    fn derive_identifier_escapes_invalid_names() {
        let cases = [
            ("get-weather", "CDRG26__get_2d_weather"),
            ("@odata.type", "CDRG26___40_odata_2e_type"),
            ("foo::bar", "CDRG26__foo_3a__3a_bar"),
            ("get weather", "CDRG26__get_20_weather"),
            ("", "CDRG26__"),
            // Reserved words are not unreserved identifiers.
            ("if", "CDRG26__if"),
            ("1abc", "CDRG26__1abc"),
            ("é", "CDRG26___e9_"),
            // A name that starts with the escape prefix is always escaped.
            ("CDRG26__foo", "CDRG26__CDRG26_5f__5f_foo"),
            ("CDRG26__", "CDRG26__CDRG26_5f__5f_"),
        ];
        for (name, expected) in cases {
            assert_eq!(encode_id(name), expected, "{name}");
        }
    }

    #[test]
    fn derive_identifier_is_injective_on_old_collisions() {
        let pairs = [
            ("get-weather", "get_weather"),
            ("get-weather", "get_2d_weather"),
            ("get-weather", "CDRG26__get_2d_weather"),
            ("a.b", "a_b"),
            ("a-b", "a.b"),
            ("", "CDRG26__"),
        ];
        for (a, b) in pairs {
            assert_ne!(encode_id(a), encode_id(b), "{a} vs {b}");
        }
    }

    #[test]
    fn derived_ids_are_valid() {
        for name in [
            "get-weather",
            "@odata.type",
            "get_weather invalid stuff : oh no",
            "",
            "if",
            "CDRG26__x",
            "日本",
        ] {
            let id = encoded_id(name);
            assert_eq!(id.to_string(), encode_id(name), "{name}");
        }
    }

    /// Inverse of `derive_identifier`, for the injectivity test. Not part of the crate.
    fn decode_identifier(id: &str) -> String {
        let Some(escaped) = id.strip_prefix(ESCAPE_PREFIX) else {
            return id.to_string();
        };
        let mut out = String::new();
        let mut chars = escaped.chars();
        while let Some(c) = chars.next() {
            if c != '_' {
                out.push(c);
                continue;
            }
            // `_{hex}_`: take hex up to the closing `_`.
            let hex: String = chars.by_ref().take_while(|c| *c != '_').collect();
            let scalar = u32::from_str_radix(&hex, 16).expect("hex");
            out.push(char::from_u32(scalar).expect("scalar"));
        }
        out
    }

    /// A left inverse exists, so `derive_identifier` is injective. Covers plain names, names
    /// that collided under the old base32 decode, and names that look like an encoding.
    #[test]
    fn derive_identifier_round_trips() {
        for name in [
            "foo",
            "get_weather",
            "get-weather",
            "get_2d_weather",
            "@odata.type",
            "a.b",
            "a_b",
            "get weather : oh no",
            "",
            "if",
            "1abc",
            "é",
            "日本",
            "CDRG26__",
            "CDRG26__foo",
            "CDRG26__get_2d_weather",
        ] {
            assert_eq!(decode_identifier(&encode_id(name)), name, "{name}");
        }
    }
}
