use crate::mock_print::{assert_output, assert_output_f64};

#[test]
fn if_true() -> anyhow::Result<()> {
    assert_output(
        "
        print \"ba\"; 
        if (true)
           print \"pka\"; 
        ",
        "pka",
    )?;
    Ok(())
}

#[test]
fn if_false() -> anyhow::Result<()> {
    assert_output(
        "
        print \"ba\";
        if (false)
           print \"pka\";
        ",
        "ba",
    )?;
    Ok(())
}
#[test]
fn if_else_true() -> anyhow::Result<()> {
    assert_output(
        "
        print \"ba\";
        if (true)
           print \"pka\";
        else
           print \"ma\";
        ",
        "pka",
    )?;
    Ok(())
}

#[test]
fn if_else_false() -> anyhow::Result<()> {
    assert_output(
        "
        print \"ba\";
        if (false)
           print \"pka\";
        else
           print \"ma\";
        ",
        "ma",
    )?;
    Ok(())
}
#[test]
fn dangling_if() -> anyhow::Result<()> {
    assert_output(
        "
        print \"ba\";
        if (false)
            if (false)
               print \"pka\";
            else
               print \"ma\";
        ",
        "ba",
    )?;
    Ok(())
}

#[test]
fn var_decl() -> anyhow::Result<()> {
    assert_output_f64(
        "
            var a = 5;
            print a;
        ",
        5.0,
    )?;
    Ok(())
}

#[test]
fn var_default() -> anyhow::Result<()> {
    assert_output(
        "
            var a;
            print a;
        ",
        "nil",
    )?;
    Ok(())
}
