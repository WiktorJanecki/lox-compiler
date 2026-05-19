use crate::ast::{Ast, Id, Node, NodeID};
use crate::codegen::gen_stmt::gen_statement;
use crate::codegen::lox_value::{LoxValue, LoxValueType, gen_alloc_lox_value};
use crate::codegen::{State, get_current_env, push_new_env};
use std::os::linux::raw::stat;

pub fn gen_fun_decl(
    id: &Id,
    args: &Vec<Id>,
    body_id: &NodeID,
    ast: &Ast,
    state: &mut State,
) -> anyhow::Result<()> {
    let prev_fn = state.current_fn;
    let fun = state.module.add_function(
        id,
        state
            .lox_value
            .fn_type(&vec![state.lox_value.into(); args.len()], false),
        None,
    );
    state.current_fn = fun;
    let entry = state.ctx.append_basic_block(fun, &format!("{}_entry", id));
    state.builder.position_at_end(entry);
    state.vars.insert(fun.get_name().into(), Vec::new());
    push_new_env(state)?;
    let lox_value = state.lox_value;
    let mut id = 0;
    for arg in args {
        let arg_val = fun.get_nth_param(id).unwrap().into_struct_value();
        let arg_alloca = state.builder.build_alloca(lox_value, arg).unwrap();
        state.builder.build_store(arg_alloca, arg_val)?;
        get_current_env(state).insert(arg.into(), LoxValue { ptr: arg_alloca });
        id += 1;
    }
    gen_statement(&ast.nodes[*body_id], ast, state)?;
    let mut nil = state.lox_value.get_undef();
    nil = state
        .builder
        .build_insert_value(nil, state.ctx.i8_type().const_int(0, false), 0, "tag")?
        .into_struct_value();
    nil = state
        .builder
        .build_insert_value(nil, state.ctx.i64_type().const_int(0, false), 1, "tag")?
        .into_struct_value();
    state.builder.build_return(Some(&nil))?;
    state.current_fn = prev_fn;
    state
        .builder
        .position_at_end(state.current_fn.get_last_basic_block().unwrap());
    Ok(())
}
