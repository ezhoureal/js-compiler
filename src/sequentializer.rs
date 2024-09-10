use crate::syntax::*;
fn parse_param_exps(
    params: &[Exp<()>],
    counter: &mut u32,
) -> (Vec<ImmExp>, Vec<(String, SeqExp<()>)>) {
    let mut let_bindings = vec![];
    let imm_params = params
        .iter()
        .map(|param| {
            let seq_param = sequentialize(param, counter);
            if let SeqExp::Imm(i, _) = seq_param {
                return i;
            }
            let var = format!("#var_{}", counter);
            let_bindings.push((var.clone(), seq_param));
            *counter += 1;
            return ImmExp::Var(var);
        })
        .collect();
    (imm_params, let_bindings)
}

fn generate_nested_let(bindings: &[(String, SeqExp<()>)], body: SeqExp<()>) -> SeqExp<()> {
    if bindings.is_empty() {
        return body;
    }
    SeqExp::Let {
        var: bindings[0].0.clone(),
        bound_exp: Box::new(bindings[0].1.clone()),
        body: Box::new(generate_nested_let(&bindings[1..], body)),
        ann: (),
    }
}

fn sequentialize(e: &Exp<()>, counter: &mut u32) -> SeqExp<()> {
    match e {
        Exp::Bool(b, _) => SeqExp::Imm(ImmExp::Bool(*b), ()),
        Exp::Num(i, _) => SeqExp::Imm(ImmExp::Num(*i), ()),
        Exp::Var(s, _) => SeqExp::Imm(ImmExp::Var(s.clone()), ()),
        Exp::Prim(p, exps, ann) => {
            let params: Vec<Exp<()>> = exps.iter().map(|exp| (*exp.clone())).collect();
            let (imm_params, let_bindings) = parse_param_exps(&params, counter);
            generate_nested_let(&let_bindings, SeqExp::Prim(*p, imm_params, ()))
        }
        Exp::Let {
            bindings,
            body,
            ann: _,
        } => {
            let mut optionRes: Option<SeqExp<()>> = None;
            for (var, exp) in bindings.iter().rev() {
                optionRes = Some(SeqExp::Let {
                    var: var.clone(),
                    bound_exp: Box::new(sequentialize(&exp, counter)),
                    body: if optionRes.is_some() {
                        Box::new(optionRes.unwrap())
                    } else {
                        Box::new(sequentialize(body, counter))
                    },
                    ann: (),
                })
            }
            optionRes.unwrap()
        }
        Exp::If {
            cond,
            thn,
            els,
            ann,
        } => {
            *counter += 1;
            let var_name = format!("#if_{}", counter);
            SeqExp::Let {
                var: var_name.clone(),
                bound_exp: Box::new(sequentialize(cond, counter)),
                body: Box::new(SeqExp::If {
                    cond: ImmExp::Var(var_name),
                    thn: Box::new(sequentialize(thn, counter)),
                    els: Box::new(sequentialize(els, counter)),
                    ann: (),
                }),
                ann: (),
            }
        }
        Exp::FunDefs { decls, body, ann } => {
            let seq_decls = decls
                .iter()
                .map(|decl| SeqFunDecl {
                    name: decl.name.clone(),
                    parameters: decl.parameters.clone(),
                    body: sequentialize(&decl.body, counter),
                    ann: (),
                })
                .collect();
            SeqExp::FunDefs {
                decls: seq_decls,
                body: Box::new(sequentialize(&body, counter)),
                ann: (),
            }
        }
        Exp::Call(func, args, _) => {
            unimplemented!("called function = {}, arg size = {}", func, args.len())
        },
        Exp::InternalTailCall(func, params, _) => {
            let (imm_params, let_bindings) = parse_param_exps(params, counter);
            generate_nested_let(
                &let_bindings,
                SeqExp::InternalTailCall(func.clone(), imm_params, ()),
            )
        }
        Exp::ExternalCall {
            fun_name,
            args,
            is_tail,
            ann,
        } => {
            let (imm_params, let_bindings) = parse_param_exps(args, counter);
            generate_nested_let(
                &let_bindings,
                SeqExp::ExternalCall {
                    fun_name: fun_name.clone(),
                    args: imm_params,
                    is_tail: is_tail.clone(),
                    ann: (),
                },
            )
        }
    }
}

pub fn seq_prog(decls: &[FunDecl<Exp<()>, ()>], p: &Exp<()>) -> SeqProg<()> {
    let mut counter = 0;
    SeqProg {
        funs: decls
            .iter()
            .map(|decl| {
                let seq_body = sequentialize(&decl.body, &mut counter);
                FunDecl {
                    name: decl.name.clone(),
                    parameters: decl.parameters.clone(),
                    body: seq_body,
                    ann: (),
                }
            })
            .collect(),
        main: sequentialize(p, &mut counter),
        ann: (),
    }
}
