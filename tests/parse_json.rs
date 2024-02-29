#[cfg(not(feature = "metadata"))]
mod without_metadata {

    #[test]
    #[cfg(not(feature = "no_function"))]
    #[cfg(not(feature = "no_index"))]
    #[cfg(not(feature = "no_object"))]
    fn test_parse_json() {
        let engine = Engine::new();
        let mut scope = Scope::new();

        let map = engine
            .eval_with_scope::<rhai::Map>(
                &mut scope,
                r#"
                parse_json("{\
                    \"name\": \"John Doe\",\
                    \"age\": 43,\
                    \"address\": {\
                        \"street\": \"10 Downing Street\",\
                        \"city\": \"London\"\
                    },\
                    \"phones\": [\
                        \"+44 1234567\",\
                        \"+44 2345678\"\
                    ]\
                }")
            "#,
            )
            .unwrap();

        assert_eq!(map.len(), 4);
        assert_eq!(map["name"].clone().into_immutable_string().expect("name should exist"), "John Doe");
        assert_eq!(map["age"].as_int().expect("age should exist"), 43);
        assert_eq!(map["phones"].clone().into_typed_array::<String>().expect("phones should exist"), ["+44 1234567", "+44 2345678"]);

        let address = map["address"].read_lock::<rhai::Map>().expect("address should exist");
        assert_eq!(address["city"].clone().into_immutable_string().expect("address.city should exist"), "London");
        assert_eq!(address["street"].clone().into_immutable_string().expect("address.street should exist"), "10 Downing Street");
    }

    #[test]
    #[cfg(feature = "no_index")]
    #[cfg(not(feature = "no_function"))]
    fn test_parse_json_err_no_index() {
        let engine = Engine::new();
        let mut scope = Scope::new();

        let err = engine
            .eval_with_scope::<rhai::Dynamic>(
                &mut scope,
                r#"
                parse_json("{\
                    \"v\": [\
                        1,\
                        2\
                    ]\
                }")
            "#,
            )
            .unwrap_err();

        assert!(matches!(err.as_ref(), rhai::EvalAltResult::ErrorParsing(
            ParseErrorType::BadInput(LexError::UnexpectedInput(token)), pos)
                if token == "[" && *pos == rhai::Position::new(1, 7)));
    }

    #[test]
    #[cfg(feature = "no_object")]
    #[cfg(not(feature = "no_function"))]
    fn test_parse_json_err_no_object() {
        let engine = Engine::new();
        let mut scope = Scope::new();

        let err = engine
            .eval_with_scope::<rhai::Dynamic>(
                &mut scope,
                r#"
                parse_json("{\
                    \"v\": {\
                        \"a\": 1,\
                        \"b\": 2,\
                    }\
                }")
            "#,
            )
            .unwrap_err();

        assert!(matches!(err.as_ref(), rhai::EvalAltResult::ErrorFunctionNotFound(msg, pos)
            if msg == "parse_json (&str | ImmutableString | String)" && *pos == rhai::Position::new(2, 13)));
    }
}

#[cfg(feature = "metadata")]
mod with_metadata {

    #[test]
    #[cfg(not(feature = "no_function"))]
    #[cfg(not(feature = "no_index"))]
    #[cfg(not(feature = "no_object"))]
    fn test_parse_json() {
        let engine = Engine::new();
        let mut scope = Scope::new();

        let map = engine
            .eval_with_scope::<rhai::Map>(
                &mut scope,
                r#"
                parse_json("{\
                    \"name\": \"John Doe\",\
                    \"age\": 43,\
                    \"address\": {\
                        \"street\": \"10 Downing Street\",\
                        \"city\": \"London\"\
                    },\
                    \"phones\": [\
                        \"+44 1234567\",\
                        \"+44 2345678\"\
                    ]\
                }")
            "#,
            )
            .unwrap();

        assert_eq!(map.len(), 4);
        assert_eq!(map["name"].clone().into_immutable_string().expect("name should exist"), "John Doe");
        assert_eq!(map["age"].as_int().expect("age should exist"), 43);
        assert_eq!(map["phones"].clone().into_typed_array::<String>().expect("phones should exist"), ["+44 1234567", "+44 2345678"]);

        let address = map["address"].read_lock::<rhai::Map>().expect("address should exist");
        assert_eq!(address["city"].clone().into_immutable_string().expect("address.city should exist"), "London");
        assert_eq!(address["street"].clone().into_immutable_string().expect("address.street should exist"), "10 Downing Street");
    }

    #[test]
    #[cfg(feature = "no_index")]
    #[cfg(not(feature = "no_function"))]
    fn test_parse_json_err_no_index() {
        let engine = Engine::new();
        let mut scope = Scope::new();

        let err = engine
            .eval_with_scope::<rhai::Dynamic>(
                &mut scope,
                r#"
                parse_json("{\
                    \"v\": [\
                        1,\
                        2\
                    ]\
                }")
            "#,
            )
            .unwrap_err();

        assert!(matches!(err.as_ref(), rhai::EvalAltResult::ErrorParsing(
            ParseErrorType::BadInput(LexError::UnexpectedInput(token)), pos)
                if token == "[" && *pos == rhai::Position::new(1, 7)));
    }

    #[test]
    #[cfg(feature = "no_object")]
    #[cfg(not(feature = "no_function"))]
    fn test_parse_json_err_no_object() {
        let engine = Engine::new();
        let mut scope = Scope::new();

        let err = engine
            .eval_with_scope::<rhai::Dynamic>(
                &mut scope,
                r#"
                parse_json("{\
                    \"v\": {\
                        \"a\": 1,\
                        \"b\": 2,\
                    }\
                }")
            "#,
            )
            .unwrap_err();

        assert!(matches!(err.as_ref(), rhai::EvalAltResult::ErrorFunctionNotFound(msg, pos)
            if msg == "parse_json (&str | ImmutableString | String)" && *pos == rhai::Position::new(2, 13)));
    }
}
