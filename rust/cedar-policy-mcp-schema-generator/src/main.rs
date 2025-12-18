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

use cedar_policy_mcp_schema_generator::{CliArgs, ErrorFormat};
use clap::Parser;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args = CliArgs::parse();

    let err_hook: Option<miette::ErrorHook> = match args.get_error_format() {
        ErrorFormat::Human => None, // This is the default.
        ErrorFormat::Plain => Some(Box::new(|_| {
            Box::new(miette::NarratableReportHandler::new())
        })),
        ErrorFormat::Json => Some(Box::new(|_| Box::new(miette::JSONReportHandler::new()))),
    };
    if let Some(err_hook) = err_hook {
        #[expect(
            clippy::expect_used,
            reason = "The function `set_hook` returns an error if a hook has already been installed. We have just entered `main`, so we know a hook has not been installed."
        )]
        miette::set_hook(err_hook).expect("failed to install error-reporting hook");
    }

    match args.exec() {
        Ok(_) => ExitCode::SUCCESS,
        Err(err) => {
            let report = miette::Report::new(err);
            eprintln!("{:?}", report);
            ExitCode::FAILURE
        }
    }
}
