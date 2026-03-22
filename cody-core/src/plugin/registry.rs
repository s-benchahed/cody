use std::collections::HashMap;
use std::sync::Arc;
use crate::plugin::LanguagePlugin;
use crate::plugin::{javascript::JavaScriptPlugin, typescript::TypeScriptPlugin,
                    python::PythonPlugin, ruby::RubyPlugin, rust_lang::RustPlugin};

pub type PluginRegistry = HashMap<String, Arc<dyn LanguagePlugin>>;

pub fn build_registry() -> PluginRegistry {
    let mut map: PluginRegistry = HashMap::new();
    let plugins: Vec<Arc<dyn LanguagePlugin>> = vec![
        Arc::new(JavaScriptPlugin),
        Arc::new(TypeScriptPlugin),
        Arc::new(PythonPlugin),
        Arc::new(RubyPlugin),
        Arc::new(RustPlugin),
    ];
    for plugin in plugins {
        for ext in plugin.extensions() {
            map.insert(ext.to_string(), Arc::clone(&plugin));
        }
    }
    map
}

pub fn get_plugin<'a>(registry: &'a PluginRegistry, path: &std::path::Path) -> Option<&'a Arc<dyn LanguagePlugin>> {
    let ext = path.extension()?.to_str()?.to_lowercase();
    registry.get(&ext)
}
