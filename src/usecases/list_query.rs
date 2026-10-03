use crate::domain::connection_type::ConnectionType;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ListRequestContext {
    pub connection: ConnectionType,
    pub generation: u64,
}

pub enum QueryUnitFile {
    Finished(ListRequestContext, HashMap<String, String>),
    Error(ListRequestContext, String),
}
