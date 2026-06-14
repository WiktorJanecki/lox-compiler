use crate::ast::{Ast, Id, Node, NodeID};
use crate::codegen::gen_fun::gen_method_decl;
use crate::codegen::lox_value::{
    LoxValue, LoxValueType, gen_alloc_heap_lox_value, gen_load_ptr, gen_store_ptr,
    gen_unpack_lox_value,
};
use crate::codegen::string_literals::StringLiterals;
use crate::codegen::{State, gen_block, gen_panic_call, get_current_env, get_var_from_env};
use inkwell::AddressSpace;
use inkwell::IntPredicate;

/// Generates: lox_get_field(instance_ptr: ptr, name: ptr) -> ptr (LoxValue* or null)
/// Generates: lox_set_field(instance_ptr: ptr, name: ptr, val_ptr: ptr)
/// Generates: lox_get_method(class_ptr: ptr, name: ptr) -> ptr (LoxValue* or null)
pub fn gen_runtime_helpers(state: &mut State) -> anyhow::Result<()> {
    gen_get_field(state)?;
    gen_set_field(state)?;
    gen_get_method(state)?;
    Ok(())
}

fn gen_get_field<'a>(state: &mut State<'a>) -> anyhow::Result<()> {
    let ctx = state.ctx;
    let ptr_t = ctx.ptr_type(AddressSpace::default());
    let i64_t = ctx.i64_type();

    let fn_type = ptr_t.fn_type(&[ptr_t.into(), ptr_t.into()], false);
    let func = state.module.add_function("lox_get_field", fn_type, None);

    let b_entry = ctx.append_basic_block(func, "entry");
    let b_loop_check = ctx.append_basic_block(func, "loop_check");
    let b_loop_body = ctx.append_basic_block(func, "loop_body");
    let b_found = ctx.append_basic_block(func, "found");
    let b_not_found = ctx.append_basic_block(func, "not_found");

    let builder = ctx.create_builder();
    builder.position_at_end(b_entry);

    let instance_ptr = func.get_nth_param(0).unwrap().into_pointer_value();
    let field_name = func.get_nth_param(1).unwrap().into_pointer_value();

    // load n_fields (field index 1 of instance_type)
    let n_fields_ptr = builder.build_struct_gep(state.instance_type, instance_ptr, 1, "n_fields_ptr")?;
    let n_fields = builder.build_load(i64_t, n_fields_ptr, "n_fields")?.into_int_value();
    // load field_names (field index 2)
    let names_ptr = builder.build_struct_gep(state.instance_type, instance_ptr, 2, "names_ptr")?;
    let field_names_arr = builder.build_load(ptr_t, names_ptr, "field_names")?.into_pointer_value();
    // load fields (field index 3)
    let vals_ptr = builder.build_struct_gep(state.instance_type, instance_ptr, 3, "vals_ptr")?;
    let fields_arr = builder.build_load(ptr_t, vals_ptr, "fields")?.into_pointer_value();

    let i = builder.build_alloca(i64_t, "i")?;
    builder.build_store(i, i64_t.const_zero())?;
    builder.build_unconditional_branch(b_loop_check)?;

    builder.position_at_end(b_loop_check);
    let i_val = builder.build_load(i64_t, i, "i_val")?.into_int_value();
    let done = builder.build_int_compare(IntPredicate::SGE, i_val, n_fields, "done")?;
    builder.build_conditional_branch(done, b_not_found, b_loop_body)?;

    builder.position_at_end(b_loop_body);
    let i_val2 = builder.build_load(i64_t, i, "i_val2")?.into_int_value();
    let name_elem = unsafe { builder.build_gep(ptr_t, field_names_arr, &[i_val2], "name_elem")? };
    let name = builder.build_load(ptr_t, name_elem, "name")?.into_pointer_value();
    let strcmp = state.module.get_function("strcmp").unwrap();
    let cmp = builder.build_call(strcmp, &[name.into(), field_name.into()], "cmp")?
        .try_as_basic_value().basic().unwrap().into_int_value();
    let eq = builder.build_int_compare(IntPredicate::EQ, cmp, ctx.i32_type().const_zero(), "eq")?;
    let next_i = builder.build_int_add(i_val2, i64_t.const_int(1, false), "next_i")?;
    builder.build_store(i, next_i)?;
    builder.build_conditional_branch(eq, b_found, b_loop_check)?;

    builder.position_at_end(b_found);
    // i was already incremented; field index = i_val2
    let val_elem = unsafe { builder.build_gep(ptr_t, fields_arr, &[i_val2], "val_elem")? };
    let val_ptr = builder.build_load(ptr_t, val_elem, "val_ptr")?.into_pointer_value();
    builder.build_return(Some(&val_ptr))?;

    builder.position_at_end(b_not_found);
    builder.build_return(Some(&ptr_t.const_null()))?;

    Ok(())
}

fn gen_set_field<'a>(state: &mut State<'a>) -> anyhow::Result<()> {
    let ctx = state.ctx;
    let ptr_t = ctx.ptr_type(AddressSpace::default());
    let i64_t = ctx.i64_type();

    let fn_type = ctx.void_type().fn_type(&[ptr_t.into(), ptr_t.into(), ptr_t.into()], false);
    let func = state.module.add_function("lox_set_field", fn_type, None);

    let b_entry = ctx.append_basic_block(func, "entry");
    let b_loop_check = ctx.append_basic_block(func, "loop_check");
    let b_loop_body = ctx.append_basic_block(func, "loop_body");
    let b_found = ctx.append_basic_block(func, "found");
    let b_add_new = ctx.append_basic_block(func, "add_new");

    let builder = ctx.create_builder();
    builder.position_at_end(b_entry);

    let instance_ptr = func.get_nth_param(0).unwrap().into_pointer_value();
    let field_name = func.get_nth_param(1).unwrap().into_pointer_value();
    let val_ptr = func.get_nth_param(2).unwrap().into_pointer_value();

    let n_fields_gep = builder.build_struct_gep(state.instance_type, instance_ptr, 1, "nf_gep")?;
    let n_fields = builder.build_load(i64_t, n_fields_gep, "n_fields")?.into_int_value();
    let names_gep = builder.build_struct_gep(state.instance_type, instance_ptr, 2, "ng_gep")?;
    let field_names_arr = builder.build_load(ptr_t, names_gep, "field_names")?.into_pointer_value();
    let vals_gep = builder.build_struct_gep(state.instance_type, instance_ptr, 3, "vg_gep")?;
    let fields_arr = builder.build_load(ptr_t, vals_gep, "fields")?.into_pointer_value();

    let i = builder.build_alloca(i64_t, "i")?;
    builder.build_store(i, i64_t.const_zero())?;
    builder.build_unconditional_branch(b_loop_check)?;

    builder.position_at_end(b_loop_check);
    let i_val = builder.build_load(i64_t, i, "i_val")?.into_int_value();
    let done = builder.build_int_compare(IntPredicate::SGE, i_val, n_fields, "done")?;
    builder.build_conditional_branch(done, b_add_new, b_loop_body)?;

    builder.position_at_end(b_loop_body);
    let i_val2 = builder.build_load(i64_t, i, "i_val2")?.into_int_value();
    let name_elem = unsafe { builder.build_gep(ptr_t, field_names_arr, &[i_val2], "name_elem")? };
    let name = builder.build_load(ptr_t, name_elem, "name")?.into_pointer_value();
    let strcmp = state.module.get_function("strcmp").unwrap();
    let cmp = builder.build_call(strcmp, &[name.into(), field_name.into()], "cmp")?
        .try_as_basic_value().basic().unwrap().into_int_value();
    let eq = builder.build_int_compare(IntPredicate::EQ, cmp, ctx.i32_type().const_zero(), "eq")?;
    let next_i = builder.build_int_add(i_val2, i64_t.const_int(1, false), "next_i")?;
    builder.build_store(i, next_i)?;
    builder.build_conditional_branch(eq, b_found, b_loop_check)?;

    builder.position_at_end(b_found);
    let val_elem = unsafe { builder.build_gep(ptr_t, fields_arr, &[i_val2], "val_elem")? };
    builder.build_store(val_elem, val_ptr)?;
    builder.build_return(None)?;

    // add new field
    builder.position_at_end(b_add_new);
    let malloc = state.module.get_function("malloc").unwrap();
    let realloc = state.module.get_function("realloc").unwrap();

    let new_n = builder.build_int_add(n_fields, i64_t.const_int(1, false), "new_n")?;
    let ptr_size = i64_t.const_int(8, false); // sizeof(ptr) = 8 on 64-bit

    // realloc field_names array
    let new_names_size = builder.build_int_mul(new_n, ptr_size, "new_names_size")?;
    let new_names = builder.build_call(realloc, &[field_names_arr.into(), new_names_size.into()], "new_names")?
        .try_as_basic_value().basic().unwrap().into_pointer_value();
    // duplicate the field_name string
    let strlen = state.module.get_function("strlen").unwrap();
    let strcpy = state.module.get_function("strcpy").unwrap();
    let name_len = builder.build_call(strlen, &[field_name.into()], "name_len")?
        .try_as_basic_value().basic().unwrap().into_int_value();
    let name_buf_size = builder.build_int_add(name_len, i64_t.const_int(1, false), "name_buf_size")?;
    let name_copy = builder.build_call(malloc, &[name_buf_size.into()], "name_copy")?
        .try_as_basic_value().basic().unwrap().into_pointer_value();
    builder.build_call(strcpy, &[name_copy.into(), field_name.into()], "_")?;
    // store copied name at index n_fields
    let new_name_slot = unsafe { builder.build_gep(ptr_t, new_names, &[n_fields], "new_name_slot")? };
    builder.build_store(new_name_slot, name_copy)?;

    // realloc fields array
    let new_vals_size = builder.build_int_mul(new_n, ptr_size, "new_vals_size")?;
    let new_vals = builder.build_call(realloc, &[fields_arr.into(), new_vals_size.into()], "new_vals")?
        .try_as_basic_value().basic().unwrap().into_pointer_value();
    let new_val_slot = unsafe { builder.build_gep(ptr_t, new_vals, &[n_fields], "new_val_slot")? };
    builder.build_store(new_val_slot, val_ptr)?;

    // update instance fields
    builder.build_store(n_fields_gep, new_n)?;
    builder.build_store(names_gep, new_names)?;
    builder.build_store(vals_gep, new_vals)?;
    builder.build_return(None)?;

    Ok(())
}

fn gen_get_method<'a>(state: &mut State<'a>) -> anyhow::Result<()> {
    let ctx = state.ctx;
    let ptr_t = ctx.ptr_type(AddressSpace::default());
    let i64_t = ctx.i64_type();

    let fn_type = ptr_t.fn_type(&[ptr_t.into(), ptr_t.into()], false);
    let func = state.module.add_function("lox_get_method", fn_type, None);

    let b_entry = ctx.append_basic_block(func, "entry");
    let b_null_check = ctx.append_basic_block(func, "null_check");
    let b_loop_check = ctx.append_basic_block(func, "loop_check");
    let b_loop_body = ctx.append_basic_block(func, "loop_body");
    let b_found = ctx.append_basic_block(func, "found");
    let b_try_super = ctx.append_basic_block(func, "try_super");
    let b_not_found = ctx.append_basic_block(func, "not_found");

    let builder = ctx.create_builder();
    builder.position_at_end(b_entry);

    let class_ptr = func.get_nth_param(0).unwrap().into_pointer_value();
    let method_name = func.get_nth_param(1).unwrap().into_pointer_value();

    // alloca in entry block to satisfy Windows SEH unwind requirements
    let i = builder.build_alloca(i64_t, "i")?;

    let is_null = builder.build_is_null(class_ptr, "is_null")?;
    builder.build_conditional_branch(is_null, b_not_found, b_null_check)?;

    builder.position_at_end(b_null_check);
    let n_methods_ptr = builder.build_struct_gep(state.class_type, class_ptr, 2, "nm_ptr")?;
    let n_methods = builder.build_load(i64_t, n_methods_ptr, "n_methods")?.into_int_value();
    let method_names_ptr = builder.build_struct_gep(state.class_type, class_ptr, 3, "mn_ptr")?;
    let method_names_arr = builder.build_load(ptr_t, method_names_ptr, "method_names")?.into_pointer_value();
    let methods_ptr = builder.build_struct_gep(state.class_type, class_ptr, 4, "mp_ptr")?;
    let methods_arr = builder.build_load(ptr_t, methods_ptr, "methods")?.into_pointer_value();

    builder.build_store(i, i64_t.const_zero())?;
    builder.build_unconditional_branch(b_loop_check)?;

    builder.position_at_end(b_loop_check);
    let i_val = builder.build_load(i64_t, i, "i_val")?.into_int_value();
    let done = builder.build_int_compare(IntPredicate::SGE, i_val, n_methods, "done")?;
    builder.build_conditional_branch(done, b_try_super, b_loop_body)?;

    builder.position_at_end(b_loop_body);
    let i_val2 = builder.build_load(i64_t, i, "i_val2")?.into_int_value();
    let name_elem = unsafe { builder.build_gep(ptr_t, method_names_arr, &[i_val2], "name_elem")? };
    let name = builder.build_load(ptr_t, name_elem, "name")?.into_pointer_value();
    let strcmp = state.module.get_function("strcmp").unwrap();
    let cmp = builder.build_call(strcmp, &[name.into(), method_name.into()], "cmp")?
        .try_as_basic_value().basic().unwrap().into_int_value();
    let eq = builder.build_int_compare(IntPredicate::EQ, cmp, ctx.i32_type().const_zero(), "eq")?;
    let next_i = builder.build_int_add(i_val2, i64_t.const_int(1, false), "next_i")?;
    builder.build_store(i, next_i)?;
    builder.build_conditional_branch(eq, b_found, b_loop_check)?;

    builder.position_at_end(b_found);
    let val_elem = unsafe { builder.build_gep(ptr_t, methods_arr, &[i_val2], "val_elem")? };
    let method_lox_val = builder.build_load(ptr_t, val_elem, "method_lox_val")?.into_pointer_value();
    builder.build_return(Some(&method_lox_val))?;

    // try superclass
    builder.position_at_end(b_try_super);
    let super_ptr_gep = builder.build_struct_gep(state.class_type, class_ptr, 1, "super_ptr_gep")?;
    let super_ptr = builder.build_load(ptr_t, super_ptr_gep, "super_ptr")?.into_pointer_value();
    let get_method_fn = state.module.get_function("lox_get_method").unwrap();
    let result = builder.build_call(get_method_fn, &[super_ptr.into(), method_name.into()], "super_result")?
        .try_as_basic_value().basic().unwrap().into_pointer_value();
    builder.build_return(Some(&result))?;

    builder.position_at_end(b_not_found);
    builder.build_return(Some(&ptr_t.const_null()))?;

    Ok(())
}

pub fn gen_class_decl(
    name: &Id,
    base_name: Option<&str>,
    method_ids: &[NodeID],
    ast: &Ast,
    state: &mut State,
) -> anyhow::Result<()> {
    let ptr_t = state.ctx.ptr_type(AddressSpace::default());
    let i64_t = state.ctx.i64_type();
    let malloc = state.module.get_function("malloc").unwrap();

    // get superclass LoxValue* if any
    let super_class_ptr = if let Some(base) = base_name {
        let base_val = get_var_from_env(base, state)?.clone();
        let (tag, _) = gen_unpack_lox_value(&base_val, state)?;
        let b_ok = gen_block("super_ok", state);
        let b_err = gen_block("super_err", state);
        let class_tag = LoxValueType::Class.llvm_int(state.ctx);
        let is_class = state.builder.build_int_compare(IntPredicate::EQ, tag, class_tag, "is_class")?;
        state.builder.build_conditional_branch(is_class, b_ok, b_err)?;
        state.builder.position_at_end(b_err);
        gen_panic_call(StringLiterals::ReNotAnInstance, state)?;
        state.builder.position_at_end(b_ok);
        gen_load_ptr(&base_val, state)?
    } else {
        ptr_t.const_null()
    };

    // alloc LoxClass struct
    let class_struct_size = state.class_type.size_of().unwrap();
    let class_mem = state.builder.build_call(malloc, &[class_struct_size.into()], "class_mem")?
        .try_as_basic_value().basic().unwrap().into_pointer_value();

    // store name cstr
    let name_cstr = state.builder.build_global_string_ptr(name, "class_name")?.as_pointer_value();
    let name_field = state.builder.build_struct_gep(state.class_type, class_mem, 0, "name_f")?;
    state.builder.build_store(name_field, name_cstr)?;

    // store superclass ptr
    let super_field = state.builder.build_struct_gep(state.class_type, class_mem, 1, "super_f")?;
    state.builder.build_store(super_field, super_class_ptr)?;

    let n_methods = method_ids.len() as u64;

    // store n_methods
    let nm_field = state.builder.build_struct_gep(state.class_type, class_mem, 2, "nm_f")?;
    state.builder.build_store(nm_field, i64_t.const_int(n_methods, false))?;

    if n_methods == 0 {
        let names_field = state.builder.build_struct_gep(state.class_type, class_mem, 3, "nf_f")?;
        state.builder.build_store(names_field, ptr_t.const_null())?;
        let methods_field = state.builder.build_struct_gep(state.class_type, class_mem, 4, "mf_f")?;
        state.builder.build_store(methods_field, ptr_t.const_null())?;
    } else {
        let ptr_size = i64_t.const_int(8, false);
        let arr_size = state.builder.build_int_mul(
            i64_t.const_int(n_methods, false),
            ptr_size,
            "arr_size",
        )?;

        let method_names_arr = state.builder.build_call(malloc, &[arr_size.into()], "mnames")?
            .try_as_basic_value().basic().unwrap().into_pointer_value();
        let methods_arr = state.builder.build_call(malloc, &[arr_size.into()], "methods")?
            .try_as_basic_value().basic().unwrap().into_pointer_value();

        for (i, method_id) in method_ids.iter().enumerate() {
            if let Node::FunDecl(method_name, args, body_id) = &ast.nodes[*method_id] {
                // generate as method so 'this' is always env[0]
                gen_method_decl(method_name, args, body_id, ast, state)?;

                // retrieve the closure LoxValue stored by gen_fun_decl
                let closure_val = get_var_from_env(method_name, state)?.clone();

                // heap-allocate a copy of the closure LoxValue to store in the methods array
                let lox_val_size = state.lox_value.size_of().unwrap();
                let method_slot = state.builder.build_call(malloc, &[lox_val_size.into()], "method_slot")?
                    .try_as_basic_value().basic().unwrap().into_pointer_value();
                let loaded = state.builder.build_load(state.lox_value, closure_val.ptr, "method_val")?;
                state.builder.build_store(method_slot, loaded)?;

                let idx = i64_t.const_int(i as u64, false);
                let name_cstr = state.builder.build_global_string_ptr(method_name, "mn_cstr")?.as_pointer_value();
                let name_slot = unsafe { state.builder.build_gep(ptr_t, method_names_arr, &[idx], "name_slot")? };
                state.builder.build_store(name_slot, name_cstr)?;
                let method_ptr_slot = unsafe { state.builder.build_gep(ptr_t, methods_arr, &[idx], "method_ptr_slot")? };
                state.builder.build_store(method_ptr_slot, method_slot)?;
            }
        }

        let names_field = state.builder.build_struct_gep(state.class_type, class_mem, 3, "nf_f2")?;
        state.builder.build_store(names_field, method_names_arr)?;
        let methods_field = state.builder.build_struct_gep(state.class_type, class_mem, 4, "mf_f2")?;
        state.builder.build_store(methods_field, methods_arr)?;
    }

    // store class as LoxValue(Class) in current scope
    let class_lox = gen_alloc_heap_lox_value(LoxValueType::Class, state)?;
    gen_store_ptr(&class_lox, LoxValueType::Class, class_mem, state)?;
    get_current_env(state).insert(name.clone(), class_lox);
    Ok(())
}

/// Create a bound method: a new closure with `this` prepended to env
pub fn bind_method<'a>(
    method_lox_ptr: inkwell::values::PointerValue<'a>,
    this_lox: &LoxValue<'a>,
    state: &mut State<'a>,
) -> anyhow::Result<LoxValue<'a>> {
    let ptr_t = state.ctx.ptr_type(AddressSpace::default());
    let i64_t = state.ctx.i64_type();
    let malloc = state.module.get_function("malloc").unwrap();

    // load the method's LoxValue (Closure tag, data = closure obj ptr)
    let method_lox = LoxValue { ptr: method_lox_ptr };
    let closure_obj_ptr = gen_load_ptr(&method_lox, state)?;

    // read n_env from existing closure
    let n_env_gep = state.builder.build_struct_gep(state.closure_type, closure_obj_ptr, 2, "n_env_gep")?;
    let n_env = state.builder.build_load(i64_t, n_env_gep, "n_env")?.into_int_value();
    let old_env_gep = state.builder.build_struct_gep(state.closure_type, closure_obj_ptr, 1, "old_env_gep")?;
    let old_env = state.builder.build_load(ptr_t, old_env_gep, "old_env")?.into_pointer_value();
    let fn_ptr_gep = state.builder.build_struct_gep(state.closure_type, closure_obj_ptr, 0, "fn_ptr_gep")?;
    let fn_ptr = state.builder.build_load(ptr_t, fn_ptr_gep, "fn_ptr")?.into_pointer_value();

    // env[0] was a null placeholder set at class-declaration time.
    // Replace it with this_lox and copy env[1..] unchanged — same total size.
    let ptr_size = i64_t.const_int(8, false);
    let new_env_size = state.builder.build_int_mul(n_env, ptr_size, "new_env_size")?;
    let new_env = state.builder.build_call(malloc, &[new_env_size.into()], "new_env")?
        .try_as_basic_value().basic().unwrap().into_pointer_value();

    let this_slot = unsafe { state.builder.build_gep(ptr_t, new_env, &[i64_t.const_zero()], "this_slot")? };
    state.builder.build_store(this_slot, this_lox.ptr)?;

    let n_to_copy = state.builder.build_int_sub(n_env, i64_t.const_int(1, false), "n_to_copy")?;
    let src_start = unsafe { state.builder.build_gep(ptr_t, old_env, &[i64_t.const_int(1, false)], "src_start")? };
    let dst_start = unsafe { state.builder.build_gep(ptr_t, new_env, &[i64_t.const_int(1, false)], "dst_start")? };
    copy_env_loop(src_start, dst_start, n_to_copy, i64_t.const_zero(), state)?;

    // alloc new closure obj
    let closure_size = state.closure_type.size_of().unwrap();
    let new_closure = state.builder.build_call(malloc, &[closure_size.into()], "bound_closure")?
        .try_as_basic_value().basic().unwrap().into_pointer_value();
    let fp_f = state.builder.build_struct_gep(state.closure_type, new_closure, 0, "fp_f")?;
    state.builder.build_store(fp_f, fn_ptr)?;
    let env_f = state.builder.build_struct_gep(state.closure_type, new_closure, 1, "env_f")?;
    state.builder.build_store(env_f, new_env)?;
    let ne_f = state.builder.build_struct_gep(state.closure_type, new_closure, 2, "ne_f")?;
    state.builder.build_store(ne_f, n_env)?;

    let result = super::lox_value::gen_alloc_lox_value(LoxValueType::Closure, state)?;
    gen_store_ptr(&result, LoxValueType::Closure, new_closure, state)?;
    Ok(result)
}

fn copy_env_loop<'a>(
    src: inkwell::values::PointerValue<'a>,
    dst: inkwell::values::PointerValue<'a>,
    count: inkwell::values::IntValue<'a>,
    dst_offset: inkwell::values::IntValue<'a>,
    state: &mut State<'a>,
) -> anyhow::Result<()> {
    let ptr_t = state.ctx.ptr_type(AddressSpace::default());
    let i64_t = state.ctx.i64_type();

    let b_check = gen_block("copy_check", state);
    let b_body = gen_block("copy_body", state);
    let b_merge = gen_block("copy_merge", state);

    let i = super::build_entry_block_alloca(i64_t, "copy_i", state)?;
    state.builder.build_store(i, i64_t.const_zero())?;
    state.builder.build_unconditional_branch(b_check)?;

    state.builder.position_at_end(b_check);
    let i_val = state.builder.build_load(i64_t, i, "ci")?.into_int_value();
    let done = state.builder.build_int_compare(IntPredicate::SGE, i_val, count, "done")?;
    state.builder.build_conditional_branch(done, b_merge, b_body)?;

    state.builder.position_at_end(b_body);
    let i_val2 = state.builder.build_load(i64_t, i, "ci2")?.into_int_value();
    let src_elem = unsafe { state.builder.build_gep(ptr_t, src, &[i_val2], "src_elem")? };
    let val = state.builder.build_load(ptr_t, src_elem, "env_val")?.into_pointer_value();
    let dst_idx = state.builder.build_int_add(i_val2, dst_offset, "dst_idx")?;
    let dst_elem = unsafe { state.builder.build_gep(ptr_t, dst, &[dst_idx], "dst_elem")? };
    state.builder.build_store(dst_elem, val)?;
    let next_i = state.builder.build_int_add(i_val2, i64_t.const_int(1, false), "next_i")?;
    state.builder.build_store(i, next_i)?;
    state.builder.build_unconditional_branch(b_check)?;

    state.builder.position_at_end(b_merge);
    Ok(())
}

/// Create a new LoxInstance of a class and call init if present
pub fn gen_instance_creation<'a>(
    class_lox: &LoxValue<'a>,
    args: &[NodeID],
    ast: &Ast,
    state: &mut State<'a>,
) -> anyhow::Result<LoxValue<'a>> {
    let ptr_t = state.ctx.ptr_type(AddressSpace::default());
    let i64_t = state.ctx.i64_type();
    let malloc = state.module.get_function("malloc").unwrap();

    let class_ptr = gen_load_ptr(class_lox, state)?;

    // alloc LoxInstance
    let inst_size = state.instance_type.size_of().unwrap();
    let inst_mem = state.builder.build_call(malloc, &[inst_size.into()], "inst_mem")?
        .try_as_basic_value().basic().unwrap().into_pointer_value();

    // { class_ptr, 0, null, null }
    let cls_f = state.builder.build_struct_gep(state.instance_type, inst_mem, 0, "cls_f")?;
    state.builder.build_store(cls_f, class_ptr)?;
    let nf_f = state.builder.build_struct_gep(state.instance_type, inst_mem, 1, "nf_f")?;
    state.builder.build_store(nf_f, i64_t.const_zero())?;
    let fn_f = state.builder.build_struct_gep(state.instance_type, inst_mem, 2, "fn_f")?;
    state.builder.build_store(fn_f, ptr_t.const_null())?;
    let fv_f = state.builder.build_struct_gep(state.instance_type, inst_mem, 3, "fv_f")?;
    state.builder.build_store(fv_f, ptr_t.const_null())?;

    // wrap in LoxValue(Instance)
    let inst_lox = super::lox_value::gen_alloc_lox_value(LoxValueType::Instance, state)?;
    gen_store_ptr(&inst_lox, LoxValueType::Instance, inst_mem, state)?;

    // call init if present
    let init_name_cstr = state.builder.build_global_string_ptr("init", "init_name")?.as_pointer_value();
    let get_method_fn = state.module.get_function("lox_get_method").unwrap();
    let init_ptr = state.builder.build_call(get_method_fn, &[class_ptr.into(), init_name_cstr.into()], "init_ptr")?
        .try_as_basic_value().basic().unwrap().into_pointer_value();

    let b_has_init = gen_block("has_init", state);
    let b_no_init = gen_block("no_init", state);
    let b_after_init = gen_block("after_init", state);

    let has_init = state.builder.build_is_not_null(init_ptr, "has_init")?;
    state.builder.build_conditional_branch(has_init, b_has_init, b_no_init)?;

    state.builder.position_at_end(b_has_init);
    let bound = bind_method(init_ptr, &inst_lox, state)?;
    // call bound init with args
    super::gen_expr::call_closure_value(&bound, args, ast, state)?;
    state.builder.build_unconditional_branch(b_after_init)?;

    state.builder.position_at_end(b_no_init);
    state.builder.build_unconditional_branch(b_after_init)?;

    state.builder.position_at_end(b_after_init);
    Ok(inst_lox)
}
