use crate::mock_print::{assert_output, assert_output_f64};

#[test]
fn fun_basic_call() -> anyhow::Result<()> {
    assert_output(r#"fun greet() { print "hi"; } greet();"#, "hi")
}

#[test]
fn fun_return_value() -> anyhow::Result<()> {
    assert_output_f64("fun add(a, b) { return a + b; } print add(1.0, 2.0);", 3.0)
}

#[test]
fn fun_closure_capture() -> anyhow::Result<()> {
    assert_output_f64("var x = 5.0; fun getX() { return x; } print getX();", 5.0)
}

#[test]
fn fun_counter() -> anyhow::Result<()> {
    assert_output(
        r#"
fun makeCounter() {
    var count = 0;
    fun inc() {
        count = count + 1;
        return count;
    }
    return inc;
}
var c = makeCounter();
print c();
print c();
"#,
        "1\n2",
    )
}

#[test]
fn fun_recursive() -> anyhow::Result<()> {
    assert_output_f64(
        r#"
fun fib(n) {
    if (n <= 1.0) return n;
    return fib(n - 1.0) + fib(n - 2.0);
}
print fib(7.0);
"#,
        13.0,
    )
}

#[test]
fn fun_first_class() -> anyhow::Result<()> {
    assert_output(
        r#"
fun apply(f, x) { return f(x); }
fun double(n) { return n + n; }
print apply(double, "ab");
"#,
        "abab",
    )
}
