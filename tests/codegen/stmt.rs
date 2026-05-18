use crate::mock_print::{assert_output, assert_output_f64, should_runtime_error};

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

#[test]
fn var_undeclared() {
    // should compile time err
    assert!(should_runtime_error("print a;").is_err());
}

#[test]
fn block_shadow() -> anyhow::Result<()> {
    assert_output_f64(
        "
            var a = 5;
            {
                var a = 6;
                print a;        
            }
        ",
        6.0,
    )?;
    assert_output_f64(
        "
            var a = 5;
            {
                var a = 6;
            }
            print a;        
        ",
        5.0,
    )?;
    assert_output_f64(
        "
            var a = 5;
            {
                print a;        
                var a = 6;
            }
        ",
        5.0,
    )?;
    assert_output_f64(
        "
            var a = 5;
            print a;        
            {
                var a = 6;
            }
        ",
        5.0,
    )?;
    Ok(())
}

#[test]
fn block_search_up() -> anyhow::Result<()> {
    assert_output_f64(
        "
            var a = 5;
            {
                var b = 3;
                print a;
            }
        ",
        5.0,
    )?;
    assert_output_f64(
        "
            var a = 5;
            {
                var b = 3;
                print b;
            }
        ",
        3.0,
    )?;

    assert_output_f64(
        "
            var a = 5;
            {
                var b = 3;
            }
            print a;
        ",
        5.0,
    )?;

    // should compile time err
    assert!(
        should_runtime_error(
            "
            var a = 5;
            {
                var b = 3;
            }
            print b;
        ",
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn while_none() -> anyhow::Result<()> {
    assert_output(
        "
            while(false)
                print 1;
        ",
        "",
    )?;
    Ok(())
}

#[test]
fn while_some() -> anyhow::Result<()> {
    assert_output_f64(
        "
            var a = 0;
            while(a < 5)
            {
               a = a + 1;
            }
            print a;
        ",
        5.0,
    )?;
    Ok(())
}
#[test]
fn for_some() -> anyhow::Result<()> {
    assert_output_f64(
        "
            for( var a = 0; a < 5; a = a + 1)
            {
            }
            print a;
        ",
        5.0,
    )?;
    Ok(())
}
