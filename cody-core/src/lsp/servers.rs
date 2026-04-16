use std::collections::HashMap;

/// Specification for a language server process.
#[derive(Debug, Clone)]
pub struct ServerSpec {
    pub binary:      &'static str,
    pub args:        &'static [&'static str],
    /// Language IDs (as stored in file_meta.language) served by this binary.
    pub languages:   &'static [&'static str],
    /// LSP `languageId` string sent in textDocument/didOpen.
    pub language_id: &'static str,
}

static SPECS: &[ServerSpec] = &[
    ServerSpec {
        binary:      "typescript-language-server",
        args:        &["--stdio"],
        languages:   &["typescript", "javascript"],
        language_id: "typescript",
    },
    ServerSpec {
        binary:      "rust-analyzer",
        args:        &[],
        languages:   &["rust"],
        language_id: "rust",
    },
    ServerSpec {
        binary:      "pyright-langserver",
        args:        &["--stdio"],
        languages:   &["python"],
        language_id: "python",
    },
    ServerSpec {
        binary:      "pylsp",
        args:        &[],
        languages:   &["python"],
        language_id: "python",
    },
    // Go: official language server
    ServerSpec {
        binary:      "gopls",
        args:        &["serve"],
        languages:   &["go"],
        language_id: "go",
    },
    // Ruby: ruby-lsp (Shopify) preferred, solargraph as fallback
    ServerSpec {
        binary:      "ruby-lsp",
        args:        &[],
        languages:   &["ruby"],
        language_id: "ruby",
    },
    ServerSpec {
        binary:      "solargraph",
        args:        &["stdio"],
        languages:   &["ruby"],
        language_id: "ruby",
    },
];

/// Returns the first available server spec for each language,
/// keyed by the language name (as stored in file_meta).
pub fn detect() -> HashMap<String, &'static ServerSpec> {
    let mut map: HashMap<String, &'static ServerSpec> = HashMap::new();
    for spec in SPECS {
        // Check binary is in PATH
        if which(spec.binary).is_none() { continue; }
        for lang in spec.languages {
            map.entry(lang.to_string()).or_insert(spec);
        }
    }
    map
}

fn which(binary: &str) -> Option<std::path::PathBuf> {
    std::env::var_os("PATH").and_then(|path| {
        std::env::split_paths(&path).find_map(|dir| {
            let candidate = dir.join(binary);
            if candidate.exists() { Some(candidate) } else { None }
        })
    })
}
