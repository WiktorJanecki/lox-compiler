# Lox compiler

Lox to LLVM compiler written in Rust using [LALRPOP](https://github.com/lalrpop/lalrpop) parser generator.
Lox is functional and objective script language designed by Robert Nystrom in his book - Crafting Interpreters. 

## Usage Instructions
Make sure you have [Rust](https://rust-lang.org/) toolchain installed and added to your PATH

Next install [LLVM-22.1](https://github.com/llvm/llvm-project/releases/tag/llvmorg-22.1.3) toolchain, add it to PATH and (only on windows) create environmental variable LLVM_SYS_221_PREFIX=toolchain_path.

Quick tip: for [some reason](https://github.com/llvm/llvm-project/issues/35139) official llvm prebuild binaries on windows don't come with llvm-config. You need to build them by yourself. I spent too much time on this issue...

Finally, you can run compiler directly using cargo or build it and install in your system path
```bash
cargo run ./examples/hello_world.lox
```
```bash
cargo install --path .

loxc ./path_to_lox_script.lox
loxc <filename> <arguments>
```

Compiler by default emits executable with the same name as source file using following pipeline:
```
input.lox --> input.ll --> input.o --> input.exe
         loxc         llc         linker
```
This behaviour can be changed by following arguments:
- `-o filename` change output file name  
- `--emit=exe` change type of output file to executable (default)
- `--emit=llvm-ir` change type of output file to [LLVM IR](https://llvm.org/docs/LangRef.html)
- `--emit=obj` change type of output file to object files
- `--help` display all arguments and default values

Example:
```bash
loxc examples/hello_world.lox --emit-llvm-ir -o output.ll
```

## Authors
Project made for university course by:
- Wiktor Janecki: wjanecki@student.agh.edu.pl
- Dmytro Harasiuk: harasiuk@student.agh.edu.pl

## Syntax Grammar
Parser generator file: [grammar.lalrpop](src/grammar.lalrpop)
```lox
program        → declaration* EOF ;

declaration    → classDecl
               | funDecl
               | varDecl
               | statement ;

classDecl      → "class" IDENTIFIER ( "<" IDENTIFIER )?
                 "{" function* "}" ;
funDecl        → "fun" function ;
varDecl        → "var" IDENTIFIER ( "=" expression )? ";" ;

statement      → exprStmt
               | forStmt
               | ifStmt
               | printStmt
               | returnStmt
               | whileStmt
               | block ;

exprStmt       → expression ";" ;
forStmt        → "for" "(" ( varDecl | exprStmt | ";" )
                           expression? ";"
                           expression? ")" statement ;
ifStmt         → "if" "(" expression ")" statement
                 ( "else" statement )? ;
printStmt      → "print" expression ";" ;
returnStmt     → "return" expression? ";" ;
whileStmt      → "while" "(" expression ")" statement ;
block          → "{" declaration* "}" ;


expression     → assignment ;

assignment     → ( call "." )? IDENTIFIER "=" assignment
               | logic_or ;

logic_or       → logic_and ( "or" logic_and )* ;
logic_and      → equality ( "and" equality )* ;
equality       → comparison ( ( "!=" | "==" ) comparison )* ;
comparison     → term ( ( ">" | ">=" | "<" | "<=" ) term )* ;
term           → factor ( ( "-" | "+" ) factor )* ;
factor         → unary ( ( "/" | "*" ) unary )* ;

unary          → ( "!" | "-" ) unary | call ;
call           → primary ( "(" arguments? ")" | "." IDENTIFIER )* ;
primary        → "true" | "false" | "nil" | "this"
               | NUMBER | STRING | IDENTIFIER | "(" expression ")"
               | "super" "." IDENTIFIER ;
               
function       → IDENTIFIER "(" parameters? ")" block ;
parameters     → IDENTIFIER ( "," IDENTIFIER )* ;
arguments      → expression ( "," expression )* ;

NUMBER         → DIGIT+ ( "." DIGIT+ )? ;
STRING         → "\"" <any char except "\"">* "\"" ;
IDENTIFIER     → ALPHA ( ALPHA | DIGIT )* ;
ALPHA          → "a" ... "z" | "A" ... "Z" | "_" ;
DIGIT          → "0" ... "9" ;
```