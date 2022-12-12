//! Module containing unit tests.
#![cfg(test)]

/// This test is to make sure no code changes increase the sizes of critical data structures.
#[test]
fn check_struct_sizes() {
    use crate::*;
    use std::mem::size_of;

    const IS_32_BIT: bool = cfg!(target_pointer_width = "32");
    const PACKED: bool = cfg!(all(
        target_pointer_width = "32",
        feature = "only_i32",
        any(feature = "no_float", feature = "f32_float")
    ));

    assert_eq!(size_of::<Dynamic>(), if PACKED { 8 } else { 16 });
    assert_eq!(size_of::<Option<Dynamic>>(), if PACKED { 8 } else { 16 });
    assert_eq!(
        size_of::<Position>(),
        if cfg!(feature = "no_position") { 0 } else { 4 }
    );
    assert_eq!(
        size_of::<tokenizer::Token>(),
        if IS_32_BIT { 8 } else { 16 }
    );
    assert_eq!(size_of::<ast::Expr>(), if PACKED { 12 } else { 16 });
    assert_eq!(size_of::<Option<ast::Expr>>(), if PACKED { 12 } else { 16 });
    assert_eq!(size_of::<ast::Stmt>(), if IS_32_BIT { 12 } else { 16 });
    assert_eq!(
        size_of::<Option<ast::Stmt>>(),
        if IS_32_BIT { 12 } else { 16 }
    );

    #[cfg(feature = "internals")]
    {
        assert_eq!(
            size_of::<CallableFunction>(),
            if IS_32_BIT { 12 } else { 24 }
        );
        assert_eq!(
            size_of::<module::FuncInfo>(),
            if IS_32_BIT { 16 } else { 32 }
        );
    }

    #[cfg(target_pointer_width = "64")]
    {
        assert_eq!(size_of::<Scope>(), 536);
        assert_eq!(
            size_of::<FnPtr>(),
            if cfg!(feature = "no_function") {
                64
            } else {
                72
            }
        );
        assert_eq!(size_of::<LexError>(), 56);
        assert_eq!(
            size_of::<ParseError>(),
            if cfg!(feature = "no_position") { 8 } else { 16 }
        );
        assert_eq!(size_of::<EvalAltResult>(), 64);
        assert_eq!(
            size_of::<NativeCallContext>(),
            if cfg!(feature = "no_position") {
                48
            } else {
                56
            }
        );
    }
}
