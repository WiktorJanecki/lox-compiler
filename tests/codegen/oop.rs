use crate::mock_print::{assert_output, assert_output_f64};

#[test]
fn oop_basic_method() -> anyhow::Result<()> {
    assert_output(
        r#"
class Dog {
    speak() { print "Woof"; }
}
var d = Dog();
d.speak();
"#,
        "Woof",
    )
}

#[test]
fn oop_this_counter() -> anyhow::Result<()> {
    assert_output_f64(
        r#"
class Counter {
    init() { this.n = 0; }
    inc() { this.n = this.n + 1; }
    get() { return this.n; }
}
var c = Counter();
c.inc();
c.inc();
print c.get();
"#,
        2.0,
    )
}

#[test]
fn oop_init_args() -> anyhow::Result<()> {
    assert_output(
        r#"
class Box {
    init(v) { this.val = v; }
    get() { return this.val; }
}
var b = Box("hello");
print b.get();
"#,
        "hello",
    )
}

#[test]
fn oop_field_access() -> anyhow::Result<()> {
    assert_output(
        r#"
class Point {
    init(x, y) { this.x = x; this.y = y; }
}
var p = Point("a", "b");
print p.x;
print p.y;
"#,
        "a\nb",
    )
}

#[test]
fn oop_inherit() -> anyhow::Result<()> {
    assert_output(
        r#"
class Animal {
    speak() { print "..."; }
}
class Dog < Animal {
    speak() { super.speak(); print "Woof"; }
}
Dog().speak();
"#,
        "...\nWoof",
    )
}

#[test]
fn oop_method_chained() -> anyhow::Result<()> {
    assert_output_f64(
        r#"
class Acc {
    init() { this.sum = 0; }
    add(x) { this.sum = this.sum + x; }
    val() { return this.sum; }
}
var a = Acc();
a.add(3.0);
a.add(4.0);
print a.val();
"#,
        7.0,
    )
}
