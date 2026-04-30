use anyhow::anyhow;
use crate::ast;
use crate::ast::{Ast, Node};
use inkwell::AddressSpace;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::{Linkage, Module};
use inkwell::types::BasicMetadataTypeEnum::PointerType;
use inkwell::values::BasicMetadataValueEnum::PointerValue;
use inkwell::values::GlobalValue;

fn gen_extern_functions(module: &Module) {
    // printf
    let context = module.get_context();
    let str = context.ptr_type(AddressSpace::default());
    let tp = context.i32_type().fn_type(&[PointerType(str)], true);
    module.add_function("printf", tp, Some(Linkage::External));
}

fn gen_begin_main(module: &Module, builder: &Builder) {
    let context = module.get_context();
    let main_fn = module.add_function("main", context.i32_type().fn_type(&[], false), None);
    let entry = context.append_basic_block(main_fn, "entry");
    builder.position_at_end(entry);
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
            let expr_val = gen_expr(ast.nodes.get(*expr_id).unwrap(), module, ast,builder).unwrap(); // todo return result
            builder.build_call(
                printf,
                &[PointerValue(expr_val.as_pointer_value())],
                "printf",
            ).unwrap();
        }
        Node::ReturnStmt(_) => {}
        Node::WhileStmt(_, _) => {}
        Node::Block(_) => {}
        _ => unreachable!("Stmt node can only have statements -> assured during parsing"),
    }
}

fn gen_expr<'c>(expr: & Node, module: &'c Module, ast: & Ast, builder: &'c Builder) -> anyhow::Result<GlobalValue<'c>> {
    match expr{
        Node::Assignment(_, _, _) => todo!(),
        Node::LogicOr(_, _) => todo!(),
        Node::LogicAnd(_, _) => todo!(),
        Node::Equality(_, _, _) => todo!(),
        Node::Comparison(_, _, _) => todo!(),
        Node::Term(_, _, _) => todo!(),
        Node::Factor(_, _, _) => todo!(),
        Node::Unary(_, _) => todo!(),
        Node::Call => todo!(),
        Node::Identifier(_) => todo!(),
        Node::Super(_) => todo!(),
        Node::Grouping(_) => todo!(),
        Node::Number(_) => todo!(),
        Node::String(s) => {
            builder.build_global_string_ptr(&format!("{s}\n"), "name").map_err(|_| anyhow!("er"))
        }
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
    gen_begin_main(&module, &builder);

    println!("DECLS: {}", ast.program.len());
    for decl_id in ast.program.clone() {
        println!("DECL: {decl_id}");
        let decl = ast.nodes.get(decl_id).unwrap();
        gen_declaration(decl, &module, &ast, &builder);
    }

    gen_return_zero(&module, &builder)?;

    Ok(module)
}
