use inkwell::AddressSpace;
use inkwell::context::Context;
use inkwell::module::Linkage;
use inkwell::types::AnyTypeEnum::ArrayType;
use inkwell::types::BasicMetadataTypeEnum::PointerType;
use inkwell::values::AnyValueEnum::IntValue;
use inkwell::values::BasicMetadataValueEnum::PointerValue;
use crate::ast;

// fn add_extern_functions(modul){
//
// }

pub fn codegen(ast: ast::Ast, context: &mut Context) -> anyhow::Result<inkwell::module::Module<'_>> {
    let module = context.create_module("main");
    let builder = context.create_builder();
    let str = context.ptr_type(AddressSpace::default());
    let tp = context.i32_type().fn_type(&[PointerType(str)], true);
    let printf =  module.add_function("printf", tp, Some(Linkage::External) );


    let main_fn = module.add_function("main", context.i32_type().fn_type(&[], false), None);
    let entry = context.append_basic_block(main_fn, "entry");
    builder.position_at_end(entry);
    let global_str = builder.build_global_string_ptr("hello\n", "name")?;
    builder.build_call(printf, &[PointerValue(global_str.as_pointer_value())], "printf")?;

    let zero = context.i32_type().const_int(0, false);
    builder.build_return(Some(&zero))?;

    Ok(module)
}
