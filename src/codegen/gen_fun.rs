use crate::ast::{Ast, Id, Node, NodeID};
use crate::codegen::gen_stmt::gen_statement;
use crate::codegen::lox_value::{LoxValue, LoxValueType, gen_alloc_heap_lox_value, gen_store_ptr};
use crate::codegen::{State, get_current_env, get_var_from_env, push_new_env};
use inkwell::AddressSpace;
use std::collections::HashSet;

/// Walk the AST subtree rooted at node_id and collect all Identifier references
/// that are NOT in `bound` (i.e., free variables).
/// Also handles nested FunDecl: the nested function's params are bound inside it,
/// and its name is bound in the outer scope after the declaration.
pub fn collect_free_vars(
    node_id: NodeID,
    ast: &Ast,
    bound: &mut HashSet<String>,
    free: &mut Vec<String>,
) {
    match &ast.nodes[node_id] {
        Node::Identifier(id) => {
            if !bound.contains(id) && !free.contains(id) {
                free.push(id.clone());
            }
        }
        Node::This => {
            // 'this' treated as an implicit free var named "this"
            let id = "this".to_string();
            if !bound.contains(&id) && !free.contains(&id) {
                free.push(id);
            }
        }
        Node::VarDecl(id, rhs) => {
            collect_free_vars(*rhs, ast, bound, free);
            bound.insert(id.clone());
        }
        Node::FunDecl(id, params, body) => {
            let mut inner_bound = bound.clone();
            inner_bound.insert(id.clone());
            for p in params {
                inner_bound.insert(p.clone());
            }
            collect_free_vars(*body, ast, &mut inner_bound, free);
            bound.insert(id.clone());
        }
        Node::ClassDecl(id, _base, methods) => {
            for mid in methods {
                collect_free_vars(*mid, ast, bound, free);
            }
            bound.insert(id.clone());
        }
        Node::Assignment(maybe_obj, id, rhs) => {
            if let Some(obj_id) = maybe_obj {
                collect_free_vars(*obj_id, ast, bound, free);
            } else if !bound.contains(id) && !free.contains(id) {
                free.push(id.clone());
            }
            collect_free_vars(*rhs, ast, bound, free);
        }
        Node::Binary(l, _, r) => {
            collect_free_vars(*l, ast, bound, free);
            collect_free_vars(*r, ast, bound, free);
        }
        Node::Unary(n, _) => collect_free_vars(*n, ast, bound, free),
        Node::Call(callee, args) => {
            collect_free_vars(*callee, ast, bound, free);
            for arg in args {
                collect_free_vars(*arg, ast, bound, free);
            }
        }
        Node::Grouping(n) => collect_free_vars(*n, ast, bound, free),
        Node::Block(decls) => {
            let mut block_bound = bound.clone();
            for d in decls {
                collect_free_vars(*d, ast, &mut block_bound, free);
            }
        }
        Node::Stmt(n) => collect_free_vars(*n, ast, bound, free),
        Node::ExprStmt(n) => collect_free_vars(*n, ast, bound, free),
        Node::PrintStmt(n) => collect_free_vars(*n, ast, bound, free),
        Node::ReturnStmt(n) => collect_free_vars(*n, ast, bound, free),
        Node::IfStmt(cond, then, else_) => {
            collect_free_vars(*cond, ast, bound, free);
            collect_free_vars(*then, ast, bound, free);
            if let Some(e) = else_ {
                collect_free_vars(*e, ast, bound, free);
            }
        }
        Node::WhileStmt(cond, body) => {
            collect_free_vars(*cond, ast, bound, free);
            collect_free_vars(*body, ast, bound, free);
        }
        Node::Get(obj, _) => collect_free_vars(*obj, ast, bound, free),
        Node::Super(_) => {
            // super requires 'this' to be in scope
            let id = "this".to_string();
            if !bound.contains(&id) && !free.contains(&id) {
                free.push(id);
            }
        }
        // literals
        Node::Number(_) | Node::String(_) | Node::Bool(_) | Node::Nil => {}
    }
}

pub fn gen_fun_decl(
    id: &Id,
    args: &[Id],
    body_id: &NodeID,
    ast: &Ast,
    state: &mut State,
) -> anyhow::Result<()> {
    gen_fun_decl_inner(id, args, body_id, ast, state, false)
}

/// Generate a method: same as fun_decl but 'this' is always env[0]
pub fn gen_method_decl(
    id: &Id,
    args: &[Id],
    body_id: &NodeID,
    ast: &Ast,
    state: &mut State,
) -> anyhow::Result<()> {
    gen_fun_decl_inner(id, args, body_id, ast, state, true)
}

fn gen_fun_decl_inner(
    id: &Id,
    args: &[Id],
    body_id: &NodeID,
    ast: &Ast,
    state: &mut State,
    is_method: bool,
) -> anyhow::Result<()> {
    let prev_fn = state.current_fn;
    let prev_position = state.builder.get_insert_block().unwrap();

    // collect free variables (vars used in body that aren't declared locally)
    let mut bound: HashSet<String> = args.iter().cloned().collect();
    let mut free_vars: Vec<String> = Vec::new();
    if is_method {
        // 'this' is always env[0] for methods
        free_vars.push("this".to_string());
        bound.insert("this".to_string());
    }
    collect_free_vars(*body_id, ast, &mut bound, &mut free_vars);
    // if the function references itself (direct recursion), note its index for patching after creation
    let self_ref_idx = free_vars.iter().position(|v| v == id);

    let ptr_t = state.ctx.ptr_type(AddressSpace::default());
    let i64_t = state.ctx.i64_type();

    // function signature: (ptr env, LoxValue arg1, ...)
    let mut param_types = vec![ptr_t.into()];
    for _ in args {
        param_types.push(state.lox_value.into());
    }
    let fn_type = state.lox_value.fn_type(&param_types, false);
    let fun = state.module.add_function(id, fn_type, None);

    state.current_fn = fun;
    let entry = state.ctx.append_basic_block(fun, &format!("{}_entry", id));
    state.builder.position_at_end(entry);
    state.vars.insert(fun.get_name().into(), Vec::new());
    push_new_env(state)?;

    let env_param = fun.get_nth_param(0).unwrap().into_pointer_value();

    // set up captured variable access (load LoxValue* from env[i])
    for (i, var_name) in free_vars.iter().enumerate() {
        let slot = unsafe {
            state.builder.build_gep(
                ptr_t,
                env_param,
                &[i64_t.const_int(i as u64, false)],
                "env_slot",
            )?
        };
        let captured_ptr = state
            .builder
            .build_load(ptr_t, slot, var_name)?
            .into_pointer_value();
        get_current_env(state).insert(var_name.clone(), LoxValue { ptr: captured_ptr });
    }

    // set up parameters (skip index 0 = env pointer)
    for (i, arg_name) in args.iter().enumerate() {
        let arg_val = fun
            .get_nth_param((i + 1) as u32)
            .unwrap()
            .into_struct_value();
        let arg_ptr = gen_alloc_heap_lox_value(LoxValueType::Nil, state)?;
        state.builder.build_store(arg_ptr.ptr, arg_val)?;
        get_current_env(state).insert(arg_name.clone(), arg_ptr);
    }

    gen_statement(&ast.nodes[*body_id], ast, state)?;

    // ensure all blocks have a terminator (implicit nil return)
    for block in fun.get_basic_blocks() {
        if block.get_terminator().is_none() {
            state.builder.position_at_end(block);
            let mut nil = state.lox_value.get_undef();
            nil = state
                .builder
                .build_insert_value(nil, state.ctx.i8_type().const_int(0, false), 0, "tag")?
                .into_struct_value();
            nil = state
                .builder
                .build_insert_value(nil, state.ctx.i64_type().const_int(0, false), 1, "val")?
                .into_struct_value();
            state.builder.build_return(Some(&nil))?;
        }
    }

    // restore caller context
    state.current_fn = prev_fn;
    state.builder.position_at_end(prev_position);

    // build closure object in the CALLER's context
    let malloc = state.module.get_function("malloc").unwrap();
    let n_env = free_vars.len() as u64;

    // allocate env array and fill with pointers to captured vars
    let env_arr_ptr = if n_env > 0 {
        let ptr_size = i64_t.const_int(8, false);
        let env_arr_size =
            state
                .builder
                .build_int_mul(i64_t.const_int(n_env, false), ptr_size, "env_arr_size")?;
        let env_arr = state
            .builder
            .build_call(malloc, &[env_arr_size.into()], "env_arr")?
            .try_as_basic_value()
            .basic()
            .unwrap()
            .into_pointer_value();
        for (i, var_name) in free_vars.iter().enumerate() {
            let idx = i64_t.const_int(i as u64, false);
            let slot = unsafe {
                state
                    .builder
                    .build_gep(ptr_t, env_arr, &[idx], "env_slot")?
            };
            // null placeholder for: (a) methods' this slot, (b) self-recursive reference
            let ptr_to_store = if (is_method && i == 0) || self_ref_idx == Some(i) {
                ptr_t.const_null()
            } else {
                get_var_from_env(var_name, state)?.ptr
            };
            state.builder.build_store(slot, ptr_to_store)?;
        }
        env_arr
    } else {
        ptr_t.const_null()
    };

    // allocate closure struct: { fn_ptr, env_arr, n_env }
    let closure_size = state.closure_type.size_of().unwrap();
    let closure_mem = state
        .builder
        .build_call(malloc, &[closure_size.into()], "closure_mem")?
        .try_as_basic_value()
        .basic()
        .unwrap()
        .into_pointer_value();

    let fn_ptr = fun.as_global_value().as_pointer_value();
    let fp_field = state
        .builder
        .build_struct_gep(state.closure_type, closure_mem, 0, "fp_f")?;
    state.builder.build_store(fp_field, fn_ptr)?;
    let env_field = state
        .builder
        .build_struct_gep(state.closure_type, closure_mem, 1, "env_f")?;
    state.builder.build_store(env_field, env_arr_ptr)?;
    let ne_field = state
        .builder
        .build_struct_gep(state.closure_type, closure_mem, 2, "ne_f")?;
    state
        .builder
        .build_store(ne_field, i64_t.const_int(n_env, false))?;

    // create LoxValue(Closure) and register in current scope
    let closure_lox = gen_alloc_heap_lox_value(LoxValueType::Closure, state)?;
    gen_store_ptr(&closure_lox, LoxValueType::Closure, closure_mem, state)?;
    get_current_env(state).insert(id.clone(), closure_lox);

    // patch self-reference slot so the function can call itself recursively
    if let Some(self_i) = self_ref_idx {
        let idx = i64_t.const_int(self_i as u64, false);
        let self_slot = unsafe {
            state
                .builder
                .build_gep(ptr_t, env_arr_ptr, &[idx], "self_slot")?
        };
        let self_ptr = get_var_from_env(id, state)?.ptr;
        state.builder.build_store(self_slot, self_ptr)?;
    }

    Ok(())
}
