#![no_main]
use rhai::{Dynamic, Engine, OptimizationLevel};

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use std::time::{Duration, Instant};

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
    let start = Instant::now();
    engine.on_progress(move |_| {
        // We need fuzzing to be fast, so we'll stop executing after 1s.
        if start.elapsed() > Duration::from_secs(1) {
            Some(Dynamic::UNIT)
        } else {
            None
        }
    });
    let engine = engine;

    _ = engine.run(ctx.script);
});
