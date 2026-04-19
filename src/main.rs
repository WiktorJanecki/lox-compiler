mod ast;

use lalrpop_util::lalrpop_mod;

lalrpop_mod!(pub gram);
fn main() {
    let output = crate::gram::PrimaryParser::new().parse("((tw_oj123))").unwrap();
    println!("Hello, fworld!");
}
