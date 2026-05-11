use std::{
    fmt, io,
    path::{Path, PathBuf},
};

use thiserror::Error;

pub type Result<T, E = AppError> = std::result::Result<T, E>;

#[derive(Debug, Error)]
pub enum AppError {
    #[error(transparent)]
    Config(#[from] TomlConfigError),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigOrigin {
    File(PathBuf),
    Inline,
}

impl ConfigOrigin {
    pub fn file(path: impl AsRef<Path>) -> Self {
        Self::File(path.as_ref().to_path_buf())
    }
}

impl fmt::Display for ConfigOrigin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::File(path) => write!(f, "config file `{}`", path.display()),
            Self::Inline => f.write_str("inline config"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceLocation {
    pub line: usize,
    pub column: usize,
}

impl SourceLocation {
    pub fn from_offset(input: &str, offset: usize) -> Self {
        let mut line = 1;
        let mut column = 1;

        for ch in input[..offset.min(input.len())].chars() {
            if ch == '\n' {
                line += 1;
                column = 1;
            } else {
                column += 1;
            }
        }

        Self { line, column }
    }
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
#[error("`{field}` {message}")]
pub struct ConfigValidationError {
    field: String,
    message: String,
}

impl ConfigValidationError {
    pub fn new(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
        }
    }

    pub fn field(&self) -> &str {
        &self.field
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

#[derive(Debug)]
pub enum TomlConfigError {
    Read {
        origin: ConfigOrigin,
        source: io::Error,
    },
    Parse {
        origin: ConfigOrigin,
        location: Option<SourceLocation>,
        message: String,
    },
    Validation {
        origin: ConfigOrigin,
        source: ConfigValidationError,
    },
}

impl TomlConfigError {
    pub fn read(path: impl AsRef<Path>, source: io::Error) -> Self {
        Self::Read {
            origin: ConfigOrigin::file(path),
            source,
        }
    }

    pub fn parse(origin: ConfigOrigin, input: &str, source: toml::de::Error) -> Self {
        let location = source
            .span()
            .map(|span| SourceLocation::from_offset(input, span.start));

        Self::Parse {
            origin,
            location,
            message: source.message().to_owned(),
        }
    }

    pub fn validation(origin: ConfigOrigin, source: ConfigValidationError) -> Self {
        Self::Validation { origin, source }
    }
}

impl fmt::Display for TomlConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { origin, source } => write!(f, "failed to read {origin}: {source}"),
            Self::Parse {
                origin,
                location: Some(location),
                message,
            } => write!(
                f,
                "failed to parse {origin} at line {}, column {}: {message}",
                location.line, location.column
            ),
            Self::Parse {
                origin,
                location: None,
                message,
            } => write!(f, "failed to parse {origin}: {message}"),
            Self::Validation { origin, source } => {
                write!(f, "invalid {origin}: {source}")
            }
        }
    }
}

impl std::error::Error for TomlConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read { source, .. } => Some(source),
            Self::Validation { source, .. } => Some(source),
            Self::Parse { .. } => None,
        }
    }
}
