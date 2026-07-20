use crate::diag::Span;

#[derive(Debug, Clone)]
pub struct Program {
    pub links: Vec<String>,
    pub structs: Vec<StructDef>,
    pub functions: Vec<Function>,
}

#[derive(Debug, Clone)]
pub struct StructDef {
    pub name: String,
    #[allow(dead_code)]
    pub name_span: Span,
    /// Template params, e.g. `struct Box<T>`
    pub generic_params: Vec<String>,
    pub fields: Vec<StructField>,
    /// Methods / virtual domain fns declared inside the struct body.
    pub methods: Vec<Method>,
    #[allow(dead_code)]
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct StructField {
    pub name: String,
    pub name_span: Span,
    pub ty: Type,
}

#[derive(Debug, Clone)]
pub struct Method {
    pub is_virtual: bool,
    pub func: Function,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamMode {
    /// Pass by value (default)
    Value,
    /// `ref T x` — pointer, auto-deref on use
    Ref,
    /// `out T x` — pointer, must be written
    Out,
    /// Move owned value into callee (`own T x`)
    Own,
    /// Pass a fractional share (`share[n/d] T x`)
    Share,
    /// Pass joint co-ownership (`joint T x`)
    Joint,
    /// Shared borrow (`&T x`)
    Borrow,
    /// Exclusive borrow (`&mut T x`)
    BorrowMut,
}

/// Fractional share numerator/denominator for 按份共有.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fraction {
    pub num: u32,
    pub den: u32,
}

impl Fraction {
    pub fn new(num: u32, den: u32) -> Self {
        Self { num, den }
    }

    pub fn is_valid(self) -> bool {
        self.den > 0 && self.num > 0 && self.num <= self.den
    }
}

/// Ownership kind attached to bindings / types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OwnKind {
    /// Legacy unmanaged binding (backward compatible; copy-like for scalars)
    #[default]
    None,
    /// Unique ownership (Rust-style)
    Own,
    /// 按份共有 — co-ownership by shares
    Share(Fraction),
    /// 共同共有 — undivided joint ownership
    Joint,
}

/// Full ownership specifier: kind + 排他/整体 flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OwnSpec {
    pub kind: OwnKind,
    /// 排他 — exclusive; no concurrent shared access
    pub exclusive: bool,
    /// 整体 — dispositions apply to the whole object
    pub whole: bool,
}

impl OwnSpec {
    pub fn own_exclusive() -> Self {
        Self {
            kind: OwnKind::Own,
            exclusive: true,
            whole: false,
        }
    }

    pub fn is_managed(self) -> bool {
        !matches!(self.kind, OwnKind::None)
    }

    pub fn is_own(self) -> bool {
        matches!(self.kind, OwnKind::Own)
    }

    pub fn is_share(self) -> bool {
        matches!(self.kind, OwnKind::Share(_))
    }

    pub fn is_joint(self) -> bool {
        matches!(self.kind, OwnKind::Joint)
    }

    pub fn fraction(self) -> Option<Fraction> {
        match self.kind {
            OwnKind::Share(f) => Some(f),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Param {
    pub mode: ParamMode,
    pub name: String,
    pub name_span: Span,
    pub ty: Type,
}

#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    pub name_span: Span,
    /// Template params, e.g. `fn id<T>`
    pub generic_params: Vec<String>,
    pub params: Vec<Param>,
    /// `None` = void (no `->` return type)
    pub ret_ty: Option<Type>,
    pub body: Vec<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    Int,
    Float,
    Str,
    Bool,
    #[allow(dead_code)]
    Void,
    /// Inferred from initializer (`var` / `let`)
    Infer,
    Named(String),
    /// Generic type parameter `T`
    Generic(String),
    /// Applied template `Box<int>`
    Apply {
        name: String,
        args: Vec<Type>,
    },
    /// Pointer / ref pointee carrier used after lowering (`ref int` → Ptr(Int) in LLVM)
    Ptr(Box<Type>),
    Array {
        elem: Box<Type>,
        len: i64,
    },
    /// Owned / co-owned carrier: `own T`, `share[n/d] T`, `joint T`
    Owned {
        spec: OwnSpec,
        inner: Box<Type>,
    },
    /// Borrowed reference type: `&T` / `&mut T`
    Borrowed {
        mutable: bool,
        inner: Box<Type>,
    },
}

#[derive(Debug, Clone)]
pub struct Stmt {
    pub span: Span,
    pub kind: StmtKind,
}

#[derive(Debug, Clone)]
pub enum StmtKind {
    VarDecl {
        name: String,
        name_span: Span,
        ty: Type,
        /// Ownership of this binding (may also be encoded in `ty` as `Type::Owned`)
        own: OwnSpec,
        init: Expr,
    },
    ArrayDecl {
        name: String,
        name_span: Span,
        elem: Type,
        len: i64,
    },
    IndexAssign {
        base: String,
        base_span: Span,
        index: Expr,
        value: Expr,
    },
    Assign {
        name: String,
        name_span: Span,
        value: Expr,
    },
    /// `*p = value` or through ref
    DerefAssign {
        ptr: Expr,
        value: Expr,
    },
    FieldAssign {
        base: String,
        base_span: Span,
        field: String,
        field_span: Span,
        value: Expr,
    },
    If {
        cond: Expr,
        then_body: Vec<Stmt>,
        else_body: Vec<Stmt>,
    },
    While {
        cond: Expr,
        body: Vec<Stmt>,
    },
    /// `for i in 0..n { ... }` — `end` is exclusive
    For {
        name: String,
        name_span: Span,
        start: Expr,
        end: Expr,
        body: Vec<Stmt>,
    },
    Break,
    Continue,
    /// `print a, b;` (also `print(a)` via Call)
    Print(Vec<Expr>),
    Return(Option<Expr>),
    Expr(Expr),
}

#[derive(Debug, Clone)]
pub struct Expr {
    pub span: Span,
    pub kind: ExprKind,
}

#[derive(Debug, Clone)]
pub enum ExprKind {
    IntLit(i64),
    FloatLit(f64),
    StrLit(String),
    BoolLit(bool),
    Ident(String),
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
    },
    Binary {
        op: BinOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Call {
        callee: String,
        callee_span: Span,
        /// Explicit template args at call site: `id<int>(1)`
        type_args: Vec<Type>,
        args: Vec<Expr>,
    },
    /// `obj.method(args)` — desugars to domain/method call
    MethodCall {
        receiver: Box<Expr>,
        method: String,
        method_span: Span,
        args: Vec<Expr>,
    },
    /// `Point { x: 1, y: 2 }` or `Box<int> { value: 1 }`
    StructLit {
        name: String,
        name_span: Span,
        type_args: Vec<Type>,
        fields: Vec<(String, Span, Expr)>,
    },
    /// `p.x`
    Field {
        base: Box<Expr>,
        field: String,
        field_span: Span,
    },
    Index {
        base: Box<Expr>,
        index: Box<Expr>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    /// `&x` — shared borrow
    AddrOf,
    /// `&mut x` — exclusive borrow
    AddrOfMut,
    /// `*p`
    Deref,
    /// `move x` — explicit move of owned value
    Move,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
}

impl Expr {
    pub fn int_lit(span: Span, n: i64) -> Self {
        Self {
            span,
            kind: ExprKind::IntLit(n),
        }
    }

    pub fn bool_lit(span: Span, b: bool) -> Self {
        Self {
            span,
            kind: ExprKind::BoolLit(b),
        }
    }

    pub fn float_lit(span: Span, n: f64) -> Self {
        Self {
            span,
            kind: ExprKind::FloatLit(n),
        }
    }
}

impl Type {
    pub fn is_void(&self) -> bool {
        matches!(self, Type::Void)
    }

    pub fn mono_name(&self) -> String {
        match self {
            Type::Int => "int".into(),
            Type::Float => "float".into(),
            Type::Str => "str".into(),
            Type::Bool => "bool".into(),
            Type::Void => "void".into(),
            Type::Infer => "infer".into(),
            Type::Named(n) | Type::Generic(n) => n.clone(),
            Type::Apply { name, args } => {
                let parts: Vec<_> = args.iter().map(|a| a.mono_name()).collect();
                format!("{name}__{}", parts.join("_"))
            }
            Type::Ptr(inner) => format!("ptr_{}", inner.mono_name()),
            Type::Array { elem, len } => format!("arr_{}_{len}", elem.mono_name()),
            Type::Owned { spec, inner } => {
                let flag = match (spec.exclusive, spec.whole) {
                    (true, true) => "ex_wh_",
                    (true, false) => "ex_",
                    (false, true) => "wh_",
                    (false, false) => "",
                };
                let kind = match spec.kind {
                    OwnKind::None => "unmanaged",
                    OwnKind::Own => "own",
                    OwnKind::Share(f) => return format!(
                        "{flag}share_{}_{}_{}",
                        f.num,
                        f.den,
                        inner.mono_name()
                    ),
                    OwnKind::Joint => "joint",
                };
                format!("{flag}{kind}_{}", inner.mono_name())
            }
            Type::Borrowed { mutable, inner } => {
                if *mutable {
                    format!("mutref_{}", inner.mono_name())
                } else {
                    format!("ref_{}", inner.mono_name())
                }
            }
        }
    }

    /// Strip ownership / borrow wrappers to the payload type.
    pub fn peel(&self) -> &Type {
        match self {
            Type::Owned { inner, .. } | Type::Borrowed { inner, .. } | Type::Ptr(inner) => {
                inner.peel()
            }
            other => other,
        }
    }

    pub fn own_spec(&self) -> OwnSpec {
        match self {
            Type::Owned { spec, .. } => *spec,
            Type::Borrowed { mutable, .. } => OwnSpec {
                kind: OwnKind::None,
                exclusive: *mutable,
                whole: false,
            },
            _ => OwnSpec::default(),
        }
    }
}
