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

// PANIC SAFETY: All parsed identifiers are constants which we know are valid
#![allow(clippy::unwrap_used)]

use cedar_policy_core::ast::{AnyId, EntityType, Name, UnreservedId};
use cedar_policy_core::validator::RawName;
use std::sync::LazyLock;

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
}
