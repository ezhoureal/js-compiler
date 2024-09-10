use crate::syntax::{Exp, FunDecl, SeqExp, SeqProg, SurfProg};

use std::collections::{HashSet};

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
    Ok(())
    // panic!("NYI: check_prog")
}

fn uniquify(e: &Exp<u32>) -> Exp<()> {
    panic!("NYI: uniquify")
}

// Parse Calls into either DirectCall or ClosureCall
fn eliminate_closures<Ann>(e: &Exp<Ann>) -> Exp<()> {
    panic!("NYI: uniquify")
}

// Identify which functions should be lifted to the top level
fn should_lift<Ann>(p: &Exp<Ann>) -> HashSet<String> {
    panic!("NYI: should lift")
}

// Lift some functions to global definitions
fn lambda_lift<Ann>(p: &Exp<Ann>) -> (Vec<FunDecl<Exp<()>, ()>>, Exp<()>) {
    panic!("NYI: lambda_lift")
}

fn sequentialize(e: &Exp<u32>) -> SeqExp<()> {
    panic!("NYI: sequentialize")
}

fn seq_prog(decls: &[FunDecl<Exp<u32>, u32>], p: &Exp<u32>) -> SeqProg<()> {
    panic!("NYI: seq_prog")
}

pub fn compile_to_string<Span>(p: &SurfProg<Span>) -> Result<String, CompileErr<Span>>
where
    Span: Clone,
{
    panic!("NYI: compile_to_string")
}
