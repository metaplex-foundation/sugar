use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("cound not parse the config file ({0})")]
    ParseError(String),

    #[error("missing configuration file '{0}'")]
    MissingFileError(String),

    #[error("the configuration file path is invalid ('{0}' is a drectory)")]
    InvalidPathError(String),

    #[error("cound not open config file '{0}'")]
    PermissionError(String),

    #[error("invalid cluster '{0}'")]
    InvalidCluster(String),

    #[error("invalid upload method '{0}'")]
    InvalidUploadMethod(String),
}
