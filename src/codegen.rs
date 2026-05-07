use crate::ast;
use crate::ast::{Ast, Node};
use inkwell::AddressSpace;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::{Linkage, Module};
use inkwell::types::BasicMetadataTypeEnum::PointerType;
use inkwell::types::StructType;
use inkwell::values::FunctionValue;

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
    let printf = state
        .module
        .get_function("printf")
        .expect("used after gen_extern_functions");

    match stmt {
        Node::ExprStmt(_) => todo!(),
        Node::IfStmt(_, _, _) => todo!(),
        Node::PrintStmt(expr_id) => {
            let lox_val = gen_expr(ast.nodes.get(*expr_id).unwrap(), ast, state)?;
            let index_ptr =
                state
                    .builder
                    .build_struct_gep(state.lox_value, lox_val, 0, "tag_ptr")?;
            let union_ptr =
                state
                    .builder
                    .build_struct_gep(state.lox_value, lox_val, 1, "union_ptr")?;

            let tag_val = state
                .builder
                .build_load(lox_index_type(state.ctx), index_ptr, "tag")?
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

            let nil_literal = state.builder.build_global_string_ptr("<nil>\n", "str")?;
            state
                .builder
                .build_call(printf, &[nil_literal.as_pointer_value().into()], "printf")
                .unwrap();
            state.builder.build_unconditional_branch(merge_block)?;

            state.builder.position_at_end(bool_block);
            // TODO: ALL THOSE FORMAT LITERALS should be created only once
            let bool_literal = state.builder.build_global_string_ptr("%d\n", "str")?;
            let bool_type = state.ctx.bool_type();
            let bool_val = state.builder.build_load(bool_type, union_ptr, "bool")?;
            state.builder.build_call(
                printf,
                &[bool_literal.as_pointer_value().into(), bool_val.into()],
                "printf",
            )?;
            state.builder.build_unconditional_branch(merge_block)?;

            state.builder.position_at_end(num_block);
            let float_literal = state.builder.build_global_string_ptr("%f\n", "str")?;
            let float_type = state.ctx.f64_type();
            let float_val = state.builder.build_load(float_type, union_ptr, "float")?;
            state.builder.build_call(
                printf,
                &[float_literal.as_pointer_value().into(), float_val.into()],
                "printf",
            )?;
            state.builder.build_unconditional_branch(merge_block)?;

            state.builder.position_at_end(str_block);
            let str_literal = state.builder.build_global_string_ptr("%s\n", "str")?;
            let str_type = state.ctx.ptr_type(AddressSpace::default());
            let str_val = state.builder.build_load(str_type, union_ptr, "str_val")?;
            state.builder.build_call(
                printf,
                &[str_literal.as_pointer_value().into(), str_val.into()],
                "printf",
            )?;
            state.builder.build_unconditional_branch(merge_block)?;

            state.builder.position_at_end(merge_block);
            Ok(())
        }
        Node::ReturnStmt(_) => todo!(),
        Node::WhileStmt(_, _) => todo!(),
        Node::Block(_) => todo!(),
        _ => unreachable!("Stmt node can only have statements -> assured during parsing"),
    }
}

fn gen_string<'a>(
    val: &str,
    state: &mut State<'a>,
) -> anyhow::Result<inkwell::values::PointerValue<'a>> {
    let ptr = state.builder.build_alloca(state.lox_value, "lox")?;
    let index_ptr = state
        .builder
        .build_struct_gep(state.lox_value, ptr, 0, "index")?;
    let union_ptr = state
        .builder
        .build_struct_gep(state.lox_value, ptr, 1, "union")?;

    let index_val = LoxValueType::String as u64;
    let str_global_ptr = state
        .builder
        .build_global_string_ptr(val, "cstr")?
        .as_pointer_value();
    state
        .builder
        .build_store(index_ptr, state.ctx.i8_type().const_int(index_val, false))?;
    state.builder.build_store(union_ptr, str_global_ptr)?;

    Ok(ptr)
}
fn gen_number<'a>(
    number: f64,
    state: &mut State<'a>,
) -> anyhow::Result<inkwell::values::PointerValue<'a>> {
    let ptr = state.builder.build_alloca(state.lox_value, "lox")?;
    let index_ptr = state
        .builder
        .build_struct_gep(state.lox_value, ptr, 0, "index")?;
    let union_ptr = state
        .builder
        .build_struct_gep(state.lox_value, ptr, 1, "union")?;

    let index_val = LoxValueType::Number as u64;
    state
        .builder
        .build_store(index_ptr, state.ctx.i8_type().const_int(index_val, false))?;
    state
        .builder
        .build_store(union_ptr, state.ctx.f64_type().const_float(number))?;

    Ok(ptr)
}
fn gen_bool<'a>(
    val: bool,
    state: &mut State<'a>,
) -> anyhow::Result<inkwell::values::PointerValue<'a>> {
    let ptr = state.builder.build_alloca(state.lox_value, "lox")?;
    let index_ptr = state
        .builder
        .build_struct_gep(state.lox_value, ptr, 0, "index")?;
    let union_ptr = state
        .builder
        .build_struct_gep(state.lox_value, ptr, 1, "union")?;

    let index_val = LoxValueType::Bool as u64;
    state
        .builder
        .build_store(index_ptr, state.ctx.i8_type().const_int(index_val, false))?;
    state.builder.build_store(
        union_ptr,
        state
            .ctx
            .bool_type()
            .const_int(if val { 1 } else { 0 }, false),
    )?;

    Ok(ptr)
}

fn gen_nil<'a>(state: &mut State<'a>) -> anyhow::Result<inkwell::values::PointerValue<'a>> {
    let ptr = state.builder.build_alloca(state.lox_value, "lox")?;
    let index_ptr = state
        .builder
        .build_struct_gep(state.lox_value, ptr, 0, "index")?;

    let index_val = LoxValueType::Nil as u64;
    state
        .builder
        .build_store(index_ptr, state.ctx.i8_type().const_int(index_val, false))?;

    Ok(ptr)
}
fn gen_expr<'a>(
    expr: &Node,
    _ast: &Ast,
    state: &mut State<'a>,
) -> anyhow::Result<inkwell::values::PointerValue<'a>> {
    match expr {
        Node::Assignment(_, _, _) => todo!(),
        Node::Binary(_, _, _) => todo!(),
        Node::Unary(_, _) => todo!(),
        Node::Call => todo!(),
        Node::Identifier(_) => todo!(),
        Node::Super(_) => todo!(),
        Node::Grouping(_) => todo!(),
        Node::Number(n) => gen_number(*n, state),
        Node::String(s) => gen_string(s, state),
        Node::Bool(b) => gen_bool(*b, state),
        Node::Nil => gen_nil(state),
        Node::This => todo!(),
        _ => unreachable!(),
    }
}

struct State<'a> {
    ctx: &'a Context,
    module: Module<'a>,
    builder: Builder<'a>,

    #[allow(unused)]
    panic_fn: FunctionValue<'a>, // exit with runtime error
    lox_value: StructType<'a>, // tagged union of all lox types
}

#[derive(Copy, Clone)]
enum LoxValueType {
    Nil,
    Number,
    Bool,
    String,

    #[allow(clippy::upper_case_acronyms)]
    SIZE,
}
impl LoxValueType {
    fn llvm_int<'a>(&self, ctx: &'a inkwell::context::Context) -> inkwell::values::IntValue<'a> {
        ctx.i8_type().const_int(*self as u64, false)
    }
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

    let mut state = State {
        ctx: context,
        module,
        builder,
        panic_fn,
        lox_value,
    };

    gen_begin_main(&mut state); // builder at @main.entry

    for decl_id in ast.program.clone() {
        let decl = ast.nodes.get(decl_id).unwrap();
        gen_declaration(decl, &ast, &mut state)?;
    }
    // builder should be at @main.entry
    gen_return_zero(&mut state)?;

    println!("{}", state.module.to_string());
    Ok(state.module)
}
