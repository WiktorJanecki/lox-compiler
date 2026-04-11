# Lox compiler

Lox to LLVM compiler written in Rust using ANTLR parser generator.
Lox is functional and objective script language designed by Robert Nystrom in his book - Crafting Interpreters. 

## Usage Instructions
Make sure you have [Rust](https://rust-lang.org/) toolchain installed and added to your PATH


You can then run compiler directly using cargo or build it and install in your system path
```bash
cargo run . ./examples/hello_world.lox
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

Example:
```bash
loxc examples/hello_world.lox --emit-llvm-ir -o output.ll
```

## Authors
Project made for university course by:
- Wiktor Janecki: wjanecki@student.agh.edu.pl
- Dmytro Harasiuk: harasiuk@student.agh.edu.pl


