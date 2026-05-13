use crate::ast::{Ast, Node, Operator};
use crate::codegen::lox_value::{gen_alloc_lox_value, gen_store_number};
use crate::codegen::{
    LoxValue, LoxValueType, State, StringLiterals, gen_panic_call, get_current_env, lox_index_type,
};
use inkwell::{FloatPredicate, IntPredicate};

fn gen_string<'a>(val: &str, state: &mut State<'a>) -> anyhow::Result<LoxValue<'a>> {
    let lox = gen_alloc_lox_value(LoxValueType::String, state)?;
    let str_global_ptr = state
        .builder
        .build_global_string_ptr(val, "cstr")?
        .as_pointer_value();
    state.builder.build_store(lox.union_ptr, str_global_ptr)?;

    Ok(lox)
}
fn gen_number<'a>(number: f64, state: &mut State<'a>) -> anyhow::Result<LoxValue<'a>> {
    let lox = gen_alloc_lox_value(LoxValueType::Number, state)?;
    state
        .builder
        .build_store(lox.union_ptr, state.ctx.f64_type().const_float(number))?;

    Ok(lox)
}
fn gen_bool<'a>(val: bool, state: &mut State<'a>) -> anyhow::Result<LoxValue<'a>> {
    let lox = gen_alloc_lox_value(LoxValueType::Bool, state)?;
    state.builder.build_store(
        lox.union_ptr,
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

    let left_tag_val = state
        .builder
        .build_load(lox_index_type(state.ctx), left.index_ptr, "left_tag")?
        .into_int_value();
    let right_tag_val = state
        .builder
        .build_load(lox_index_type(state.ctx), right.index_ptr, "right_tag")?
        .into_int_value();

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
        inkwell::IntPredicate::EQ,
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
        .build_load(float_t, left.union_ptr, "left_fval")?
        .into_float_value();
    let right_fval = state
        .builder
        .build_load(float_t, right.union_ptr, "right_fval")?
        .into_float_value();
    let sum_fval = state
        .builder
        .build_float_add(left_fval, right_fval, "sum_fval")?;
    gen_store_number(&lox_result, sum_fval, state)?;

    state.builder.build_unconditional_branch(merge_block)?;

    // STRING
    state.builder.position_at_end(str_block);
    // assert both types are str

    // check if other is str else panic
    // alloca new lox value, set tag to str, set val to concatenated cstring XD

    // TODO: finish
    // state.builder.build_unconditional_branch(merge_block)?;
    // TODO: uncomment and delete below
    state.builder.build_unreachable()?;

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

    let left_tag_val = state
        .builder
        .build_load(lox_index_type(state.ctx), left.index_ptr, "left_tag")?
        .into_int_value();
    let right_tag_val = state
        .builder
        .build_load(lox_index_type(state.ctx), right.index_ptr, "right_tag")?
        .into_int_value();

    let parent_func = state.current_fn;
    let merge_block = state.ctx.append_basic_block(parent_func, "print.merge");
    let unsupported_block = state
        .ctx
        .append_basic_block(parent_func, "print.panic.unsupported");

    // Compare types -> if mismatched panic
    let comp = state.builder.build_int_compare(
        inkwell::IntPredicate::EQ,
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
        .build_load(float_t, left.union_ptr, "left_fval")?
        .into_float_value();
    let right_fval = state
        .builder
        .build_load(float_t, right.union_ptr, "right_fval")?
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

    let left_tag = state
        .builder
        .build_load(lox_index_type(state.ctx), left.index_ptr, "left_tag")?
        .into_int_value();
    let right_tag = state
        .builder
        .build_load(lox_index_type(state.ctx), right.index_ptr, "right_tag")?
        .into_int_value();

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
        .build_load(float_type, left.union_ptr, "left_fval")?
        .into_float_value();
    let right_fval = state
        .builder
        .build_load(float_type, right.union_ptr, "right_fval")?
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

    state.builder.build_store(lox_result.union_ptr, comp)?;

    Ok(lox_result)
}
pub fn gen_expr<'a>(expr: &Node, ast: &Ast, state: &mut State<'a>) -> anyhow::Result<LoxValue<'a>> {
    match expr {
        Node::Assignment(_, _, _) => todo!(),
        Node::Binary(l, op, r) => match op {
            Operator::Eq => todo!(),
            Operator::Neq => todo!(),
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
            Operator::Or => todo!(),
            Operator::And => todo!(),
            Operator::Not => unreachable!(),
        },
        Node::Unary(_, _) => todo!(),
        Node::Call => todo!(),
        Node::Identifier(id) => {
            let env = get_current_env(state);
            if let Some(var) = env.get(id) {
                return Ok(var.clone());
            }
            anyhow::bail!(format!("Usage of undeclared variable: {}", id))
        }
        Node::Super(_) => todo!(),
        Node::Grouping(expr_id) => gen_expr(&ast.nodes[*expr_id], ast, state),
        Node::Number(n) => gen_number(*n, state),
        Node::String(s) => gen_string(s, state),
        Node::Bool(b) => gen_bool(*b, state),
        Node::Nil => gen_nil(state),
        Node::This => todo!(),
        _ => unreachable!(),
    }
}
