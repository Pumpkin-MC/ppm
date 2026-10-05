// Copyright (C) 2026 Pumpkin-MC Contributors
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

pub mod c;
pub mod csharp;
pub mod go;
pub mod python;
pub mod rust;
pub mod typescript;

#[derive(Debug, Clone)]
pub struct TemplateContext {
    pub name: String,
    pub author: String,
    pub description: String,
    pub include_command: bool,
    pub include_event: bool,
}

impl TemplateContext {
    pub fn struct_name(&self) -> String {
        let mut s = String::new();
        let mut capitalize = true;
        for c in self.name.chars() {
            if c == '-' || c == '_' || c == ' ' {
                capitalize = true;
            } else if capitalize {
                s.extend(c.to_uppercase());
                capitalize = false;
            } else {
                s.push(c);
            }
        }
        if s.is_empty() {
            "Plugin".to_string()
        } else {
            s
        }
    }
}
