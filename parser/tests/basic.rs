extern crate gluon_base as base;
extern crate gluon_parser as parser;
extern crate env_logger;
#[macro_use]
extern crate log;

mod support;

use base::ast::*;
use base::pos::{self, BytePos, Span, Spanned};
use base::types::{Alias, ArcType, Field, Generic, Kind, Type};
use parser::{parse_string, Error};
use support::MockEnv;

pub fn intern(s: &str) -> String {
    String::from(s)
}

type SpExpr = SpannedExpr<String>;

fn no_loc<T>(value: T) -> Spanned<T, BytePos> {
    pos::spanned(Span {
                     start: BytePos::from(0),
                     end: BytePos::from(0),
                 },
                 value)
}

fn binop(l: SpExpr, s: &str, r: SpExpr) -> SpExpr {
    no_loc(Expr::Infix(Box::new(l), TypedIdent::new(intern(s)), Box::new(r)))
}

fn int(i: i64) -> SpExpr {
    no_loc(Expr::Literal(Literal::Int(i)))
}

fn let_(s: &str, e: SpExpr, b: SpExpr) -> SpExpr {
    let_a(s, &[], e, b)
}

fn let_a(s: &str, args: &[&str], e: SpExpr, b: SpExpr) -> SpExpr {
    no_loc(Expr::LetBindings(vec![ValueBinding {
                                      comment: None,
                                      name: no_loc(Pattern::Ident(TypedIdent::new(intern(s)))),
                                      typ: Type::hole(),
                                      args: args.iter()
                                          .map(|i| TypedIdent::new(intern(i)))
                                          .collect(),
                                      expr: e,
                                  }],
                             Box::new(b)))
}

fn id(s: &str) -> SpExpr {
    no_loc(Expr::Ident(TypedIdent::new(intern(s))))
}

fn field(s: &str, typ: ArcType<String>) -> Field<String> {
    Field {
        name: intern(s),
        typ: typ,
    }
}

fn typ(s: &str) -> ArcType<String> {
    assert!(s.len() != 0);
    match s.parse() {
        Ok(b) => Type::builtin(b),
        Err(()) if s.starts_with(char::is_lowercase) => generic_ty(s),
        Err(()) => Type::ident(intern(s)),
    }
}

fn generic_ty(s: &str) -> ArcType<String> {
    Type::generic(generic(s))
}

fn generic(s: &str) -> Generic<String> {
    Generic {
        kind: Kind::variable(0),
        id: intern(s),
    }
}

fn app(e: SpExpr, args: Vec<SpExpr>) -> SpExpr {
    no_loc(Expr::App(Box::new(e), args))
}

fn if_else(p: SpExpr, if_true: SpExpr, if_false: SpExpr) -> SpExpr {
    no_loc(Expr::IfElse(Box::new(p), Box::new(if_true), Box::new(if_false)))
}

fn case(e: SpExpr, alts: Vec<(Pattern<String>, SpExpr)>) -> SpExpr {
    no_loc(Expr::Match(Box::new(e),
                       alts.into_iter()
                           .map(|(p, e)| {
                               Alternative {
                                   pattern: no_loc(p),
                                   expr: e,
                               }
                           })
                           .collect()))
}

fn lambda(name: &str, args: Vec<String>, body: SpExpr) -> SpExpr {
    no_loc(Expr::Lambda(Lambda {
        id: TypedIdent::new(intern(name)),
        args: args.into_iter().map(|id| TypedIdent::new(id)).collect(),
        body: Box::new(body),
    }))
}

fn type_decl(name: String,
             args: Vec<Generic<String>>,
             typ: ArcType<String>,
             body: SpExpr)
             -> SpExpr {
    type_decls(vec![TypeBinding {
                        comment: None,
                        name: name.clone(),
                        alias: Alias::new(name, args, typ),
                    }],
               body)
}

fn type_decls(binds: Vec<TypeBinding<String>>, body: SpExpr) -> SpExpr {
    no_loc(Expr::TypeBindings(binds, Box::new(body)))
}

fn record(fields: Vec<(String, Option<SpExpr>)>) -> SpExpr {
    record_a(Vec::new(), fields)
}

fn record_a(types: Vec<(String, Option<ArcType<String>>)>,
            fields: Vec<(String, Option<SpExpr>)>)
            -> SpExpr {
    no_loc(Expr::Record {
        typ: Type::hole(),
        types: types,
        exprs: fields,
    })
}

fn field_access(expr: SpExpr, field: &str) -> SpExpr {
    no_loc(Expr::Projection(Box::new(expr), intern(field), Type::hole()))
}

fn array(fields: Vec<SpExpr>) -> SpExpr {
    no_loc(Expr::Array(Array {
        typ: Type::hole(),
        exprs: fields,
    }))
}

fn parse(input: &str) -> Result<SpannedExpr<String>, (Option<SpannedExpr<String>>, Error)> {
    parse_string(&mut MockEnv::new(), input)
}

macro_rules! parse_new {
    ($input:expr) => {{
        // Replace windows line endings so that byte positins match up on multiline expressions
        let input = $input.replace("\r\n", "\n");
        parse(&input).unwrap_or_else(|(_, err)| panic!("{}", err))
    }}
}

#[test]
fn dangling_in() {
    let _ = ::env_logger::init();
    // Check that the lexer does not insert an extra `in`
    let text = r#"
let x = 1
in

let y = 2
y
"#;
    let e = parse_new!(text);
    assert_eq!(e, let_("x", int(1), let_("y", int(2), id("y"))));
}

#[test]
fn expression() {
    let _ = ::env_logger::init();
    let e = parse("2 * 3 + 4");
    assert_eq!(e, Ok(binop(binop(int(2), "*", int(3)), "+", int(4))));
    let e = parse(r#"\x y -> x + y"#);
    assert_eq!(e,
               Ok(lambda("",
                         vec![intern("x"), intern("y")],
                         binop(id("x"), "+", id("y")))));
    let e = parse(r#"type Test = Int in 0"#);
    assert_eq!(e, Ok(type_decl(intern("Test"), vec![], typ("Int"), int(0))));
}

#[test]
fn application() {
    let _ = ::env_logger::init();
    let e = parse_new!("let f = \\x y -> x + y in f 1 2");
    let a = let_("f",
                 lambda("",
                        vec![intern("x"), intern("y")],
                        binop(id("x"), "+", id("y"))),
                 app(id("f"), vec![int(1), int(2)]));
    assert_eq!(e, a);
}

#[test]
fn if_else_test() {
    let _ = ::env_logger::init();
    let e = parse_new!("if True then 1 else 0");
    let a = if_else(id("True"), int(1), int(0));
    assert_eq!(e, a);
}

#[test]
fn let_type_decl() {
    let _ = ::env_logger::init();
    let e = parse_new!("let f: Int = \\x y -> x + y in f 1 2");
    match e.value {
        Expr::LetBindings(bind, _) => assert_eq!(bind[0].typ, typ("Int")),
        _ => assert!(false),
    }
}
#[test]
fn let_args() {
    let _ = ::env_logger::init();
    let e = parse_new!("let f x y = y in f 2");
    assert_eq!(e,
               let_a("f", &["x", "y"], id("y"), app(id("f"), vec![int(2)])));
}

#[test]
fn type_decl_record() {
    let _ = ::env_logger::init();
    let e = parse_new!("type Test = { x: Int, y: {} } in 1");
    let record = Type::record(Vec::new(),
                              vec![field("x", typ("Int")),
                                   field("y", Type::record(vec![], vec![]))]);
    assert_eq!(e, type_decl(intern("Test"), vec![], record, int(1)));
}

#[test]
fn type_mutually_recursive() {
    let _ = ::env_logger::init();
    let e = parse_new!("type Test = | Test Int and Test2 = { x: Int, y: {} } in 1");
    let test = Type::variants(vec![(intern("Test"),
                                    Type::function(vec![typ("Int")], typ("Test")))]);
    let test2 = Type::record(Vec::new(),
                             vec![Field {
                                      name: intern("x"),
                                      typ: typ("Int"),
                                  },
                                  Field {
                                      name: intern("y"),
                                      typ: Type::record(vec![], vec![]),
                                  }]);
    let binds = vec![
        TypeBinding {
            comment: None,
            name: intern("Test"),
            alias: Alias::new(intern("Test"), Vec::new(), test),
        },
        TypeBinding {
            comment: None,
            name: intern("Test2"),
            alias: Alias::new(intern("Test2"), Vec::new(), test2),
        },
        ];
    assert_eq!(e, type_decls(binds, int(1)));
}

#[test]
fn field_access_test() {
    let _ = ::env_logger::init();
    let e = parse_new!("{ x = 1 }.x");
    assert_eq!(e,
               field_access(record(vec![(intern("x"), Some(int(1)))]), "x"));
}

#[test]
fn builtin_op() {
    let _ = ::env_logger::init();
    let e = parse_new!("x #Int+ 1");
    assert_eq!(e, binop(id("x"), "#Int+", int(1)));
}

#[test]
fn op_identifier() {
    let _ = ::env_logger::init();
    let e = parse_new!("let (==) = \\x y -> x #Int== y in (==) 1 2");
    assert_eq!(e,
               let_("==",
                    lambda("",
                           vec![intern("x"), intern("y")],
                           binop(id("x"), "#Int==", id("y"))),
                    app(id("=="), vec![int(1), int(2)])));
}
#[test]
fn variant_type() {
    let _ = ::env_logger::init();
    let e = parse_new!("type Option a = | None | Some a in Some 1");
    let option = Type::app(typ("Option"), vec![typ("a")]);
    let none = Type::function(vec![], option.clone());
    let some = Type::function(vec![typ("a")], option.clone());
    assert_eq!(e,
               type_decl(intern("Option"),
                         vec![generic("a")],
                         Type::variants(vec![(intern("None"), none), (intern("Some"), some)]),
                         app(id("Some"), vec![int(1)])));
}
#[test]
fn case_expr() {
    let _ = ::env_logger::init();
    let text = r#"
match None with
    | Some x -> x
    | None -> 0"#;
    let e = parse(text);
    assert_eq!(e,
               Ok(case(id("None"),
                       vec![(Pattern::Constructor(TypedIdent::new(intern("Some")),
                                                  vec![TypedIdent::new(intern("x"))]),
                             id("x")),
                            (Pattern::Constructor(TypedIdent::new(intern("None")), vec![]),
                             int(0))])));
}
#[test]
fn array_expr() {
    let _ = ::env_logger::init();
    let e = parse_new!("[1, a]");
    assert_eq!(e, array(vec![int(1), id("a")]));
}
#[test]
fn operator_expr() {
    let _ = ::env_logger::init();
    let e = parse_new!("test + 1 * 23 #Int- test");
    assert_eq!(e,
               binop(binop(id("test"), "+", binop(int(1), "*", int(23))),
                     "#Int-",
                     id("test")));
}

#[test]
fn record_trailing_comma() {
    let _ = ::env_logger::init();
    let e = parse_new!("{ y, x = z,}");
    assert_eq!(e,
               record(vec![("y".into(), None), ("x".into(), Some(id("z")))]));
}

#[test]
fn array_trailing_comma() {
    let _ = ::env_logger::init();
    let e = parse_new!("[y, 1, 2,]");
    assert_eq!(e, array(vec![id("y"), int(1), int(2)]));
}

#[test]
fn record_pattern() {
    let _ = ::env_logger::init();
    let e = parse_new!("match x with | { y, x = z } -> z");
    let pattern = Pattern::Record {
        typ: Type::hole(),
        types: Vec::new(),
        fields: vec![(intern("y"), None), (intern("x"), Some(intern("z")))],
    };
    assert_eq!(e, case(id("x"), vec![(pattern, id("z"))]));
}
#[test]
fn let_pattern() {
    let _ = ::env_logger::init();
    let e = parse_new!("let {x, y} = test in x");
    assert_eq!(e,
               no_loc(Expr::LetBindings(vec![ValueBinding {
                                                 comment: None,
                                                 name: no_loc(Pattern::Record {
                                                     typ: Type::hole(),
                                                     types: Vec::new(),
                                                     fields: vec![(intern("x"), None),
                                                                  (intern("y"), None)],
                                                 }),
                                                 typ: Type::hole(),
                                                 args: vec![],
                                                 expr: id("test"),
                                             }],
                                        Box::new(id("x")))));
}

#[test]
fn associated_record() {
    let _ = ::env_logger::init();
    let e = parse_new!("type Test a = { Fn, x: a } in { Fn = Int -> Array Int, Test, x = 1 }");

    let test_type = Type::record(vec![Field {
                                          name: String::from("Fn"),
                                          typ: Alias::new(String::from("Fn"), vec![], typ("Fn")),
                                      }],
                                 vec![Field {
                                          name: intern("x"),
                                          typ: typ("a"),
                                      }]);
    let fn_type = Type::function(vec![typ("Int")], Type::array(typ("Int")));
    let record = record_a(vec![(intern("Fn"), Some(fn_type)), (intern("Test"), None)],
                          vec![(intern("x"), Some(int(1)))]);
    assert_eq!(e,
               type_decl(intern("Test"), vec![generic("a")], test_type, record));
}

#[test]
fn span_identifier() {
    let _ = ::env_logger::init();

    let e = parse_new!("test");
    assert_eq!(e.span,
               Span {
                   start: BytePos::from(0),
                   end: BytePos::from(4),
               });
}


#[test]
fn span_integer() {
    let _ = ::env_logger::init();

    let e = parse_new!("1234");
    assert_eq!(e.span,
               Span {
                   start: BytePos::from(0),
                   end: BytePos::from(4),
               });
}

// FIXME The span of string literals includes the spaces after them
#[test]
#[ignore]
fn span_string_literal() {
    let _ = ::env_logger::init();

    let e = parse_new!(r#" "test" "#);
    assert_eq!(e.span,
               Span {
                   start: BytePos::from(1),
                   end: BytePos::from(7),
               });
}

#[test]
fn span_app() {
    let _ = ::env_logger::init();

    let e = parse_new!(r#" f 123 "asd""#);
    assert_eq!(e.span,
               Span {
                   start: BytePos::from(1),
                   end: BytePos::from(12),
               });
}

#[test]
fn span_match() {
    let _ = ::env_logger::init();

    let e = parse_new!(r#"
match False with
    | True -> "asd"
    | False -> ""
"#);
    assert_eq!(e.span,
               Span {
                   start: BytePos::from(1),
                   end: BytePos::from(55),
               });
}

#[test]
fn span_if_else() {
    let _ = ::env_logger::init();

    let e = parse_new!(r#"
if True then
    1
else
    123.45
"#);
    assert_eq!(e.span,
               Span {
                   start: BytePos::from(1),
                   end: BytePos::from(35),
               });
}

#[test]
fn span_byte() {
    let _ = ::env_logger::init();

    let e = parse_new!(r#"124b"#);
    assert_eq!(e.span,
               Span {
                   start: BytePos::from(0),
                   end: BytePos::from(4),
               });
}

#[test]
fn span_field_access() {
    let _ = ::env_logger::init();
    let expr = parse_new!("record.x");
    assert_eq!(expr.span,
               Span {
                   start: BytePos::from(0),
                   end: BytePos::from(8),
               });
    match expr.value {
        Expr::Projection(ref e, _, _) => {
            assert_eq!(e.span,
                       Span {
                           start: BytePos::from(0),
                           end: BytePos::from(6),
                       });
        }
        _ => panic!(),
    }
}

#[test]
fn comment_on_let() {
    let _ = ::env_logger::init();
    let text = r#"
/// The identity function
let id x = x
id
"#;
    let e = parse_new!(text);
    assert_eq!(e,
               no_loc(Expr::LetBindings(vec![ValueBinding {
                                                 comment: Some("The identity function".into()),
                                                 name: no_loc(Pattern::Ident(TypedIdent::new(intern("id")))),
                                                 typ: Type::hole(),
                                                 args: vec![TypedIdent::new(intern("x"))],
                                                 expr: id("x"),
                                             }],
                                        Box::new(id("id")))));
}

#[test]
fn comment_on_type() {
    let _ = ::env_logger::init();
    let text = r#"
/** Test type */
type Test = Int
id
"#;
    let e = parse_new!(text);
    assert_eq!(e,
               type_decls(vec![TypeBinding {
                                   comment: Some("Test type ".into()),
                                   name: intern("Test"),
                                   alias: Alias::new(intern("Test"), Vec::new(), typ("Int")),
                               }],
                          id("id")));
}

#[test]
fn comment_after_integer() {
    let _ = ::env_logger::init();
    let text = r#"
let x = 1

/** Test type */
type Test = Int
id
"#;
    let e = parse_new!(text);
    assert_eq!(e,
               let_a("x",
                     &[],
                     int(1),
                     type_decls(vec![TypeBinding {
                                         comment: Some("Test type ".into()),
                                         name: intern("Test"),
                                         alias: Alias::new(intern("Test"), Vec::new(), typ("Int")),
                                     }],
                                id("id"))));
}

#[test]
fn merge_line_comments() {
    let _ = ::env_logger::init();
    let text = r#"
/// Merge
/// consecutive
/// line comments.
type Test = Int
id
"#;
    let e = parse_new!(text);
    assert_eq!(e,
               type_decls(vec![TypeBinding {
                                   comment: Some("Merge\nconsecutive\nline comments.".into()),
                                   name: intern("Test"),
                                   alias: Alias::new(intern("Test"), Vec::new(), typ("Int")),
                               }],
                          id("id")));
}

#[test]
fn partial_field_access() {
    let _ = ::env_logger::init();
    let text = r#"test."#;
    let e = parse(text);
    assert!(e.is_err());
    assert_eq!(e.unwrap_err().0,
               Some(Spanned {
                   span: Span {
                       start: BytePos::from(0),
                       end: BytePos::from(0),
                   },
                   value: Expr::Projection(Box::new(id("test")), intern(""), Type::hole()),
               }));
}

#[test]
fn partial_field_access_in_block() {
    let _ = ::env_logger::init();
    let text = r#"
test.
test
"#;
    let e = parse(text);
    assert!(e.is_err());
    assert_eq!(e.unwrap_err().0,
               Some(Spanned {
                   span: Span {
                       start: BytePos::from(0),
                       end: BytePos::from(0),
                   },
                   value: Expr::Block(vec![Spanned {
                                               span: Span {
                                                   start: BytePos::from(0),
                                                   end: BytePos::from(0),
                                               },
                                               value: Expr::Projection(Box::new(id("test")),
                                                                       intern(""),
                                                                       Type::hole()),
                                           },
                                           id("test")]),
               }));
}

#[test]
fn function_operator_application() {
    let _ = ::env_logger::init();
    let text = r#"
let x: ((->) Int Int) = x
x
"#;
    let e = parse(text);
    assert_eq!(e,
               Ok(no_loc(Expr::LetBindings(vec![ValueBinding {
                                                    comment: None,
                                                    name: no_loc(Pattern::Ident(TypedIdent::new(intern("x")))),
                                                    typ: Type::app(typ("->"),
                                                                   vec![typ("Int"), typ("Int")]),
                                                    args: vec![],
                                                    expr: id("x"),
                                                }],
                                           Box::new(id("x"))))));
}

#[test]
fn quote_in_identifier() {
    let _ = ::env_logger::init();
    let e = parse_new!("let f' = \\x y -> x + y in f' 1 2");
    let a = let_("f'",
                 lambda("",
                        vec![intern("x"), intern("y")],
                        binop(id("x"), "+", id("y"))),
                 app(id("f'"), vec![int(1), int(2)]));
    assert_eq!(e, a);
}
