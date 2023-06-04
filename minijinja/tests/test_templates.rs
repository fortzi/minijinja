use std::collections::BTreeMap;
use std::fmt::Write;
use std::{env, fs};

use minijinja::value::{StructObject, Value};
use minijinja::{context, Environment, Error, State};

use similar_asserts::assert_eq;

#[test]
fn test_vm() {
    let mut refs = Vec::new();
    insta::glob!("inputs/refs/*", |entry| {
        let filename = entry.file_name().unwrap();
        let filename = filename.to_str().unwrap();
        if filename.ends_with(".txt") || filename.ends_with(".html") {
            let source = fs::read_to_string(entry).unwrap();
            refs.push((entry.to_path_buf(), source));
        }
    });

    insta::glob!("inputs/*", |path| {
        if !path.metadata().unwrap().is_file() {
            return;
        }
        let filename = path.file_name().unwrap().to_str().unwrap();
        let contents = std::fs::read_to_string(path).unwrap();
        let mut iter = contents.splitn(2, "\n---\n");
        let mut env = Environment::new();
        let ctx: serde_json::Value = serde_json::from_str(iter.next().unwrap()).unwrap();

        for (path, source) in &refs {
            let ref_filename = path.file_name().unwrap().to_str().unwrap();
            env.add_template(ref_filename, source).unwrap();
        }

        let content = iter.next().unwrap();
        let rendered = if let Err(err) = env.add_template(filename, content) {
            let mut rendered = format!("!!!SYNTAX ERROR!!!\n\n{err:#?}\n\n");
            writeln!(rendered, "{err:#}").unwrap();
            rendered
        } else {
            let template = env.get_template(filename).unwrap();

            match template.render(&ctx) {
                Ok(mut rendered) => {
                    rendered.push('\n');
                    rendered
                }
                Err(err) => {
                    let mut rendered = format!("!!!ERROR!!!\n\n{err:#?}\n\n");

                    writeln!(rendered, "{err:#}").unwrap();
                    let mut err = &err as &dyn std::error::Error;
                    while let Some(next_err) = err.source() {
                        writeln!(rendered).unwrap();
                        writeln!(rendered, "caused by: {next_err:#}").unwrap();
                        err = next_err;
                    }

                    rendered
                }
            }
        };

        insta::with_settings!({
            info => &ctx,
            description => content.trim_end(),
            omit_expression => true
        }, {
            insta::assert_snapshot!(&rendered);
        });
    });
}

#[test]
fn test_vm_block_fragments() {
    let mut refs = Vec::new();
    insta::glob!("fragment-inputs/refs/*", |entry| {
        let filename = entry.file_name().unwrap();
        let filename = filename.to_str().unwrap();
        if filename.ends_with(".txt") || filename.ends_with(".html") {
            let source = fs::read_to_string(entry).unwrap();
            refs.push((entry.to_path_buf(), source));
        }
    });

    insta::glob!("fragment-inputs/*", |path| {
        if !path.metadata().unwrap().is_file() {
            return;
        }
        let filename = path.file_name().unwrap().to_str().unwrap();
        let contents = std::fs::read_to_string(path).unwrap();
        let mut iter = contents.splitn(2, "\n---\n");
        let mut env = Environment::new();
        let ctx: serde_json::Value = serde_json::from_str(iter.next().unwrap()).unwrap();

        for (path, source) in &refs {
            let ref_filename = path.file_name().unwrap().to_str().unwrap();
            env.add_template(ref_filename, source).unwrap();
        }

        let content = iter.next().unwrap();
        let rendered = if let Err(err) = env.add_template(filename, content) {
            let mut rendered = format!("!!!SYNTAX ERROR!!!\n\n{err:#?}\n\n");
            writeln!(rendered, "{err:#}").unwrap();
            rendered
        } else {
            let template = env.get_template(filename).unwrap();

            match template
                .eval_to_module(&ctx)
                .and_then(|mut x| x.render_block("fragment"))
            {
                Ok(mut rendered) => {
                    rendered.push('\n');
                    rendered
                }
                Err(err) => {
                    let mut rendered = format!("!!!ERROR!!!\n\n{err:#?}\n\n");

                    writeln!(rendered, "{err:#}").unwrap();
                    let mut err = &err as &dyn std::error::Error;
                    while let Some(next_err) = err.source() {
                        writeln!(rendered).unwrap();
                        writeln!(rendered, "caused by: {next_err:#}").unwrap();
                        err = next_err;
                    }

                    rendered
                }
            }
        };

        insta::with_settings!({
            info => &ctx,
            description => content.trim_end(),
            omit_expression => true
        }, {
            insta::assert_snapshot!(&rendered);
        });
    });
}

#[test]
fn test_custom_filter() {
    fn test_filter(_: &State, value: String) -> Result<String, Error> {
        Ok(format!("[{value}]"))
    }

    let mut ctx = BTreeMap::new();
    ctx.insert("var", 42);

    let mut env = Environment::new();
    env.add_filter("test", test_filter);
    env.add_template("test", "{{ var|test }}").unwrap();
    let tmpl = env.get_template("test").unwrap();
    let rv = tmpl.render(&ctx).unwrap();
    assert_eq!(rv, "[42]");
}

#[test]
fn test_items_and_dictsort_with_structs() {
    struct MyStruct;

    impl StructObject for MyStruct {
        fn get_field(&self, name: &str) -> Option<Value> {
            match name {
                "a" => Some(Value::from("A")),
                "b" => Some(Value::from("B")),
                _ => None,
            }
        }

        fn static_fields(&self) -> Option<&'static [&'static str]> {
            Some(&["b", "a"][..])
        }
    }

    insta::assert_snapshot!(
        minijinja::render!("{{ x|items }}", x => Value::from_struct_object(MyStruct)),
        @r###"[["b", "B"], ["a", "A"]]"###
    );
    insta::assert_snapshot!(
        minijinja::render!("{{ x|dictsort }}", x => Value::from_struct_object(MyStruct)),
        @r###"[["a", "A"], ["b", "B"]]"###
    );
}

#[test]
fn test_urlencode_with_struct() {
    struct MyStruct;

    impl StructObject for MyStruct {
        fn get_field(&self, name: &str) -> Option<Value> {
            match name {
                "a" => Some(Value::from("a 1")),
                "b" => Some(Value::from("b 2")),
                _ => None,
            }
        }

        fn static_fields(&self) -> Option<&'static [&'static str]> {
            Some(&["a", "b"][..])
        }
    }

    insta::assert_snapshot!(
        minijinja::render!("{{ x|urlencode }}", x => Value::from_struct_object(MyStruct)),
        @"a=a%201&b=b%202"
    );
}

#[test]
fn test_single() {
    let mut env = Environment::new();
    env.add_template("simple", "Hello {{ name }}!").unwrap();
    let tmpl = env.get_template("simple").unwrap();
    let rv = tmpl.render(context!(name => "Peter")).unwrap();
    assert_eq!(rv, "Hello Peter!");
}

#[test]
fn test_values_scientific_notation() {
    let mut env = Environment::new();
    env.add_template("sci1", "VALUE = {{ value or -12.4E-4 }}")
        .unwrap();
    let tmpl = env.get_template("sci1").unwrap();
    let rv = tmpl.render(context!(value => -12.4E-3)).unwrap();
    assert_eq!(rv, "VALUE = -0.0124");
    let rv = tmpl.render(context!());
    // assert_eq!(rv, "VALUE = -0.00124");
    assert!(rv.is_ok());

    env.add_template("sci2", "VALUE = {{ value or 1.4E4 }}")
        .unwrap();
    let tmpl = env.get_template("sci2").unwrap();
    let rv = tmpl.render(context!());
    assert!(rv.is_ok());

    env.add_template("sci3", "VALUE = {{ value or 1.4e+4}}")
        .unwrap();
    let tmpl = env.get_template("sci3").unwrap();
    let rv = tmpl.render(context!());
    assert!(rv.is_ok());

    env.add_template("sci4", "VALUE = {{ 1.4+4}}").unwrap();
    let tmpl = env.get_template("sci4").unwrap();
    let rv = tmpl.render(context!());
    assert!(rv.is_ok());

    env.add_template("sci5", "VALUE = {{ 1.4+1E-1}}").unwrap();
    let tmpl = env.get_template("sci5").unwrap();
    let rv = tmpl.render(context!());
    assert!(rv.is_ok());

    env.add_template("sci6", "VALUE = {{ 1.0E0+1.0}}").unwrap();
    let tmpl = env.get_template("sci6").unwrap();
    let rv = tmpl.render(context!());
    assert!(rv.is_ok());
}

#[test]
fn test_auto_escaping() {
    let mut env = Environment::new();
    env.add_template("index.html", "{{ var }}").unwrap();
    #[cfg(feature = "json")]
    {
        env.add_template("index.js", "{{ var }}").unwrap();
    }
    env.add_template("index.txt", "{{ var }}").unwrap();

    // html
    let tmpl = env.get_template("index.html").unwrap();
    let rv = tmpl.render(context!(var => "<script>")).unwrap();
    insta::assert_snapshot!(rv, @"&lt;script&gt;");

    // JSON
    #[cfg(feature = "json")]
    {
        use minijinja::value::Value;
        let tmpl = env.get_template("index.js").unwrap();
        let rv = tmpl.render(context!(var => "foo\"bar'baz")).unwrap();
        insta::assert_snapshot!(rv, @r###""foo\"bar'baz""###);
        let rv = tmpl
            .render(context!(var => [Value::from(true), Value::from("<foo>"), Value::from(())]))
            .unwrap();
        insta::assert_snapshot!(rv, @r###"[true,"<foo>",null]"###);
    }

    // Text
    let tmpl = env.get_template("index.txt").unwrap();
    let rv = tmpl.render(context!(var => "foo\"bar'baz")).unwrap();
    insta::assert_snapshot!(rv, @r###"foo"bar'baz"###);
}

#[test]
fn test_loop_changed() {
    let rv = minijinja::render!(
        r#"
        {%- for i in items -%}
          {% if loop.changed(i) %}{{ i }}{% endif %}
        {%- endfor -%}
        "#,
        items => vec![1, 1, 1, 2, 3, 4, 4, 5],
    );
    assert_eq!(rv, "12345");
}

// ideally this would work, but unfortunately the way serde flatten works makes it
// impossible for us to support with the internal optimizations in the value model.
// see https://github.com/mitsuhiko/minijinja/issues/222
#[derive(Debug, serde::Serialize)]
struct Bad {
    a: i32,
    #[serde(flatten)]
    more: Value,
}

#[test]
#[should_panic = "can only flatten structs and maps"]
fn test_flattening() {
    let ctx = Bad {
        a: 42,
        more: Value::from(BTreeMap::from([("b", 23)])),
    };

    let env = Environment::new();
    env.render_str("{{ debug() }}", ctx).unwrap();
}

#[test]
fn test_flattening_sub_item_good() {
    let bad = Bad {
        a: 42,
        more: Value::from(BTreeMap::from([("b", 23)])),
    };

    let ctx = context!(bad, good => "good");
    let env = Environment::new();

    // we are not touching a bad value, so we are good
    let rv = env.render_str("{{ good }}", ctx).unwrap();
    assert_eq!(rv, "good");
}

#[test]
#[should_panic = "can only flatten structs and maps"]
fn test_flattening_sub_item_bad_lookup() {
    let bad = Bad {
        a: 42,
        more: Value::from(BTreeMap::from([("b", 23)])),
    };

    let ctx = context!(bad, good => "good");
    let env = Environment::new();

    // resolving an invalid value will fail
    env.render_str("{{ bad }}", ctx).unwrap();
}

#[test]
#[should_panic = "can only flatten structs and maps"]
fn test_flattening_sub_item_bad_attr() {
    let bad = Bad {
        a: 42,
        more: Value::from(BTreeMap::from([("b", 23)])),
    };

    let ctx = context!(good => context!(bad));
    let env = Environment::new();

    // resolving an invalid value will fail, even in an attribute lookup
    env.render_str("{% if good.bad %}...{% endif %}", ctx)
        .unwrap();
}

#[test]
fn test_flattening_sub_item_shielded_print() {
    let bad = Bad {
        a: 42,
        more: Value::from(BTreeMap::from([("b", 23)])),
    };

    let ctx = context!(good => context!(bad));
    let env = Environment::new();

    // this on the other hand is okay
    let value = env.render_str("{{ good }}", ctx).unwrap();
    assert_eq!(
        value,
        r#"{"bad": <invalid value: can only flatten structs and maps (got an enum)>}"#
    );
}

#[test]
#[cfg(feature = "custom_syntax")]
fn test_custom_syntax() {
    let mut env = Environment::new();
    env.set_syntax(minijinja::Syntax {
        block_start: "{".into(),
        block_end: "}".into(),
        variable_start: "${".into(),
        variable_end: "}".into(),
        comment_start: "{*".into(),
        comment_end: "*}".into(),
    })
    .unwrap();

    // this on the other hand is okay
    let value = env
        .render_str("{for x in range(3)}${x}{endfor}{* nothing *}", ())
        .unwrap();
    assert_eq!(value, r"012");
}

#[test]
fn test_undeclared_variables() {
    let mut env = Environment::new();
    env.add_template(
        "demo",
        "{% set x = foo %}{{ x }}{{ bar.baz }}{{ bar.blub }}",
    )
    .unwrap();
    let tmpl = env.get_template("demo").unwrap();
    let undeclared = tmpl.undeclared_variables(false);
    assert_eq!(
        undeclared,
        ["foo", "bar"].into_iter().map(|x| x.to_string()).collect()
    );
    let undeclared = tmpl.undeclared_variables(true);
    dbg!(&undeclared);
    assert_eq!(
        undeclared,
        ["foo", "bar.baz", "bar.blub"]
            .into_iter()
            .map(|x| x.to_string())
            .collect()
    );
}

#[test]
fn test_block_fragments() {
    let mut env = Environment::new();
    env.add_template(
        "demo",
        "I am outside the fragment{% block foo %}foo{% endblock %}So am I!",
    )
    .unwrap();
    let tmpl = env.get_template("demo").unwrap();

    let rv_a = tmpl.render(()).unwrap();
    let rv_b = tmpl
        .eval_to_module(())
        .unwrap()
        .render_block("foo")
        .unwrap();

    assert_eq!(rv_a, "I am outside the fragmentfooSo am I!");
    assert_eq!(rv_b, "foo");
}

#[test]
fn test_module() {
    let mut env = Environment::new();
    env.add_template(
        "foo.html",
        r#"
        {% set global = variable * 2 %}
        {% macro something() %}{{ global }}{% endmacro %}
        {% block baz %}[{{ global }}]{% endblock %}
    "#,
    )
    .unwrap();
    let template = env.get_template("foo.html").unwrap();
    let mut module = template
        .eval_to_module(context! {
            variable => 23
        })
        .unwrap();
    assert_eq!(module.get_global("range"), None);
    assert_eq!(module.get_global("global"), Some(Value::from(23 * 2)));
    assert_eq!(module.call_macro("something", &[]).unwrap(), "46");
    assert_eq!(module.render_block("baz").unwrap(), "[46]");
}
