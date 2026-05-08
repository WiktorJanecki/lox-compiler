use crate::ast;
use crate::ast::{Ast, Node, Operator};
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::{Linkage, Module};
use inkwell::types::BasicMetadataTypeEnum::PointerType;
use inkwell::types::StructType;
use inkwell::values::FunctionValue;
use inkwell::{AddressSpace, types, values};

fn gen_extern_functions(module: &Module) {
    // printf
    let context = module.get_context();
    let str = context.ptr_type(AddressSpace::default());
    let tp = context.i32_type().fn_type(&[PointerType(str)], true);
    module.add_function("printf", tp, Some(Linkage::External));
    let tp = context
        .void_type()
        .fn_type(&[context.i32_type().into()], false);
    module.add_function("exit", tp, Some(Linkage::External));
}

fn gen_panic_fn<'a>(module: &Module<'a>, builder: &Builder) -> anyhow::Result<FunctionValue<'a>> {
    let context = module.get_context();
    let str = context.ptr_type(AddressSpace::default());
    let tp = context.void_type().fn_type(&[PointerType(str)], false);
    let panic_fn = module.add_function("panic", tp, None);
    let block = context.append_basic_block(panic_fn, "entry");
    builder.position_at_end(block);
    let arg = panic_fn.get_first_param().unwrap();
    let printf = module.get_function("printf").unwrap();
    builder.build_call(printf, &[arg.into()], "_")?;
    let exit = module.get_function("exit").unwrap();
    builder.build_call(
        exit,
        &[context.i32_type().const_int(u64::MAX, false).into()],
        "_",
    )?;
    builder.build_unreachable()?;
    Ok(panic_fn)
}
fn gen_lox_object(ctx: &'_ Context) -> StructType<'_> {
    let type_i8 = lox_index_type(ctx);
    let type_biggest = ctx.i64_type(); // Should be as big as the biggest value
    ctx.struct_type(&[type_i8.into(), type_biggest.into()], true)
}

fn gen_begin_main(state: &mut State) {
    let main_fn = state
        .module
        .add_function("main", state.ctx.i32_type().fn_type(&[], false), None);
    let entry = state.ctx.append_basic_block(main_fn, "entry");
    state.builder.position_at_end(entry);
}

fn gen_return_zero(state: &mut State) -> anyhow::Result<()> {
    let zero = state.ctx.i32_type().const_int(0, false);
    state.builder.build_return(Some(&zero))?;
    Ok(())
}

fn gen_declaration(decl: &Node, ast: &Ast, state: &mut State) -> anyhow::Result<()> {
    match decl {
        Node::VarDecl(_, _) => todo!(),
        Node::ClassDecl(_, _, _) => todo!(),
        Node::FunDecl(_, _, _) => todo!(),
        Node::Stmt(stmt_id) => gen_statement(&ast.nodes[*stmt_id], ast, state),
        _ => unreachable!("In program vector only decl nodes are pushed during parsing"),
    }
}

fn gen_statement(stmt: &Node, ast: &Ast, state: &mut State) -> anyhow::Result<()> {
    match stmt {
        Node::ExprStmt(_) => todo!(),
        Node::IfStmt(_, _, _) => todo!(),
        Node::PrintStmt(expr_id) => {
            // evaluate expr
            let lox_val = gen_expr(ast.nodes.get(*expr_id).unwrap(), ast, state)?;
            // print
            gen_print_stmt(lox_val, state)
        }
        Node::ReturnStmt(_) => todo!(),
        Node::WhileStmt(_, _) => todo!(),
        Node::Block(_) => todo!(),
        _ => unreachable!("Stmt node can only have statements -> assured during parsing"),
    }
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

    let nil_literal = state.builder.build_global_string_ptr("<nil>\n", "str")?;
    state
        .builder
        .build_call(printf, &[nil_literal.as_pointer_value().into()], "printf")
        .unwrap();
    state.builder.build_unconditional_branch(merge_block)?;

    state.builder.position_at_end(bool_block);
    // TODO: ALL THOSE FORMAT LITERALS should be created only once
    let bool_literal = state.builder.build_global_string_ptr("%d\n", "str")?;
    let bool_type = state.ctx.bool_type();
    let bool_val = state
        .builder
        .build_load(bool_type, lox_val.union_ptr, "bool")?;
    state.builder.build_call(
        printf,
        &[bool_literal.as_pointer_value().into(), bool_val.into()],
        "printf",
    )?;
    state.builder.build_unconditional_branch(merge_block)?;

    state.builder.position_at_end(num_block);
    let float_literal = state.builder.build_global_string_ptr("%f\n", "str")?;
    let float_type = state.ctx.f64_type();
    let float_val = state
        .builder
        .build_load(float_type, lox_val.union_ptr, "float")?;
    state.builder.build_call(
        printf,
        &[float_literal.as_pointer_value().into(), float_val.into()],
        "printf",
    )?;
    state.builder.build_unconditional_branch(merge_block)?;

    state.builder.position_at_end(str_block);
    let str_literal = state.builder.build_global_string_ptr("%s\n", "str")?;
    let str_type = state.ctx.ptr_type(AddressSpace::default());
    let str_val = state
        .builder
        .build_load(str_type, lox_val.union_ptr, "str_val")?;
    state.builder.build_call(
        printf,
        &[str_literal.as_pointer_value().into(), str_val.into()],
        "printf",
    )?;
    state.builder.build_unconditional_branch(merge_block)?;

    state.builder.position_at_end(merge_block);
    Ok(())
}

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

    let parent_func = state
        .builder
        .get_insert_block()
        .unwrap()
        .get_parent()
        .unwrap();
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

    state.builder.position_at_end(mismatched_block); // TODO: dont generate messages every add node
    let error_msg = state.builder.build_global_string_ptr(
        "Runtime error: Mismatched types used on + operand\n",
        "errstr",
    )?;
    state
        .builder
        .build_call(state.panic_fn, &[error_msg.as_pointer_value().into()], "_")?;
    state.builder.build_unreachable()?;

    state.builder.position_at_end(unsupported_block);
    let error_msg = state.builder.build_global_string_ptr(
        "Runtime error: Only Number and String can be added using + operand\n",
        "errstr",
    )?;
    state
        .builder
        .build_call(state.panic_fn, &[error_msg.as_pointer_value().into()], "_")?;
    state.builder.build_unreachable()?;

    /// NUMBER
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

fn gen_minus<'a>(
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

    let parent_func = state
        .builder
        .get_insert_block()
        .unwrap()
        .get_parent()
        .unwrap();
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
        .build_conditional_branch(comp, cont,unsupported_block)?;
    state.builder.position_at_end(cont);

    let comp_if_int = state.builder.build_int_compare(
        inkwell::IntPredicate::EQ,
        left_tag_val,
        lox_index_type(state.ctx).const_int(LoxValueType::Number as u64, false),
        "if_numb",
    )?;
    state
        .builder
        .build_conditional_branch(comp_if_int, merge_block,unsupported_block)?;

    state.builder.position_at_end(unsupported_block);
    let error_msg = state.builder.build_global_string_ptr(
        "Runtime error: Only Number can be substracted using - operand\n",
        "errstr",
    )?;
    state
        .builder
        .build_call(state.panic_fn, &[error_msg.as_pointer_value().into()], "_")?;
    state.builder.build_unreachable()?;
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
    let result_fval = state
        .builder
        .build_float_sub(left_fval, right_fval, "minus_fval")?;
    gen_store_number(&lox_result, result_fval, state)?;
    Ok(lox_result)
}
fn gen_expr<'a>(expr: &Node, ast: &Ast, state: &mut State<'a>) -> anyhow::Result<LoxValue<'a>> {
    match expr {
        Node::Assignment(_, _, _) => todo!(),
        Node::Binary(l, op, r) => match op {
            Operator::Eq => todo!(),
            Operator::Neq => todo!(),
            Operator::Geq => todo!(),
            Operator::Leq => todo!(),
            Operator::Less => todo!(),
            Operator::Greater => todo!(),
            Operator::Plus => gen_plus(&ast.nodes[*l], &ast.nodes[*r], ast, state),
            Operator::Minus => gen_minus(&ast.nodes[*l], &ast.nodes[*r], ast, state),
            Operator::Mul => todo!(),
            Operator::Div => todo!(),
            Operator::Or => todo!(),
            Operator::And => todo!(),
            Operator::Not => unreachable!(),
        },
        Node::Unary(_, _) => todo!(),
        Node::Call => todo!(),
        Node::Identifier(_) => todo!(),
        Node::Super(_) => todo!(),
        Node::Grouping(_) => todo!(),
        Node::Number(n) => gen_number(*n, state),
        Node::String(s) => gen_string(s, state),
        Node::Bool(b) => gen_bool(*b, state),
        Node::Nil => gen_nil(state),
        Node::This => todo!(),
        _ => unreachable!(),
    }
}

struct State<'a> {
    ctx: &'a Context,
    module: Module<'a>,
    builder: Builder<'a>,

    #[allow(unused)]
    panic_fn: FunctionValue<'a>, // exit with runtime error
    lox_value: StructType<'a>, // tagged union of all lox types
}

#[derive(Copy, Clone)]
enum LoxValueType {
    Nil,
    Number,
    Bool,
    String,

    #[allow(clippy::upper_case_acronyms)]
    SIZE,
}
impl LoxValueType {
    fn llvm_int<'a>(&self, ctx: &'a inkwell::context::Context) -> inkwell::values::IntValue<'a> {
        ctx.i8_type().const_int(*self as u64, false)
    }
}

struct LoxValue<'a> {
    ptr: values::PointerValue<'a>,
    union_ptr: values::PointerValue<'a>,
    index_ptr: values::PointerValue<'a>,
}

fn gen_unpack_lox_value<'a>(
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

fn gen_alloc_lox_value<'a>(
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

    let index_val = typee as u64;
    state.builder.build_store(
        index_ptr,
        lox_index_type(state.ctx).const_int(index_val, false),
    )?;
    Ok(LoxValue {
        ptr,
        union_ptr,
        index_ptr,
    })
}

fn gen_store_number<'a>(
    var: &LoxValue<'a>,
    num: values::FloatValue<'a>,
    state: &mut State<'a>,
) -> anyhow::Result<()> {
    let index_val = LoxValueType::Number as u64;
    state.builder.build_store(
        var.index_ptr,
        lox_index_type(state.ctx).const_int(index_val, false),
    )?;
    state.builder.build_store(var.union_ptr, num)?;
    Ok(())
}

fn gen_store_string<'a>(
    var: &LoxValue<'a>,
    cstr: values::PointerValue<'a>,
    state: &mut State<'a>,
) -> anyhow::Result<()> {
    let index_val = LoxValueType::String as u64;
    state.builder.build_store(
        var.index_ptr,
        lox_index_type(state.ctx).const_int(index_val, false),
    )?;
    state.builder.build_store(var.union_ptr, cstr)?;
    Ok(())
}

fn gen_store_bool<'a>(
    var: &LoxValue<'a>,
    bol: values::IntValue<'a>,
    state: &mut State<'a>,
) -> anyhow::Result<()> {
    let index_val = LoxValueType::Bool as u64;
    state.builder.build_store(
        var.index_ptr,
        lox_index_type(state.ctx).const_int(index_val, false),
    )?;
    state.builder.build_store(var.union_ptr, bol)?;
    Ok(())
}
// it should be const but cannot cuz depends on context
fn lox_index_type(ctx: &'_ Context) -> inkwell::types::IntType<'_> {
    ctx.i8_type()
}

pub fn codegen(ast: ast::Ast, context: &'_ mut Context) -> anyhow::Result<Module<'_>> {
    let module = context.create_module("main");
    let builder = context.create_builder();

    // generate pseudo runtime
    gen_extern_functions(&module);
    let panic_fn = gen_panic_fn(&module, &builder)?;
    let lox_value = gen_lox_object(context);

    let mut state = State {
        ctx: context,
        module,
        builder,
        panic_fn,
        lox_value,
    };

    gen_begin_main(&mut state); // builder at @main.entry

    for decl_id in ast.program.clone() {
        let decl = ast.nodes.get(decl_id).unwrap();
        gen_declaration(decl, &ast, &mut state)?;
    }
    // builder should be at @main.entry
    gen_return_zero(&mut state)?;

    println!("{}", state.module.to_string());
    if let Err(err) = state.module.verify() {
        eprintln!("Błąd weryfikacji IR: {}", err.to_string());
    }
    Ok(state.module)
}
