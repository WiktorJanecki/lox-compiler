use std::fmt::{Debug, Display, Formatter};

pub struct ParserError {
    pub errors: Vec<String>,
}

impl Debug for ParserError {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        for err in self.errors.iter() {
            writeln!(f, "{}", err)?;
        }
        Ok(())
    }
}

impl Display for ParserError {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self, f) 
    }
}

impl std::error::Error for ParserError {}
