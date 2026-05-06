use crate::ast;
use crate::ast::{Ast, Node};
use anyhow::anyhow;
use inkwell::AddressSpace;
use inkwell::basic_block::BasicBlock;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::{Linkage, Module};
use inkwell::types::BasicMetadataTypeEnum::{IntType, PointerType};
use inkwell::values::BasicMetadataValueEnum::PointerValue;
use inkwell::values::{BasicValueEnum, FunctionValue, GlobalValue};

fn gen_extern_functions(module: &Module) {
    // printf
    let context = module.get_context();
    let str = context.ptr_type(AddressSpace::default());
    let tp = context.i32_type().fn_type(&[PointerType(str)], true);
    module.add_function("printf", tp, Some(Linkage::External));
    let tp = context.void_type().fn_type(&[context.i32_type().into()], false);
    module.add_function("exit", tp, Some(Linkage::External));
}

fn gen_panic_fn<'a>(module: &'a Module, builder: &Builder) -> FunctionValue<'a> {
    let context = module.get_context();
    let str = context.ptr_type(AddressSpace::default());
    let tp = context.void_type().fn_type(&[PointerType(str)], false);
    let panic_fn = module.add_function("panic", tp, None);
    let block = context.append_basic_block(panic_fn, "entry");
    builder.position_at_end(block);
    let arg = panic_fn.get_first_param().unwrap();
    let printf = module.get_function("printf").unwrap();
    builder.build_call(printf, &[arg.into()], "_");
    let exit = module.get_function("exit").unwrap();
    builder.build_call(exit, &[context.i32_type().const_int(u64::MAX, false).into()], "_");
    builder.build_unreachable();
    panic_fn
}

fn gen_begin_main<'a>(module: &'a Module, builder: &Builder) -> BasicBlock<'a> {
    let context = module.get_context();
    let main_fn = module.add_function("main", context.i32_type().fn_type(&[], false), None);
    let entry = context.append_basic_block(main_fn, "entry");
    builder.position_at_end(entry);
    entry
}

fn gen_return_zero(module: &Module, builder: &Builder) -> anyhow::Result<()> {
    let context = module.get_context();
    let zero = context.i32_type().const_int(0, false);
    builder.build_return(Some(&zero))?;
    Ok(())
}

fn gen_declaration(decl: &Node, module: &Module, ast: &Ast, builder: &Builder) {
    match decl {
        Node::VarDecl(_, _) => todo!(),
        Node::ClassDecl(_, _, _) => todo!(),
        Node::FunDecl(_, _, _) => todo!(),
        Node::Stmt(stmtID) => gen_statement(&ast.nodes[*stmtID], module, ast, builder),
        _ => unreachable!("In program vector only decl nodes are pushed during parsing"),
    }
}

fn gen_statement(stmt: &Node, module: &Module, ast: &Ast, builder: &Builder) {
    let printf = module.get_function("printf").unwrap();
    match stmt {
        Node::ExprStmt(_) => {}
        Node::IfStmt(_, _, _) => {}
        Node::PrintStmt(expr_id) => {
            let expr_val =
                gen_expr(ast.nodes.get(*expr_id).unwrap(), module, ast, builder).unwrap(); // todo return result
            builder
                .build_call(
                    printf,
                    &[PointerValue(expr_val.as_pointer_value())],
                    "printf",
                )
                .unwrap();
        }
        Node::ReturnStmt(_) => {}
        Node::WhileStmt(_, _) => {}
        Node::Block(_) => {}
        _ => unreachable!("Stmt node can only have statements -> assured during parsing"),
    }
}

fn gen_expr<'c>(
    expr: &Node,
    module: &'c Module,
    ast: &Ast,
    builder: &'c Builder,
) -> anyhow::Result<GlobalValue<'c>> {
    match expr {
        Node::Assignment(_, _, _) => todo!(),
        Node::Binary(_, _, _) => todo!(),
        Node::Unary(_, _) => todo!(),
        Node::Call => todo!(),
        Node::Identifier(_) => todo!(),
        Node::Super(_) => todo!(),
        Node::Grouping(_) => todo!(),
        Node::Number(_) => todo!(),
        Node::String(s) => builder
            .build_global_string_ptr(&format!("{s}\n"), "name")
            .map_err(|_| anyhow!("er")),
        Node::Bool(_) => todo!(),
        Node::Nil => todo!(),
        Node::This => todo!(),
        _ => unreachable!(),
    }
}

pub fn codegen(ast: ast::Ast, context: &mut Context) -> anyhow::Result<Module<'_>> {
    let module = context.create_module("main");
    let builder = context.create_builder();

    gen_extern_functions(&module);
    let panic_fn = gen_panic_fn(&module, &builder);
    let entry_block = gen_begin_main(&module, &builder);
    builder.position_at_end(entry_block);
    let str = builder.build_global_string_ptr("PANIC\n", "panic_msg");
    builder.build_call(panic_fn, &[PointerValue(str?.as_pointer_value())], "_");

    println!("DECLS: {}", ast.program.len());
    for decl_id in ast.program.clone() {
        println!("DECL: {decl_id}");
        let decl = ast.nodes.get(decl_id).unwrap();
        gen_declaration(decl, &module, &ast, &builder);
    }
    gen_return_zero(&module, &builder)?;

    println!("{}", module.to_string());
    Ok(module)
}
