use std::collections::{HashMap, HashSet};

use crate::{compile::CompileErr, syntax::*};

static I63_MAX: i64 = 0x3F_FF_FF_FF_FF_FF_FF_FF;
static I63_MIN: i64 = -0x40_00_00_00_00_00_00_00;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Symbol {
    Func(usize),
    Var,
}

pub fn check_prog<Span>(
    e: &Exp<Span>,
    symbols: &HashMap<String, Symbol>,
) -> Result<(), CompileErr<Span>>
where
    Span: Clone,
{
    match e {
        Exp::Num(i, ann) => {
            if *i > I63_MAX || *i < I63_MIN {
                return Err(CompileErr::Overflow {
                    num: *i,
                    location: ann.clone(),
                });
            }
            Ok(())
        }
        Exp::Var(name, ann) => {
            if !symbols.contains_key(name) {
                return Err(CompileErr::UnboundVariable {
                    unbound: name.clone(),
                    location: ann.clone(),
                });
            }
            Ok(())
        }
        Exp::Prim(_, exps, _) => {
            for e in exps {
                check_prog(e, symbols)?;
            }
            Ok(())
        }
        Exp::Let {
            bindings,
            body,
            ann,
        } => {
            let mut scoped_symbols = symbols.clone();
            let mut appeared = HashSet::new();
            for (name, value) in bindings {
                if appeared.contains(name) {
                    return Err(CompileErr::DuplicateBinding {
                        duplicated_name: name.clone(),
                        location: ann.clone(),
                    });
                }
                appeared.insert(name);
                scoped_symbols.insert(name.clone(), Symbol::Var);
                check_prog(value, &scoped_symbols)?;
            }
            check_prog(body, &scoped_symbols)
        }
        Exp::Bool(_, _) => Ok(()),
        Exp::If {
            cond,
            thn,
            els,
            ann,
        } => {
            check_prog(cond, symbols)?;
            check_prog(&thn, symbols)?;
            check_prog(&els, symbols)?;
            Ok(())
        }
        Exp::FunDefs { decls, body, ann } => {
            let mut scoped_symbols = symbols.clone();
            let mut mutual_funcs = HashSet::<String>::new();
            for decl in decls {
                if mutual_funcs.contains(&decl.name) {
                    return Err(CompileErr::DuplicateFunName {
                        duplicated_name: decl.name.clone(),
                        location: ann.clone(),
                    });
                }
                mutual_funcs.insert(decl.name.clone());
                scoped_symbols.insert(decl.name.clone(), Symbol::Func(decl.parameters.len()));
            }
            for decl in decls {
                for param in &decl.parameters {
                    scoped_symbols.insert(param.clone(), Symbol::Var);
                }
                check_prog(&decl.body, &scoped_symbols)?;
            }
            check_prog(body, &scoped_symbols)
        }
        Exp::Call(func, params, ann) => {
            if !symbols.contains_key(func) {
                return Err(CompileErr::UndefinedFunction {
                    undefined: func.clone(),
                    location: ann.clone(),
                });
            }
            match &symbols[func] {
                Symbol::Func(param_size) => {
                    if params.len() != *param_size {
                        return Err(CompileErr::FunctionCalledWrongArity {
                            function_name: func.clone(),
                            correct_arity: *param_size,
                            arity_used: params.len(),
                            location: ann.clone(),
                        });
                    }
                }
                Symbol::Var => {
                    return Err(CompileErr::ValueUsedAsFunction {
                        variable_name: func.clone(),
                        location: ann.clone(),
                    });
                }
            }
            for p in params {
                check_prog(p, &symbols)?;
            }
            Ok(())
        }
        Exp::InternalTailCall(_, _, _) => todo!(),
        Exp::ExternalCall {
            args,
            is_tail,
            ann,
            fun,
        } => todo!(),
    }
}
