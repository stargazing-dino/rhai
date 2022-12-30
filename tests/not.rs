use rhai::{Engine, EvalAltResult};

#[test]
fn test_not() -> Result<(), Box<EvalAltResult>> {
    let engine = Engine::new();

    assert!(!engine
        .eval::<bool>("let not_true = !true; not_true")
        .unwrap());

    #[cfg(not(feature = "no_function"))]
    assert!(engine.eval::<bool>("fn not(x) { !x } not(false)").unwrap());

    assert!(engine.eval::<bool>("!!!!true").unwrap());

    Ok(())
}
