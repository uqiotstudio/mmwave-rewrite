use thiserror::Error;

#[derive(Debug, Error)]
pub enum RadarInitError {
    #[error("Port unavailable")]
    PortUnavailable(String),
    #[error("Port not found")]
    PortNotFound(String),
    #[error("Inaccessible config")]
    InaccessibleConfig(String),
}

#[derive(Debug)]
pub enum RadarReadError {
    Disconnected,
    Timeout,
    NotConnected,
    ParseError(ParseError),
}

impl std::fmt::Display for RadarReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            RadarReadError::Disconnected => "Disconnected",
            RadarReadError::Timeout => "Timeout",
            RadarReadError::NotConnected => "NotConnected",
            RadarReadError::ParseError(_) => "ParseError",
        })
    }
}

impl std::error::Error for RadarReadError {}

#[derive(Debug, Error)]
pub enum RadarWriteError {
    #[error("Port unavailable")]
    PortUnavailable,
    #[error("Not connected")]
    NotConnected,
    #[error("Disconnected")]
    Disconnected,
}

#[derive(Debug)]
pub enum ParseError {
    DataLengthMismatch,
    MalformedData,
    UnimplementedTlvType(String),
}
