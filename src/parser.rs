use crate::ast::Ast;
use crate::error::ParserError;
use lalrpop_util::lalrpop_mod;

lalrpop_mod!(pub grammar);

pub fn parse(source: &'_ str) -> Result<Ast, ParserError> {
    let mut ast = Ast::new();
    let mut errors = Vec::new();
    if let Err(e) = grammar::ProgramParser::new().parse(&mut ast, source) {
        errors.push(format!("{e}"));
        return Err(ParserError { errors });
    }
    Ok(ast)
}
