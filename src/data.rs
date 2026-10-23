//! JSON content files. They load through Bevy's asset server so the same code works natively and in the browser.

use std::collections::HashMap;

use bevy::asset::io::Reader;
use bevy::asset::{AssetLoader, LoadContext};
use bevy::prelude::*;
use serde::Deserialize;

/// The raw text of a JSON (or JSON-with-comments) file; parsed by whoever needs it.
#[derive(Asset, TypePath)]
pub struct JsonDoc(pub String);

#[derive(Default, TypePath)]
pub struct JsonDocLoader;

impl AssetLoader for JsonDocLoader {
    type Asset = JsonDoc;
    type Settings = ();
    type Error = std::io::Error;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _context: &mut LoadContext<'_>,
    ) -> Result<JsonDoc, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        String::from_utf8(bytes).map(JsonDoc).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }

    fn extensions(&self) -> &[&str] {
        &["json"]
    }
}

/// The parts of an enemies.json entry this build uses.
#[derive(Deserialize, Clone, Debug)]
#[serde(default)]
pub struct EnemyDef {
    pub name: String,
    pub sprite: String,
    pub size: [f32; 2],
    #[serde(rename = "move")]
    pub movement: String,
    pub speed: f32,
    pub hp: f32,
    pub flying: bool,
    pub sight: f32,
}

impl Default for EnemyDef {
    fn default() -> Self {
        Self {
            name: String::new(),
            sprite: "monster_03".into(),
            size: [12.0, 10.0],
            movement: "walk".into(),
            speed: 20.0,
            hp: 1.0,
            flying: false,
            sight: 96.0,
        }
    }
}

pub fn parse_enemies(text: &str) -> Result<HashMap<String, EnemyDef>, String> {
    serde_json::from_str(&strip_jsonc(text)).map_err(|e| format!("enemies.json: {e}"))
}

/// Removes `//` comments and trailing commas, which the hand-written data files use, so serde_json can read them.
pub fn strip_jsonc(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    let mut in_string = false;
    while let Some(c) = chars.next() {
        if in_string {
            out.push(c);
            if c == '\\' {
                out.extend(chars.next());
            } else if c == '"' {
                in_string = false;
            }
        } else if c == '"' {
            in_string = true;
            out.push(c);
        } else if c == '/' && chars.peek() == Some(&'/') {
            while chars.next_if(|&n| n != '\n').is_some() {}
        } else if c == ',' {
            let rest = chars.clone().find(|n| !n.is_whitespace());
            if !matches!(rest, Some('}' | ']')) {
                out.push(c);
            }
        } else {
            out.push(c);
        }
    }
    out
}
