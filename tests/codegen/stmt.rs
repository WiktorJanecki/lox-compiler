use crate::mock_print::assert_output;

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
