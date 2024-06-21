use std::error::Error;

#[derive(Debug)]
pub enum RadarInitError {
    PortUnavailable(String),
    PortNotFound(String),
    InaccessibleConfig(String),
}

impl Into<Box<dyn Error>> for RadarInitError {
    fn into(self) -> Box<dyn Error> {
        format!("{:?}", self).into()
    }
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
