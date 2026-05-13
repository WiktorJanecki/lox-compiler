use crate::ast::{Ast, Node, NodeID};
use crate::codegen::gen_expr::gen_expr;
use crate::codegen::lox_value::{gen_truthiness, LoxValue, LoxValueType};
use crate::codegen::string_literals::{global_string_literal, StringLiterals};
use crate::codegen::{lox_index_type, State};
use inkwell::AddressSpace;

pub fn gen_statement(stmt: &Node, ast: &Ast, state: &mut State) -> anyhow::Result<()> {
    match stmt {
        Node::ExprStmt(expr_id) => {
            let _ = gen_expr(&ast.nodes[*expr_id], ast, state)?;
            Ok(())
        }
        Node::IfStmt(expr_id, stmt_id, else_stmt_id) => {
            let lox_val = gen_expr(&ast.nodes[*expr_id], ast, state)?;
            gen_if_stmt(&lox_val, stmt_id, else_stmt_id, ast, state)
        }
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

fn gen_if_stmt<'a>(
    lox_val: &LoxValue<'a>,
    if_stmt: &NodeID,
    else_stmt: &Option<NodeID>,
    ast: &Ast,
    state: &mut State<'a>,
) -> anyhow::Result<()> {
    let parent_func = state
        .builder
        .get_insert_block()
        .unwrap()
        .get_parent()
        .unwrap();
    let if_true = state.ctx.append_basic_block(parent_func, "if_true");
    let if_false = state.ctx.append_basic_block(parent_func, "if_false");
    let after_if = state.ctx.append_basic_block(parent_func, "after_if");

    let truth = gen_truthiness(lox_val, state)?;
    state
        .builder
        .build_conditional_branch(truth, if_true, if_false)?;

    state.builder.position_at_end(if_true);
    gen_statement(&ast.nodes[*if_stmt], ast, state)?;
    state.builder.build_unconditional_branch(after_if)?;

    state.builder.position_at_end(if_false);
    if let Some(else_id) = else_stmt {
        gen_statement(&ast.nodes[*else_id], ast, state)?;
    }
    state.builder.build_unconditional_branch(after_if)?;

    state.builder.position_at_end(after_if);

    Ok(())
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
    let true_block = state.ctx.append_basic_block(parent_func, "print.bool.true");
    let false_block = state.ctx.append_basic_block(parent_func, "print.bool.fals");
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
        .build_call(printf, &[nil_literal.into()], "printf")?;
    state.builder.build_unconditional_branch(merge_block)?;

    state.builder.position_at_end(bool_block);

    let bool_type = state.ctx.bool_type();
    let bool_val = state
        .builder
        .build_load(bool_type, lox_val.union_ptr, "bool")?
        .into_int_value();
    state
        .builder
        .build_conditional_branch(bool_val, true_block, false_block)?;

    state.builder.position_at_end(true_block);
    let true_literal = global_string_literal(StringLiterals::PrintfTrue, state);
    state
        .builder
        .build_call(printf, &[true_literal.into()], "printf")?;
    state.builder.build_unconditional_branch(merge_block)?;

    state.builder.position_at_end(false_block);
    let false_literal = global_string_literal(StringLiterals::PrintfFalse, state);
    state
        .builder
        .build_call(printf, &[false_literal.into()], "printf")?;
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
