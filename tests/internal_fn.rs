#![cfg(not(feature = "no_function"))]
use rhai::{Engine, EvalAltResult, ParseErrorType, INT};

#[test]
fn test_internal_fn() {
    let engine = Engine::new();

    assert_eq!(engine.eval::<INT>("fn add_me(a, b) { a+b } add_me(3, 4)").unwrap(), 7);
    assert_eq!(engine.eval::<INT>("fn add_me(a, b,) { a+b } add_me(3, 4,)").unwrap(), 7);
    assert_eq!(engine.eval::<INT>("fn bob() { return 4; 5 } bob()").unwrap(), 4);
    assert_eq!(engine.eval::<INT>("fn add(x, n) { x + n } add(40, 2)").unwrap(), 42);
    assert_eq!(engine.eval::<INT>("fn add(x, n,) { x + n } add(40, 2,)").unwrap(), 42);
    assert_eq!(engine.eval::<INT>("fn add(x, n) { x + n } let a = 40; add(a, 2); a").unwrap(), 40);
    #[cfg(not(feature = "no_object"))]
    assert_eq!(engine.eval::<INT>("fn add(n) { this + n } let x = 40; x.add(2)").unwrap(), 42);
    #[cfg(not(feature = "no_object"))]
    assert_eq!(engine.eval::<INT>("fn add(n) { this += n; } let x = 40; x.add(2); x").unwrap(), 42);
    assert_eq!(engine.eval::<INT>("fn mul2(x) { x * 2 } mul2(21)").unwrap(), 42);
    assert_eq!(engine.eval::<INT>("fn mul2(x) { x *= 2 } let a = 21; mul2(a); a").unwrap(), 21);
    #[cfg(not(feature = "no_object"))]
    assert_eq!(engine.eval::<INT>("fn mul2() { this * 2 } let x = 21; x.mul2()").unwrap(), 42);
    #[cfg(not(feature = "no_object"))]
    assert_eq!(engine.eval::<INT>("fn mul2() { this *= 2; } let x = 21; x.mul2(); x").unwrap(), 42);
}

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Default)]
struct TestStruct(INT);

impl Clone for TestStruct {
    fn clone(&self) -> Self {
        Self(self.0 + 1)
    }
}

#[test]
fn test_internal_fn_take() {
    let mut engine = Engine::new();

    engine.register_type_with_name::<TestStruct>("TestStruct").register_fn("new_ts", |x: INT| TestStruct(x));

    assert_eq!(
        engine
            .eval::<TestStruct>(
                "
                    let x = new_ts(0);
                    for n in 0..41 { x = x }
                    x
                ",
            )
            .unwrap(),
        TestStruct(42)
    );

    assert_eq!(
        engine
            .eval::<TestStruct>(
                "
                    let x = new_ts(0);
                    for n in 0..41 { x = take(x) }
                    take(x)
                ",
            )
            .unwrap(),
        TestStruct(0)
    );
}

#[test]
fn test_internal_fn_big() {
    let engine = Engine::new();

    assert_eq!(
        engine
            .eval::<INT>(
                "
                    fn math_me(a, b, c, d, e, f) {
                        a - b * c + d * e - f
                    }
                    math_me(100, 5, 2, 9, 6, 32)
                ",
            )
            .unwrap(),
        112
    );
}

#[test]
fn test_internal_fn_overloading() {
    let engine = Engine::new();

    assert_eq!(
        engine
            .eval::<INT>(
                "
                    fn abc(x,y,z) { 2*x + 3*y + 4*z + 888 }
                    fn abc(x,y) { x + 2*y + 88 }
                    fn abc() { 42 }
                    fn abc(x) { x - 42 }

                    abc() + abc(1) + abc(1,2) + abc(1,2,3)
                "
            )
            .unwrap(),
        1002
    );

    assert_eq!(
        *engine
            .compile(
                "
                    fn abc(x) { x + 42 }
                    fn abc(x) { x - 42 }
                "
            )
            .unwrap_err()
            .err_type(),
        ParseErrorType::FnDuplicatedDefinition("abc".to_string(), 1)
    );
}

#[test]
fn test_internal_fn_params() {
    let engine = Engine::new();

    // Expect duplicated parameters error
    assert!(matches!(
        engine.compile("fn hello(x, x) { x }").unwrap_err().err_type(),
        ParseErrorType::FnDuplicatedParam(a, b) if a == "hello" && b == "x"));
}

#[test]
fn test_function_pointers() {
    let engine = Engine::new();

    assert_eq!(engine.eval::<String>(r#"type_of(Fn("abc"))"#).unwrap(), "Fn");

    assert_eq!(
        engine
            .eval::<INT>(
                r#"
                    fn foo(x) { 40 + x }

                    let f = Fn("foo");
                    call(f, 2)
                "#
            )
            .unwrap(),
        42
    );

    #[cfg(not(feature = "no_object"))]
    assert_eq!(
        engine
            .eval::<INT>(
                r#"
                    fn foo(x) { 40 + x }

                    let fn_name = "f";
                    fn_name += "oo";

                    let f = Fn(fn_name);
                    f.call(2)
                "#
            )
            .unwrap(),
        42
    );

    #[cfg(not(feature = "no_object"))]
    assert!(matches!(
        *engine.eval::<INT>(r#"let f = Fn("abc"); f.call(0)"#).unwrap_err(),
        EvalAltResult::ErrorFunctionNotFound(f, ..) if f.starts_with("abc (")
    ));

    #[cfg(not(feature = "no_object"))]
    assert_eq!(
        engine
            .eval::<INT>(
                r#"
                    fn foo(x) { 40 + x }

                    let x = #{ action: Fn("foo") };
                    x.action.call(2)
                "#
            )
            .unwrap(),
        42
    );

    #[cfg(not(feature = "no_object"))]
    assert_eq!(
        engine
            .eval::<INT>(
                r#"
                    fn foo(x) { this.data += x; }

                    let x = #{ data: 40, action: Fn("foo") };
                    x.action(2);
                    x.data
                "#
            )
            .unwrap(),
        42
    );
}

#[test]
fn test_internal_fn_bang() {
    let engine = Engine::new();

    assert_eq!(
        engine
            .eval::<INT>(
                "
                    fn foo(y) { x += y; x }

                    let x = 41;
                    let y = 999;

                    foo!(1) + x
                "
            )
            .unwrap(),
        84
    );

    assert!(engine
        .eval::<INT>(
            "
                fn foo(y) { x += y; x }

                let x = 41;
                let y = 999;

                foo(1) + x
            "
        )
        .is_err());

    #[cfg(not(feature = "no_object"))]
    assert!(matches!(
        engine
            .compile(
                "
                    fn foo() { this += x; }

                    let x = 41;
                    let y = 999;

                    y.foo!();
                "
            )
            .unwrap_err()
            .err_type(),
        ParseErrorType::MalformedCapture(..)
    ));
}

#[test]
fn test_internal_fn_is_def() {
    let engine = Engine::new();

    assert!(engine
        .eval::<bool>(
            r#"
                fn foo(x) { x + 1 }
                is_def_fn("foo", 1)
            "#
        )
        .unwrap());
    assert!(!engine
        .eval::<bool>(
            r#"
                fn foo(x) { x + 1 }
                is_def_fn("bar", 1)
            "#
        )
        .unwrap());
    assert!(!engine
        .eval::<bool>(
            r#"
                fn foo(x) { x + 1 }
                is_def_fn("foo", 0)
            "#
        )
        .unwrap());
}
