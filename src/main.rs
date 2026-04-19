mod ast;
mod parser;

fn main() {
    let _ = parser::parse("2+2 == 5;");
    println!("Hello, fworld!");
}
