#![cfg(not(feature = "no_optimize"))]

use rhai::{Engine, EvalAltResult, Module, OptimizationLevel, Scope, INT};

#[test]
fn test_optimizer() -> Result<(), Box<EvalAltResult>> {
    let mut engine = Engine::new();
    engine.set_optimization_level(OptimizationLevel::Full);

    #[cfg(not(feature = "no_function"))]
    assert_eq!(
        engine.eval::<INT>(
            "
                fn foo(x) { print(x); return; }
                fn foo2(x) { if x > 0 {} return; }
                42
            "
        )?,
        42
    );

    Ok(())
}

#[test]
fn test_optimizer_run() -> Result<(), Box<EvalAltResult>> {
    fn run_test(engine: &mut Engine) -> Result<(), Box<EvalAltResult>> {
        assert_eq!(engine.eval::<INT>("if true { 42 } else { 123 }")?, 42);
        assert_eq!(
            engine.eval::<INT>("if 1 == 1 || 2 > 3 { 42 } else { 123 }")?,
            42
        );
        assert_eq!(
            engine.eval::<INT>(r#"const abc = "hello"; if abc < "foo" { 42 } else { 123 }"#)?,
            123
        );
        Ok(())
    }

    let mut engine = Engine::new();

    engine.set_optimization_level(OptimizationLevel::None);
    run_test(&mut engine)?;

    engine.set_optimization_level(OptimizationLevel::Simple);
    run_test(&mut engine)?;

    engine.set_optimization_level(OptimizationLevel::Full);
    run_test(&mut engine)?;

    // Override == operator
    engine.register_fn("==", |_x: INT, _y: INT| false);

    engine.set_optimization_level(OptimizationLevel::Simple);

    assert_eq!(
        engine.eval::<INT>("if 1 == 1 || 2 > 3 { 42 } else { 123 }")?,
        42
    );

    engine.set_fast_operators(false);

    assert_eq!(
        engine.eval::<INT>("if 1 == 1 || 2 > 3 { 42 } else { 123 }")?,
        123
    );

    engine.set_optimization_level(OptimizationLevel::Full);

    assert_eq!(
        engine.eval::<INT>("if 1 == 1 || 2 > 3 { 42 } else { 123 }")?,
        123
    );

    Ok(())
}

#[cfg(feature = "metadata")]
#[cfg(not(feature = "no_module"))]
#[cfg(not(feature = "no_function"))]
#[cfg(not(feature = "no_position"))]
#[test]
fn test_optimizer_parse() -> Result<(), Box<EvalAltResult>> {
    let mut engine = Engine::new();

    engine.set_optimization_level(OptimizationLevel::Simple);

    let ast = engine.compile("{ const DECISION = false; if DECISION { 42 } else { 123 } }")?;

    assert_eq!(
        format!("{ast:?}"),
        r#"AST { source: None, doc: None, resolver: None, body: [Expr(123 @ 1:53)] }"#
    );

    let ast = engine.compile("const DECISION = false; if DECISION { 42 } else { 123 }")?;

    assert_eq!(
        format!("{ast:?}"),
        r#"AST { source: None, doc: None, resolver: None, body: [Var(("DECISION" @ 1:7, false @ 1:18, None), CONSTANT, 1:1), Expr(123 @ 1:51)] }"#
    );

    let ast = engine.compile("if 1 == 2 { 42 }")?;

    assert_eq!(
        format!("{ast:?}"),
        r#"AST { source: None, doc: None, resolver: None, body: [] }"#
    );

    engine.set_optimization_level(OptimizationLevel::Full);

    let ast = engine.compile("abs(-42)")?;

    assert_eq!(
        format!("{ast:?}"),
        r#"AST { source: None, doc: None, resolver: None, body: [Expr(42 @ 1:1)] }"#
    );

    let ast = engine.compile("NUMBER")?;

    assert_eq!(
        format!("{ast:?}"),
        r#"AST { source: None, doc: None, resolver: None, body: [Expr(Variable(NUMBER) @ 1:1)] }"#
    );

    let mut module = Module::new();
    module.set_var("NUMBER", 42 as INT);

    engine.register_global_module(module.into());

    let ast = engine.compile("NUMBER")?;

    assert_eq!(
        format!("{ast:?}"),
        r#"AST { source: None, doc: None, resolver: None, body: [Expr(42 @ 1:1)] }"#
    );

    Ok(())
}

#[cfg(not(feature = "no_function"))]
#[test]
fn test_optimizer_scope() -> Result<(), Box<EvalAltResult>> {
    const SCRIPT: &str = "
        fn foo() { FOO }
        foo()
    ";

    let engine = Engine::new();
    let mut scope = Scope::new();

    scope.push_constant("FOO", 42 as INT);

    let ast = engine.compile_with_scope(&scope, SCRIPT)?;

    scope.push("FOO", 123 as INT);

    assert_eq!(engine.eval_ast::<INT>(&ast)?, 42);
    assert_eq!(engine.eval_ast_with_scope::<INT>(&mut scope, &ast)?, 42);

    let ast = engine.compile_with_scope(&scope, SCRIPT)?;

    assert!(engine.eval_ast_with_scope::<INT>(&mut scope, &ast).is_err());

    Ok(())
}
