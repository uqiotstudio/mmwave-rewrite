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

#[derive(Debug, Error)]
pub enum RadarReadError {
    #[error("Disconnected")]
    Disconnected,
    #[error("Timeout")]
    Timeout,
    #[error("Not Connected")]
    NotConnected,
    #[error("Parse Error {0}")]
    ParseError(ParseError),
}

#[derive(Debug, Error)]
pub enum RadarWriteError {
    #[error("Port unavailable")]
    PortUnavailable,
    #[error("Not connected")]
    NotConnected,
    #[error("Disconnected")]
    Disconnected,
}

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("Data Length Mismatch")]
    DataLengthMismatch,
    #[error("Malformed Data")]
    MalformedData,
    #[error("Unimplemented Tlv Type {0}")]
    UnimplementedTlvType(String),
}
