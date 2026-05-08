use crate::ast;
use crate::ast::{Ast, Node, Operator};
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::{Linkage, Module};
use inkwell::types::BasicMetadataTypeEnum::PointerType;
use inkwell::types::StructType;
use inkwell::values::FunctionValue;
use inkwell::{AddressSpace, types, values};
use crate::codegen::gen_expr::gen_expr;
use crate::codegen::lox_value::{LoxValue, LoxValueType};
use crate::codegen::string_literals::{gen_global_string_literals, global_string_literal, StringLiterals};

mod gen_expr;
mod lox_value;
mod string_literals;

fn gen_extern_functions(module: &Module) {
    // printf
    let context = module.get_context();
    let str = context.ptr_type(AddressSpace::default());
    let tp = context.i32_type().fn_type(&[PointerType(str)], true);
    module.add_function("printf", tp, Some(Linkage::External));
    let tp = context
        .void_type()
        .fn_type(&[context.i32_type().into()], false);
    module.add_function("exit", tp, Some(Linkage::External));
}

fn gen_panic_fn<'a>(module: &Module<'a>, builder: &Builder) -> anyhow::Result<FunctionValue<'a>> {
    let context = module.get_context();
    let str = context.ptr_type(AddressSpace::default());
    let tp = context.void_type().fn_type(&[PointerType(str)], false);
    let panic_fn = module.add_function("panic", tp, None);
    let block = context.append_basic_block(panic_fn, "entry");
    builder.position_at_end(block);
    let arg = panic_fn.get_first_param().unwrap();
    let printf = module.get_function("printf").unwrap();
    builder.build_call(printf, &[arg.into()], "_")?;
    let exit = module.get_function("exit").unwrap();
    builder.build_call(
        exit,
        &[context.i32_type().const_int(u64::MAX, false).into()],
        "_",
    )?;
    builder.build_unreachable()?;
    Ok(panic_fn)
}
fn gen_lox_object(ctx: &'_ Context) -> StructType<'_> {
    let type_i8 = lox_index_type(ctx);
    let type_biggest = ctx.i64_type(); // Should be as big as the biggest value
    ctx.struct_type(&[type_i8.into(), type_biggest.into()], true)
}

fn gen_begin_main(state: &mut State) {
    let main_fn = state
        .module
        .add_function("main", state.ctx.i32_type().fn_type(&[], false), None);
    let entry = state.ctx.append_basic_block(main_fn, "entry");
    state.builder.position_at_end(entry);
}

fn gen_return_zero(state: &mut State) -> anyhow::Result<()> {
    let zero = state.ctx.i32_type().const_int(0, false);
    state.builder.build_return(Some(&zero))?;
    Ok(())
}

fn gen_declaration(decl: &Node, ast: &Ast, state: &mut State) -> anyhow::Result<()> {
    match decl {
        Node::VarDecl(_, _) => todo!(),
        Node::ClassDecl(_, _, _) => todo!(),
        Node::FunDecl(_, _, _) => todo!(),
        Node::Stmt(stmt_id) => gen_statement(&ast.nodes[*stmt_id], ast, state),
        _ => unreachable!("In program vector only decl nodes are pushed during parsing"),
    }
}

fn gen_statement(stmt: &Node, ast: &Ast, state: &mut State) -> anyhow::Result<()> {
    match stmt {
        Node::ExprStmt(expr_id) => {
            let _ = gen_expr(&ast.nodes[*expr_id],ast,state)?;
            Ok(())
        },
        Node::IfStmt(_, _, _) => todo!(),
        Node::PrintStmt(expr_id) => {
            let lox_val = gen_expr(&ast.nodes[*expr_id], ast, state)?;
            gen_print_stmt(lox_val, state)
        }
        Node::ReturnStmt(_) => todo!(),
        Node::WhileStmt(_, _) => todo!(),
        Node::Block(_) => todo!(),
        _ => unreachable!("Stmt node can only have statements -> assured during parsing"),
    }
}

fn gen_print_stmt(lox_val: LoxValue, state: &mut State) -> anyhow::Result<()> {
    let printf = state
        .module
        .get_function("printf")
        .expect("used after gen_extern_functions");
    let tag_val = state
        .builder
        .build_load(lox_index_type(state.ctx), lox_val.index_ptr, "tag")?
        .into_int_value();
    let parent_func = state
        .builder
        .get_insert_block()
        .unwrap()
        .get_parent()
        .unwrap();
    let nil_block = state.ctx.append_basic_block(parent_func, "print.nil");
    let num_block = state.ctx.append_basic_block(parent_func, "print.number");
    let str_block = state.ctx.append_basic_block(parent_func, "print.string");
    let bool_block = state.ctx.append_basic_block(parent_func, "print.bool");
    let merge_block = state.ctx.append_basic_block(parent_func, "print.merge");
    let unreach_block = state.ctx.append_basic_block(parent_func, "print.unreach");

    let cases = &[
        (LoxValueType::Nil.llvm_int(state.ctx), nil_block),
        (LoxValueType::Number.llvm_int(state.ctx), num_block),
        (LoxValueType::Bool.llvm_int(state.ctx), bool_block),
        (LoxValueType::String.llvm_int(state.ctx), str_block),
    ];
    assert_eq!(cases.len(), LoxValueType::SIZE as usize);

    state.builder.build_switch(tag_val, unreach_block, cases)?;

    state.builder.position_at_end(unreach_block);
    state.builder.build_unreachable()?;

    state.builder.position_at_end(nil_block);

    let nil_literal = global_string_literal(StringLiterals::PrintfNil, state);
    state
        .builder
        .build_call(printf, &[nil_literal.into()], "printf")
        .unwrap();
    state.builder.build_unconditional_branch(merge_block)?;

    state.builder.position_at_end(bool_block);

    let bool_literal = global_string_literal(StringLiterals::PrintfBool, state);
    let bool_type = state.ctx.bool_type();
    let bool_val = state
        .builder
        .build_load(bool_type, lox_val.union_ptr, "bool")?;
    state
        .builder
        .build_call(printf, &[bool_literal.into(), bool_val.into()], "printf")?;
    state.builder.build_unconditional_branch(merge_block)?;

    state.builder.position_at_end(num_block);
    let float_literal = global_string_literal(StringLiterals::PrintfNumber, state);
    let float_type = state.ctx.f64_type();
    let float_val = state
        .builder
        .build_load(float_type, lox_val.union_ptr, "float")?;
    state
        .builder
        .build_call(printf, &[float_literal.into(), float_val.into()], "printf")?;
    state.builder.build_unconditional_branch(merge_block)?;

    state.builder.position_at_end(str_block);
    let str_literal = global_string_literal(StringLiterals::PrintfString, state);
    let str_type = state.ctx.ptr_type(AddressSpace::default());
    let str_val = state
        .builder
        .build_load(str_type, lox_val.union_ptr, "str_val")?;
    state
        .builder
        .build_call(printf, &[str_literal.into(), str_val.into()], "printf")?;
    state.builder.build_unconditional_branch(merge_block)?;

    state.builder.position_at_end(merge_block);
    Ok(())
}

struct State<'a> {
    ctx: &'a Context,
    module: Module<'a>,
    builder: Builder<'a>,

    panic_fn: FunctionValue<'a>, // exit with runtime error
    lox_value: StructType<'a>,   // tagged union of all lox types
    string_literals: [values::PointerValue<'a>; StringLiterals::SIZE as usize],
}


// it should be const but cannot cuz depends on context
fn lox_index_type(ctx: &'_ Context) -> inkwell::types::IntType<'_> {
    ctx.i8_type()
}


pub fn codegen(ast: ast::Ast, context: &'_ mut Context) -> anyhow::Result<Module<'_>> {
    let module = context.create_module("main");
    let builder = context.create_builder();

    // generate pseudo runtime
    gen_extern_functions(&module);
    let panic_fn = gen_panic_fn(&module, &builder)?;
    let lox_value = gen_lox_object(context);
    let string_literals = gen_global_string_literals(&builder)?;

    let mut state = State {
        ctx: context,
        module,
        builder,
        panic_fn,
        lox_value,
        string_literals,
    };

    gen_begin_main(&mut state); // builder at @main.entry

    for decl_id in ast.program.clone() {
        let decl = ast.nodes.get(decl_id).unwrap();
        gen_declaration(decl, &ast, &mut state)?;
    }
    // builder should be at @main.entry
    gen_return_zero(&mut state)?;

    println!("{}", state.module.to_string());
    if let Err(err) = state.module.verify() {
        eprintln!("Błąd weryfikacji IR: {}", err.to_string());
    }
    Ok(state.module)
}
