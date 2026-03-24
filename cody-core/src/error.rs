use thiserror::Error;

#[derive(Debug, Error)]
pub enum CodyError {
    #[error("parse error in {file}: {msg}")]
    Parse { file: String, msg: String },

    #[error("unsupported language for file: {0}")]
    UnsupportedLanguage(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Other(#[from] anyhow::Error),
}
