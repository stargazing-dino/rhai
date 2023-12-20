#![no_main]
use rhai::{Dynamic, Engine, OptimizationLevel};

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use std::time::Instant;

#[derive(Debug, Clone, Arbitrary)]
struct Ctx<'a> {
    script: &'a str,
    optimization_level: OptimizationLevel,
}

fuzz_target!(|ctx: Ctx| {
    let mut engine = Engine::new();

    engine.set_max_string_size(1000);
    engine.set_max_array_size(500);
    engine.set_max_map_size(500);
    engine.set_max_variables(1000);
    engine.set_max_modules(1000);
    engine.set_max_call_levels(10);
    engine.set_max_expr_depths(50, 5);
    engine.set_optimization_level(ctx.optimization_level);

    // Limit the length of scripts.
    let script = ctx.script.chars().take(32 * 1024).collect::<String>();

    // We need fuzzing to be fast, so we'll stop executing after 1s.
    let start = Instant::now();
    engine.on_progress(move |_| (start.elapsed().as_millis() > 1000).then_some(Dynamic::UNIT));

    let engine = engine;

    _ = engine.run(&script);
});
