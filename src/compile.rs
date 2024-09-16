use crate::asm::{instrs_to_string, JmpArg, Offset};
use crate::asm::{Arg32, Arg64, BinArgs, Instr, MemRef, MovArgs, Reg, Reg32};
use crate::checker;
use crate::error_handler::*;
use crate::lambda_lift::lambda_lift;
use crate::sequentializer;
use crate::syntax::{
    Exp, FunDecl, ImmExp, Prim, SeqExp, SeqProg, SurfFunDecl, SurfProg, VarOrLabel,
};

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
    let res = checker::check_prog(p, &HashSet::new());
    res
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
static TYPE_MASK: u32 = 0b111;

fn imm_to_arg64(imm: &ImmExp, vars: &HashMap<String, i32>) -> Arg64 {
    match &imm {
        ImmExp::Num(i) => Arg64::Signed(*i << 1),
        ImmExp::Var(s) => Arg64::Mem(MemRef {
            reg: Reg::Rsp,
            offset: Offset::Constant(vars[s]),
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
        res.extend(vec![
            Instr::Mov(MovArgs::ToReg(Reg::Rdx, imm_to_arg64(&exps[0], vars))),
            Instr::Mov(MovArgs::ToReg(Reg::Rax, imm_to_arg64(&exps[1], vars))),
        ]);
    } else {
        // exps[0] - exps[1]
        res.extend(vec![
            Instr::Mov(MovArgs::ToReg(Reg::Rax, imm_to_arg64(&exps[0], vars))),
            Instr::Mov(MovArgs::ToReg(Reg::Rdx, imm_to_arg64(&exps[1], vars))),
        ]);
    }
    res.extend(cmp_check(Reg::Rax));
    res.extend(cmp_check(Reg::Rdx));

    res.extend(vec![
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
        Instr::Je(JmpArg::Label(ARITH_ERROR.to_string())),
    ]
}

fn cmp_check(reg: Reg) -> Vec<Instr> {
    vec![
        Instr::Mov(MovArgs::ToReg(Reg::Rcx, Arg64::Reg(reg))),
        Instr::And(BinArgs::ToReg(Reg::Rcx, Arg32::Signed(1))),
        Instr::Cmp(BinArgs::ToReg(Reg::Rcx, Arg32::Signed(1))),
        Instr::Je(JmpArg::Label(CMP_ERROR.to_string())),
    ]
}

fn logic_check(reg: Reg) -> Vec<Instr> {
    vec![
        Instr::Mov(MovArgs::ToReg(Reg::Rcx, Arg64::Reg(reg))),
        Instr::And(BinArgs::ToReg(Reg::Rcx, Arg32::Signed(1))),
        Instr::Cmp(BinArgs::ToReg(Reg::Rcx, Arg32::Signed(0))),
        Instr::Je(JmpArg::Label(LOGIC_ERROR.to_string())),
    ]
}

fn if_check(reg: Reg) -> Vec<Instr> {
    vec![
        Instr::Mov(MovArgs::ToReg(Reg::Rcx, Arg64::Reg(reg))),
        Instr::And(BinArgs::ToReg(Reg::Rcx, Arg32::Signed(1))),
        Instr::Cmp(BinArgs::ToReg(Reg::Rcx, Arg32::Signed(0))),
        Instr::Je(JmpArg::Label(IF_ERROR.to_string())),
    ]
}

// result:
// Rax: address
// R8: index
fn array_access(address: &ImmExp, index: &ImmExp, vars: &HashMap<String, i32>) -> Vec<Instr> {
    vec![
        Instr::Mov(MovArgs::ToReg(Reg::Rax, imm_to_arg64(address, vars))),
        Instr::Mov(MovArgs::ToReg(Reg::Rdx, Arg64::Reg(Reg::Rax))),
        Instr::And(BinArgs::ToReg(Reg::Rdx, Arg32::Unsigned(TYPE_MASK))),
        Instr::Cmp(BinArgs::ToReg(Reg::Rdx, Arg32::Unsigned(1))),
        Instr::Jne(JmpArg::Label(NON_ARRAY_ERROR.to_string())),
        Instr::Sub(BinArgs::ToReg(Reg::Rax, Arg32::Unsigned(1))),
        Instr::Mov(MovArgs::ToReg(Reg::R8, imm_to_arg64(index, vars))),
        Instr::Mov(MovArgs::ToReg(Reg::R9, Arg64::Reg(Reg::R8))),
        Instr::And(BinArgs::ToReg(Reg::R9, Arg32::Unsigned(0b1))),
        Instr::Cmp(BinArgs::ToReg(Reg::R9, Arg32::Unsigned(0b1))),
        Instr::Je(JmpArg::Label(INDEX_ERROR.to_string())),
        Instr::Sar(BinArgs::ToReg(Reg::R8, Arg32::Unsigned(1))),
        Instr::Cmp(BinArgs::ToMem(
            MemRef {
                reg: Reg::Rax,
                offset: Offset::Constant(0),
            },
            Reg32::Reg(Reg::R8),
        )),
        Instr::Jle(JmpArg::Label(INDEX_OUT_OF_BOUNDS.to_string())),
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
            //
            match p {
                Prim::Add => {
                    let mut res = imm_to_rax(&exps[0], vars);
                    res.extend(arith_check(Reg::Rax));
                    res.push(Instr::Mov(MovArgs::ToReg(
                        Reg::Rdx,
                        imm_to_arg64(&exps[1], vars),
                    )));
                    res.extend(arith_check(Reg::Rdx));
                    res.push(Instr::Add(BinArgs::ToReg(Reg::Rax, Arg32::Reg(Reg::Rdx))));
                    res.push(Instr::Jo(JmpArg::Label(OVERFLOW.to_string())));
                    res
                }
                Prim::Sub => {
                    let mut res = imm_to_rax(&exps[0], vars);
                    res.extend(arith_check(Reg::Rax));
                    res.push(Instr::Mov(MovArgs::ToReg(
                        Reg::Rdx,
                        imm_to_arg64(&exps[1], vars),
                    )));
                    res.extend(arith_check(Reg::Rdx));
                    res.push(Instr::Sub(BinArgs::ToReg(Reg::Rax, Arg32::Reg(Reg::Rdx))));
                    res.push(Instr::Jo(JmpArg::Label(OVERFLOW.to_string())));
                    res
                }
                Prim::Mul => {
                    let mut res = imm_to_rax(&exps[0], vars);
                    res.extend(arith_check(Reg::Rax));
                    res.push(Instr::Mov(MovArgs::ToReg(
                        Reg::Rdx,
                        imm_to_arg64(&exps[1], vars),
                    )));
                    res.extend(arith_check(Reg::Rdx));
                    res.push(Instr::Sar(BinArgs::ToReg(Reg::Rdx, Arg32::Signed(1))));
                    res.push(Instr::IMul(BinArgs::ToReg(Reg::Rax, Arg32::Reg(Reg::Rdx))));
                    res.push(Instr::Jo(JmpArg::Label(OVERFLOW.to_string())));
                    res
                }
                Prim::Add1 => {
                    let mut res = imm_to_rax(&exps[0], vars);
                    res.extend(arith_check(Reg::Rax));
                    res.push(Instr::Add(BinArgs::ToReg(Reg::Rax, Arg32::Unsigned(0x2))));
                    res.push(Instr::Jo(JmpArg::Label(OVERFLOW.to_string())));
                    res
                }
                Prim::Sub1 => {
                    let mut res = imm_to_rax(&exps[0], vars);
                    res.extend(arith_check(Reg::Rax));
                    res.push(Instr::Sub(BinArgs::ToReg(Reg::Rax, Arg32::Unsigned(0x2))));
                    res.push(Instr::Jo(JmpArg::Label(OVERFLOW.to_string())));
                    res
                }
                Prim::Not => {
                    let mut res = imm_to_rax(&exps[0], vars);
                    res.extend(logic_check(Reg::Rax));
                    static BOOL_MASK: u64 = 0x80_00_00_00_00_00_00_00;
                    res.push(Instr::Mov(MovArgs::ToReg(
                        Reg::Rdx,
                        Arg64::Unsigned(BOOL_MASK),
                    )));
                    res.push(Instr::Xor(BinArgs::ToReg(Reg::Rax, Arg32::Reg(Reg::Rdx))));
                    res
                }
                Prim::Print => {
                    vec![
                        Instr::Mov(MovArgs::ToReg(Reg::Rdi, imm_to_arg64(&exps[0], vars))),
                        Instr::Sub(BinArgs::ToReg(
                            Reg::Rsp,
                            Arg32::Signed(align_stack(stack) + 8),
                        )),
                        Instr::Call(JmpArg::Label("print_snake_val".to_string())),
                        Instr::Add(BinArgs::ToReg(
                            Reg::Rsp,
                            Arg32::Signed(align_stack(stack) + 8),
                        )),
                    ]
                }
                Prim::IsBool => {
                    *counter += 1;
                    let fls_label = format!("false_{}", counter);
                    let done_label = format!("cmp_done_{}", counter);
                    vec![
                        Instr::Mov(MovArgs::ToReg(Reg::Rax, imm_to_arg64(&exps[0], vars))),
                        Instr::And(BinArgs::ToReg(Reg::Rax, Arg32::Unsigned(0b1111))),
                        Instr::Cmp(BinArgs::ToReg(Reg::Rax, Arg32::Unsigned(0b1111))),
                        Instr::Jne(JmpArg::Label(fls_label.clone())),
                        Instr::Mov(MovArgs::ToReg(Reg::Rax, Arg64::Unsigned(SNAKE_TRU))),
                        Instr::Jmp(JmpArg::Label(done_label.clone())),
                        Instr::Label(fls_label),
                        Instr::Mov(MovArgs::ToReg(Reg::Rax, Arg64::Unsigned(SNAKE_FLS))),
                        Instr::Label(done_label),
                    ]
                }
                Prim::IsNum => {
                    let mut res = imm_to_rax(&exps[0], vars);
                    res.push(Instr::Mov(MovArgs::ToReg(
                        Reg::Rdx,
                        Arg64::Unsigned(SNAKE_FLS),
                    )));
                    res.push(Instr::Xor(BinArgs::ToReg(Reg::Rax, Arg32::Reg(Reg::Rdx))));
                    res.push(Instr::Shl(BinArgs::ToReg(Reg::Rax, Arg32::Signed(63))));
                    res.push(Instr::Or(BinArgs::ToReg(Reg::Rax, Arg32::Reg(Reg::Rdx))));
                    res
                }
                Prim::And => {
                    let mut res = imm_to_rax(&exps[0], vars);
                    res.extend(logic_check(Reg::Rax));
                    res.push(Instr::Mov(MovArgs::ToReg(
                        Reg::Rdx,
                        imm_to_arg64(&exps[1], vars),
                    )));
                    res.extend(logic_check(Reg::Rdx));
                    res.push(Instr::And(BinArgs::ToReg(Reg::Rax, Arg32::Reg(Reg::Rdx))));
                    res
                }
                Prim::Or => {
                    let mut res = imm_to_rax(&exps[0], vars);
                    res.extend(logic_check(Reg::Rax));
                    res.push(Instr::Mov(MovArgs::ToReg(
                        Reg::Rdx,
                        imm_to_arg64(&exps[1], vars),
                    )));
                    res.extend(logic_check(Reg::Rdx));
                    res.push(Instr::Or(BinArgs::ToReg(Reg::Rax, Arg32::Reg(Reg::Rdx))));
                    res
                }
                Prim::Lt => {
                    let mut res = imm_to_rax(&exps[0], vars);
                    res.extend(sub_for_cmp(exps, vars, false));
                    res.extend(is_neg());
                    res
                }
                Prim::Gt => {
                    let mut res = imm_to_rax(&exps[0], vars);
                    res.extend(sub_for_cmp(exps, vars, true));
                    res.extend(is_neg());
                    res
                }
                Prim::Le => {
                    let mut res = imm_to_rax(&exps[0], vars);
                    res.extend(sub_for_cmp(exps, vars, true));
                    res.extend(is_non_neg());
                    res
                }
                Prim::Ge => {
                    let mut res = imm_to_rax(&exps[0], vars);
                    res.extend(sub_for_cmp(exps, vars, false));
                    res.extend(is_non_neg());
                    res
                }
                Prim::Eq => {
                    let mut res = imm_to_rax(&exps[0], vars);
                    res.push(Instr::Mov(MovArgs::ToReg(
                        Reg::Rdx,
                        imm_to_arg64(&exps[1], vars),
                    )));
                    *counter += 1;
                    let fls_label = format!("false_{}", counter);
                    let done_label = format!("cmp_done_{}", counter);
                    res.extend(vec![
                        Instr::Cmp(BinArgs::ToReg(Reg::Rax, Arg32::Reg(Reg::Rdx))),
                        Instr::Jne(JmpArg::Label(fls_label.clone())),
                        Instr::Mov(MovArgs::ToReg(Reg::Rax, Arg64::Unsigned(SNAKE_TRU))),
                        Instr::Jmp(JmpArg::Label(done_label.clone())),
                        Instr::Label(fls_label),
                        Instr::Mov(MovArgs::ToReg(Reg::Rax, Arg64::Unsigned(SNAKE_FLS))),
                        Instr::Label(done_label),
                    ]);
                    res
                }
                Prim::Neq => {
                    let mut res = imm_to_rax(&exps[0], vars);
                    res.push(Instr::Mov(MovArgs::ToReg(
                        Reg::Rdx,
                        imm_to_arg64(&exps[1], vars),
                    )));
                    *counter += 1;
                    let fls_label = format!("false_{}", counter);
                    let done_label = format!("cmp_done_{}", counter);
                    res.extend(vec![
                        Instr::Cmp(BinArgs::ToReg(Reg::Rax, Arg32::Reg(Reg::Rdx))),
                        Instr::Je(JmpArg::Label(fls_label.clone())),
                        Instr::Mov(MovArgs::ToReg(Reg::Rax, Arg64::Unsigned(SNAKE_TRU))),
                        Instr::Jmp(JmpArg::Label(done_label.clone())),
                        Instr::Label(fls_label),
                        Instr::Mov(MovArgs::ToReg(Reg::Rax, Arg64::Unsigned(SNAKE_FLS))),
                        Instr::Label(done_label),
                    ]);
                    res
                }
                Prim::Length => {
                    vec![
                        Instr::Mov(MovArgs::ToReg(Reg::Rax, imm_to_arg64(&exps[0], vars))),
                        Instr::Mov(MovArgs::ToReg(Reg::Rdx, Arg64::Reg(Reg::Rax))),
                        Instr::And(BinArgs::ToReg(Reg::Rdx, Arg32::Unsigned(TYPE_MASK))),
                        Instr::Cmp(BinArgs::ToReg(Reg::Rdx, Arg32::Unsigned(1))),
                        Instr::Jne(JmpArg::Label(NON_ARRAY_ERROR.to_string())),
                        Instr::Xor(BinArgs::ToReg(Reg::Rax, Arg32::Unsigned(1))),
                        // check address valid
                        Instr::Mov(MovArgs::ToReg(
                            Reg::Rax,
                            Arg64::Mem(MemRef {
                                reg: Reg::Rax,
                                offset: Offset::Constant(0),
                            }),
                        )),
                    ]
                }
                Prim::IsFun => {
                    *counter += 1;
                    let fls_label = format!("false_{}", counter);
                    let done_label = format!("cmp_done_{}", counter);
                    vec![
                        Instr::Mov(MovArgs::ToReg(Reg::Rax, imm_to_arg64(&exps[0], vars))),
                        Instr::Mov(MovArgs::ToReg(Reg::Rdx, Arg64::Reg(Reg::Rax))),
                        Instr::And(BinArgs::ToReg(Reg::Rdx, Arg32::Unsigned(TYPE_MASK))),
                        Instr::Cmp(BinArgs::ToReg(Reg::Rdx, Arg32::Unsigned(0b11))),
                        Instr::Jne(JmpArg::Label(fls_label.clone())),
                        Instr::Mov(MovArgs::ToReg(Reg::Rax, Arg64::Unsigned(SNAKE_TRU))),
                        Instr::Jmp(JmpArg::Label(done_label.clone())),
                        Instr::Label(fls_label),
                        Instr::Mov(MovArgs::ToReg(Reg::Rax, Arg64::Unsigned(SNAKE_FLS))),
                        Instr::Label(done_label),
                    ]
                }
                Prim::IsArray => {
                    *counter += 1;
                    let fls_label = format!("false_{}", counter);
                    let done_label = format!("cmp_done_{}", counter);
                    vec![
                        Instr::Mov(MovArgs::ToReg(Reg::Rax, imm_to_arg64(&exps[0], vars))),
                        Instr::Mov(MovArgs::ToReg(Reg::Rdx, Arg64::Reg(Reg::Rax))),
                        Instr::And(BinArgs::ToReg(Reg::Rdx, Arg32::Unsigned(TYPE_MASK))),
                        Instr::Cmp(BinArgs::ToReg(Reg::Rdx, Arg32::Unsigned(1))),
                        Instr::Jne(JmpArg::Label(fls_label.clone())),
                        Instr::Mov(MovArgs::ToReg(Reg::Rax, Arg64::Unsigned(SNAKE_TRU))),
                        Instr::Jmp(JmpArg::Label(done_label.clone())),
                        Instr::Label(fls_label),
                        Instr::Mov(MovArgs::ToReg(Reg::Rax, Arg64::Unsigned(SNAKE_FLS))),
                        Instr::Label(done_label),
                    ]
                }
                Prim::GetCode => {
                    vec![
                        Instr::Mov(MovArgs::ToReg(Reg::Rax, imm_to_arg64(&exps[0], vars))),
                        Instr::Mov(MovArgs::ToReg(
                            Reg::Rax,
                            Arg64::Mem(MemRef {
                                reg: Reg::Rax,
                                offset: Offset::Constant(0),
                            }),
                        )),
                    ]
                }
                Prim::GetEnv => {
                    vec![
                        Instr::Mov(MovArgs::ToReg(Reg::Rax, imm_to_arg64(&exps[0], vars))),
                        Instr::Mov(MovArgs::ToReg(
                            Reg::Rax,
                            Arg64::Mem(MemRef {
                                reg: Reg::Rax,
                                offset: Offset::Constant(8),
                            }),
                        )),
                    ]
                }
                Prim::CheckArityAndUntag(arg_size) => {
                    vec![
                        Instr::Mov(MovArgs::ToReg(Reg::Rax, imm_to_arg64(&exps[0], vars))),
                        Instr::Mov(MovArgs::ToReg(Reg::Rdx, Arg64::Reg(Reg::Rax))),
                        Instr::And(BinArgs::ToReg(Reg::Rdx, Arg32::Unsigned(TYPE_MASK))),
                        Instr::Cmp(BinArgs::ToReg(Reg::Rdx, Arg32::Unsigned(0b11))),
                        Instr::Jne(JmpArg::Label(NON_CLOSURE_ERROR.to_string())),
                        Instr::And(BinArgs::ToReg(Reg::Rax, Arg32::Unsigned(0b000))),
                        // check arg size
                        Instr::Mov(MovArgs::ToReg(
                            Reg::R8,
                            Arg64::Mem(MemRef {
                                reg: Reg::Rax,
                                offset: Offset::Constant(8),
                            }),
                        )),
                        Instr::Cmp(BinArgs::ToReg(
                            Reg::R8,
                            Arg32::Unsigned((*arg_size).try_into().unwrap()),
                        )),
                        Instr::Jne(JmpArg::Label(LAMBDA_ARITY_ERROR.to_string())),
                    ]
                }
                Prim::ArrayGet => {
                    let mut res = array_access(&exps[0], &exps[1], vars);
                    res.push(Instr::Mov(MovArgs::ToReg(
                        Reg::Rax,
                        Arg64::Mem(MemRef {
                            reg: Reg::Rax,
                            offset: Offset::Computed {
                                reg: Reg::R8,
                                factor: 8, // numbers need / 2 to get value
                                constant: 8,
                            },
                        }),
                    )));
                    res
                }
                Prim::ArraySet => {
                    let mut res = array_access(&exps[0], &exps[1], vars);
                    res.extend(vec![
                        Instr::Mov(MovArgs::ToReg(Reg::R9, imm_to_arg64(&exps[2], vars))),
                        Instr::Mov(MovArgs::ToMem(
                            MemRef {
                                reg: Reg::Rax,
                                offset: Offset::Computed {
                                    reg: Reg::R8,
                                    factor: 8,
                                    constant: 8,
                                },
                            },
                            Reg32::Reg(Reg::R9),
                        )),
                        Instr::Add(BinArgs::ToReg(Reg::Rax, Arg32::Unsigned(1))),
                    ]);
                    res
                }
                Prim::MakeArray => {
                    let len: u32 = exps.len().try_into().unwrap();
                    let mut res = vec![Instr::Mov(MovArgs::ToMem(
                        MemRef {
                            reg: Reg::R15,
                            offset: Offset::Constant(0),
                        },
                        Reg32::Unsigned(len),
                    ))];
                    for (i, exp) in exps.iter().enumerate() {
                        res.extend(vec![
                            Instr::Mov(MovArgs::ToReg(Reg::Rax, imm_to_arg64(&exp, vars))),
                            Instr::Mov(MovArgs::ToMem(
                                MemRef {
                                    reg: Reg::R15,
                                    offset: Offset::Constant((8 * (i + 1)).try_into().unwrap()),
                                },
                                Reg32::Reg(Reg::Rax),
                            )),
                        ]);
                    }
                    res.extend(vec![
                        Instr::Mov(MovArgs::ToReg(Reg::Rax, Arg64::Reg(Reg::R15))),
                        Instr::Add(BinArgs::ToReg(Reg::Rax, Arg32::Unsigned(1))),
                        Instr::Add(BinArgs::ToReg(Reg::R15, Arg32::Unsigned((len + 1) * 8))),
                    ]);
                    res
                }
            }
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
                    offset: Offset::Constant(offset),
                },
                Reg32::Reg(Reg::Rax),
            )));
            vars.insert(var.clone(), offset);

            res.extend(compile_to_instrs_inner(
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
            res.extend(if_check(Reg::Rax));
            *counter += 1;
            let els_label = format!("else_{}", counter);
            let done_label = format!("done_{}", counter);
            res.extend(vec![
                Instr::Mov(MovArgs::ToReg(Reg::Rdx, Arg64::Unsigned(SNAKE_FLS))),
                Instr::Cmp(BinArgs::ToReg(Reg::Rax, Arg32::Reg(Reg::Rdx))),
                Instr::Je(JmpArg::Label(els_label.clone())),
            ]);

            res.extend(compile_to_instrs_inner(
                thn,
                counter,
                stack,
                &mut vars.clone(),
                functions,
            ));
            res.push(Instr::Jmp(JmpArg::Label(done_label.clone())));

            res.push(Instr::Label(els_label));
            res.extend(compile_to_instrs_inner(
                els, counter, stack, vars, functions,
            ));
            res.push(Instr::Label(done_label));
            res
        }
        SeqExp::FunDefs { decls, body, ann } => {
            // locally defined functions
            *counter += 1;
            let body_label = format!("body_{}", counter);
            let mut res = vec![Instr::Jmp(JmpArg::Label(body_label.clone()))];
            // handle mutually recursive functions
            for decl in decls {
                functions.insert(decl.name.clone(), stack);
            }
            for decl in decls {
                push_params(stack, vars, &decl.parameters);
                res.push(Instr::Label(format!("func_{}", decl.name.clone())));
                res.extend(stack_check());
                res.extend(compile_to_instrs_inner(
                    &decl.body,
                    counter,
                    i32::try_from(decl.parameters.len()).unwrap() + stack,
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
            assert!(functions.contains_key(func), "function {} not found", func);
            return compile_tail_call(func.clone(), args, stack, functions[func], vars);
        }
        SeqExp::ExternalCall {
            args,
            is_tail,
            ann,
            fun,
        } => {
            if *is_tail {
                match fun {
                    VarOrLabel::Label(fun_str) => {
                        return compile_tail_call(fun_str.clone(), args, stack, 0, vars);
                    }
                    VarOrLabel::Var(func) => {
                        return vec![
                            Instr::Mov(MovArgs::ToReg(
                                Reg::Rax,
                                imm_to_arg64(&ImmExp::Var(func.to_string()), vars),
                            )),
                            Instr::Jmp(JmpArg::Reg(Reg::Rax)),
                        ];
                    }
                }
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
                        offset: Offset::Constant(-(stack_top + offset)),
                    },
                    Reg32::Reg(Reg::Rax),
                )));
                offset += 8;
            }
            res.push(Instr::Sub(BinArgs::ToReg(
                Reg::Rsp,
                Arg32::Signed(stack_top),
            )));
            match fun {
                VarOrLabel::Label(fun_str) => {
                    res.push(Instr::Call(JmpArg::Label(format!("func_{}", fun_str))));
                }
                VarOrLabel::Var(func) => {
                    res.extend(vec![
                        Instr::Mov(MovArgs::ToReg(
                            Reg::Rax,
                            imm_to_arg64(&ImmExp::Var(func.to_string()), vars),
                        )),
                        Instr::Jmp(JmpArg::Reg(Reg::Rax)),
                    ]);
                }
            }
            res.push(Instr::Add(BinArgs::ToReg(
                Reg::Rsp,
                Arg32::Signed(stack_top),
            )));
            res
        }
        SeqExp::MakeClosure {
            arity,
            label,
            env,
            ann,
        } => {
            vec![
                Instr::RelativeLoadAddress(Reg::Rax, label.clone()),
                Instr::Mov(MovArgs::ToMem(
                    MemRef {
                        reg: Reg::R15,
                        offset: Offset::Constant(0),
                    },
                    Reg32::Reg(Reg::Rax),
                )),
                Instr::Mov(MovArgs::ToReg(Reg::R8, Arg64::Unsigned(*arity as u64))),
                Instr::Mov(MovArgs::ToMem(
                    MemRef {
                        reg: Reg::R15,
                        offset: Offset::Constant(8),
                    },
                    Reg32::Reg(Reg::R8),
                )),
                Instr::Mov(MovArgs::ToReg(Reg::R8, imm_to_arg64(&env, vars))),
                Instr::Mov(MovArgs::ToMem(
                    MemRef {
                        reg: Reg::R15,
                        offset: Offset::Constant(16),
                    },
                    Reg32::Reg(Reg::R8),
                )),
                Instr::Mov(MovArgs::ToReg(Reg::Rax, Arg64::Reg(Reg::R15))),
                Instr::Add(BinArgs::ToReg(Reg::Rax, Arg32::Unsigned(0b11))),
                Instr::Add(BinArgs::ToReg(Reg::R15, Arg32::Unsigned(24))),
            ]
        }
        SeqExp::Semicolon { e1, e2, ann } => todo!(),
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
                -8 * (i32::try_from(var_args.len()).unwrap() + stack + 1),
            );
            res.push(Instr::Mov(MovArgs::ToReg(
                Reg::Rax,
                imm_to_arg64(arg, &vars),
            )));
            res.push(Instr::Mov(MovArgs::ToMem(
                MemRef {
                    reg: Reg::Rsp,
                    offset: Offset::Constant(var_args[v]),
                },
                Reg32::Reg(Reg::Rax),
            )));
        }
    }
    println!("decl stack = {}, arg size = {}", decl_stack, args.len());
    for (i, arg) in args.iter().enumerate() {
        let offset: i32 = -8 * (i32::try_from(i).unwrap() + decl_stack + 1);
        if let ImmExp::Var(v) = arg {
            res.push(Instr::Mov(MovArgs::ToReg(
                Reg::Rax,
                Arg64::Mem(MemRef {
                    reg: Reg::Rsp,
                    offset: Offset::Constant(var_args[v]),
                }),
            )));
            res.push(Instr::Mov(MovArgs::ToMem(
                MemRef {
                    reg: Reg::Rsp,
                    offset: Offset::Constant(offset),
                },
                Reg32::Reg(Reg::Rax),
            )));
        } else {
            res.push(Instr::Mov(MovArgs::ToReg(
                Reg::Rax,
                imm_to_arg64(arg, vars),
            )));
            res.push(Instr::Mov(MovArgs::ToMem(
                MemRef {
                    reg: Reg::Rsp,
                    offset: Offset::Constant(offset),
                },
                Reg32::Reg(Reg::Rax),
            )));
        }
    }
    res.push(Instr::Jmp(JmpArg::Label(format!("func_{}", func))));
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
    is.extend(stack_check());
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

pub fn compile_to_string<Span>(p: &SurfProg<Span>) -> Result<String, CompileErr<Span>>
where
    Span: Clone,
{
    checker::check_prog(p, &HashSet::new())?;
    let (global_functions, main) = lambda_lift(&p);
    println!("global function size = {}", global_functions.len());
    println!("main = {:?}", main);
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
section .data
        HEAP:    times 1024 dq 0
section .text
        global start_here
        extern snake_error
        extern print_snake_val
{}
{}
start_here:
        push r15            ; save the original value in r15
        sub rsp, 8          ; padding to ensure the correct alignment
        lea r15, [rel HEAP] ; load the address of the HEAP into r15 using rip-relative addressing
        add r15, 7                       ; add 7 to get above the next multiple of 8
        mov r8, 0xfffffffffffffff8 ; load a scratch register with the necessary mask
        and r15, r8                ; and then round back down.
        call main           ; call into the actual code for the main expression of the program
        add rsp, 8          ; remove the padding
        pop r15             ; restore the original to r15
        ret
main:
{}
{}
",
        instrs_to_string(&error_handle_instr()),
        functions_is,
        instrs_to_string(&stack_check()),
        main_is
    );
    println!("{}", res);
    Ok(res)
}
