use crate::mock_print::{assert_output, assert_output_f64};

#[test]
fn print_number() -> anyhow::Result<()> {
    assert_output_f64("print 5;", 5.0)?;
    assert_output_f64("print 6.9;", 6.9)?;

    Ok(())
}

#[test]
fn print_bool() -> anyhow::Result<()> {
    assert_output("print true;", "true")?;
    assert_output("print false;", "false")?;
    Ok(())
}

#[test]
fn print_nil() -> anyhow::Result<()> {
    assert_output("print nil;", "nil")?;
    Ok(())
}

#[test]
fn print_string() -> anyhow::Result<()> {
    assert_output("print \"mama\";","mama")?;
    Ok(())
}
