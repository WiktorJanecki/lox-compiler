use crate::codegen::{State, lox_index_type};
use inkwell::AddressSpace;
use inkwell::values;

/// Lox value is a tagged union. This enum must be used to map between tag integer value
#[derive(Copy, Clone)]
pub enum LoxValueType {
    Nil,
    Number,
    Bool,
    String,
    Closure,
    Instance,
    Class,

    #[allow(clippy::upper_case_acronyms)]
    SIZE,
}
impl LoxValueType {
    pub fn llvm_int<'a>(
        &self,
        ctx: &'a inkwell::context::Context,
    ) -> inkwell::values::IntValue<'a> {
        ctx.i8_type().const_int(*self as u64, false)
    }
}

/// LLVM representation of dynamic typed variable. This should be used to do all expressions instead of pure llvm types
#[derive(Clone)]
pub struct LoxValue<'a> {
    pub ptr: values::PointerValue<'a>,
}

pub fn gen_unpack_lox_value<'a>(
    val: &LoxValue<'a>,
    state: &mut State<'a>,
) -> anyhow::Result<(values::IntValue<'a>, values::PointerValue<'a>)> {
    let index_ptr = state
        .builder
        .build_struct_gep(state.lox_value, val.ptr, 0, "left_tag_ptr")?;
    let union_ptr =
        state
            .builder
            .build_struct_gep(state.lox_value, val.ptr, 1, "left_union_ptr")?;

    let tag_val = state
        .builder
        .build_load(lox_index_type(state.ctx), index_ptr, "left_tag")?
        .into_int_value();

    Ok((tag_val, union_ptr))
}

/// Stack-allocated LoxValue (for temporaries).
/// Alloca is hoisted to the function entry block to prevent stack overflow in loops.
pub fn gen_alloc_lox_value<'a>(
    typee: LoxValueType,
    state: &mut State<'a>,
) -> anyhow::Result<LoxValue<'a>> {
    let ptr = super::build_entry_block_alloca(state.lox_value, "lox_val", state)?;
    let index_ptr = state
        .builder
        .build_struct_gep(state.lox_value, ptr, 0, "index")?;
    state
        .builder
        .build_store(index_ptr, typee.llvm_int(state.ctx))?;
    Ok(LoxValue { ptr })
}

/// Heap-allocated LoxValue (for variables and closures — survives stack frame)
pub fn gen_alloc_heap_lox_value<'a>(
    typee: LoxValueType,
    state: &mut State<'a>,
) -> anyhow::Result<LoxValue<'a>> {
    let malloc_fn = state.module.get_function("malloc").unwrap();
    let size = state.lox_value.size_of().unwrap();
    let ptr = state
        .builder
        .build_call(malloc_fn, &[size.into()], "heap_lox")?
        .try_as_basic_value()
        .basic()
        .unwrap()
        .into_pointer_value();
    let index_ptr = state
        .builder
        .build_struct_gep(state.lox_value, ptr, 0, "index")?;
    state
        .builder
        .build_store(index_ptr, typee.llvm_int(state.ctx))?;
    Ok(LoxValue { ptr })
}

pub fn gen_store_number<'a>(
    var: &LoxValue<'a>,
    num: values::FloatValue<'a>,
    state: &mut State<'a>,
) -> anyhow::Result<()> {
    let index_ptr = state
        .builder
        .build_struct_gep(state.lox_value, var.ptr, 0, "index_ptr")?;
    let union_ptr = state
        .builder
        .build_struct_gep(state.lox_value, var.ptr, 1, "union_ptr")?;
    state
        .builder
        .build_store(index_ptr, LoxValueType::Number.llvm_int(state.ctx))?;
    state.builder.build_store(union_ptr, num)?;
    Ok(())
}

pub fn gen_store_string<'a>(
    var: &LoxValue<'a>,
    cstr: values::PointerValue<'a>,
    state: &mut State<'a>,
) -> anyhow::Result<()> {
    let index_ptr = state
        .builder
        .build_struct_gep(state.lox_value, var.ptr, 0, "index_ptr")?;
    let union_ptr = state
        .builder
        .build_struct_gep(state.lox_value, var.ptr, 1, "union_ptr")?;
    state
        .builder
        .build_store(index_ptr, LoxValueType::String.llvm_int(state.ctx))?;
    state.builder.build_store(union_ptr, cstr)?;
    Ok(())
}

pub fn gen_store_bool<'a>(
    var: &LoxValue<'a>,
    bol: values::IntValue<'a>,
    state: &mut State<'a>,
) -> anyhow::Result<()> {
    let index_ptr = state
        .builder
        .build_struct_gep(state.lox_value, var.ptr, 0, "index_ptr")?;
    let union_ptr = state
        .builder
        .build_struct_gep(state.lox_value, var.ptr, 1, "union_ptr")?;
    state
        .builder
        .build_store(index_ptr, LoxValueType::Bool.llvm_int(state.ctx))?;
    state.builder.build_store(union_ptr, bol)?;
    Ok(())
}

/// Store a pointer (closure/instance/class struct) as i64 in the LoxValue data field
pub fn gen_store_ptr<'a>(
    var: &LoxValue<'a>,
    tag: LoxValueType,
    obj_ptr: values::PointerValue<'a>,
    state: &mut State<'a>,
) -> anyhow::Result<()> {
    let index_ptr = state
        .builder
        .build_struct_gep(state.lox_value, var.ptr, 0, "index_ptr")?;
    let union_ptr = state
        .builder
        .build_struct_gep(state.lox_value, var.ptr, 1, "union_ptr")?;
    state
        .builder
        .build_store(index_ptr, tag.llvm_int(state.ctx))?;
    let as_int = state
        .builder
        .build_ptr_to_int(obj_ptr, state.ctx.i64_type(), "ptr_as_i64")?;
    state.builder.build_store(union_ptr, as_int)?;
    Ok(())
}

/// Load a pointer from the LoxValue data field (inverse of gen_store_ptr)
pub fn gen_load_ptr<'a>(
    var: &LoxValue<'a>,
    state: &mut State<'a>,
) -> anyhow::Result<values::PointerValue<'a>> {
    let union_ptr = state
        .builder
        .build_struct_gep(state.lox_value, var.ptr, 1, "union_ptr")?;
    let as_int = state
        .builder
        .build_load(state.ctx.i64_type(), union_ptr, "ptr_i64")?
        .into_int_value();
    let ptr = state.builder.build_int_to_ptr(
        as_int,
        state.ctx.ptr_type(AddressSpace::default()),
        "obj_ptr",
    )?;
    Ok(ptr)
}

pub fn gen_truthiness<'a>(
    lox_val: &LoxValue<'a>,
    state: &mut State<'a>,
) -> anyhow::Result<values::IntValue<'a>> {
    let (tag_val, union_ptr) = gen_unpack_lox_value(lox_val, state)?;
    let parent_func = state.current_fn;
    let bool_block = state.ctx.append_basic_block(parent_func, "print.bool");
    let true_block = state.ctx.append_basic_block(parent_func, "print.true");
    let false_block = state.ctx.append_basic_block(parent_func, "print.false");
    let merge_block = state.ctx.append_basic_block(parent_func, "print.merge");
    let unreach_block = state.ctx.append_basic_block(parent_func, "print.unreach");

    let cases = &[
        (LoxValueType::Nil.llvm_int(state.ctx), false_block),
        (LoxValueType::Number.llvm_int(state.ctx), true_block),
        (LoxValueType::Bool.llvm_int(state.ctx), bool_block),
        (LoxValueType::String.llvm_int(state.ctx), true_block),
        (LoxValueType::Closure.llvm_int(state.ctx), true_block),
        (LoxValueType::Instance.llvm_int(state.ctx), true_block),
        (LoxValueType::Class.llvm_int(state.ctx), true_block),
    ];
    assert_eq!(cases.len(), LoxValueType::SIZE as usize);

    let bool_type = state.ctx.bool_type();
    let result = gen_alloc_lox_value(LoxValueType::Bool, state)?;
    state.builder.build_switch(tag_val, unreach_block, cases)?;

    state.builder.position_at_end(unreach_block);
    state.builder.build_unreachable()?;

    {
        state.builder.position_at_end(bool_block);
        let result_union_ptr =
            state
                .builder
                .build_struct_gep(state.lox_value, result.ptr, 1, "result_union_ptr")?;
        let bool_val = state
            .builder
            .build_load(bool_type, union_ptr, "bool_val")?
            .into_int_value();
        state.builder.build_store(result_union_ptr, bool_val)?;
        state.builder.build_unconditional_branch(merge_block)?;
    }

    {
        state.builder.position_at_end(true_block);
        let result_union_ptr =
            state
                .builder
                .build_struct_gep(state.lox_value, result.ptr, 1, "result_union_ptr")?;
        state
            .builder
            .build_store(result_union_ptr, bool_type.const_int(1, false))?;
        state.builder.build_unconditional_branch(merge_block)?;
    }

    {
        state.builder.position_at_end(false_block);
        let result_union_ptr =
            state
                .builder
                .build_struct_gep(state.lox_value, result.ptr, 1, "result_union_ptr")?;
        state
            .builder
            .build_store(result_union_ptr, bool_type.const_zero())?;
        state.builder.build_unconditional_branch(merge_block)?;
    }

    state.builder.position_at_end(merge_block);

    let bool_val = unwrap_bool(&result, state)?;
    Ok(bool_val)
}

pub fn unwrap_bool<'a>(
    val: &LoxValue<'a>,
    state: &mut State<'a>,
) -> anyhow::Result<values::IntValue<'a>> {
    let union_ptr = state
        .builder
        .build_struct_gep(state.lox_value, val.ptr, 1, "union_ptr")?;
    let bool_type = state.ctx.bool_type();
    let bool_val = state
        .builder
        .build_load(bool_type, union_ptr, "bool_val")?
        .into_int_value();
    Ok(bool_val)
}
