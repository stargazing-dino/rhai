use crate::def_package;
use crate::plugin::*;
use crate::types::dynamic::Tag;
use crate::{Dynamic, RhaiResultOf, ERR, INT};
#[cfg(feature = "no_std")]
use std::prelude::v1::*;

def_package! {
    /// Package of core language features.
    pub LanguageCorePackage(lib) {
        lib.standard = true;

        combine_with_exported_module!(lib, "core", core_functions);

        #[cfg(not(feature = "no_function"))]
        #[cfg(not(feature = "no_index"))]
        #[cfg(not(feature = "no_object"))]
        combine_with_exported_module!(lib, "reflection", reflection_functions);
    }
}

#[export_module]
mod core_functions {
    /// Return the _tag_ of a `Dynamic` value.
    ///
    /// # Example
    ///
    /// ```rhai
    /// let x = "hello, world!";
    ///
    /// x.tag = 42;
    ///
    /// print(x.tag);           // prints 42
    /// ```
    #[rhai_fn(name = "tag", get = "tag", pure)]
    pub fn get_tag(value: &mut Dynamic) -> INT {
        value.tag() as INT
    }
    /// Set the _tag_ of a `Dynamic` value.
    ///
    /// # Example
    ///
    /// ```rhai
    /// let x = "hello, world!";
    ///
    /// x.tag = 42;
    ///
    /// print(x.tag);           // prints 42
    /// ```
    #[rhai_fn(name = "set_tag", set = "tag", return_raw)]
    pub fn set_tag(value: &mut Dynamic, tag: INT) -> RhaiResultOf<()> {
        const TAG_MIN: Tag = Tag::MIN;
        const TAG_MAX: Tag = Tag::MAX;

        if tag < TAG_MIN as INT {
            Err(ERR::ErrorArithmetic(
                format!(
                    "{tag} is too small to fit into a tag (must be between {TAG_MIN} and {TAG_MAX})"
                ),
                Position::NONE,
            )
            .into())
        } else if tag > TAG_MAX as INT {
            Err(ERR::ErrorArithmetic(
                format!(
                    "{tag} is too large to fit into a tag (must be between {TAG_MIN} and {TAG_MAX})"
                ),
                Position::NONE,
            )
            .into())
        } else {
            value.set_tag(tag as Tag);
            Ok(())
        }
    }

    /// Block the current thread for a particular number of `seconds`.
    #[cfg(not(feature = "no_float"))]
    #[cfg(not(feature = "no_std"))]
    #[rhai_fn(name = "sleep")]
    pub fn sleep_float(seconds: crate::FLOAT) {
        if seconds <= 0.0 {
            return;
        }

        #[cfg(not(feature = "f32_float"))]
        std::thread::sleep(std::time::Duration::from_secs_f64(seconds));
        #[cfg(feature = "f32_float")]
        std::thread::sleep(std::time::Duration::from_secs_f32(seconds));
    }
    /// Block the current thread for a particular number of `seconds`.
    #[cfg(not(feature = "no_std"))]
    pub fn sleep(seconds: INT) {
        if seconds <= 0 {
            return;
        }
        std::thread::sleep(std::time::Duration::from_secs(seconds as u64));
    }

    /// Parse a JSON string into a value.
    ///
    /// # Example
    ///
    /// ```rhai
    /// let m = parse_json(`{"a":1, "b":2, "c":3}`);
    ///
    /// print(m);       // prints #{"a":1, "b":2, "c":3}
    /// ```
    #[cfg(not(feature = "no_index"))]
    #[cfg(not(feature = "no_object"))]
    #[cfg(feature = "metadata")]
    #[rhai_fn(return_raw)]
    pub fn parse_json(_ctx: NativeCallContext, json: &str) -> RhaiResultOf<Dynamic> {
        serde_json::from_str(json).map_err(|err| err.to_string().into())
    }
}

#[cfg(not(feature = "no_function"))]
#[cfg(not(feature = "no_index"))]
#[cfg(not(feature = "no_object"))]
#[export_module]
mod reflection_functions {
    pub fn get_fn_metadata_list(ctx: NativeCallContext) -> crate::Array {
        collect_fn_metadata(ctx, |_, _, _, _, _| true)
    }
    #[rhai_fn(name = "get_fn_metadata_list")]
    pub fn get_fn_metadata(ctx: NativeCallContext, name: &str) -> crate::Array {
        collect_fn_metadata(ctx, |_, _, n, _, _| n == name)
    }
    #[rhai_fn(name = "get_fn_metadata_list")]
    pub fn get_fn_metadata2(ctx: NativeCallContext, name: &str, params: INT) -> crate::Array {
        if params < 0 || params > crate::MAX_USIZE_INT {
            crate::Array::new()
        } else {
            collect_fn_metadata(ctx, |_, _, n, p, _| p == (params as usize) && n == name)
        }
    }
}

#[cfg(not(feature = "no_function"))]
#[cfg(not(feature = "no_index"))]
#[cfg(not(feature = "no_object"))]
fn collect_fn_metadata(
    ctx: NativeCallContext,
    filter: impl Fn(FnNamespace, FnAccess, &str, usize, &crate::Shared<crate::ast::ScriptFnDef>) -> bool
        + Copy,
) -> crate::Array {
    use crate::{ast::ScriptFnDef, Array, Map};

    // Create a metadata record for a function.
    fn make_metadata(
        dict: &mut crate::types::StringsInterner,
        #[cfg(not(feature = "no_module"))] namespace: crate::Identifier,
        func: &ScriptFnDef,
    ) -> Map {
        let mut map = Map::new();

        #[cfg(not(feature = "no_module"))]
        if !namespace.is_empty() {
            map.insert("namespace".into(), dict.get(namespace).into());
        }
        map.insert("name".into(), dict.get(func.name.as_str()).into());
        map.insert(
            "access".into(),
            dict.get(match func.access {
                FnAccess::Public => "public",
                FnAccess::Private => "private",
            })
            .into(),
        );
        map.insert(
            "is_anonymous".into(),
            func.name.starts_with(crate::engine::FN_ANONYMOUS).into(),
        );
        map.insert(
            "params".into(),
            func.params
                .iter()
                .map(|p| dict.get(p.as_str()).into())
                .collect::<Array>()
                .into(),
        );
        #[cfg(feature = "metadata")]
        if !func.comments.is_empty() {
            map.insert(
                "comments".into(),
                func.comments
                    .iter()
                    .map(|s| dict.get(s.as_ref()).into())
                    .collect::<Array>()
                    .into(),
            );
        }

        map
    }

    let dict = &mut crate::types::StringsInterner::new();
    let mut list = Array::new();

    ctx.iter_namespaces()
        .flat_map(Module::iter_script_fn)
        .filter(|(s, a, n, p, f)| filter(*s, *a, n, *p, f))
        .for_each(|(.., f)| {
            list.push(
                make_metadata(
                    dict,
                    #[cfg(not(feature = "no_module"))]
                    crate::Identifier::new_const(),
                    f,
                )
                .into(),
            );
        });

    ctx.engine()
        .global_modules
        .iter()
        .flat_map(|m| m.iter_script_fn())
        .filter(|(ns, a, n, p, f)| filter(*ns, *a, n, *p, f))
        .for_each(|(.., f)| {
            list.push(
                make_metadata(
                    dict,
                    #[cfg(not(feature = "no_module"))]
                    crate::Identifier::new_const(),
                    f,
                )
                .into(),
            );
        });

    #[cfg(not(feature = "no_module"))]
    ctx.engine()
        .global_sub_modules
        .values()
        .flat_map(|m| m.iter_script_fn())
        .filter(|(ns, a, n, p, f)| filter(*ns, *a, n, *p, f))
        .for_each(|(.., f)| {
            list.push(
                make_metadata(
                    dict,
                    #[cfg(not(feature = "no_module"))]
                    crate::Identifier::new_const(),
                    f,
                )
                .into(),
            );
        });

    #[cfg(not(feature = "no_module"))]
    {
        // Recursively scan modules for script-defined functions.
        fn scan_module(
            dict: &mut crate::types::StringsInterner,
            list: &mut Array,
            namespace: &str,
            module: &Module,
            filter: impl Fn(
                    FnNamespace,
                    FnAccess,
                    &str,
                    usize,
                    &crate::Shared<crate::ast::ScriptFnDef>,
                ) -> bool
                + Copy,
        ) {
            module
                .iter_script_fn()
                .filter(|(s, a, n, p, f)| filter(*s, *a, n, *p, f))
                .for_each(|(.., f)| list.push(make_metadata(dict, namespace.into(), f).into()));
            for (ns, m) in module.iter_sub_modules() {
                let ns = format!(
                    "{namespace}{}{ns}",
                    crate::tokenizer::Token::DoubleColon.literal_syntax()
                );
                scan_module(dict, list, &ns, &**m, filter);
            }
        }

        for (ns, m) in ctx.iter_imports_raw() {
            scan_module(dict, &mut list, ns, &**m, filter);
        }
    }

    list
}
