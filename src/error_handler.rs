use crate::asm::{Arg64, Instr, JmpArg, MovArgs, Reg};

pub static OVERFLOW: &str = "overflow_error";
pub static ARITH_ERROR: &str = "arith_error";
pub static CMP_ERROR: &str = "cmp_error";
pub static IF_ERROR: &str = "if_error";
pub static LOGIC_ERROR: &str = "logic_error";
pub static NON_ARRAY_ERROR: &str = "non_array_error";
pub static INDEX_ERROR: &str = "index_not_number";
pub static INDEX_OUT_OF_BOUNDS: &str = "index_out_of_bounds";
pub static SNAKE_ERROR: &str = "snake_error";

pub fn error_handle_instr() -> Vec<Instr> {
    vec![
        Instr::Label(ARITH_ERROR.to_string()),
        Instr::Mov(MovArgs::ToReg(Reg::Rdi, Arg64::Signed(0))),
        Instr::Mov(MovArgs::ToReg(Reg::Rsi, Arg64::Reg(Reg::Rax))),
        Instr::Call(JmpArg::Label(SNAKE_ERROR.to_string())),
        Instr::Label(CMP_ERROR.to_string()),
        Instr::Mov(MovArgs::ToReg(Reg::Rdi, Arg64::Signed(1))),
        Instr::Mov(MovArgs::ToReg(Reg::Rsi, Arg64::Reg(Reg::Rax))),
        Instr::Call(JmpArg::Label(SNAKE_ERROR.to_string())),
        Instr::Label(OVERFLOW.to_string()),
        Instr::Mov(MovArgs::ToReg(Reg::Rdi, Arg64::Signed(2))),
        Instr::Mov(MovArgs::ToReg(Reg::Rsi, Arg64::Reg(Reg::Rax))),
        Instr::Call(JmpArg::Label(SNAKE_ERROR.to_string())),
        Instr::Label(IF_ERROR.to_string()),
        Instr::Mov(MovArgs::ToReg(Reg::Rdi, Arg64::Signed(3))),
        Instr::Mov(MovArgs::ToReg(Reg::Rsi, Arg64::Reg(Reg::Rax))),
        Instr::Call(JmpArg::Label(SNAKE_ERROR.to_string())),
        Instr::Label(LOGIC_ERROR.to_string()),
        Instr::Mov(MovArgs::ToReg(Reg::Rdi, Arg64::Signed(4))),
        Instr::Mov(MovArgs::ToReg(Reg::Rsi, Arg64::Reg(Reg::Rax))),
        Instr::Call(JmpArg::Label(SNAKE_ERROR.to_string())),
        Instr::Label(NON_ARRAY_ERROR.to_string()),
        Instr::Mov(MovArgs::ToReg(Reg::Rdi, Arg64::Signed(5))),
        Instr::Mov(MovArgs::ToReg(Reg::Rsi, Arg64::Reg(Reg::Rax))),
        Instr::Call(JmpArg::Label(SNAKE_ERROR.to_string())),
        Instr::Label(INDEX_ERROR.to_string()),
        Instr::Mov(MovArgs::ToReg(Reg::Rdi, Arg64::Signed(6))),
        Instr::Mov(MovArgs::ToReg(Reg::Rsi, Arg64::Reg(Reg::R8))),
        Instr::Call(JmpArg::Label(SNAKE_ERROR.to_string())),
        Instr::Label(INDEX_OUT_OF_BOUNDS.to_string()),
        Instr::Mov(MovArgs::ToReg(Reg::Rdi, Arg64::Signed(7))),
        Instr::Mov(MovArgs::ToReg(Reg::Rsi, Arg64::Reg(Reg::R8))),
        Instr::Call(JmpArg::Label(SNAKE_ERROR.to_string())),
    ]
}