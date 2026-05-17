use crate::ast;
use crate::ast::{Ast, Node};
use crate::codegen::gen_expr::gen_expr;
use crate::codegen::gen_stmt::gen_statement;
use crate::codegen::lox_value::{LoxValue, LoxValueType};
use crate::codegen::string_literals::{
    StringLiterals, gen_global_string_literals, global_string_literal,
};
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::{Linkage, Module};
use inkwell::types::BasicMetadataTypeEnum::PointerType;
use inkwell::types::StructType;
use inkwell::values::FunctionValue;
use inkwell::{AddressSpace, values};
use std::collections::HashMap;
use std::ffi::CString;
use inkwell::basic_block::BasicBlock;

mod gen_expr;
mod gen_stmt;
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
    state.current_fn = main_fn;
    state.vars.insert(main_fn.get_name().to_owned(), vec![HashMap::new()]);
    state.builder.position_at_end(entry);
}

fn gen_return_zero(state: &mut State) -> anyhow::Result<()> {
    let zero = state.ctx.i32_type().const_int(0, false);
    state.builder.build_return(Some(&zero))?;
    Ok(())
}

fn get_current_env<'a, 'b>(state: &'b mut State<'a>) -> &'b mut HashMap<String, LoxValue<'a>> {
    let fn_name = state.current_fn.get_name();
    // TODO: don't check but require env created
    if state.vars.contains_key(fn_name) {
        let stack = state.vars.get_mut(fn_name).unwrap();
        if stack.is_empty() {
            stack.push(HashMap::new());
        }
        return stack.last_mut().unwrap();
    }
    state.vars.insert(fn_name.to_owned(), vec![HashMap::new()]);
    state.vars.get_mut(fn_name).unwrap().last_mut().unwrap()
}

fn pop_env(state: &mut State) -> anyhow::Result<()> {
    let fn_name = state.current_fn.get_name();
    state.vars.get_mut(fn_name).unwrap().pop();
    Ok(())
}

fn push_new_env(state: &mut State) -> anyhow::Result<()> {
    let fn_name = state.current_fn.get_name();
    state.vars.get_mut(fn_name).unwrap().push(HashMap::new());
    Ok(())
}

fn get_var_from_env<'a, 'b>(
    name: &str,
    state: &'b mut State<'a>,
) -> anyhow::Result<&'b mut LoxValue<'a>> {
    let err = format!("Usage of undeclared variable `{}`", name);
    let stack = state.vars.get_mut(state.current_fn.get_name()).unwrap();
    
    for hm in stack.iter_mut().rev(){
        if hm.contains_key(name) {
            return Ok(hm.get_mut(name).unwrap());
        }
    }

    anyhow::bail!(err);
}

fn gen_var_decl(id: &str, rval: &Node, ast: &Ast, state: &mut State) -> anyhow::Result<()> {
    let lox_value = gen_expr(rval, ast, state)?;
    get_current_env(state).insert(id.to_owned(), lox_value); // if exist overwrites correctly
    Ok(())
}

fn gen_declaration(decl: &Node, ast: &Ast, state: &mut State) -> anyhow::Result<()> {
    match decl {
        Node::VarDecl(id, expr_id) => gen_var_decl(id, &ast.nodes[*expr_id], ast, state),
        Node::ClassDecl(_, _, _) => todo!(),
        Node::FunDecl(_, _, _) => todo!(),
        Node::Stmt(stmt_id) => gen_statement(&ast.nodes[*stmt_id], ast, state),
        _ => unreachable!("In program vector only decl nodes are pushed during parsing"),
    }
}

fn gen_panic_call(msg: StringLiterals, state: &mut State) -> anyhow::Result<()> {
    let error_msg = global_string_literal(msg, state);
    state
        .builder
        .build_call(state.panic_fn, &[error_msg.into()], "_")?;
    state.builder.build_unreachable()?;
    Ok(())
}

fn gen_block<'a>(name: &str, state: &mut State<'a>) -> BasicBlock<'a> {
   state.ctx.append_basic_block(state.current_fn, name) 
}

type VariableStack<'a> = Vec<HashMap<String, LoxValue<'a>>>;

struct State<'a> {
    ctx: &'a Context,
    module: Module<'a>,
    builder: Builder<'a>,
    current_fn: FunctionValue<'a>,
    vars: HashMap<CString, VariableStack<'a>>, // stores variables for functions

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
        current_fn: panic_fn, // temp value
        vars: HashMap::new(),
        panic_fn,
        lox_value,
        string_literals,
    };

    gen_begin_main(&mut state); // builder at @main.entry, current_fn at main

    for decl_id in ast.program.clone() {
        let decl = ast.nodes.get(decl_id).unwrap();
        gen_declaration(decl, &ast, &mut state)?;
    }
    // builder should be at @main.entry
    gen_return_zero(&mut state)?;

    // println!("{}", state.module.to_string());
    if let Err(err) = state.module.verify() {
        eprintln!("Błąd weryfikacji IR: {}", err.to_string());
    }
    Ok(state.module)
}
