use rhai::{CustomType, Engine, EvalAltResult, Position, TypeBuilder};

#[test]
fn build_type() -> Result<(), Box<EvalAltResult>> {
    #[derive(Debug, Clone, PartialEq)]
    struct Vec3 {
        x: i64,
        y: i64,
        z: i64,
    }

    impl Vec3 {
        fn new(x: i64, y: i64, z: i64) -> Self {
            Self { x, y, z }
        }
        fn get_x(&mut self) -> i64 {
            self.x
        }
        fn set_x(&mut self, x: i64) {
            self.x = x
        }
        fn get_y(&mut self) -> i64 {
            self.y
        }
        fn set_y(&mut self, y: i64) {
            self.y = y
        }
        fn get_z(&mut self) -> i64 {
            self.z
        }
        fn set_z(&mut self, z: i64) {
            self.z = z
        }
        fn get_component(&mut self, idx: i64) -> Result<i64, Box<EvalAltResult>> {
            match idx {
                0 => Ok(self.x),
                1 => Ok(self.y),
                2 => Ok(self.z),
                _ => Err(Box::new(EvalAltResult::ErrorIndexNotFound(
                    idx.into(),
                    Position::NONE,
                ))),
            }
        }
    }

    impl CustomType for Vec3 {
        fn build(mut builder: TypeBuilder<Self>) {
            builder
                .with_name("Vec3")
                .with_fn("vec3", Self::new)
                .with_get_set("x", Self::get_x, Self::set_x)
                .with_get_set("y", Self::get_y, Self::set_y)
                .with_get_set("z", Self::get_z, Self::set_z)
                .with_indexer_get_result(Self::get_component);
        }
    }

    let mut engine = Engine::new();
    engine.build_type::<Vec3>();

    assert_eq!(
        engine.eval::<Vec3>(
            r#"
        let v = vec3(1, 2, 3);
        v
"#,
        )?,
        Vec3::new(1, 2, 3),
    );
    assert_eq!(
        engine.eval::<i64>(
            r#"
        let v = vec3(1, 2, 3);
        v.x
"#,
        )?,
        1,
    );
    assert_eq!(
        engine.eval::<i64>(
            r#"
        let v = vec3(1, 2, 3);
        v.y
"#,
        )?,
        2,
    );
    assert_eq!(
        engine.eval::<i64>(
            r#"
        let v = vec3(1, 2, 3);
        v.z
"#,
        )?,
        3,
    );
    assert!(engine.eval::<bool>(
        r#"
        let v = vec3(1, 2, 3);
        v.x == v[0] && v.y == v[1] && v.z == v[2]
"#,
    )?);
    assert_eq!(
        engine.eval::<Vec3>(
            r#"
        let v = vec3(1, 2, 3);
        v.x = 5;
        v.y = 6;
        v.z = 7;
        v
"#,
        )?,
        Vec3::new(5, 6, 7),
    );

    Ok(())
}
