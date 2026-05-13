use crate::codegen::{State, lox_index_type};
use inkwell::values;

/// Lox value is a tagged union. This enum must be used to map between tag integer value
#[derive(Copy, Clone)]
pub enum LoxValueType {
    Nil,
    Number,
    Bool,
    String,

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
pub struct LoxValue<'a> {
    pub ptr: values::PointerValue<'a>,
    pub union_ptr: values::PointerValue<'a>,
    pub index_ptr: values::PointerValue<'a>,
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

pub fn gen_alloc_lox_value<'a>(
    typee: LoxValueType,
    state: &mut State<'a>,
) -> anyhow::Result<LoxValue<'a>> {
    let ptr = state.builder.build_alloca(state.lox_value, "lox_val_ptr")?;
    let index_ptr = state
        .builder
        .build_struct_gep(state.lox_value, ptr, 0, "index")?;
    let union_ptr = state
        .builder
        .build_struct_gep(state.lox_value, ptr, 1, "union")?;

    state
        .builder
        .build_store(index_ptr, typee.llvm_int(state.ctx))?;
    Ok(LoxValue {
        ptr,
        union_ptr,
        index_ptr,
    })
}

pub fn gen_store_number<'a>(
    var: &LoxValue<'a>,
    num: values::FloatValue<'a>,
    state: &mut State<'a>,
) -> anyhow::Result<()> {
    state
        .builder
        .build_store(var.index_ptr, LoxValueType::Number.llvm_int(state.ctx))?;
    state.builder.build_store(var.union_ptr, num)?;
    Ok(())
}

pub fn gen_store_string<'a>(
    var: &LoxValue<'a>,
    cstr: values::PointerValue<'a>,
    state: &mut State<'a>,
) -> anyhow::Result<()> {
    state
        .builder
        .build_store(var.index_ptr, LoxValueType::String.llvm_int(state.ctx))?;
    state.builder.build_store(var.union_ptr, cstr)?;
    Ok(())
}

pub fn gen_store_bool<'a>(
    var: &LoxValue<'a>,
    bol: values::IntValue<'a>,
    state: &mut State<'a>,
) -> anyhow::Result<()> {
    state
        .builder
        .build_store(var.index_ptr, LoxValueType::Bool.llvm_int(state.ctx))?;
    state.builder.build_store(var.union_ptr, bol)?;
    Ok(())
}

pub fn gen_truthiness<'a>(lox_val: &LoxValue<'a>, state: &mut State<'a>) -> anyhow::Result<values::IntValue<'a>> {

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
    let bool_block = state.ctx.append_basic_block(parent_func, "print.bool");
    let true_block = state.ctx.append_basic_block(parent_func, "print.true");
    let false_block = state.ctx.append_basic_block(parent_func, "print.false");
    let merge_block = state.ctx.append_basic_block(parent_func, "print.merge");
    let unreach_block = state.ctx.append_basic_block(parent_func, "print.unreach");

    let cases = &[
        (LoxValueType::Nil.llvm_int(state.ctx),false_block),
        (LoxValueType::Number.llvm_int(state.ctx),true_block),
        (LoxValueType::Bool.llvm_int(state.ctx), bool_block),
        (LoxValueType::String.llvm_int(state.ctx),true_block),
    ];
    
    let result = gen_alloc_lox_value(LoxValueType::Bool, state)?;
    state.builder.build_switch(tag_val, unreach_block, cases)?;
    
    state.builder.position_at_end(unreach_block);
    state.builder.build_unreachable()?;
    
    state.builder.position_at_end(bool_block);
    let bool_type = state.ctx.bool_type();
    let bool_val = state.builder.build_load(bool_type, lox_val.union_ptr, "bool_val")?.into_int_value();
    state.builder.build_store(result.union_ptr, bool_val)?;
    state.builder.build_unconditional_branch(merge_block)?;
    
    state.builder.position_at_end(true_block);
    state.builder.build_store(result.union_ptr, bool_type.const_int(1, false))?;
    state.builder.build_unconditional_branch(merge_block)?;
    
    state.builder.position_at_end(false_block);
    state.builder.build_store(result.union_ptr, bool_type.const_zero())?;
    state.builder.build_unconditional_branch(merge_block)?;
    
    state.builder.position_at_end(merge_block);
    
    let unwrapped_result = state.builder.build_load(bool_type, result.union_ptr, "result")?;
    Ok(unwrapped_result.into_int_value())
    // TODO: function can be rewritten to not use lox value and alloca as result but good enough for now
}