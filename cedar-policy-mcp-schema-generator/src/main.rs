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

use cedar_policy_mcp_schema_generator::{SchemaGenerator, ServerDescription};
use miette::Result;

use cedar_policy_core::extensions::Extensions;
use cedar_policy_core::validator::json_schema::Fragment;

fn main() -> Result<()> {
    let description = ServerDescription::from_json_file("tool.json")?;

    #[allow(
        clippy::unwrap_used,
        reason = "It's fine if this demo main file panics"
    )]
    // PANIC SAFETY: not part of the library
    let schema_file = std::fs::File::open("tool.cedarschema").unwrap();
    let schema = Fragment::from_cedarschema_file(schema_file, Extensions::all_available())?.0;
    let mut generator = SchemaGenerator::new(schema)?;
    generator.add_actions_from_server_description(&description)?;
    println!("{}", generator.get_schema().clone().to_cedarschema()?);
    Ok(())
}
