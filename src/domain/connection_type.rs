use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConnectionType {
    Session,
    System,
}

impl fmt::Display for ConnectionType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Session => "user",
            Self::System => "system",
        })
    }
}
