fn parse_ok(str: &'static str) {
    let _ast = loxc::parser::parse(str).unwrap();
}
fn parse_err(str: &'static str) {
    let ast = loxc::parser::parse(str);
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

#[test]
fn parsing_expression_statement() {
    parse_ok("5+5;");
    parse_err("5+5");
}

#[test]
fn parsing_for_statement() {
    // minimal
    parse_ok("for (var i = 0;;){}");
    parse_ok("for (i = true;;){}");

    // stmt as body
    parse_ok("for (i = true;;) print 5;");
    parse_ok("for (i = true;;) 5;");
    parse_err("for (i = true;;) var a = 5;");

    // optional
    parse_ok("for (var i = 0; i < 5 ;     ){}");
    parse_ok("for (var i = 0;       ; i=i+1){}");
    parse_ok("for (var i = 0; i < 5 ; i=i+1){}");

    parse_err("for (var i = 0; i < 5 ; var a = 5){}");
    parse_err("for (var i = 0; var a = 5 ; i++){}");
}

#[test]
fn parsing_if_statement() {
    parse_ok("if (true){}");
    parse_ok("if (true) a = 5;");
    parse_ok("if (true) print a;");
    parse_err("if (true) var a = 5;");
    parse_err("if (var a = 5){}");
    parse_err("if (print a){}");

    parse_ok("if (true){} else{}");
    parse_ok("if (true){} else a = 5;");
    parse_ok("if (true){} else print 5;");
    parse_err("if (true){} else var d = 5;");
    parse_err("if (true){} else {} else {};");
}

#[test]
fn parsing_print_statement() {
    parse_ok("print 5;");
    parse_ok("print (1+5)*\"text\";");
    parse_err("print var d = 5;");
    parse_err("print print d;");
    parse_err("print (print d);");
}

#[test]
fn parsing_return_statement() {
    parse_ok("return;");
    parse_ok("return 5;");
    parse_err("return print 5;");
    parse_err("return var d = 5;");
}

#[test]
fn parse_while_statement() {
    parse_ok("while (true){}");
    parse_ok("while (true) print 5;");
    parse_err("while (true) var d = 5;");
    parse_err("while (var d = 5){}");
    parse_err("while (print 5){}");
}

#[test]
fn parse_block_statement() {
    parse_ok("{print 5;}");
    parse_ok("{var d = 5;}");
    parse_ok("{{print 5;}}");
    parse_ok("{print 5;{print 5;}}");

    parse_err("{print 5;");
    parse_err("print 5;}");
}

#[test]
fn parse_class_declaration() {
    parse_ok("class name {}");
    parse_ok("class name < base {}");
    parse_ok("class name{ nazwa(){} }");
    parse_ok("class name{ nazwa(){} nazwa3(){} }");
}

#[test]
fn parse_function_declaration() {
    parse_ok("fun nazwa() {}");
    parse_ok("fun nazwa(arg) {}");
    parse_ok("fun nazwa(arg, arg, arg, argg) {}");
    parse_err("fun nazwa() print 5;");
    parse_err("fun nazwa() var a = 5;");
}

#[test]
fn parse_call() {
    parse_ok("wywolanie();");
    parse_ok("wywolanie(arg);");
    parse_ok("wywolanie(arg, args);");
    parse_ok("obj.mthd();");
    parse_ok("obj.mthd(arg);");
    parse_ok("obj.mthd(arg, args);");
    parse_ok("obj.demeter.rule.not.satisfied();");
    parse_ok("a = method.member;");
    parse_ok("method.member = 5;");
}
