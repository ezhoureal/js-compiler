pub type SurfProg<Ann> = Exp<Ann>;
pub type SurfFunDecl<Ann> = FunDecl<Exp<Ann>, Ann>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FunDecl<E, Ann> {
    pub name: String,
    pub parameters: Vec<String>,
    pub body: E,
    pub ann: Ann,
}

/* Expressions */
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Exp<Ann> {
    Num(i64, Ann),
    Bool(bool, Ann),
    Var(String, Ann),
    Prim(Prim, Vec<Box<Exp<Ann>>>, Ann),
    Let {
        bindings: Vec<(String, Exp<Ann>)>, // new binding declarations
        body: Box<Exp<Ann>>,               // the expression in which the new variables are bound
        ann: Ann,
    },
    If {
        cond: Box<Exp<Ann>>,
        thn: Box<Exp<Ann>>,
        els: Box<Exp<Ann>>,
        ann: Ann,
    },

    Semicolon {
        e1: Box<Exp<Ann>>,
        e2: Box<Exp<Ann>>,
        ann: Ann,
    },

    FunDefs {
        decls: Vec<FunDecl<Exp<Ann>, Ann>>,
        body: Box<Exp<Ann>>,
        ann: Ann,
    },
    Lambda {
        parameters: Vec<String>,
        body: Box<Exp<Ann>>,
        ann: Ann,
    },
    MakeClosure {
        arity: usize,
        label: String,
        env: Box<Exp<Ann>>,
        ann: Ann,
    },

    // A call that may or may not require a closure
    Call(Box<Exp<Ann>>, Vec<Exp<Ann>>, Ann),

    // A call to a dynamically determined closure
    ClosureCall(Box<Exp<Ann>>, Vec<Exp<Ann>>, Ann),
    // A direct call to a known function definition
    DirectCall(String, Vec<Exp<Ann>>, Ann),

    // A local tail call to a static function
    InternalTailCall(String, Vec<Exp<Ann>>, Ann),
    // A global function call to either a static function or the code
    // pointer of a closure
    ExternalCall {
        fun: VarOrLabel,
        args: Vec<Exp<Ann>>,
        is_tail: bool,
        ann: Ann,
    },
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Prim {
    // unary
    Add1,
    Sub1,
    Not,
    Length,
    Print,
    IsBool,
    IsNum,
    IsFun,
    IsArray,
    // Internal-only unary forms
    GetCode,
    GetEnv,
    CheckArityAndUntag(usize),

    // binary
    Add,
    Sub,
    Mul,
    And,
    Or,
    Lt,
    Gt,
    Le,
    Ge,
    Eq,
    Neq,
    ArrayGet, // first arg is array, second is index

    // trinary
    ArraySet, // first arg is array, second is index, third is new value

    // 0 or more arguments
    MakeArray,
}

/* Sequential Expressions */
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SeqProg<Ann> {
    pub funs: Vec<FunDecl<SeqExp<Ann>, Ann>>,
    pub main: SeqExp<Ann>,
    pub ann: Ann,
}

pub type SeqFunDecl<Ann> = FunDecl<SeqExp<Ann>, Ann>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImmExp {
    Num(i64),
    Bool(bool),
    Var(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VarOrLabel {
    Var(String),
    Label(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SeqExp<Ann> {
    Imm(ImmExp, Ann),
    Prim(Prim, Vec<ImmExp>, Ann),
    MakeClosure {
        arity: usize,
        label: String,
        env: ImmExp,
        ann: Ann,
    },
    Let {
        var: String,
        bound_exp: Box<SeqExp<Ann>>,
        body: Box<SeqExp<Ann>>,
        ann: Ann,
    },
    If {
        cond: ImmExp,
        thn: Box<SeqExp<Ann>>,
        els: Box<SeqExp<Ann>>,
        ann: Ann,
    },
    // Local function definitions
    // These should only be called using InternalTailCall
    FunDefs {
        decls: Vec<FunDecl<SeqExp<Ann>, Ann>>,
        body: Box<SeqExp<Ann>>,
        ann: Ann,
    },

    // An internal tail call to a locally defined function.
    // Implemented by setting arguments and then jmp in Assembly
    InternalTailCall(String, Vec<ImmExp>, Ann),

    // A call to one of the top-level function definitions
    // Uses the Snake Calling Convention v0
    // marked to indicate whether it is a tail call or not
    ExternalCall {
        fun: VarOrLabel,
        args: Vec<ImmExp>,
        is_tail: bool,
        ann: Ann,
    },

    Semicolon {
        e1: Box<SeqExp<Ann>>,
        e2: Box<SeqExp<Ann>>,
        ann: Ann,
    },
}
