use std::collections::HashMap;

/// Assigns $v1, $v2, ... names to values flowing through a trace.
/// The same (medium, key_norm) always gets the same name within a trace.
pub struct BaggageMap {
    map:     HashMap<String, String>,
    counter: usize,
}

impl BaggageMap {
    pub fn new() -> Self {
        Self { map: HashMap::new(), counter: 0 }
    }

    /// Get or create a baggage name for (medium, key_norm) or a data-flow path.
    pub fn get_or_assign(&mut self, canonical_key: &str) -> String {
        if let Some(name) = self.map.get(canonical_key) {
            return name.clone();
        }
        self.counter += 1;
        let name = format!("$v{}", self.counter);
        self.map.insert(canonical_key.to_string(), name.clone());
        name
    }

    pub fn all_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.map.values().cloned().collect();
        names.sort();
        names
    }
}

impl Default for BaggageMap {
    fn default() -> Self { Self::new() }
}
