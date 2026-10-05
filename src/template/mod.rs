// Copyright (C) 2026 Pumpkin-MC Contributors
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

pub mod languages;
pub mod templates;
pub mod wizard;

pub use languages::PluginLanguage;
pub use wizard::{run_wizard, WizardConfig};
