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
