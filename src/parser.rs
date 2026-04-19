use crate::ast::Ast;
use lalrpop_util::lexer::Token;
use lalrpop_util::{ParseError, lalrpop_mod};

lalrpop_mod!(pub grammar);

pub fn parse(source: &'_ str) -> Result<Ast, Vec<ParseError<usize, Token<'_>, &'_ str>>> {
    let mut ast = Ast::new();
    let mut errors = Vec::new();
    if let Err(e) = grammar::ProgramParser::new().parse(&mut ast, source) {
        errors.push(e);
        return Err(errors);
    }
    Ok(ast)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn parse_ok(str: &'static str) {
        let _ast = parse(str).unwrap();
    }
    fn parse_err(str: &'static str) {
        let ast = parse(str);
        assert!(ast.is_err());
    }

    #[test]
    fn parsing_literals() {
        parse_ok("2;");
        parse_ok("2.2;");
        parse_err(".2;"); // lox specification
        parse_ok("true;");
        parse_ok("false;");
        parse_ok("nil;");
        parse_ok("this;");
        parse_ok("super.member;");
        parse_ok("variable;");
        parse_ok(r#"  "stringliteral";  "#);
    }
    #[test]
    fn parsing_expressions() {
        parse_ok("true != false;");
        parse_ok("(2 + 4.0) * 2 > 12;");
        parse_ok("!true or (variable = 5) < 5 and false;");
        parse_ok("-5 == -5.0;")
    }

    #[test]
    fn parsing_statements() {
        parse_ok("var d;");
        parse_ok("var d = very - long + expr;");
        parse_ok("print haha;");
    }
}
