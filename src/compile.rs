use crate::asm::instrs_to_string;
use crate::asm::{Arg32, Arg64, BinArgs, Instr, Loc, MemRef, MovArgs, Reg, Reg32};
use crate::checker;
use crate::lambda_lift::lambda_lift;
use crate::sequentializer;
use crate::syntax::{Exp, FunDecl, ImmExp, Prim, SeqExp, SeqProg, SurfFunDecl, SurfProg};

use std::collections::{HashMap, HashSet};
use std::convert::{TryFrom, TryInto};

#[derive(Debug, PartialEq, Eq)]
pub enum CompileErr<Span> {
    UnboundVariable {
        unbound: String,
        location: Span,
    },
    UndefinedFunction {
        undefined: String,
        location: Span,
    },
    // The Span here is the Span of the let-expression that has the two duplicated bindings
    DuplicateBinding {
        duplicated_name: String,
        location: Span,
    },

    Overflow {
        num: i64,
        location: Span,
    },

    DuplicateFunName {
        duplicated_name: String,
        location: Span, // the location of the 2nd function
    },

    DuplicateArgName {
        duplicated_name: String,
        location: Span,
    },
}


pub fn check_prog<Span>(p: &SurfProg<Span>) -> Result<(), CompileErr<Span>>
where
    Span: Clone,
{
    let res = checker::check_prog(p, &HashMap::new());
    res
}

// Parse Calls into either DirectCall or ClosureCall
fn eliminate_closures<Ann>(e: &Exp<Ann>) -> Exp<()> {
    panic!("NYI: uniquify")
}

// Identify which functions should be lifted to the top level
fn should_lift<Ann>(p: &Exp<Ann>) -> HashSet<String> {
    panic!("NYI: should lift")
}



// returns instruction to move imm to Rax
fn imm_to_rax(imm: &ImmExp, vars: &HashMap<String, i32>) -> Vec<Instr> {
    vec![Instr::Mov(MovArgs::ToReg(
        Reg::Rax,
        imm_to_arg64(imm, vars),
    ))]
}

static SNAKE_TRU: u64 = 0xFF_FF_FF_FF_FF_FF_FF_FF;
static SNAKE_FLS: u64 = 0x7F_FF_FF_FF_FF_FF_FF_FF;

static OVERFLOW: &str = "overflow_error";
static ARITH_ERROR: &str = "arith_error";
static CMP_ERROR: &str = "cmp_error";
static IF_ERROR: &str = "if_error";
static LOGIC_ERROR: &str = "logic_error";
static SNAKE_ERROR: &str = "snake_error";

fn imm_to_arg64(imm: &ImmExp, vars: &HashMap<String, i32>) -> Arg64 {
    match &imm {
        ImmExp::Num(i) => Arg64::Signed(*i << 1),
        ImmExp::Var(s) => Arg64::Mem(MemRef {
            reg: Reg::Rsp,
            offset: vars[s],
        }),
        ImmExp::Bool(b) => {
            if *b {
                Arg64::Unsigned(SNAKE_TRU)
            } else {
                Arg64::Unsigned(SNAKE_FLS)
            }
        }
    }
}

fn sub_for_cmp(exps: &Vec<ImmExp>, vars: &HashMap<String, i32>, reverse: bool) -> Vec<Instr> {
    let mut res = vec![];
    if reverse {
        // exps[1] - exps[0]
        res.append(&mut vec![
            Instr::Mov(MovArgs::ToReg(Reg::Rdx, imm_to_arg64(&exps[0], vars))),
            Instr::Mov(MovArgs::ToReg(Reg::Rax, imm_to_arg64(&exps[1], vars))),
        ]);
    } else {
        // exps[0] - exps[1]
        res.append(&mut vec![
            Instr::Mov(MovArgs::ToReg(Reg::Rax, imm_to_arg64(&exps[0], vars))),
            Instr::Mov(MovArgs::ToReg(Reg::Rdx, imm_to_arg64(&exps[1], vars))),
        ]);
    }
    res.append(&mut cmp_check(Reg::Rax));
    res.append(&mut cmp_check(Reg::Rdx));

    res.append(&mut vec![
        Instr::Sar(BinArgs::ToReg(Reg::Rax, Arg32::Signed(1))),
        Instr::Sar(BinArgs::ToReg(Reg::Rdx, Arg32::Signed(1))),
        Instr::Sub(BinArgs::ToReg(Reg::Rax, Arg32::Reg(Reg::Rdx))),
    ]);
    res
}

/// return instructions to convert sign in Rax to boolean value
fn is_neg() -> Vec<Instr> {
    static NEG_MASK: u64 = 0x7F_FF_FF_FF_FF_FF_FF_FF;
    vec![
        Instr::Mov(MovArgs::ToReg(Reg::Rdx, Arg64::Unsigned(NEG_MASK))),
        Instr::Or(BinArgs::ToReg(Reg::Rax, Arg32::Reg(Reg::Rdx))),
    ]
}

fn is_non_neg() -> Vec<Instr> {
    static NON_NEG_MASK: u64 = 0x80_00_00_00_00_00_00_00;
    vec![
        Instr::Mov(MovArgs::ToReg(Reg::Rdx, Arg64::Unsigned(NON_NEG_MASK))),
        Instr::Xor(BinArgs::ToReg(Reg::Rax, Arg32::Reg(Reg::Rdx))),
        Instr::Mov(MovArgs::ToReg(Reg::Rdx, Arg64::Unsigned(SNAKE_FLS))),
        Instr::Or(BinArgs::ToReg(Reg::Rax, Arg32::Reg(Reg::Rdx))),
    ]
}

fn arith_check(reg: Reg) -> Vec<Instr> {
    vec![
        Instr::Mov(MovArgs::ToReg(Reg::Rcx, Arg64::Reg(reg))),
        Instr::And(BinArgs::ToReg(Reg::Rcx, Arg32::Signed(1))),
        Instr::Cmp(BinArgs::ToReg(Reg::Rcx, Arg32::Signed(1))),
        Instr::Je(ARITH_ERROR.to_string()),
    ]
}

fn cmp_check(reg: Reg) -> Vec<Instr> {
    vec![
        Instr::Mov(MovArgs::ToReg(Reg::Rcx, Arg64::Reg(reg))),
        Instr::And(BinArgs::ToReg(Reg::Rcx, Arg32::Signed(1))),
        Instr::Cmp(BinArgs::ToReg(Reg::Rcx, Arg32::Signed(1))),
        Instr::Je(CMP_ERROR.to_string()),
    ]
}

fn logic_check(reg: Reg) -> Vec<Instr> {
    vec![
        Instr::Mov(MovArgs::ToReg(Reg::Rcx, Arg64::Reg(reg))),
        Instr::And(BinArgs::ToReg(Reg::Rcx, Arg32::Signed(1))),
        Instr::Cmp(BinArgs::ToReg(Reg::Rcx, Arg32::Signed(0))),
        Instr::Je(LOGIC_ERROR.to_string()),
    ]
}

fn if_check(reg: Reg) -> Vec<Instr> {
    vec![
        Instr::Mov(MovArgs::ToReg(Reg::Rcx, Arg64::Reg(reg))),
        Instr::And(BinArgs::ToReg(Reg::Rcx, Arg32::Signed(1))),
        Instr::Cmp(BinArgs::ToReg(Reg::Rcx, Arg32::Signed(0))),
        Instr::Je(IF_ERROR.to_string()),
    ]
}

// [vars] variable name -> offset from rsp in stack (negative number)
// [functions] function name -> stack size when function is declared
fn compile_to_instrs_inner<'a, 'b>(
    e: &'a SeqExp<()>,
    counter: &mut u32,
    stack: i32,
    vars: &'b mut HashMap<String, i32>,
    functions: &mut HashMap<String, i32>,
) -> Vec<Instr> {
    match e {
        SeqExp::Imm(exp, _) => imm_to_rax(exp, vars),
        SeqExp::Prim(p, exps, _) => {
            let mut res = imm_to_rax(&exps[0], vars);
            //
            match p {
                Prim::Add => {
                    res.append(&mut arith_check(Reg::Rax));
                    res.push(Instr::Mov(MovArgs::ToReg(
                        Reg::Rdx,
                        imm_to_arg64(&exps[1], vars),
                    )));
                    res.append(&mut arith_check(Reg::Rdx));
                    res.push(Instr::Add(BinArgs::ToReg(Reg::Rax, Arg32::Reg(Reg::Rdx))));
                    res.push(Instr::Jo(OVERFLOW.to_string()));
                }
                Prim::Sub => {
                    res.append(&mut arith_check(Reg::Rax));
                    res.push(Instr::Mov(MovArgs::ToReg(
                        Reg::Rdx,
                        imm_to_arg64(&exps[1], vars),
                    )));
                    res.append(&mut arith_check(Reg::Rdx));
                    res.push(Instr::Sub(BinArgs::ToReg(Reg::Rax, Arg32::Reg(Reg::Rdx))));
                    res.push(Instr::Jo(OVERFLOW.to_string()));
                }
                Prim::Mul => {
                    res.append(&mut arith_check(Reg::Rax));
                    res.push(Instr::Mov(MovArgs::ToReg(
                        Reg::Rdx,
                        imm_to_arg64(&exps[1], vars),
                    )));
                    res.append(&mut arith_check(Reg::Rdx));
                    res.push(Instr::Sar(BinArgs::ToReg(Reg::Rdx, Arg32::Signed(1))));
                    res.push(Instr::IMul(BinArgs::ToReg(Reg::Rax, Arg32::Reg(Reg::Rdx))));
                    res.push(Instr::Jo(OVERFLOW.to_string()));
                }
                Prim::Add1 => {
                    res.append(&mut arith_check(Reg::Rax));
                    res.push(Instr::Add(BinArgs::ToReg(Reg::Rax, Arg32::Unsigned(0x2))));
                    res.push(Instr::Jo(OVERFLOW.to_string()));
                }
                Prim::Sub1 => {
                    res.append(&mut arith_check(Reg::Rax));
                    res.push(Instr::Sub(BinArgs::ToReg(Reg::Rax, Arg32::Unsigned(0x2))));
                    res.push(Instr::Jo(OVERFLOW.to_string()));
                }
                Prim::Not => {
                    res.append(&mut logic_check(Reg::Rax));
                    static BOOL_MASK: u64 = 0x80_00_00_00_00_00_00_00;
                    res.push(Instr::Mov(MovArgs::ToReg(
                        Reg::Rdx,
                        Arg64::Unsigned(BOOL_MASK),
                    )));
                    res.push(Instr::Xor(BinArgs::ToReg(Reg::Rax, Arg32::Reg(Reg::Rdx))));
                }
                Prim::Print => {
                    res = vec![
                        Instr::Mov(MovArgs::ToReg(Reg::Rdi, imm_to_arg64(&exps[0], vars))),
                        Instr::Sub(BinArgs::ToReg(
                            Reg::Rsp,
                            Arg32::Signed(align_stack(stack) + 8),
                        )),
                        Instr::Call("print_snake_val".to_string()),
                        Instr::Add(BinArgs::ToReg(
                            Reg::Rsp,
                            Arg32::Signed(align_stack(stack) + 8),
                        )),
                    ];
                }
                Prim::IsBool => {
                    res.push(Instr::Mov(MovArgs::ToReg(
                        Reg::Rdx,
                        Arg64::Unsigned(SNAKE_FLS),
                    )));
                    res.push(Instr::And(BinArgs::ToReg(Reg::Rax, Arg32::Reg(Reg::Rdx))));
                    res.push(Instr::Shl(BinArgs::ToReg(Reg::Rax, Arg32::Signed(63))));
                    res.push(Instr::Or(BinArgs::ToReg(Reg::Rax, Arg32::Reg(Reg::Rdx))));
                }
                Prim::IsNum => {
                    res.push(Instr::Mov(MovArgs::ToReg(
                        Reg::Rdx,
                        Arg64::Unsigned(SNAKE_FLS),
                    )));
                    res.push(Instr::Xor(BinArgs::ToReg(Reg::Rax, Arg32::Reg(Reg::Rdx))));
                    res.push(Instr::Shl(BinArgs::ToReg(Reg::Rax, Arg32::Signed(63))));
                    res.push(Instr::Or(BinArgs::ToReg(Reg::Rax, Arg32::Reg(Reg::Rdx))));
                }
                Prim::And => {
                    res.append(&mut logic_check(Reg::Rax));
                    res.push(Instr::Mov(MovArgs::ToReg(
                        Reg::Rdx,
                        imm_to_arg64(&exps[1], vars),
                    )));
                    res.append(&mut logic_check(Reg::Rdx));
                    res.push(Instr::And(BinArgs::ToReg(Reg::Rax, Arg32::Reg(Reg::Rdx))));
                }
                Prim::Or => {
                    res.append(&mut logic_check(Reg::Rax));
                    res.push(Instr::Mov(MovArgs::ToReg(
                        Reg::Rdx,
                        imm_to_arg64(&exps[1], vars),
                    )));
                    res.append(&mut logic_check(Reg::Rdx));
                    res.push(Instr::Or(BinArgs::ToReg(Reg::Rax, Arg32::Reg(Reg::Rdx))));
                }
                Prim::Lt => {
                    res.append(&mut sub_for_cmp(exps, vars, false));
                    res.append(&mut is_neg());
                }
                Prim::Gt => {
                    res.append(&mut sub_for_cmp(exps, vars, true));
                    res.append(&mut is_neg());
                }
                Prim::Le => {
                    res.append(&mut sub_for_cmp(exps, vars, true));
                    res.append(&mut is_non_neg());
                }
                Prim::Ge => {
                    res.append(&mut sub_for_cmp(exps, vars, false));
                    res.append(&mut is_non_neg());
                }
                Prim::Eq => {
                    res.push(Instr::Mov(MovArgs::ToReg(
                        Reg::Rdx,
                        imm_to_arg64(&exps[1], vars),
                    )));
                    *counter += 1;
                    let fls_label = format!("false_{}", counter);
                    let done_label = format!("cmp_done_{}", counter);
                    res.append(&mut vec![
                        Instr::Cmp(BinArgs::ToReg(Reg::Rax, Arg32::Reg(Reg::Rdx))),
                        Instr::Jne(fls_label.clone()),
                        Instr::Mov(MovArgs::ToReg(Reg::Rax, Arg64::Unsigned(SNAKE_TRU))),
                        Instr::Jmp(done_label.clone()),
                        Instr::Label(fls_label),
                        Instr::Mov(MovArgs::ToReg(Reg::Rax, Arg64::Unsigned(SNAKE_FLS))),
                        Instr::Label(done_label),
                    ]);
                }
                Prim::Neq => {
                    res.push(Instr::Mov(MovArgs::ToReg(
                        Reg::Rdx,
                        imm_to_arg64(&exps[1], vars),
                    )));
                    *counter += 1;
                    let fls_label = format!("false_{}", counter);
                    let done_label = format!("cmp_done_{}", counter);
                    res.append(&mut vec![
                        Instr::Cmp(BinArgs::ToReg(Reg::Rax, Arg32::Reg(Reg::Rdx))),
                        Instr::Je(fls_label.clone()),
                        Instr::Mov(MovArgs::ToReg(Reg::Rax, Arg64::Unsigned(SNAKE_TRU))),
                        Instr::Jmp(done_label.clone()),
                        Instr::Label(fls_label),
                        Instr::Mov(MovArgs::ToReg(Reg::Rax, Arg64::Unsigned(SNAKE_FLS))),
                        Instr::Label(done_label),
                    ]);
                }
            }
            res
        }
        SeqExp::Let {
            var,
            bound_exp,
            body,
            ann,
        } => {
            let mut res = compile_to_instrs_inner(&bound_exp, counter, stack, vars, functions);
            let offset: i32 = ((stack + 1) * -8).try_into().unwrap();
            res.push(Instr::Mov(MovArgs::ToMem(
                MemRef {
                    reg: Reg::Rsp,
                    offset: offset,
                },
                Reg32::Reg(Reg::Rax),
            )));
            vars.insert(var.clone(), offset);

            res.append(&mut compile_to_instrs_inner(
                &body,
                counter,
                stack + 1,
                vars,
                functions,
            ));
            res
        }
        SeqExp::If {
            cond,
            thn,
            els,
            ann,
        } => {
            let mut res = imm_to_rax(cond, vars);
            res.append(&mut if_check(Reg::Rax));
            *counter += 1;
            let els_label = format!("else_{}", counter);
            let done_label = format!("done_{}", counter);
            res.append(&mut vec![
                Instr::Mov(MovArgs::ToReg(Reg::Rdx, Arg64::Unsigned(SNAKE_FLS))),
                Instr::Cmp(BinArgs::ToReg(Reg::Rax, Arg32::Reg(Reg::Rdx))),
                Instr::Je(els_label.clone()),
            ]);

            res.append(&mut compile_to_instrs_inner(
                thn,
                counter,
                stack,
                &mut vars.clone(),
                functions,
            ));
            res.push(Instr::Jmp(done_label.clone()));

            res.push(Instr::Label(els_label));
            res.append(&mut compile_to_instrs_inner(
                els, counter, stack, vars, functions,
            ));
            res.push(Instr::Label(done_label));
            res
        }
        SeqExp::FunDefs { decls, body, ann } => {
            // locally defined functions
            *counter += 1;
            let body_label = format!("body_{}", counter);
            let mut res = vec![Instr::Jmp(body_label.clone())];
            for decl in decls {
                functions.insert(decl.name.clone(), stack);
                push_params(stack, vars, &decl.parameters);
                res.push(Instr::Label(format!("func_{}", decl.name.clone())));
                res.extend(compile_to_instrs_inner(
                    &decl.body,
                    counter,
                    i32::try_from(decl.parameters.len()).unwrap(),
                    vars,
                    functions,
                ));
                res.push(Instr::Ret);
            }
            res.push(Instr::Label(body_label));
            res.extend(compile_to_instrs_inner(
                &body, counter, stack, vars, functions,
            ));
            res
        }
        SeqExp::InternalTailCall(func, args, _) => {
            return compile_tail_call(func.clone(), args, stack, functions[func], vars);
        }
        SeqExp::ExternalCall {
            fun_name,
            args,
            is_tail,
            ann,
        } => {
            if *is_tail {
                return compile_tail_call(fun_name.clone(), args, stack, 0, vars);
            }
            let mut res = vec![];
            let stack_top = align_stack(stack);
            // record called function's parameters to [stack]
            let mut offset = 16; // extra 8 is return address alloc
            for arg in args {
                res.push(Instr::Mov(MovArgs::ToReg(
                    Reg::Rax,
                    imm_to_arg64(arg, vars),
                )));
                res.push(Instr::Mov(MovArgs::ToMem(
                    MemRef {
                        reg: Reg::Rsp,
                        offset: -(stack_top + offset),
                    },
                    Reg32::Reg(Reg::Rax),
                )));
                offset += 8;
            }
            res.push(Instr::Sub(BinArgs::ToReg(
                Reg::Rsp,
                Arg32::Signed(stack_top),
            )));
            res.push(Instr::Call(format!("func_{}", fun_name)));
            res.push(Instr::Add(BinArgs::ToReg(
                Reg::Rsp,
                Arg32::Signed(stack_top),
            )));
            res
        }
    }
}

fn compile_tail_call(
    func: String,
    args: &[ImmExp],
    stack: i32,
    decl_stack: i32, // stack size when the called function is declared
    vars: &HashMap<String, i32>,
) -> Vec<Instr> {
    let mut res = vec![];
    // overwrite current stack with function arguments
    // need to save variables to lower stack addresses to avoid overwriting them
    let mut var_args = HashMap::<String, i32>::new();
    for arg in args {
        if let ImmExp::Var(v) = arg {
            if var_args.contains_key(v) {
                continue;
            }
            var_args.insert(
                v.clone(),
                8 * (i32::try_from(var_args.len()).unwrap() + stack + 1),
            );
            res.push(Instr::Mov(MovArgs::ToReg(
                Reg::Rax,
                imm_to_arg64(arg, &vars),
            )));
            res.push(Instr::Mov(MovArgs::ToMem(
                MemRef {
                    reg: Reg::Rsp,
                    offset: var_args[v],
                },
                Reg32::Reg(Reg::Rax),
            )));
        }
    }
    for (i, arg) in args.iter().enumerate() {
        let offset: i32 = -8 * (i32::try_from(i).unwrap() + decl_stack + 1);
        if let ImmExp::Var(v) = arg {
            res.push(Instr::Mov(MovArgs::ToReg(
                Reg::Rax,
                Arg64::Mem(MemRef {
                    reg: Reg::Rsp,
                    offset: var_args[v],
                }),
            )));
            res.push(Instr::Mov(MovArgs::ToMem(
                MemRef {
                    reg: Reg::Rsp,
                    offset: offset,
                },
                Reg32::Reg(Reg::Rax),
            )));
            continue;
        }
        res.push(Instr::Mov(MovArgs::ToReg(
            Reg::Rax,
            imm_to_arg64(arg, vars),
        )));
        res.push(Instr::Mov(MovArgs::ToMem(
            MemRef {
                reg: Reg::Rsp,
                offset: offset,
            },
            Reg32::Reg(Reg::Rax),
        )));
    }
    res.push(Instr::Jmp(format!("func_{}", func)));
    res
}

/* Feel free to add any helper functions you need */
fn compile_to_instrs(e: &SeqExp<()>, counter: &mut u32) -> Vec<Instr> {
    let mut is = compile_to_instrs_inner(e, counter, 0, &mut HashMap::new(), &mut HashMap::new());
    is.push(Instr::Ret);
    is
}

fn compile_func_to_instr(f: &FunDecl<SeqExp<()>, ()>, counter: &mut u32) -> Vec<Instr> {
    let mut is = vec![Instr::Label(format!("func_{}", f.name))];
    let mut vars = HashMap::<String, i32>::new();
    push_params(0, &mut vars, &f.parameters);
    is.extend(compile_to_instrs_inner(
        &f.body,
        counter,
        f.parameters.len().try_into().unwrap(),
        &mut vars,
        &mut HashMap::new(),
    ));
    is.push(Instr::Ret);
    is
}

fn align_stack(mut stack: i32) -> i32 {
    // internal SNAKE calls requires even stack
    // Therefore, return odd variables alloc + 1 return address alloc
    if stack % 2 == 0 {
        stack += 1;
    }
    stack *= 8;
    stack
}

fn push_params(stack: i32, vars: &mut HashMap<String, i32>, params: &[String]) {
    for (i, param) in params.iter().enumerate() {
        vars.insert(param.clone(), (i32::try_from(i).unwrap() + stack + 1) * -8);
    }
}

fn error_handle_instr() -> Vec<Instr> {
    vec![
        Instr::Label(ARITH_ERROR.to_string()),
        Instr::Mov(MovArgs::ToReg(Reg::Rdi, Arg64::Signed(0))),
        Instr::Mov(MovArgs::ToReg(Reg::Rsi, Arg64::Reg(Reg::Rax))),
        Instr::Call(SNAKE_ERROR.to_string()),
        Instr::Label(CMP_ERROR.to_string()),
        Instr::Mov(MovArgs::ToReg(Reg::Rdi, Arg64::Signed(1))),
        Instr::Mov(MovArgs::ToReg(Reg::Rsi, Arg64::Reg(Reg::Rax))),
        Instr::Call(SNAKE_ERROR.to_string()),
        Instr::Label(OVERFLOW.to_string()),
        Instr::Mov(MovArgs::ToReg(Reg::Rdi, Arg64::Signed(2))),
        Instr::Mov(MovArgs::ToReg(Reg::Rsi, Arg64::Reg(Reg::Rax))),
        Instr::Call(SNAKE_ERROR.to_string()),
        Instr::Label(IF_ERROR.to_string()),
        Instr::Mov(MovArgs::ToReg(Reg::Rdi, Arg64::Signed(3))),
        Instr::Mov(MovArgs::ToReg(Reg::Rsi, Arg64::Reg(Reg::Rax))),
        Instr::Call(SNAKE_ERROR.to_string()),
        Instr::Label(LOGIC_ERROR.to_string()),
        Instr::Mov(MovArgs::ToReg(Reg::Rdi, Arg64::Signed(4))),
        Instr::Mov(MovArgs::ToReg(Reg::Rsi, Arg64::Reg(Reg::Rax))),
        Instr::Call(SNAKE_ERROR.to_string()),
    ]
}

pub fn compile_to_string<Span>(p: &SurfProg<Span>) -> Result<String, CompileErr<Span>>
where
    Span: Clone,
{
    checker::check_prog(p, &HashMap::new())?;
    let (global_functions, main) = lambda_lift(&p);
    println!("global function size = {}", global_functions.len());
    let program = sequentializer::seq_prog(&global_functions, &main);

    let mut counter: u32 = 0;
    let functions_is: String = program
        .funs
        .iter()
        .map(|f| instrs_to_string(&compile_func_to_instr(&f, &mut counter)))
        .collect();
    let main_is = instrs_to_string(&compile_to_instrs(&program.main, &mut counter));

    let res = format!(
        "\
        section .text
        global start_here
        extern snake_error
        extern print_snake_val
{}
{}
start_here:
        call main
        ret
main:
{}
",
        instrs_to_string(&error_handle_instr()),
        functions_is,
        main_is
    );
    println!("{}", res);
    Ok(res)
}

