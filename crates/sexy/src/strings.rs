//! The game's string table (`properties/LawnStrings.txt`): `[NAME]` headers,
//! each followed by its text.
//!
//! Behaviour per ruslan831 decomp, `TodStringListReadItems` and
//! `TodStringTranslate`.

use std::collections::HashMap;

#[derive(Debug, Default, Clone)]
pub struct StringTable(HashMap<String, String>);

impl StringTable {
    /// Parses the table. Names are matched case-insensitively; values are
    /// trimmed and lose `\r`s but keep inner newlines.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut map = HashMap::new();
        let mut rest = text;
        while let Some(open) = rest.find('[') {
            let close = rest[open..].find(']').ok_or("missing ']' after '['")? + open;
            let name = rest[open + 1..close].trim();
            if name.is_empty() {
                return Err("empty string name".into());
            }
            rest = &rest[close + 1..];
            let end = rest.find('[').unwrap_or(rest.len());
            map.insert(name.to_ascii_uppercase(), rest[..end].trim().replace('\r', ""));
            rest = &rest[end..];
        }
        if !rest.trim().is_empty() {
            return Err("text outside any [NAME]".into());
        }
        Ok(Self(map))
    }

    pub fn get(&self, name: &str) -> Option<&str> {
        self.0.get(&name.to_ascii_uppercase()).map(String::as_str)
    }

    /// `[NAME]` becomes the named string; anything else is returned as is.
    pub fn translate(&self, s: &str) -> String {
        match s.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            Some(name) => self.get(name).map_or_else(|| format!("<Missing {name}>"), str::to_owned),
            None => s.to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_translates() {
        let t = StringTable::parse("[LOCALE]\r\nen_US\r\n\r\n[Loading]\nLOADING...\n\n[MULTI]\nline one\nline two\n").unwrap();
        assert_eq!(t.translate("[LOADING]"), "LOADING...");
        assert_eq!(t.get("multi"), Some("line one\nline two"));
        assert_eq!(t.translate("[NOPE]"), "<Missing NOPE>");
        assert_eq!(t.translate("plain"), "plain");
    }
}
