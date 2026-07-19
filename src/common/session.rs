use crate::common::symbol::Symbol;

#[derive(Debug, Clone)]
pub struct SessionContext {
    pub current_database: Option<Symbol>,
}

impl SessionContext {
    pub fn new() -> Self {
        Self {
            current_database: None,
        }
    }
}
