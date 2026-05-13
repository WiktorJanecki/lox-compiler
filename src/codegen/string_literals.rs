use crate::codegen::State;
use inkwell::builder::Builder;
use inkwell::values;
use std::mem::transmute;

#[repr(usize)]
pub enum StringLiterals {
    PrintfNumber,
    PrintfString,
    PrintfNil,
    PrintfTrue,
    PrintfFalse,

    RePlusMismatchedTypes,
    RePlusUnsupportedType,
    ReMinusUnsupportedType,
    ReMulUnsupportedType,
    ReDivUnsupportedType,
    ReComparisonUnsupportedType,
    ReUndeclaredVariable,

    #[allow(clippy::upper_case_acronyms)]
    SIZE,
}

fn literal_to_message(variant: StringLiterals) -> &'static str {
    match variant {
        StringLiterals::PrintfNumber => "%f\n",
        StringLiterals::PrintfString => "%s\n",
        StringLiterals::PrintfNil => "nil\n",
        StringLiterals::PrintfTrue => "true\n",
        StringLiterals::PrintfFalse => "false\n",
        StringLiterals::RePlusMismatchedTypes => {
            "Runtime error: Mismatched types used on + operand\n"
        }
        StringLiterals::RePlusUnsupportedType => {
            "Runtime error: Only Number and string can be used with + operand\n"
        }
        StringLiterals::ReMinusUnsupportedType => {
            "Runtime error: Only Number can be used with - operand\n"
        }
        StringLiterals::ReMulUnsupportedType => {
            "Runtime error: Only Number can be used with * operand\n"
        }
        StringLiterals::ReDivUnsupportedType => {
            "Runtime error: Only Number can be used with / operand\n"
        }
        StringLiterals::ReComparisonUnsupportedType => {
            "Runtime error: Only Numbers can be compared\n"
        }
        StringLiterals::ReUndeclaredVariable => "Runtime error: Usage of undeclared variable\n",
        StringLiterals::SIZE => unreachable!(),
    }
}

pub fn gen_global_string_literals<'a>(
    b: &Builder<'a>,
) -> anyhow::Result<[values::PointerValue<'a>; StringLiterals::SIZE as usize]> {
    let mut arr = [None; StringLiterals::SIZE as usize];

    for i in 0..StringLiterals::SIZE as usize {
        let mes = literal_to_message(unsafe { transmute(i) });
        let ptr = b
            .build_global_string_ptr(mes, "compiler_printf_literal")?
            .as_pointer_value();
        arr[i] = Some(ptr);
    }

    Ok(arr.map(|e| e.unwrap()))
}

pub fn global_string_literal<'a>(
    which: StringLiterals,
    state: &State<'a>,
) -> values::PointerValue<'a> {
    state.string_literals[which as usize]
}
