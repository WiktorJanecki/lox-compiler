use crate::ast::{Ast, Node, NodeID, Operator};
use crate::codegen::lox_value::{
    gen_alloc_lox_value, gen_load_ptr, gen_store_bool, gen_store_number, gen_store_string,
    gen_unpack_lox_value, unwrap_bool,
};
use crate::codegen::{
    LoxValue, LoxValueType, State, StringLiterals, gen_block, gen_panic_call, get_var_from_env,
    lox_index_type,
};
use inkwell::AddressSpace;
use inkwell::{FloatPredicate, IntPredicate};

pub fn gen_expr<'a>(expr: &Node, ast: &Ast, state: &mut State<'a>) -> anyhow::Result<LoxValue<'a>> {
    match expr {
        Node::Assignment(maybe_obj, lhs, rhs) => {
            if let Some(obj_id) = maybe_obj {
                // property assignment: obj.field = rhs
                let obj = gen_expr(&ast.nodes[*obj_id], ast, state)?;
                let (tag, _) = gen_unpack_lox_value(&obj, state)?;
                let b_ok = gen_block("set_ok", state);
                let b_err = gen_block("set_err", state);
                let inst_tag = LoxValueType::Instance.llvm_int(state.ctx);
                let is_inst =
                    state
                        .builder
                        .build_int_compare(IntPredicate::EQ, tag, inst_tag, "is_inst")?;
                state
                    .builder
                    .build_conditional_branch(is_inst, b_ok, b_err)?;
                state.builder.position_at_end(b_err);
                gen_panic_call(StringLiterals::ReNotAnInstance, state)?;
                state.builder.position_at_end(b_ok);

                let right = gen_expr(&ast.nodes[*rhs], ast, state)?;
                let inst_ptr = gen_load_ptr(&obj, state)?;
                let field_cstr = state
                    .builder
                    .build_global_string_ptr(lhs, "sfn")?
                    .as_pointer_value();

                // heap-copy the value so lox_set_field can store a stable pointer
                let lox_val_size = state.lox_value.size_of().unwrap();
                let val_copy = state
                    .builder
                    .build_call(
                        state.module.get_function("malloc").unwrap(),
                        &[lox_val_size.into()],
                        "fval_copy",
                    )?
                    .try_as_basic_value()
                    .basic()
                    .unwrap()
                    .into_pointer_value();
                let loaded = state.builder.build_load(state.lox_value, right.ptr, "fv")?;
                state.builder.build_store(val_copy, loaded)?;

                let set_field_fn = state.module.get_function("lox_set_field").unwrap();
                state.builder.build_call(
                    set_field_fn,
                    &[inst_ptr.into(), field_cstr.into(), val_copy.into()],
                    "_",
                )?;
                return Ok(right);
            }

            // plain variable assignment
            let right = gen_expr(&ast.nodes[*rhs], ast, state)?;
            let struct_type = state.lox_value;
            let block = state.builder.get_insert_block().unwrap();
            let builder = state.ctx.create_builder();
            builder.position_at_end(block);
            let found = get_var_from_env(lhs, state)?;
            let src = builder.build_load(struct_type, right.ptr, "rhs_expr")?;
            builder.build_store(found.ptr, src)?;
            Ok(right)
        }
        Node::Binary(l, op, r) => match op {
            Operator::Eq => gen_eq(&ast.nodes[*l], &ast.nodes[*r], ast, state),
            Operator::Neq => unreachable!("sugared by parser"),
            Operator::Geq => gen_comp(&ast.nodes[*l], &ast.nodes[*r], Comparisons::Geq, ast, state),
            Operator::Leq => gen_comp(&ast.nodes[*l], &ast.nodes[*r], Comparisons::Leq, ast, state),
            Operator::Less => gen_comp(&ast.nodes[*l], &ast.nodes[*r], Comparisons::Le, ast, state),
            Operator::Greater => {
                gen_comp(&ast.nodes[*l], &ast.nodes[*r], Comparisons::Ge, ast, state)
            }
            Operator::Plus => gen_plus(&ast.nodes[*l], &ast.nodes[*r], ast, state),
            Operator::Minus => gen_number_binop(
                &ast.nodes[*l],
                &ast.nodes[*r],
                GenNumberBinopAllowed::Minus,
                ast,
                state,
            ),
            Operator::Mul => gen_number_binop(
                &ast.nodes[*l],
                &ast.nodes[*r],
                GenNumberBinopAllowed::Mul,
                ast,
                state,
            ),
            Operator::Div => gen_number_binop(
                &ast.nodes[*l],
                &ast.nodes[*r],
                GenNumberBinopAllowed::Div,
                ast,
                state,
            ),
            Operator::Or => gen_or(&ast.nodes[*l], &ast.nodes[*r], ast, state),
            Operator::And => gen_and(&ast.nodes[*l], &ast.nodes[*r], ast, state),
            Operator::Not => unreachable!(),
        },
        Node::Unary(node, op) => match op {
            Operator::Not => gen_neg(&ast.nodes[*node], ast, state),
            Operator::Minus => gen_num_neg(&ast.nodes[*node], ast, state),
            _ => unreachable!(),
        },
        Node::Call(node, args) => gen_call(&ast.nodes[*node], args, ast, state),
        Node::Get(obj_id, field_name) => {
            let obj = gen_expr(&ast.nodes[*obj_id], ast, state)?;
            let (tag, _) = gen_unpack_lox_value(&obj, state)?;
            let b_ok = gen_block("get_ok", state);
            let b_err = gen_block("get_err", state);
            let inst_tag = LoxValueType::Instance.llvm_int(state.ctx);
            let is_inst =
                state
                    .builder
                    .build_int_compare(IntPredicate::EQ, tag, inst_tag, "is_inst")?;
            state
                .builder
                .build_conditional_branch(is_inst, b_ok, b_err)?;
            state.builder.position_at_end(b_err);
            gen_panic_call(StringLiterals::ReNotAnInstance, state)?;
            state.builder.position_at_end(b_ok);

            let inst_ptr = gen_load_ptr(&obj, state)?;
            let field_cstr = state
                .builder
                .build_global_string_ptr(field_name, "gfn")?
                .as_pointer_value();
            let get_field_fn = state.module.get_function("lox_get_field").unwrap();
            let field_ptr = state
                .builder
                .build_call(
                    get_field_fn,
                    &[inst_ptr.into(), field_cstr.into()],
                    "field_ptr",
                )?
                .try_as_basic_value()
                .basic()
                .unwrap()
                .into_pointer_value();

            let b_found = gen_block("field_found", state);
            let b_not_found = gen_block("field_not_found", state);
            let found = state.builder.build_is_not_null(field_ptr, "found")?;
            state
                .builder
                .build_conditional_branch(found, b_found, b_not_found)?;
            state.builder.position_at_end(b_not_found);
            gen_panic_call(StringLiterals::ReUndefinedProperty, state)?;
            state.builder.position_at_end(b_found);

            Ok(LoxValue { ptr: field_ptr })
        }
        Node::Identifier(id) => get_var_from_env(id, state).cloned(),
        Node::Super(method_name) => {
            let this_lox = get_var_from_env("this", state)?.clone();
            let inst_ptr = gen_load_ptr(&this_lox, state)?;
            let ptr_t = state.ctx.ptr_type(AddressSpace::default());
            let cls_gep =
                state
                    .builder
                    .build_struct_gep(state.instance_type, inst_ptr, 0, "cls_gep")?;
            let class_ptr = state
                .builder
                .build_load(ptr_t, cls_gep, "class_ptr")?
                .into_pointer_value();
            let super_gep =
                state
                    .builder
                    .build_struct_gep(state.class_type, class_ptr, 1, "super_gep")?;
            let super_ptr = state
                .builder
                .build_load(ptr_t, super_gep, "super_ptr")?
                .into_pointer_value();

            let mn_cstr = state
                .builder
                .build_global_string_ptr(method_name, "smn")?
                .as_pointer_value();
            let get_method_fn = state.module.get_function("lox_get_method").unwrap();
            let method_lox_ptr = state
                .builder
                .build_call(
                    get_method_fn,
                    &[super_ptr.into(), mn_cstr.into()],
                    "super_method",
                )?
                .try_as_basic_value()
                .basic()
                .unwrap()
                .into_pointer_value();

            let b_ok = gen_block("super_ok", state);
            let b_err = gen_block("super_err", state);
            let found = state
                .builder
                .build_is_not_null(method_lox_ptr, "super_found")?;
            state.builder.build_conditional_branch(found, b_ok, b_err)?;
            state.builder.position_at_end(b_err);
            gen_panic_call(StringLiterals::ReUndefinedMethod, state)?;
            state.builder.position_at_end(b_ok);

            crate::codegen::gen_class::bind_method(method_lox_ptr, &this_lox, state)
        }
        Node::Grouping(expr_id) => gen_expr(&ast.nodes[*expr_id], ast, state),
        Node::Number(n) => gen_number(*n, state),
        Node::String(s) => gen_string(s, state),
        Node::Bool(b) => gen_bool(*b, state),
        Node::Nil => gen_nil(state),
        Node::This => get_var_from_env("this", state).cloned(),
        _ => unreachable!(),
    }
}

fn gen_string<'a>(val: &str, state: &mut State<'a>) -> anyhow::Result<LoxValue<'a>> {
    let lox = gen_alloc_lox_value(LoxValueType::String, state)?;
    let union = state
        .builder
        .build_struct_gep(state.lox_value, lox.ptr, 1, "union")?;
    let str_global_ptr = state
        .builder
        .build_global_string_ptr(val, "cstr")?
        .as_pointer_value();
    state.builder.build_store(union, str_global_ptr)?;

    Ok(lox)
}
fn gen_number<'a>(number: f64, state: &mut State<'a>) -> anyhow::Result<LoxValue<'a>> {
    let lox = gen_alloc_lox_value(LoxValueType::Number, state)?;
    let union = state
        .builder
        .build_struct_gep(state.lox_value, lox.ptr, 1, "union")?;
    state
        .builder
        .build_store(union, state.ctx.f64_type().const_float(number))?;

    Ok(lox)
}
fn gen_bool<'a>(val: bool, state: &mut State<'a>) -> anyhow::Result<LoxValue<'a>> {
    let lox = gen_alloc_lox_value(LoxValueType::Bool, state)?;
    let union = state
        .builder
        .build_struct_gep(state.lox_value, lox.ptr, 1, "union")?;
    state.builder.build_store(
        union,
        state
            .ctx
            .bool_type()
            .const_int(if val { 1 } else { 0 }, false),
    )?;

    Ok(lox)
}

fn gen_nil<'a>(state: &mut State<'a>) -> anyhow::Result<LoxValue<'a>> {
    gen_alloc_lox_value(LoxValueType::Nil, state)
}

// all number binary operations will be in one function except this because string concatenation
fn gen_plus<'a>(
    l: &Node,
    r: &Node,
    ast: &Ast,
    state: &mut State<'a>,
) -> anyhow::Result<LoxValue<'a>> {
    let left = gen_expr(l, ast, state)?;
    let right = gen_expr(r, ast, state)?;

    let (left_tag_val, left_union) = gen_unpack_lox_value(&left, state)?;
    let (right_tag_val, right_union) = gen_unpack_lox_value(&right, state)?;

    let parent_func = state.current_fn;
    let num_block = state.ctx.append_basic_block(parent_func, "print.number");
    let str_block = state.ctx.append_basic_block(parent_func, "print.string");
    let merge_block = state.ctx.append_basic_block(parent_func, "print.merge");
    let unreach_block = state.ctx.append_basic_block(parent_func, "print.unreach");
    let mismatched_block = state
        .ctx
        .append_basic_block(parent_func, "print.panic.mismatched");
    let unsupported_block = state
        .ctx
        .append_basic_block(parent_func, "print.panic.unsupported");

    // Compare types -> if mismatched panic
    let comp = state.builder.build_int_compare(
        IntPredicate::EQ,
        left_tag_val,
        right_tag_val,
        "comp_tags",
    )?;
    let cont = state
        .ctx
        .append_basic_block(parent_func, "add.cmp.types.passed");
    state
        .builder
        .build_conditional_branch(comp, cont, mismatched_block)?;
    state.builder.position_at_end(cont);

    // put nil but don't use it
    let lox_result = gen_alloc_lox_value(LoxValueType::Nil, state)?;

    // only num + num and str+str is accepted, other types = instant panic
    let cases = &[
        (LoxValueType::String.llvm_int(state.ctx), str_block),
        (LoxValueType::Number.llvm_int(state.ctx), num_block),
        (LoxValueType::Bool.llvm_int(state.ctx), unsupported_block),
        (LoxValueType::Nil.llvm_int(state.ctx), unsupported_block),
        (LoxValueType::Closure.llvm_int(state.ctx), unsupported_block),
        (
            LoxValueType::Instance.llvm_int(state.ctx),
            unsupported_block,
        ),
        (LoxValueType::Class.llvm_int(state.ctx), unsupported_block),
    ];
    assert_eq!(cases.len(), LoxValueType::SIZE as usize);
    state
        .builder
        .build_switch(left_tag_val, unreach_block, cases)?;

    state.builder.position_at_end(unreach_block);
    state.builder.build_unreachable()?;

    state.builder.position_at_end(mismatched_block);
    gen_panic_call(StringLiterals::RePlusMismatchedTypes, state)?;

    state.builder.position_at_end(unsupported_block);
    gen_panic_call(StringLiterals::RePlusUnsupportedType, state)?;

    // NUMBER
    state.builder.position_at_end(num_block);
    // assert both types are number

    let float_t = state.ctx.f64_type();
    let left_fval = state
        .builder
        .build_load(float_t, left_union, "left_fval")?
        .into_float_value();
    let right_fval = state
        .builder
        .build_load(float_t, right_union, "right_fval")?
        .into_float_value();
    let sum_fval = state
        .builder
        .build_float_add(left_fval, right_fval, "sum_fval")?;
    gen_store_number(&lox_result, sum_fval, state)?;

    state.builder.build_unconditional_branch(merge_block)?;

    // STRING
    state.builder.position_at_end(str_block);
    let ptr_t = state.ctx.ptr_type(AddressSpace::default());
    let i64_t = state.ctx.i64_type();
    let left_str = state
        .builder
        .build_load(ptr_t, left_union, "left_str")?
        .into_pointer_value();
    let right_str = state
        .builder
        .build_load(ptr_t, right_union, "right_str")?
        .into_pointer_value();
    let strlen_fn = state.module.get_function("strlen").unwrap();
    let left_len = state
        .builder
        .build_call(strlen_fn, &[left_str.into()], "left_len")?
        .try_as_basic_value()
        .basic()
        .unwrap()
        .into_int_value();
    let right_len = state
        .builder
        .build_call(strlen_fn, &[right_str.into()], "right_len")?
        .try_as_basic_value()
        .basic()
        .unwrap()
        .into_int_value();
    let total = state
        .builder
        .build_int_add(left_len, right_len, "total_len")?;
    let total_plus_one =
        state
            .builder
            .build_int_add(total, i64_t.const_int(1, false), "total_plus_one")?;
    let malloc_fn = state.module.get_function("malloc").unwrap();
    let buf = state
        .builder
        .build_call(malloc_fn, &[total_plus_one.into()], "concat_buf")?
        .try_as_basic_value()
        .basic()
        .unwrap()
        .into_pointer_value();
    let strcpy_fn = state.module.get_function("strcpy").unwrap();
    let strcat_fn = state.module.get_function("strcat").unwrap();
    state
        .builder
        .build_call(strcpy_fn, &[buf.into(), left_str.into()], "_")?;
    state
        .builder
        .build_call(strcat_fn, &[buf.into(), right_str.into()], "_")?;
    gen_store_string(&lox_result, buf, state)?;
    state.builder.build_unconditional_branch(merge_block)?;

    state.builder.position_at_end(merge_block);

    Ok(lox_result)
}
// GENERICS MAGIC
enum GenNumberBinopAllowed {
    Minus,
    Mul,
    Div,
}
fn gen_number_binop<'a>(
    l: &Node,
    r: &Node,
    operator: GenNumberBinopAllowed, // I really want this compile time branching but stable rust sucks
    ast: &Ast,
    state: &mut State<'a>,
) -> anyhow::Result<LoxValue<'a>> {
    let left = gen_expr(l, ast, state)?;
    let right = gen_expr(r, ast, state)?;

    let (left_tag_val, left_union) = gen_unpack_lox_value(&left, state)?;
    let (right_tag_val, right_union) = gen_unpack_lox_value(&right, state)?;

    let parent_func = state.current_fn;
    let merge_block = state.ctx.append_basic_block(parent_func, "print.merge");
    let unsupported_block = state
        .ctx
        .append_basic_block(parent_func, "print.panic.unsupported");

    // Compare types -> if mismatched panic
    let comp = state.builder.build_int_compare(
        IntPredicate::EQ,
        left_tag_val,
        right_tag_val,
        "comp_tags",
    )?;
    let cont = state
        .ctx
        .append_basic_block(parent_func, "minus.cmp.types.passed");
    state
        .builder
        .build_conditional_branch(comp, cont, unsupported_block)?;
    state.builder.position_at_end(cont);

    let comp_if_int = state.builder.build_int_compare(
        inkwell::IntPredicate::EQ,
        left_tag_val,
        lox_index_type(state.ctx).const_int(LoxValueType::Number as u64, false),
        "if_numb",
    )?;
    state
        .builder
        .build_conditional_branch(comp_if_int, merge_block, unsupported_block)?;

    state.builder.position_at_end(unsupported_block);
    let error = match operator {
        GenNumberBinopAllowed::Minus => StringLiterals::ReMinusUnsupportedType,
        GenNumberBinopAllowed::Mul => StringLiterals::ReMulUnsupportedType,
        GenNumberBinopAllowed::Div => StringLiterals::ReDivUnsupportedType,
    };
    gen_panic_call(error, state)?;
    state.builder.position_at_end(merge_block);

    let lox_result = gen_alloc_lox_value(LoxValueType::Number, state)?;
    let float_t = state.ctx.f64_type();
    let left_fval = state
        .builder
        .build_load(float_t, left_union, "left_fval")?
        .into_float_value();
    let right_fval = state
        .builder
        .build_load(float_t, right_union, "right_fval")?
        .into_float_value();
    let result_fval = match operator {
        GenNumberBinopAllowed::Minus => state
            .builder
            .build_float_sub(left_fval, right_fval, "min_fval")?,
        GenNumberBinopAllowed::Mul => state
            .builder
            .build_float_mul(left_fval, right_fval, "mul_fval")?,
        GenNumberBinopAllowed::Div => state
            .builder
            .build_float_div(left_fval, right_fval, "div_fval")?,
    };
    gen_store_number(&lox_result, result_fval, state)?;
    Ok(lox_result)
}

enum Comparisons {
    Ge,
    Le,
    Leq,
    Geq,
}
fn gen_comp<'a>(
    l: &Node,
    r: &Node,
    operator: Comparisons,
    ast: &Ast,
    state: &mut State<'a>,
) -> anyhow::Result<LoxValue<'a>> {
    let left = gen_expr(l, ast, state)?;
    let right = gen_expr(r, ast, state)?;

    let (left_tag, left_union) = gen_unpack_lox_value(&left, state)?;
    let (right_tag, right_union) = gen_unpack_lox_value(&right, state)?;

    let parent_func = state.current_fn;
    let b_numbers = state.ctx.append_basic_block(parent_func, "numbers");
    let b_sametypes = state.ctx.append_basic_block(parent_func, "same_types");
    let b_unsuppoerted = state.ctx.append_basic_block(parent_func, "unsupported");

    let res =
        state
            .builder
            .build_int_compare(IntPredicate::EQ, left_tag, right_tag, "comp_tags")?;
    state
        .builder
        .build_conditional_branch(res, b_sametypes, b_unsuppoerted)?;

    state.builder.position_at_end(b_sametypes);
    let comp = state.builder.build_int_compare(
        IntPredicate::EQ,
        left_tag,
        LoxValueType::Number.llvm_int(state.ctx),
        "r_numbers",
    )?;
    state
        .builder
        .build_conditional_branch(comp, b_numbers, b_unsuppoerted)?;

    state.builder.position_at_end(b_unsuppoerted);
    gen_panic_call(StringLiterals::ReComparisonUnsupportedType, state)?;

    state.builder.position_at_end(b_numbers);

    let float_type = state.ctx.f64_type();
    let left_fval = state
        .builder
        .build_load(float_type, left_union, "left_fval")?
        .into_float_value();
    let right_fval = state
        .builder
        .build_load(float_type, right_union, "right_fval")?
        .into_float_value();

    let pred = match operator {
        Comparisons::Le => FloatPredicate::OLT,
        Comparisons::Leq => FloatPredicate::OLE,
        Comparisons::Ge => FloatPredicate::OGT,
        Comparisons::Geq => FloatPredicate::OGE,
    };

    let comp = state
        .builder
        .build_float_compare(pred, left_fval, right_fval, "comp")?;
    let lox_result = gen_alloc_lox_value(LoxValueType::Bool, state)?;
    let result_union =
        state
            .builder
            .build_struct_gep(state.lox_value, lox_result.ptr, 1, "result_union")?;

    state.builder.build_store(result_union, comp)?;

    Ok(lox_result)
}

fn gen_neg<'a>(node: &Node, ast: &Ast, state: &mut State<'a>) -> anyhow::Result<LoxValue<'a>> {
    let val = gen_expr(node, ast, state)?;
    let (tag, union_ptr) = gen_unpack_lox_value(&val, state)?;

    let b_bool = gen_block("bool", state);
    let b_nbool = gen_block("not_bool", state);

    let bool_int = LoxValueType::Bool.llvm_int(state.ctx);
    let comp = state
        .builder
        .build_int_compare(IntPredicate::EQ, tag, bool_int, "type_comp")?;
    state
        .builder
        .build_conditional_branch(comp, b_bool, b_nbool)?;

    state.builder.position_at_end(b_nbool);
    gen_panic_call(StringLiterals::ReLogicUnsupportedType, state)?;

    state.builder.position_at_end(b_bool);
    let bool_type = state.ctx.bool_type();
    let bval = state
        .builder
        .build_load(bool_type, union_ptr, "bval")?
        .into_int_value();
    let neg = state.builder.build_not(bval, "neg_result")?;
    let result = gen_alloc_lox_value(LoxValueType::Bool, state)?;
    gen_store_bool(&result, neg, state)?;
    Ok(result)
}

fn gen_num_neg<'a>(node: &Node, ast: &Ast, state: &mut State<'a>) -> anyhow::Result<LoxValue<'a>> {
    let val = gen_expr(node, ast, state)?;
    let (tag, union_ptr) = gen_unpack_lox_value(&val, state)?;

    let b_num = gen_block("num", state);
    let b_nnum = gen_block("not_num", state);

    let numb_int = LoxValueType::Number.llvm_int(state.ctx);
    let comp = state
        .builder
        .build_int_compare(IntPredicate::EQ, tag, numb_int, "type_comp")?;
    state
        .builder
        .build_conditional_branch(comp, b_num, b_nnum)?;

    state.builder.position_at_end(b_nnum);
    gen_panic_call(StringLiterals::ReMinusUnsupportedType, state)?;

    state.builder.position_at_end(b_num);
    let float_type = state.ctx.f64_type();
    let fval = state
        .builder
        .build_load(float_type, union_ptr, "fval")?
        .into_float_value();
    let neg = state.builder.build_float_neg(fval, "neg_result")?;
    let result = gen_alloc_lox_value(LoxValueType::Number, state)?;
    gen_store_number(&result, neg, state)?;
    Ok(result)
}

fn gen_eq<'a>(
    l: &Node,
    r: &Node,
    ast: &Ast,
    state: &mut State<'a>,
) -> anyhow::Result<LoxValue<'a>> {
    let left = gen_expr(l, ast, state)?;
    let right = gen_expr(r, ast, state)?;

    let (left_tag, left_union) = gen_unpack_lox_value(&left, state)?;
    let (right_tag, right_union) = gen_unpack_lox_value(&right, state)?;

    let result = gen_alloc_lox_value(LoxValueType::Bool, state)?;

    let b_same_type = gen_block("same_type", state);
    let b_str = gen_block("str", state);
    let b_num = gen_block("num", state);
    let b_bool = gen_block("bool", state);
    let b_false = gen_block("false", state);
    let b_nil = gen_block("nil", state); // true
    let b_ptr_eq = gen_block("ptr_eq", state); // closure/instance/class: pointer equality
    let b_merge = gen_block("merge", state);
    let b_unreach = gen_block("unreach", state);

    let comp =
        state
            .builder
            .build_int_compare(IntPredicate::EQ, left_tag, right_tag, "type_comp")?;
    state
        .builder
        .build_conditional_branch(comp, b_same_type, b_false)?;

    state.builder.position_at_end(b_same_type);
    let cases = &[
        (LoxValueType::String.llvm_int(state.ctx), b_str),
        (LoxValueType::Number.llvm_int(state.ctx), b_num),
        (LoxValueType::Bool.llvm_int(state.ctx), b_bool),
        (LoxValueType::Nil.llvm_int(state.ctx), b_nil),
        (LoxValueType::Closure.llvm_int(state.ctx), b_ptr_eq),
        (LoxValueType::Instance.llvm_int(state.ctx), b_ptr_eq),
        (LoxValueType::Class.llvm_int(state.ctx), b_ptr_eq),
    ];
    assert_eq!(cases.len(), LoxValueType::SIZE as usize);
    state.builder.build_switch(left_tag, b_unreach, cases)?;

    state.builder.position_at_end(b_unreach);
    state.builder.build_unreachable()?;

    state.builder.position_at_end(b_num);
    let float_type = state.ctx.f64_type();
    let lhs = state
        .builder
        .build_load(float_type, left_union, "left_f64")?
        .into_float_value();
    let rhs = state
        .builder
        .build_load(float_type, right_union, "right_f64")?
        .into_float_value();
    let comp = state
        .builder
        .build_float_compare(FloatPredicate::OEQ, lhs, rhs, "f64_res")?;
    gen_store_bool(&result, comp, state)?;
    state.builder.build_unconditional_branch(b_merge)?;

    state.builder.position_at_end(b_bool);
    let bool_type = state.ctx.bool_type();
    let lhs = state
        .builder
        .build_load(bool_type, left_union, "l")?
        .into_int_value();
    let rhs = state
        .builder
        .build_load(bool_type, right_union, "r")?
        .into_int_value();
    let comp = state
        .builder
        .build_int_compare(IntPredicate::EQ, lhs, rhs, "comp")?;
    gen_store_bool(&result, comp, state)?;
    state.builder.build_unconditional_branch(b_merge)?;

    state.builder.position_at_end(b_false);
    gen_store_bool(&result, bool_type.const_zero(), state)?;
    state.builder.build_unconditional_branch(b_merge)?;

    state.builder.position_at_end(b_nil);
    gen_store_bool(&result, bool_type.const_int(1, false), state)?;
    state.builder.build_unconditional_branch(b_merge)?;

    state.builder.position_at_end(b_str);
    {
        let ptr_t = state.ctx.ptr_type(AddressSpace::default());
        let left_str = state
            .builder
            .build_load(ptr_t, left_union, "left_str")?
            .into_pointer_value();
        let right_str = state
            .builder
            .build_load(ptr_t, right_union, "right_str")?
            .into_pointer_value();
        let strcmp_fn = state.module.get_function("strcmp").unwrap();
        let cmp = state
            .builder
            .build_call(
                strcmp_fn,
                &[left_str.into(), right_str.into()],
                "strcmp_res",
            )?
            .try_as_basic_value()
            .basic()
            .unwrap()
            .into_int_value();
        let str_eq = state.builder.build_int_compare(
            IntPredicate::EQ,
            cmp,
            state.ctx.i32_type().const_zero(),
            "str_eq",
        )?;
        gen_store_bool(&result, str_eq, state)?;
        state.builder.build_unconditional_branch(b_merge)?;
    }

    // closures, instances, classes: pointer equality
    state.builder.position_at_end(b_ptr_eq);
    {
        let i64_t = state.ctx.i64_type();
        let lhs = state
            .builder
            .build_load(i64_t, left_union, "l_ptr")?
            .into_int_value();
        let rhs = state
            .builder
            .build_load(i64_t, right_union, "r_ptr")?
            .into_int_value();
        let eq = state
            .builder
            .build_int_compare(IntPredicate::EQ, lhs, rhs, "ptr_eq")?;
        gen_store_bool(&result, eq, state)?;
        state.builder.build_unconditional_branch(b_merge)?;
    }

    state.builder.position_at_end(b_merge);
    Ok(result)
}

fn gen_or<'a>(
    l: &Node,
    r: &Node,
    ast: &Ast,
    state: &mut State<'a>,
) -> anyhow::Result<LoxValue<'a>> {
    let left = gen_expr(l, ast, state)?;
    let (left_tag, _) = gen_unpack_lox_value(&left, state)?;
    let result = gen_alloc_lox_value(LoxValueType::Bool, state)?;

    let b_panic = gen_block("panic", state);
    let b_merge = gen_block("merge", state);
    let bool_tag = LoxValueType::Bool.llvm_int(state.ctx);
    let comp = state
        .builder
        .build_int_compare(IntPredicate::EQ, left_tag, bool_tag, "tag_comp")?;
    let b_cont = gen_block("type_cont", state);
    state
        .builder
        .build_conditional_branch(comp, b_cont, b_panic)?;

    state.builder.position_at_end(b_panic);
    gen_panic_call(StringLiterals::ReLogicUnsupportedType, state)?;

    state.builder.position_at_end(b_cont);
    let b_ret_true = gen_block("ret_true", state);
    let b_cont = gen_block("first_val_cont", state);
    let bool_val = unwrap_bool(&left, state)?;
    state
        .builder
        .build_conditional_branch(bool_val, b_ret_true, b_cont)?;

    state.builder.position_at_end(b_ret_true);
    let bool_type = state.ctx.bool_type();
    gen_store_bool(&result, bool_type.const_int(1, false), state)?;
    state.builder.build_unconditional_branch(b_merge)?;

    // eval right
    state.builder.position_at_end(b_cont);
    let right = gen_expr(r, ast, state)?;
    let (right_tag, _) = gen_unpack_lox_value(&right, state)?;

    let b_right_tag_bool = gen_block("right_tag_bool", state);
    let comp =
        state
            .builder
            .build_int_compare(IntPredicate::EQ, left_tag, right_tag, "tag_bool")?;
    state
        .builder
        .build_conditional_branch(comp, b_right_tag_bool, b_panic)?;

    state.builder.position_at_end(b_right_tag_bool);
    let to_write = unwrap_bool(&right, state)?;
    gen_store_bool(&result, to_write, state)?;
    state.builder.build_unconditional_branch(b_merge)?;

    state.builder.position_at_end(b_merge);
    Ok(result)
}

fn gen_and<'a>(
    l: &Node,
    r: &Node,
    ast: &Ast,
    state: &mut State<'a>,
) -> anyhow::Result<LoxValue<'a>> {
    let left = gen_expr(l, ast, state)?;
    let (left_tag, _) = gen_unpack_lox_value(&left, state)?;
    let result = gen_alloc_lox_value(LoxValueType::Bool, state)?;

    let b_panic = gen_block("panic", state);
    let b_merge = gen_block("merge", state);
    let bool_tag = LoxValueType::Bool.llvm_int(state.ctx);
    let comp = state
        .builder
        .build_int_compare(IntPredicate::EQ, left_tag, bool_tag, "tag_comp")?;
    let b_cont = gen_block("type_cont", state);
    state
        .builder
        .build_conditional_branch(comp, b_cont, b_panic)?;

    state.builder.position_at_end(b_panic);
    gen_panic_call(StringLiterals::ReLogicUnsupportedType, state)?;

    state.builder.position_at_end(b_cont);
    let b_ret_false = gen_block("ret_false", state);
    let b_cont = gen_block("first_val_cont", state);
    let bool_val = unwrap_bool(&left, state)?;
    state
        .builder
        .build_conditional_branch(bool_val, b_cont, b_ret_false)?;

    state.builder.position_at_end(b_ret_false);
    let bool_type = state.ctx.bool_type();
    gen_store_bool(&result, bool_type.const_zero(), state)?;
    state.builder.build_unconditional_branch(b_merge)?;

    // eval right
    state.builder.position_at_end(b_cont);
    let right = gen_expr(r, ast, state)?;
    let (right_tag, _) = gen_unpack_lox_value(&right, state)?;

    let b_right_tag_bool = gen_block("right_tag_bool", state);
    let comp =
        state
            .builder
            .build_int_compare(IntPredicate::EQ, left_tag, right_tag, "tag_bool")?;
    state
        .builder
        .build_conditional_branch(comp, b_right_tag_bool, b_panic)?;

    state.builder.position_at_end(b_right_tag_bool);
    let to_write = unwrap_bool(&right, state)?;
    gen_store_bool(&result, to_write, state)?;
    state.builder.build_unconditional_branch(b_merge)?;

    state.builder.position_at_end(b_merge);
    Ok(result)
}

fn gen_call<'a>(
    node: &Node,
    args: &[NodeID],
    ast: &Ast,
    state: &mut State<'a>,
) -> anyhow::Result<LoxValue<'a>> {
    // Handle method calls: obj.method(args) → Get(obj, method)
    if let Node::Get(obj_id, method_name) = node {
        let obj = gen_expr(&ast.nodes[*obj_id], ast, state)?;
        let (tag, _) = gen_unpack_lox_value(&obj, state)?;
        let b_ok = gen_block("method_ok", state);
        let b_err = gen_block("method_err", state);
        let inst_tag = LoxValueType::Instance.llvm_int(state.ctx);
        let is_inst =
            state
                .builder
                .build_int_compare(IntPredicate::EQ, tag, inst_tag, "is_inst")?;
        state
            .builder
            .build_conditional_branch(is_inst, b_ok, b_err)?;
        state.builder.position_at_end(b_err);
        gen_panic_call(StringLiterals::ReNotAnInstance, state)?;
        state.builder.position_at_end(b_ok);

        let inst_ptr = crate::codegen::lox_value::gen_load_ptr(&obj, state)?;
        let cls_gep =
            state
                .builder
                .build_struct_gep(state.instance_type, inst_ptr, 0, "cls_gep")?;
        let ptr_t = state.ctx.ptr_type(AddressSpace::default());
        let class_ptr = state
            .builder
            .build_load(ptr_t, cls_gep, "class_ptr")?
            .into_pointer_value();

        let method_name_cstr = state
            .builder
            .build_global_string_ptr(method_name, "mn")?
            .as_pointer_value();
        let get_method_fn = state.module.get_function("lox_get_method").unwrap();
        let method_lox_ptr = state
            .builder
            .build_call(
                get_method_fn,
                &[class_ptr.into(), method_name_cstr.into()],
                "method_lox",
            )?
            .try_as_basic_value()
            .basic()
            .unwrap()
            .into_pointer_value();

        let b_has_method = gen_block("has_method", state);
        let b_no_method = gen_block("no_method", state);
        let has_method = state
            .builder
            .build_is_not_null(method_lox_ptr, "has_method")?;
        state
            .builder
            .build_conditional_branch(has_method, b_has_method, b_no_method)?;
        state.builder.position_at_end(b_no_method);
        gen_panic_call(StringLiterals::ReUndefinedMethod, state)?;
        state.builder.position_at_end(b_has_method);

        let bound = crate::codegen::gen_class::bind_method(method_lox_ptr, &obj, state)?;
        return call_closure_value(&bound, args, ast, state);
    }

    // General case: evaluate callee — must be Closure or Class
    let callee = gen_expr(node, ast, state)?;
    let (tag, _) = gen_unpack_lox_value(&callee, state)?;

    let b_closure = gen_block("call.closure", state);
    let b_class = gen_block("call.class", state);
    let b_err = gen_block("call.err", state);
    let b_merge = gen_block("call.merge", state);
    let result_ptr = super::build_entry_block_alloca(state.lox_value, "call.result", state)?;

    let closure_tag = LoxValueType::Closure.llvm_int(state.ctx);
    let class_tag = LoxValueType::Class.llvm_int(state.ctx);
    let is_closure =
        state
            .builder
            .build_int_compare(IntPredicate::EQ, tag, closure_tag, "is_closure")?;
    let is_class = state
        .builder
        .build_int_compare(IntPredicate::EQ, tag, class_tag, "is_class")?;

    // switch: Closure → b_closure, Class → b_class, else → b_err
    let b_type_check = gen_block("call.type_check", state);
    state
        .builder
        .build_conditional_branch(is_closure, b_closure, b_type_check)?;
    state.builder.position_at_end(b_type_check);
    state
        .builder
        .build_conditional_branch(is_class, b_class, b_err)?;

    state.builder.position_at_end(b_err);
    gen_panic_call(StringLiterals::ReCallNotCallable, state)?;

    // Closure branch
    state.builder.position_at_end(b_closure);
    let ret = call_closure_value(&callee, args, ast, state)?;
    let ret_val = state
        .builder
        .build_load(state.lox_value, ret.ptr, "ret_val")?;
    state.builder.build_store(result_ptr, ret_val)?;
    state.builder.build_unconditional_branch(b_merge)?;

    // Class branch: create instance
    state.builder.position_at_end(b_class);
    let inst = crate::codegen::gen_class::gen_instance_creation(&callee, args, ast, state)?;
    let inst_val = state
        .builder
        .build_load(state.lox_value, inst.ptr, "inst_val")?;
    state.builder.build_store(result_ptr, inst_val)?;
    state.builder.build_unconditional_branch(b_merge)?;

    state.builder.position_at_end(b_merge);
    Ok(LoxValue { ptr: result_ptr })
}

/// Call a closure LoxValue with the given argument node IDs.
/// The callee MUST be a Closure (not a Class) — use gen_call for type-checking dispatch.
pub fn call_closure_value<'a>(
    callee: &LoxValue<'a>,
    args: &[NodeID],
    ast: &Ast,
    state: &mut State<'a>,
) -> anyhow::Result<LoxValue<'a>> {
    use inkwell::types::BasicMetadataTypeEnum;

    let ptr_t = state.ctx.ptr_type(AddressSpace::default());
    let i64_t = state.ctx.i64_type();

    let (_, union_ptr) = gen_unpack_lox_value(callee, state)?;
    let closure_int = state
        .builder
        .build_load(i64_t, union_ptr, "closure_int")?
        .into_int_value();
    let closure_obj_ptr = state
        .builder
        .build_int_to_ptr(closure_int, ptr_t, "closure_ptr")?;

    let fn_ptr_field =
        state
            .builder
            .build_struct_gep(state.closure_type, closure_obj_ptr, 0, "fn_ptr_f")?;
    let fn_ptr = state
        .builder
        .build_load(ptr_t, fn_ptr_field, "fn_ptr")?
        .into_pointer_value();
    let env_arr_gep =
        state
            .builder
            .build_struct_gep(state.closure_type, closure_obj_ptr, 1, "env_arr_gep")?;
    let env_arr = state
        .builder
        .build_load(ptr_t, env_arr_gep, "env_arr")?
        .into_pointer_value();

    let mut call_args: Vec<inkwell::values::BasicMetadataValueEnum> = vec![env_arr.into()];
    for node_id in args {
        let arg = gen_expr(&ast.nodes[*node_id], ast, state)?;
        let loaded = state
            .builder
            .build_load(state.lox_value, arg.ptr, "arg")?
            .into();
        call_args.push(loaded);
    }

    let mut param_types: Vec<BasicMetadataTypeEnum> = vec![ptr_t.into()];
    for _ in args {
        param_types.push(state.lox_value.into());
    }
    let fn_type = state.lox_value.fn_type(&param_types, false);

    let returned = state
        .builder
        .build_indirect_call(fn_type, fn_ptr, &call_args, "call_ret")?;
    let result_ptr = super::build_entry_block_alloca(state.lox_value, "call_result", state)?;
    let ret_val = returned.try_as_basic_value().basic().unwrap();
    state.builder.build_store(result_ptr, ret_val)?;
    Ok(LoxValue { ptr: result_ptr })
}
