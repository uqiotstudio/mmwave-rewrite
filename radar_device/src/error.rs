#[derive(Debug)]
pub enum RadarInitError {
    PortUnavailable(String),
    PortNotFound(String),
    InaccessibleConfig(String),
}

#[derive(Debug)]
pub enum RadarReadError {
    Disconnected,
    Timeout,
    NotConnected,
    ParseError(ParseError),
}

#[derive(Debug)]
pub enum RadarWriteError {
    PortUnavailable,
    NotConnected,
    Disconnected,
}

#[derive(Debug)]
pub enum ParseError {
    DataLengthMismatch,
    MalformedData,
    UnimplementedTlvType(String),
}
