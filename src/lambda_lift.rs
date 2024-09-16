use std::{
    collections::{HashMap, HashSet},
    iter::FromIterator,
    os::unix::fs::DirBuilderExt,
};

use crate::syntax::*;

fn uniquify<Span>(e: &Exp<Span>, mapping: &HashMap<String, String>, counter: &mut u32) -> Exp<()> {
    match e {
        Exp::Let {
            bindings,
            body,
            ann,
        } => {
            let mut scoped_mapping = mapping.clone();
            let mut_bind = bindings
                .iter()
                .map(|(var, value)| {
                    *counter += 1;
                    let new_var = format!("{}", counter);
                    let mut_exp = uniquify(value, &scoped_mapping, counter);
                    scoped_mapping.insert(var.to_string(), new_var.clone());
                    return (new_var, mut_exp);
                })
                .collect();
            Exp::Let {
                bindings: mut_bind,
                body: Box::new(uniquify(&body, &scoped_mapping, counter)),
                ann: (),
            }
        }
        Exp::FunDefs { decls, body, ann } => {
            let mut scoped_mapping = mapping.clone();
            for decl in decls {
                *counter += 1;
                scoped_mapping.insert(decl.name.to_string(), format!("{}", counter));
            }
            let mut uniq_decls = vec![];
            for decl in decls {
                let mut func_scope_map = scoped_mapping.clone();
                for param in &decl.parameters {
                    *counter += 1;
                    func_scope_map.insert(param.to_string(), format!("{}", counter));
                }
                uniq_decls.push(FunDecl {
                    name: scoped_mapping[&decl.name].clone(),
                    parameters: decl
                        .parameters
                        .iter()
                        .map(|param| func_scope_map[param].clone())
                        .collect(),
                    body: uniquify(&decl.body, &func_scope_map, counter),
                    ann: (),
                })
            }
            Exp::FunDefs {
                decls: uniq_decls,
                body: Box::new(uniquify(&body, &scoped_mapping, counter)),
                ann: (),
            }
        }
        Exp::Var(v, _) => Exp::Var(mapping[v].clone(), ()),
        Exp::Num(i, _) => Exp::Num(*i, ()),
        Exp::Bool(b, _) => Exp::Bool(*b, ()),
        Exp::Prim(op, subjects, _) => {
            let uniq_sub = subjects
                .iter()
                .map(|s| Box::new(uniquify(s, mapping, counter)))
                .collect();
            Exp::Prim(*op, uniq_sub, ())
        }
        Exp::If {
            cond,
            thn,
            els,
            ann: _,
        } => Exp::If {
            cond: Box::new(uniquify(&cond, mapping, counter)),
            thn: Box::new(uniquify(&thn, mapping, counter)),
            els: Box::new(uniquify(&els, mapping, counter)),
            ann: (),
        },
        Exp::Call(func, params, _) => Exp::Call(
            Box::new(uniquify(&func, mapping, counter)),
            params
                .iter()
                .map(|param| uniquify(param, mapping, counter))
                .collect(),
            (),
        ),
        Exp::InternalTailCall(_, _, _) => todo!(),
        Exp::ExternalCall {
            args: _,
            is_tail,
            ann: _,
            fun,
        } => todo!(),
        Exp::Semicolon { e1, e2, ann } => {
            *counter += 1;
            Exp::Let {
                bindings: vec![(counter.to_string(), uniquify(e1, mapping, counter))],
                body: Box::new(uniquify(e2, mapping, counter)),
                ann: (),
            }
        }
        Exp::Lambda {
            parameters,
            body,
            ann,
        } => {
            let mut scoped_mapping = mapping.clone();
            for param in parameters {
                *counter += 1;
                scoped_mapping.insert(param.to_string(), format!("{}", counter));
            }
            Exp::Lambda {
                parameters: parameters.clone(),
                body: Box::new(uniquify(&body, &scoped_mapping, counter)),
                ann: (),
            }
        }
        Exp::MakeClosure {
            arity,
            label,
            env,
            ann,
        } => todo!(),
        Exp::ClosureCall(_, _, _) => todo!(),
        Exp::DirectCall(_, _, _) => todo!(),
    }
}

fn rewrite_call_params(
    e: &Exp<()>,
    globals: &HashMap<String, FunDecl<Exp<()>, ()>>,
    is_tail: bool,
) -> Exp<()> {
    match e {
        Exp::Prim(p, exps, _) => Exp::Prim(
            *p,
            exps.iter()
                .map(|exp| Box::new(rewrite_call_params(exp, globals, false)))
                .collect(),
            (),
        ),
        Exp::Let {
            bindings,
            body,
            ann,
        } => Exp::Let {
            bindings: bindings
                .iter()
                .map(|bind| (bind.0.clone(), rewrite_call_params(&bind.1, globals, false)))
                .collect(),
            body: Box::new(rewrite_call_params(body, globals, is_tail)),
            ann: (),
        },
        Exp::If {
            cond,
            thn,
            els,
            ann,
        } => Exp::If {
            cond: Box::new(rewrite_call_params(cond, globals, false)),
            thn: Box::new(rewrite_call_params(thn, globals, is_tail)),
            els: Box::new(rewrite_call_params(els, globals, is_tail)),
            ann: (),
        },
        Exp::FunDefs { decls, body, ann } => Exp::FunDefs {
            decls: decls
                .iter()
                .map(|decl| FunDecl {
                    name: decl.name.clone(),
                    parameters: decl.parameters.clone(),
                    body: rewrite_call_params(&decl.body, globals, true),
                    ann: (),
                })
                .collect(),
            body: Box::new(rewrite_call_params(body, globals, is_tail)),
            ann: (),
        },
        Exp::DirectCall(func, params, _) => {
            let mut mod_params: Vec<_> = params
                .iter()
                .map(|param| rewrite_call_params(param, globals, false))
                .collect();
            if !globals.contains_key(func) {
                assert!(is_tail);
                return Exp::InternalTailCall(func.clone(), mod_params, ());
            }

            println!("return external call from call, isTail = {}", is_tail);
            for p in globals[func].parameters.iter().skip(params.len()) {
                mod_params.push(Exp::Var(p.clone(), ()))
            }
            Exp::ExternalCall {
                fun: VarOrLabel::Label(func.to_string()),
                args: mod_params,
                is_tail: is_tail,
                ann: (),
            }
        }
        Exp::ClosureCall(func, args, _) => Exp::Let {
            bindings: vec![
                ("#lambda".to_string(), *func.clone()),
                (
                    "#untagged".to_string(),
                    Exp::Prim(
                        Prim::CheckArityAndUntag(args.len()),
                        vec![Box::new(Exp::Var("#lambda".to_string(), ()))],
                        (),
                    ),
                ),
                (
                    "#code_ptr".to_string(),
                    Exp::Prim(
                        Prim::GetCode,
                        vec![Box::new(Exp::Var("#untagged".to_string(), ()))],
                        (),
                    ),
                ),
                (
                    "#env".to_string(),
                    Exp::Prim(
                        Prim::GetEnv,
                        vec![Box::new(Exp::Var("#untagged".to_string(), ()))],
                        (),
                    ),
                ),
            ],
            body: Box::new(Exp::ExternalCall {
                fun: VarOrLabel::Var("#code_ptr".to_string()),
                args: args
                    .iter()
                    .map(|arg| rewrite_call_params(arg, globals, false))
                    .chain(std::iter::once(Exp::Var("#env".to_string(), ())))
                    .collect(),
                is_tail,
                ann: (),
            }),
            ann: (),
        },
        Exp::Lambda {
            parameters,
            body,
            ann,
        } => panic!(),
        Exp::MakeClosure {
            arity,
            label,
            env,
            ann,
        } => e.clone(),
        Exp::InternalTailCall(_, _, _) => todo!(),
        Exp::ExternalCall {
            fun,
            args,
            is_tail,
            ann,
        } => todo!(),
        Exp::Call(_, _, _) => todo!(),
        Exp::Semicolon { e1, e2, ann } => todo!(),
        _ => e.clone(),
    }
}

fn lift_functions(
    e: &Exp<()>,
    vars: &HashSet<String>,
    globals: &mut HashMap<String, FunDecl<Exp<()>, ()>>,
    need_lift: &HashSet<String>,
) -> Exp<()> {
    match e {
        Exp::Var(v, _) => {
            if globals.contains_key(v) {
                return Exp::MakeClosure {
                    arity: globals[v].parameters.len(),
                    label: globals[v].name.clone(),
                    env: Box::new(Exp::Prim(Prim::MakeArray, vec![], ())),
                    ann: (),
                };
            }
            return e.clone();
        }
        Exp::Prim(p, exps, _) => {
            let mut new_exps = vec![];
            for exp in exps {
                new_exps.push(Box::new(lift_functions(&exp, vars, globals, need_lift)));
            }
            Exp::Prim(*p, new_exps, ())
        }
        Exp::Let {
            bindings,
            body,
            ann,
        } => {
            let mut scoped_vars = vars.clone();
            Exp::Let {
                bindings: bindings
                    .iter()
                    .map(|bind| {
                        scoped_vars.insert(bind.0.clone());
                        let new_bind = lift_functions(&bind.1, &scoped_vars, globals, need_lift);
                        (bind.0.clone(), new_bind)
                    })
                    .collect(),
                body: Box::new(lift_functions(&body, &scoped_vars, globals, need_lift)),
                ann: (),
            }
        }
        Exp::If {
            cond,
            thn,
            els,
            ann,
        } => Exp::If {
            cond: Box::new(lift_functions(&cond, vars, globals, need_lift)),
            thn: Box::new(lift_functions(&thn, vars, globals, need_lift)),
            els: Box::new(lift_functions(&els, vars, globals, need_lift)),
            ann: (),
        },
        Exp::FunDefs { decls, body, ann } => {
            let mut new_local = vec![];
            for decl in decls {
                let mut new_decl = FunDecl {
                    name: decl.name.clone(),
                    parameters: decl.parameters.clone(),
                    body: lift_functions(&decl.body, vars, globals, need_lift),
                    ann: (),
                };
                if !need_lift.contains(&decl.name) {
                    println!("{} doesn't need lift", decl.name);
                    new_local.push(new_decl);
                    continue;
                }
                new_decl.parameters =
                    [decl.parameters.clone(), Vec::from_iter(vars.clone())].concat();
                globals.insert(decl.name.clone(), new_decl);
            }
            let new_bod = lift_functions(&body, vars, globals, need_lift);
            if !new_local.is_empty() {
                return Exp::FunDefs {
                    decls: new_local,
                    body: Box::new(new_bod),
                    ann: (),
                };
            }
            new_bod
        }
        Exp::DirectCall(func, params, _) => {
            let new_params = params
                .iter()
                .map(|param| lift_functions(param, vars, globals, need_lift))
                .collect();
            Exp::DirectCall(func.clone(), new_params, ())
        }
        Exp::ClosureCall(exp, params, _) => Exp::ClosureCall(
            Box::new(lift_functions(exp, vars, globals, need_lift)),
            params
                .iter()
                .map(|param| lift_functions(param, vars, globals, need_lift))
                .collect(),
            (),
        ),
        Exp::Lambda {
            parameters,
            body,
            ann,
        } => {
            let mut env_bindings = vec![];
            for (i, v) in vars.iter().enumerate() {
                env_bindings.push((
                    v.clone(),
                    Exp::Prim(
                        Prim::ArrayGet,
                        vec![
                            Box::new(Exp::Var("#env".to_string(), ())),
                            Box::new(Exp::Num(i as i64, ())),
                        ],
                        (),
                    ),
                ));
            }
            let decl = FunDecl {
                // use globals.len() as counter
                name: format!("lambda_{}", globals.len()),
                parameters: [parameters.clone(), vec!["#env".to_string()]].concat(),
                body: Exp::Let {
                    bindings: env_bindings,
                    body: Box::new(lift_functions(body, vars, globals, need_lift)),
                    ann: (),
                },
                ann: (),
            };
            globals.insert(decl.name.clone(), decl.clone());
            Exp::MakeClosure {
                arity: parameters.len(),
                label: decl.name.clone(),
                env: Box::new(Exp::Prim(
                    Prim::MakeArray,
                    vars.iter()
                        .map(|v| Box::new(Exp::Var(v.clone(), ())))
                        .collect(),
                    (),
                )),
                ann: (),
            }
        }
        Exp::MakeClosure {
            arity,
            label,
            env,
            ann,
        } => todo!(),
        Exp::InternalTailCall(_, _, _) => todo!(),
        Exp::ExternalCall {
            fun,
            args,
            is_tail,
            ann,
        } => todo!(),
        Exp::Call(_, _, _) => todo!(),
        Exp::Semicolon { e1, e2, ann } => todo!(),
        _ => e.clone(),
    }
}

// returns name of functions to lift
fn should_lift(p: &Exp<()>, funcs: &HashSet<String>, is_tail: bool) -> HashSet<String> {
    let mut set = HashSet::new();
    match p {
        Exp::Var(s, _) => {
            if funcs.contains(s) {
                set.insert(s.clone());
            }
        }
        Exp::Prim(_, exps, _) => {
            for exp in exps {
                set.extend(should_lift(exp, funcs, false));
            }
        }
        Exp::Let {
            bindings,
            body,
            ann,
        } => {
            for (v, bind) in bindings {
                set.extend(should_lift(bind, funcs, false));
            }
            set.extend(should_lift(body, funcs, is_tail));
        }
        Exp::If {
            cond,
            thn,
            els,
            ann,
        } => {
            set.extend(should_lift(cond, funcs, false));
            set.extend(should_lift(thn, funcs, is_tail));
            set.extend(should_lift(els, funcs, is_tail));
        }
        Exp::FunDefs { decls, body, ann } => {
            let mut scoped_funcs = funcs.clone();
            for decl in decls {
                scoped_funcs.insert(decl.name.clone());
            }
            for decl in decls {
                set.extend(should_lift(&decl.body, &scoped_funcs, true));
            }
            set.extend(should_lift(body, &scoped_funcs, is_tail));
        }
        Exp::DirectCall(func, args, _) => {
            if !is_tail {
                set.extend(funcs.clone());
            }
            for arg in args {
                set.extend(should_lift(arg, funcs, false));
            }
        }
        Exp::ClosureCall(func, params, _) => {
            set.extend(should_lift(func, funcs, false));
            for param in params {
                set.extend(should_lift(param, funcs, false));
            }
        }
        Exp::Lambda {
            parameters,
            body,
            ann,
        } => {
            set.extend(should_lift(body, funcs, true));
        }
        Exp::MakeClosure {
            arity,
            label,
            env,
            ann,
        } => todo!(),
        Exp::Call(_, _, _) => todo!(),
        Exp::InternalTailCall(_, _, _) => todo!(),
        Exp::ExternalCall {
            fun,
            args,
            is_tail,
            ann,
        } => todo!(),
        _ => (),
    }
    set
}

fn eliminate_closures(e: &Exp<()>, funcs: &HashSet<String>) -> Exp<()> {
    match e {
        Exp::Num(_, _) => e.clone(),
        Exp::Bool(_, _) => e.clone(),
        Exp::Var(_, _) => e.clone(),
        Exp::Prim(p, exps, _) => Exp::Prim(
            p.clone(),
            exps.iter()
                .map(|exp| Box::new(eliminate_closures(exp, funcs)))
                .collect(),
            (),
        ),
        Exp::Let {
            bindings,
            body,
            ann,
        } => Exp::Let {
            bindings: bindings
                .iter()
                .map(|bind| (bind.0.clone(), eliminate_closures(&bind.1, funcs)))
                .collect(),
            body: Box::new(eliminate_closures(&body, funcs)),
            ann: (),
        },
        Exp::If {
            cond,
            thn,
            els,
            ann,
        } => Exp::If {
            cond: Box::new(eliminate_closures(&cond, funcs)),
            thn: Box::new(eliminate_closures(&thn, funcs)),
            els: Box::new(eliminate_closures(els, funcs)),
            ann: (),
        },
        Exp::Semicolon { e1, e2, ann } => todo!(), // already eliminated
        Exp::FunDefs { decls, body, ann } => {
            let mut scoped_funcs = funcs.clone();
            for decl in decls {
                scoped_funcs.insert(decl.name.clone());
            }
            Exp::FunDefs {
                decls: decls
                    .iter()
                    .map(|decl| FunDecl {
                        name: decl.name.clone(),
                        parameters: decl.parameters.clone(),
                        body: eliminate_closures(&decl.body, &scoped_funcs),
                        ann: (),
                    })
                    .collect(),
                body: Box::new(eliminate_closures(body, &scoped_funcs)),
                ann: (),
            }
        }
        Exp::Lambda {
            parameters,
            body,
            ann,
        } => Exp::Lambda {
            parameters: parameters.clone(),
            body: Box::new(eliminate_closures(&body, funcs)),
            ann: (),
        },
        Exp::MakeClosure {
            arity,
            label,
            env,
            ann,
        } => todo!(),
        Exp::Call(v, params, _) => {
            let new_params = params
                .iter()
                .map(|param| eliminate_closures(param, funcs))
                .collect();
            if let Exp::Var(func, _) = *v.clone() {
                if funcs.contains(&func) {
                    return Exp::DirectCall(func, new_params, ());
                }
            }
            Exp::ClosureCall(Box::new(eliminate_closures(&v, funcs)), new_params, ())
        }
        Exp::ClosureCall(_, _, _) => todo!(),
        Exp::DirectCall(_, _, _) => todo!(),
        Exp::InternalTailCall(_, _, _) => todo!(),
        Exp::ExternalCall {
            fun,
            args,
            is_tail,
            ann,
        } => todo!(),
    }
}

// Lift some functions to global definitions
pub fn lambda_lift<Ann>(p: &Exp<Ann>) -> (Vec<FunDecl<Exp<()>, ()>>, Exp<()>) {
    let mut unique_p = uniquify(&p, &mut HashMap::new(), &mut 0);
    println!("after uniquify: {:#?}", unique_p);
    unique_p = eliminate_closures(&unique_p, &HashSet::new());
    let mut globals = HashMap::new();
    let to_lift = should_lift(&unique_p, &HashSet::new(), true);
    println!(
        "should lift len = {}, content = {:?}",
        to_lift.len(),
        to_lift
    );
    let main = lift_functions(&unique_p, &HashSet::new(), &mut globals, &to_lift);
    (
        globals
            .values()
            .map(|decl| FunDecl {
                name: decl.name.clone(),
                parameters: decl.parameters.clone(),
                body: rewrite_call_params(&decl.body, &globals, true),
                ann: (),
            })
            .collect(),
        rewrite_call_params(&main, &globals, true),
    )
    // TODO: add parameter optimization pass
}
