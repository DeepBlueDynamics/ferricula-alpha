//! Persona loaded from `agent.toml` in the memory directory.
//!
//! The engine is persona-neutral: who the agent is (name, role, voice) is
//! configuration and data, never compiled in. `agent.toml` is parsed
//! leniently — `name`/`role`/`voice` may sit at the top level or under an
//! `[agent]`, `[identity]`, or `[persona]` table, and `voice` may be a string
//! or a table of strings. The raw text is also kept so prompts can include the
//! operator's full persona configuration verbatim.

use std::fs;
use std::path::Path;

/// Parsed persona plus its raw configuration text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Persona {
    pub name: String,
    pub role: Option<String>,
    pub voice: Option<String>,
    /// Raw `agent.toml` text (or a synthesized `name = ...` line).
    pub raw: String,
}

impl Persona {
    /// Load `memory_dir/agent.toml`; fall back to `fallback_name` (the
    /// configured `[models.identity].name`) when absent or unparsable.
    pub fn load(memory_dir: &Path, fallback_name: &str) -> Self {
        match fs::read_to_string(memory_dir.join("agent.toml")) {
            Ok(raw) => Self::parse(&raw, fallback_name),
            Err(_) => Self::fallback(fallback_name),
        }
    }

    pub fn fallback(name: &str) -> Self {
        Self {
            name: name.to_string(),
            role: None,
            voice: None,
            raw: format!("name = {name:?}"),
        }
    }

    pub fn parse(raw: &str, fallback_name: &str) -> Self {
        let table = raw.parse::<toml::Table>().ok();
        let lookup = |key: &str| -> Option<String> {
            let table = table.as_ref()?;
            let mut scopes: Vec<&toml::Table> = vec![table];
            for section in ["agent", "identity", "persona"] {
                if let Some(toml::Value::Table(inner)) = table.get(section) {
                    scopes.push(inner);
                }
            }
            scopes.iter().find_map(|scope| value_text(scope.get(key)?))
        };
        let name = lookup("name")
            .filter(|n| !n.trim().is_empty())
            .unwrap_or_else(|| fallback_name.to_string());
        Self {
            name,
            role: lookup("role").filter(|s| !s.trim().is_empty()),
            voice: lookup("voice").filter(|s| !s.trim().is_empty()),
            raw: raw.to_string(),
        }
    }

    /// One-sentence self-description built from the persona, e.g.
    /// "You are Ada, a mathematician, backed by Ferricula memory."
    pub fn identity_line(&self) -> String {
        match &self.role {
            Some(role) => format!(
                "You are {}, {}, the single agent identity backed by Ferricula memory.",
                self.name,
                role.trim().trim_end_matches('.')
            ),
            None => format!(
                "You are {}, the single agent identity backed by Ferricula memory.",
                self.name
            ),
        }
    }
}

fn value_text(value: &toml::Value) -> Option<String> {
    match value {
        toml::Value::String(s) => Some(s.clone()),
        toml::Value::Array(items) => {
            let parts: Vec<String> = items.iter().filter_map(value_text).collect();
            (!parts.is_empty()).then(|| parts.join("; "))
        }
        toml::Value::Table(t) => {
            let parts: Vec<String> = t
                .iter()
                .filter_map(|(k, v)| value_text(v).map(|v| format!("{k}: {v}")))
                .collect();
            (!parts.is_empty()).then(|| parts.join("; "))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_top_level_and_nested_fields() {
        let p = Persona::parse(
            "name = \"Ada\"\nrole = \"a mathematician\"\n[voice]\ntone = \"precise\"\n",
            "Fallback",
        );
        assert_eq!(p.name, "Ada");
        assert_eq!(p.role.as_deref(), Some("a mathematician"));
        assert_eq!(p.voice.as_deref(), Some("tone: precise"));
        assert!(p.identity_line().starts_with("You are Ada, a mathematician,"));

        let p = Persona::parse("[agent]\nname = \"Bo\"\n", "Fallback");
        assert_eq!(p.name, "Bo");
        assert_eq!(p.role, None);
    }

    #[test]
    fn unparsable_or_missing_uses_fallback_name() {
        let p = Persona::parse("not = [valid", "Ferricula Agent");
        assert_eq!(p.name, "Ferricula Agent");
        assert_eq!(p.raw, "not = [valid");
        let p = Persona::load(Path::new("/definitely/not/here"), "Ferricula Agent");
        assert_eq!(p.name, "Ferricula Agent");
        assert!(p.identity_line().contains("Ferricula Agent"));
    }

    #[test]
    fn example_steve_persona_is_data_not_code() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../config/examples/steve/agent.toml");
        let raw = std::fs::read_to_string(path).unwrap();
        let p = Persona::parse(&raw, "x");
        assert_eq!(p.name, "Steve Jobs");
        assert!(p.role.is_some());
        assert!(p.voice.is_some());
    }
}
