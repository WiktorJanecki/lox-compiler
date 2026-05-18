use crate::mock_print::{assert_output, assert_output_f64, should_runtime_error};

#[test]
fn add_number() -> anyhow::Result<()> {
    assert_output_f64("print 6.8 + 0.1;", 6.9)?;
    assert_output_f64("print 0.1 + 6.8 + 0.1;", 7.0)?;
    Ok(())
}
#[test]
fn add_string() -> anyhow::Result<()> {
    // TODO: uncomment
    // assert_output("print \"sugon\" + \"deez\";", "sugondeez")?;
    // assert_output("print \"sugon\" + \"deez\" + \"nats\";", "sugondeeznats")?;

    Ok(())
}

#[test]
fn add_err() -> anyhow::Result<()> {
    should_runtime_error("print true + true;")?;
    should_runtime_error("print true + 0.1;")?;
    should_runtime_error("print 0.1 + false;")?;

    should_runtime_error("print nil + 0.1;")?;
    should_runtime_error("print 0.1 + nil;")?;
    should_runtime_error("print nil + nil;")?;

    should_runtime_error("print \"t\" + 0.1;")?;
    should_runtime_error("print 0.1 + \"t\";")?;

    should_runtime_error("print nil + 0.1;")?;
    should_runtime_error("print 0.1 + nil;")?;
    should_runtime_error("print nil + nil;")?;
    Ok(())
}
#[test]
fn minus_err() -> anyhow::Result<()> {
    should_runtime_error("print true - true;")?;
    should_runtime_error("print true - 0.1;")?;
    should_runtime_error("print 0.1 - false;")?;

    should_runtime_error("print nil - 0.1;")?;
    should_runtime_error("print 0.1 - nil;")?;
    should_runtime_error("print nil - nil;")?;

    should_runtime_error("print \"t\" - 0.1;")?;
    should_runtime_error("print 0.1 - \"t\";")?;

    should_runtime_error("print nil - 0.1;")?;
    should_runtime_error("print 0.1 - nil;")?;
    should_runtime_error("print nil - nil;")?;
    Ok(())
}
#[test]
fn minus_number() -> anyhow::Result<()> {
    assert_output_f64("print 7.0 - 0.1;", 6.9)?;
    assert_output_f64("print 7.0 - 10;", -3.0)?;
    Ok(())
}

#[test]
fn mul_err() -> anyhow::Result<()> {
    should_runtime_error("print true * true;")?;
    should_runtime_error("print true * 0.1;")?;
    should_runtime_error("print 0.1 * false;")?;

    should_runtime_error("print nil * 0.1;")?;
    should_runtime_error("print 0.1 * nil;")?;
    should_runtime_error("print nil * nil;")?;

    should_runtime_error("print \"t\" * 0.1;")?;
    should_runtime_error("print 0.1 * \"t\";")?;

    should_runtime_error("print nil * 0.1;")?;
    should_runtime_error("print 0.1 * nil;")?;
    should_runtime_error("print nil * nil;")?;
    Ok(())
}

#[test]
fn mul_number() -> anyhow::Result<()> {
    assert_output_f64("print 7.0 * 0.1;", 0.7)?;
    Ok(())
}

#[test]
fn div_err() -> anyhow::Result<()> {
    should_runtime_error("print true / true;")?;
    should_runtime_error("print true / 0.1;")?;
    should_runtime_error("print 0.1 / false;")?;

    should_runtime_error("print nil / 0.1;")?;
    should_runtime_error("print 0.1 / nil;")?;
    should_runtime_error("print nil / nil;")?;

    should_runtime_error("print \"t\" / 0.1;")?;
    should_runtime_error("print 0.1 / \"t\";")?;

    should_runtime_error("print nil / 0.1;")?;
    should_runtime_error("print 0.1 / nil;")?;
    should_runtime_error("print nil / nil;")?;
    Ok(())
}

#[test]
fn zero_div() -> anyhow::Result<()> {
    assert_output("print 5 / 0;", "inf")?; // lox specification
    Ok(())
}

#[test]
fn div_number() -> anyhow::Result<()> {
    assert_output_f64("print 7.0 / 0.1;", 70.)?;
    Ok(())
}

#[test]
fn factors() -> anyhow::Result<()> {
    assert_output_f64("print 1 + 2 * 3 + 4;", 1.0 + 2.0 * 3.0 + 4.0)?;
    assert_output_f64("print 1 + 2 * 3 / 4;", 1.0 + 2.0 * 3.0 / 4.0)?;
    assert_output_f64("print 1 + 2 * 3 + 0.1 / 4;", 1.0 + 2.0 * 3.0 + 0.1 / 4.0)?;

    Ok(())
}

#[test]
fn grouping() -> anyhow::Result<()> {
    assert_output_f64("print (1 + 2) * (3 + 4);", (1.0 + 2.0) * (3.0 + 4.0))?;
    assert_output("print(((((true)))));", "true")?;

    Ok(())
}

#[test]
fn comparison() -> anyhow::Result<()> {
    assert_output("print 1 < 2;", "true")?;
    assert_output("print 1 > 2;", "false")?;
    assert_output("print 1 >= 1;", "true")?;
    assert_output("print 1 <= 1;", "true")?;

    Ok(())
}
#[test]
fn comparison_err() -> anyhow::Result<()> {
    should_runtime_error("print 1 < true;")?;
    should_runtime_error("print true < 1;")?;
    should_runtime_error("print true < false;")?;

    should_runtime_error("print 1 <= nil;")?;
    should_runtime_error("print nil <= 1;")?;
    should_runtime_error("print nil <= nil;")?;

    should_runtime_error("print 1 > \"t\";")?;
    should_runtime_error("print \"t\" > 1;")?;
    should_runtime_error("print \"t\" > \"t\";")?;

    Ok(())
}
#[test]
fn assigment() -> anyhow::Result<()> {
    assert_output_f64("var a = 5; print a = 6;", 6.)?;
    assert_output_f64(
        "
        var a;
        var b;
        var c;
        a = b = c = 2;
        print a + b + c;
    ",
        6.0,
    )?;
    Ok(())
}

#[test]
fn equality_num() -> anyhow::Result<()> {
    assert_output("print 1.0 == 6.9;", "false")?;
    assert_output("print 1.0 != 6.9;", "true")?;
    Ok(())
}

#[test]
fn equality_bool() -> anyhow::Result<()> {
    assert_output("print false == true;", "false")?;
    assert_output("print true != true;", "false")?;
    Ok(())
}

#[test]
fn equality_nil() -> anyhow::Result<()> {
    assert_output("print nil == nil;", "true")?;
    assert_output("print nil != nil;", "false")?;
    Ok(())
}
#[test]
fn equality_str() -> anyhow::Result<()> {
    // TODO finish
    Ok(())
}

#[test]
fn equality_mismatched() -> anyhow::Result<()> {
    assert_output("print \"sdf\" == nil;", "false")?;
    assert_output("print 5.1 != false;", "true")?;
    Ok(())
}

#[test]
fn negation() -> anyhow::Result<()> {
    assert_output("print !false;", "true")?;
    assert_output("print !true;", "false")?;
    should_runtime_error("print !5.0;")?;
    should_runtime_error("print !nil;")?;
    should_runtime_error("print !\"dfasoij\";")?;
    Ok(())
}

#[test]
fn unary_minus() -> anyhow::Result<()> {
    assert_output_f64("print -5;", -5.0)?;
    assert_output_f64("print ---5;", -5.0)?;

    should_runtime_error("print -true;")?;
    should_runtime_error("print -nil;")?;
    should_runtime_error("print -\"fdsaf\";")?;

    Ok(())
}

#[test]
fn or_expr() -> anyhow::Result<()> {
    assert_output("print true or true;", "true")?;
    assert_output("print false or true;", "true")?;
    assert_output("print true or false;", "true")?;
    assert_output("print false or false;", "false")?;
    Ok(())
}

#[test]
fn or_chained() -> anyhow::Result<()> {
    assert_output("print false or false  or true or false or false;", "true")?;
    assert_output("print false or false or false or false;", "false")?;
    Ok(())
}

#[test]
fn or_short_circ() -> anyhow::Result<()> {
    assert_output("
        var a = false;
        false or (a = true);
        print a;
    ", "true")?;
    assert_output("
        var a = false;
        true or (a = true);
        print a;
    ", "false")?;
    Ok(())
}

#[test]
fn and_expr() -> anyhow::Result<()> {
    assert_output("print true and true;", "true")?;
    assert_output("print false and true;", "false")?;
    assert_output("print true and false;", "false")?;
    assert_output("print false and false;", "false")?;
    Ok(())
}

#[test]
fn and_chained() -> anyhow::Result<()> {
    assert_output("print false and false and true and false;", "false")?;
    assert_output("print true and true and true;", "true")?;
    Ok(())
}

#[test]
fn and_short_circ() -> anyhow::Result<()> {
    assert_output("
        var a = false;
        false and (a = true);
        print a;
    ", "false")?;
    assert_output("
        var a = false;
        true and (a = true);
        print a;
    ", "true")?;
    Ok(())
}

#[test]
fn copy() -> anyhow::Result<()> {
    assert_output_f64("
        var a = 0;
        var b = a;
        a = a + 1;
        print b;
    ", 0.0)?;
    Ok(())
}