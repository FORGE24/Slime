use crate::ast::*;
use crate::diag::{Diagnostic, Span};
use crate::lexer::{SpannedToken, Token};

pub struct Parser {
    tokens: Vec<SpannedToken>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<SpannedToken>) -> Self {
        Self { tokens, pos: 0 }
    }

    pub fn parse(mut self) -> Result<Program, Diagnostic> {
        let mut links = Vec::new();
        let mut functions = Vec::new();
        let mut structs = Vec::new();

        while !self.is_eof() {
            match &self.peek().kind {
                Token::Link(path) => {
                    links.push(path.clone());
                    self.bump();
                }
                Token::Namespace => {
                    self.parse_namespace(&mut functions, &mut structs)?;
                }
                Token::Struct | Token::Domain => {
                    structs.push(self.parse_struct()?);
                }
                Token::Fn | Token::Virtual => {
                    functions.push(self.parse_function()?);
                }
                _ => {
                    let tok = self.peek().clone();
                    return Err(Diagnostic::error(format!(
                        "expected item, found {}",
                        tok.kind
                    ))
                    .label(tok.span, "expected `fn`, `struct`, or `namespace` here")
                    .help("top-level items are `fn`, `struct`, `namespace`, or `!link`"));
                }
            }
        }

        Ok(Program {
            links,
            structs,
            functions,
        })
    }

    fn peek(&self) -> &SpannedToken {
        if let Some(t) = self.tokens.get(self.pos) {
            t
        } else {
            self.tokens.last().expect("token stream is never empty (has Eof)")
        }
    }

    fn peek_kind(&self) -> &Token {
        &self.peek().kind
    }

    fn bump(&mut self) -> SpannedToken {
        let tok = self
            .tokens
            .get(self.pos)
            .cloned()
            .unwrap_or_else(|| SpannedToken {
                kind: Token::Eof,
                span: self
                    .tokens
                    .last()
                    .map(|t| Span::new(t.span.end as usize, t.span.end as usize))
                    .unwrap_or_else(Span::dummy),
            });
        if self.pos < self.tokens.len() {
            self.pos += 1;
        }
        tok
    }

    fn is_eof(&self) -> bool {
        matches!(self.peek_kind(), Token::Eof)
    }

    fn unexpected(&self, expected: &str) -> Diagnostic {
        let tok = self.peek();
        Diagnostic::error(format!("expected {expected}, found {}", tok.kind))
            .label(tok.span, format!("expected {expected}"))
    }

    fn expect_kind(&mut self, expected: Token) -> Result<SpannedToken, Diagnostic> {
        let tok = self.bump();
        if tok.kind == expected {
            Ok(tok)
        } else {
            Err(Diagnostic::error(format!(
                "expected {}, found {}",
                expected, tok.kind
            ))
            .label(tok.span, format!("expected {}", expected)))
        }
    }

    fn parse_namespace(
        &mut self,
        functions: &mut Vec<Function>,
        structs: &mut Vec<StructDef>,
    ) -> Result<(), Diagnostic> {
        self.expect_kind(Token::Namespace)?;
        let (ns, _) = self.parse_dotted_name()?;
        self.expect_kind(Token::Semicolon)?;

        let mut local_structs: Vec<String> = Vec::new();
        let mut ns_fns: Vec<Function> = Vec::new();
        let mut ns_structs: Vec<StructDef> = Vec::new();

        while !self.is_eof() {
            match self.peek_kind() {
                Token::Fn | Token::Virtual => {
                    ns_fns.push(self.parse_function()?);
                }
                Token::Struct | Token::Domain => {
                    let s = self.parse_struct()?;
                    local_structs.push(s.name.clone());
                    ns_structs.push(s);
                }
                Token::End => {
                    self.bump();
                    if matches!(self.peek_kind(), Token::Namespace) {
                        self.bump();
                    }
                    if matches!(self.peek_kind(), Token::Semicolon) {
                        self.bump();
                    }
                    break;
                }
                _ => {
                    return Err(self
                        .unexpected("`fn`, `struct`, or `end namespace`")
                        .help("close the namespace with `end namespace;`"));
                }
            }
        }

        for s in &mut ns_structs {
            let short = s.name.clone();
            s.name = format!("{ns}.{short}");
            prefix_types_in_struct(s, &ns, &local_structs);
        }
        for f in &mut ns_fns {
            if f.name != "main" {
                f.name = format!("{ns}.{}", f.name);
            }
            prefix_types_in_function(f, &ns, &local_structs);
        }
        structs.append(&mut ns_structs);
        functions.append(&mut ns_fns);
        Ok(())
    }

    fn parse_struct(&mut self) -> Result<StructDef, Diagnostic> {
        let start = match self.peek_kind() {
            Token::Struct | Token::Domain => self.bump(),
            _ => return Err(self.unexpected("`struct` or `domain`")),
        };
        let name_tok = self.bump();
        let Token::Ident(name) = name_tok.kind else {
            return Err(Diagnostic::error(format!(
                "expected type name, found {}",
                name_tok.kind
            ))
            .label(name_tok.span, "expected identifier")
            .help("syntax: `struct Name { ... }` or `domain Name { ... }`"));
        };
        let generic_params = self.parse_generic_params()?;
        self.expect_kind(Token::LBrace)?;
        let mut fields = Vec::new();
        let mut methods = Vec::new();
        while !matches!(self.peek_kind(), Token::RBrace | Token::Eof) {
            if matches!(self.peek_kind(), Token::Fn | Token::Virtual) {
                let is_virtual = matches!(self.peek_kind(), Token::Virtual);
                let meth = self.parse_function()?;
                // Default receiver name if first param missing: inject later in mono/codegen
                methods.push(Method {
                    is_virtual,
                    func: meth,
                });
                continue;
            }
            let (ty, _) = self.parse_type()?;
            let fname = self.bump();
            let Token::Ident(fnm) = fname.kind else {
                return Err(Diagnostic::error(format!(
                    "expected field name, found {}",
                    fname.kind
                ))
                .label(fname.span, "expected identifier"));
            };
            fields.push(StructField {
                name: fnm,
                name_span: fname.span,
                ty,
            });
            self.optional_semi();
        }
        let end = self.expect_kind(Token::RBrace)?;
        Ok(StructDef {
            name,
            name_span: name_tok.span,
            generic_params,
            fields,
            methods,
            span: start.span.merge(end.span),
        })
    }

    fn parse_generic_params(&mut self) -> Result<Vec<String>, Diagnostic> {
        if !matches!(self.peek_kind(), Token::Lt) {
            return Ok(Vec::new());
        }
        self.bump();
        let mut params = Vec::new();
        if !matches!(self.peek_kind(), Token::Gt) {
            loop {
                let tok = self.bump();
                let Token::Ident(name) = tok.kind else {
                    return Err(Diagnostic::error(format!(
                        "expected generic parameter name, found {}",
                        tok.kind
                    ))
                    .label(tok.span, "expected identifier")
                    .help("example: `fn id<T>(T x) -> T`"));
                };
                params.push(name);
                if matches!(self.peek_kind(), Token::Comma) {
                    self.bump();
                    continue;
                }
                break;
            }
        }
        self.expect_kind(Token::Gt)?;
        Ok(params)
    }

    fn parse_type_args(&mut self) -> Result<Vec<Type>, Diagnostic> {
        if !matches!(self.peek_kind(), Token::Lt) {
            return Ok(Vec::new());
        }
        self.bump();
        let mut args = Vec::new();
        if !matches!(self.peek_kind(), Token::Gt) {
            loop {
                args.push(self.parse_type()?.0);
                if matches!(self.peek_kind(), Token::Comma) {
                    self.bump();
                    continue;
                }
                break;
            }
        }
        self.expect_kind(Token::Gt)?;
        Ok(args)
    }

    /// Disambiguate `id<int>(...)` / `Box<int>{...}` from `i < 3`.
    fn looks_like_type_args(&self) -> bool {
        if !matches!(self.peek_kind(), Token::Lt) {
            return false;
        }
        match self.tokens.get(self.pos + 1).map(|t| &t.kind) {
            Some(
                Token::Int
                | Token::Float
                | Token::Str
                | Token::Bool
                | Token::Ident(_),
            ) => matches!(
                self.tokens.get(self.pos + 2).map(|t| &t.kind),
                Some(Token::Gt | Token::Comma)
            ),
            _ => false,
        }
    }

    fn parse_dotted_name(&mut self) -> Result<(String, Span), Diagnostic> {
        let first = self.bump();
        let Some(s) = path_segment_name(&first.kind) else {
            return Err(Diagnostic::error(format!(
                "expected identifier, found {}",
                first.kind
            ))
            .label(first.span, "expected identifier"));
        };
        let mut parts = vec![s];
        let mut span = first.span;
        while matches!(self.peek_kind(), Token::Dot) {
            let dot = self.bump();
            span = span.merge(dot.span);
            let part = self.bump();
            if let Some(s) = path_segment_name(&part.kind) {
                parts.push(s);
                span = span.merge(part.span);
            } else {
                return Err(Diagnostic::error(format!(
                    "expected identifier after `.`, found {}",
                    part.kind
                ))
                .label(part.span, "expected identifier"));
            }
        }
        Ok((parts.join("."), span))
    }

    fn parse_function(&mut self) -> Result<Function, Diagnostic> {
        let mut start_span = self.peek().span;
        if matches!(self.peek_kind(), Token::Virtual) {
            start_span = self.bump().span;
        }
        let fn_tok = self.expect_kind(Token::Fn)?;
        start_span = start_span.merge(fn_tok.span);
        let name_tok = self.bump();
        let Token::Ident(name) = name_tok.kind else {
            return Err(Diagnostic::error(format!(
                "expected function name, found {}",
                name_tok.kind
            ))
            .label(name_tok.span, "expected identifier")
            .help("function syntax: `fn name() { ... }` or `fn id<T>(T x) -> T`"));
        };
        let name_span = name_tok.span;
        let generic_params = self.parse_generic_params()?;

        let mut params = Vec::new();
        if matches!(self.peek_kind(), Token::LParen) {
            self.bump();
            if !matches!(self.peek_kind(), Token::RParen) {
                loop {
                    let mode = match self.peek_kind() {
                        Token::Ref => {
                            self.bump();
                            ParamMode::Ref
                        }
                        Token::Out => {
                            self.bump();
                            ParamMode::Out
                        }
                        Token::Own
                        | Token::Share
                        | Token::Joint
                        | Token::Exclusive
                        | Token::Whole
                        | Token::Amp => {
                            // Ownership / borrow encoded in the type; mode derived below.
                            ParamMode::Value
                        }
                        _ => ParamMode::Value,
                    };
                    let (ty, _) = self.parse_type()?;
                    let mode = match (&mode, &ty) {
                        (ParamMode::Value, Type::Owned { spec, .. }) => match spec.kind {
                            OwnKind::Own => ParamMode::Own,
                            OwnKind::Share(_) => ParamMode::Share,
                            OwnKind::Joint => ParamMode::Joint,
                            OwnKind::None => ParamMode::Value,
                        },
                        (ParamMode::Value, Type::Borrowed { mutable: false, .. }) => {
                            ParamMode::Borrow
                        }
                        (ParamMode::Value, Type::Borrowed { mutable: true, .. }) => {
                            ParamMode::BorrowMut
                        }
                        (m, _) => *m,
                    };
                    let pname = self.bump();
                    let Token::Ident(pn) = pname.kind else {
                        return Err(Diagnostic::error(format!(
                            "expected parameter name, found {}",
                            pname.kind
                        ))
                        .label(pname.span, "expected identifier"));
                    };
                    params.push(Param {
                        mode,
                        name: pn,
                        name_span: pname.span,
                        ty,
                    });
                    if matches!(self.peek_kind(), Token::Comma) {
                        self.bump();
                        continue;
                    }
                    break;
                }
            }
            self.expect_kind(Token::RParen)?;
        }

        let ret_ty = if matches!(self.peek_kind(), Token::Arrow) {
            self.bump();
            Some(self.parse_type()?.0)
        } else {
            None
        };

        let body = self.parse_block()?;
        let span = start_span.merge(body.last().map(|s| s.span).unwrap_or(name_span));
        Ok(Function {
            name,
            name_span,
            generic_params,
            params,
            ret_ty,
            body,
            span,
        })
    }

    fn parse_type(&mut self) -> Result<(Type, Span), Diagnostic> {
        // Optional flags: exclusive / whole (any order, before kind)
        let mut exclusive = false;
        let mut whole = false;
        let mut flag_span: Option<Span> = None;
        loop {
            match self.peek_kind() {
                Token::Exclusive => {
                    let t = self.bump();
                    exclusive = true;
                    flag_span = Some(flag_span.map_or(t.span, |s| s.merge(t.span)));
                }
                Token::Whole => {
                    let t = self.bump();
                    whole = true;
                    flag_span = Some(flag_span.map_or(t.span, |s| s.merge(t.span)));
                }
                _ => break,
            }
        }

        // Borrowed: `&T` / `&mut T`
        if matches!(self.peek_kind(), Token::Amp) {
            let amp = self.bump();
            let mutable = if matches!(self.peek_kind(), Token::Mut) {
                self.bump();
                true
            } else {
                false
            };
            let (inner, ispan) = self.parse_type()?;
            let span = flag_span
                .map_or(amp.span, |s| s.merge(amp.span))
                .merge(ispan);
            if exclusive || whole {
                return Err(Diagnostic::error(
                    "`exclusive`/`whole` apply to owned bindings, not borrows",
                )
                .label(span, "invalid borrow type"));
            }
            return Ok((
                Type::Borrowed {
                    mutable,
                    inner: Box::new(inner),
                },
                span,
            ));
        }

        // Owned forms: own / share[n/d] / joint
        let owned_kind = match self.peek_kind() {
            Token::Own => {
                let t = self.bump();
                Some((OwnKind::Own, t.span))
            }
            Token::Share => {
                let t = self.bump();
                self.expect_kind(Token::LBracket)?;
                let num_tok = self.bump();
                let Token::IntLit(num) = num_tok.kind else {
                    return Err(Diagnostic::error(format!(
                        "expected share numerator, found {}",
                        num_tok.kind
                    ))
                    .label(num_tok.span, "expected integer literal")
                    .help("share syntax: `share[1/2] int x`"));
                };
                self.expect_kind(Token::Slash)?;
                let den_tok = self.bump();
                let Token::IntLit(den) = den_tok.kind else {
                    return Err(Diagnostic::error(format!(
                        "expected share denominator, found {}",
                        den_tok.kind
                    ))
                    .label(den_tok.span, "expected integer literal"));
                };
                self.expect_kind(Token::RBracket)?;
                if num <= 0 || den <= 0 || num > den {
                    return Err(Diagnostic::error(
                        "share fraction must satisfy 0 < num <= den",
                    )
                    .label(num_tok.span.merge(den_tok.span), "invalid fraction"));
                }
                Some((
                    OwnKind::Share(Fraction::new(num as u32, den as u32)),
                    t.span.merge(den_tok.span),
                ))
            }
            Token::Joint => {
                let t = self.bump();
                Some((OwnKind::Joint, t.span))
            }
            _ => None,
        };

        if let Some((kind, kspan)) = owned_kind {
            let (inner, ispan) = self.parse_base_type()?;
            let span = flag_span
                .map_or(kspan, |s| s.merge(kspan))
                .merge(ispan);
            let exclusive = exclusive || matches!(kind, OwnKind::Own);
            return Ok((
                Type::Owned {
                    spec: OwnSpec {
                        kind,
                        exclusive,
                        whole,
                    },
                    inner: Box::new(inner),
                },
                span,
            ));
        }

        if exclusive || whole {
            return Err(Diagnostic::error(
                "`exclusive`/`whole` require `own`, `share[...]`, or `joint`",
            )
            .label(
                flag_span.unwrap_or_else(Span::dummy),
                "ownership flag without kind",
            ));
        }

        self.parse_base_type()
    }

    fn parse_base_type(&mut self) -> Result<(Type, Span), Diagnostic> {
        let tok = self.bump();
        let (mut ty, mut span) = match tok.kind {
            Token::Int => (Type::Int, tok.span),
            Token::Float => (Type::Float, tok.span),
            Token::Str => (Type::Str, tok.span),
            Token::Bool => (Type::Bool, tok.span),
            Token::Ident(name) => {
                let mut parts = vec![name];
                let mut sp = tok.span;
                while matches!(self.peek_kind(), Token::Dot) {
                    self.bump();
                    let part = self.bump();
                    if let Some(p) = path_segment_name(&part.kind) {
                        sp = sp.merge(part.span);
                        parts.push(p);
                    } else {
                        return Err(Diagnostic::error(format!(
                            "expected type path segment, found {}",
                            part.kind
                        ))
                        .label(part.span, "expected identifier"));
                    }
                }
                (Type::Named(parts.join(".")), sp)
            }
            other => {
                return Err(Diagnostic::error(format!("expected type, found {other}"))
                    .label(tok.span, "expected type")
                    .help(
                        "types: `int`, `float`, `str`, `bool`, `own T`, `share[n/d] T`, `joint T`, `&T`, `&mut T`",
                    ));
            }
        };
        if matches!(self.peek_kind(), Token::Lt) {
            let args = self.parse_type_args()?;
            if let Type::Named(name) = ty {
                ty = Type::Apply { name, args };
            } else {
                return Err(
                    Diagnostic::error("only named types can take template arguments")
                        .label(span, "invalid template base"),
                );
            }
        }
        let _ = span;
        Ok((ty, span))
    }

    fn parse_block(&mut self) -> Result<Vec<Stmt>, Diagnostic> {
        self.expect_kind(Token::LBrace)?;
        let mut stmts = Vec::new();
        while !matches!(self.peek_kind(), Token::RBrace | Token::Eof) {
            stmts.push(self.parse_stmt()?);
        }
        self.expect_kind(Token::RBrace)?;
        Ok(stmts)
    }

    fn parse_stmt(&mut self) -> Result<Stmt, Diagnostic> {
        match self.peek_kind() {
            Token::Int
            | Token::Float
            | Token::Str
            | Token::Bool
            | Token::Own
            | Token::Share
            | Token::Joint
            | Token::Exclusive
            | Token::Whole
            | Token::Amp => self.parse_var_decl(),
            Token::Var | Token::Let => self.parse_infer_var_decl(),
            Token::Move => {
                // `move x;` as expression statement (or allow as expr in other contexts)
                let expr = self.parse_expr()?;
                let span = expr.span;
                self.optional_semi();
                Ok(Stmt {
                    span,
                    kind: StmtKind::Expr(expr),
                })
            }
            Token::If => self.parse_if(),
            Token::While => self.parse_while(),
            Token::For => self.parse_for(),
            Token::Break => {
                let tok = self.bump();
                self.optional_semi();
                Ok(Stmt {
                    span: tok.span,
                    kind: StmtKind::Break,
                })
            }
            Token::Continue => {
                let tok = self.bump();
                self.optional_semi();
                Ok(Stmt {
                    span: tok.span,
                    kind: StmtKind::Continue,
                })
            }
            Token::Print => self.parse_print_stmt(),
            Token::Return => {
                let start = self.bump();
                let value = if matches!(self.peek_kind(), Token::Semicolon | Token::RBrace) {
                    None
                } else {
                    Some(self.parse_expr()?)
                };
                let end = value.as_ref().map(|e| e.span).unwrap_or(start.span);
                self.optional_semi();
                Ok(Stmt {
                    span: start.span.merge(end),
                    kind: StmtKind::Return(value),
                })
            }
            Token::Goto => {
                let start = self.bump();
                self.expect_kind(Token::Dot)?;
                match self.bump() {
                    SpannedToken {
                        kind: Token::End, ..
                    } => {}
                    SpannedToken {
                        kind: Token::Ident(ref s),
                        ..
                    } if s == "end" => {}
                    other => {
                        return Err(Diagnostic::error(format!(
                            "expected `end`, found {}",
                            other.kind
                        ))
                        .label(other.span, "expected `end`")
                        .help("early return uses `goto.end.this.fn`"));
                    }
                }
                self.expect_kind(Token::Dot)?;
                self.expect_ident("this")?;
                self.expect_kind(Token::Dot)?;
                let end = match self.bump() {
                    t if matches!(t.kind, Token::Fn) => t.span,
                    SpannedToken {
                        kind: Token::Ident(ref s),
                        span,
                    } if s == "fn" => span,
                    other => {
                        return Err(Diagnostic::error(format!(
                            "expected `fn`, found {}",
                            other.kind
                        ))
                        .label(other.span, "expected `fn`")
                        .help("early return uses `goto.end.this.fn`"));
                    }
                };
                self.optional_semi();
                Ok(Stmt {
                    span: start.span.merge(end),
                    kind: StmtKind::Return(None),
                })
            }
            Token::Star => {
                let star = self.bump();
                let addr = self.parse_unary()?;
                self.expect_kind(Token::Eq)?;
                let value = self.parse_expr()?;
                let span = star.span.merge(value.span);
                self.optional_semi();
                return Ok(Stmt {
                    span,
                    kind: StmtKind::DerefAssign { ptr: addr, value },
                });
            }
            Token::Ident(_) => {
                if self.looks_like_typed_var_decl() {
                    return self.parse_var_decl();
                }
                if let Some(op) = self.looks_like_compound_field_assign() {
                    return self.parse_compound_field_assign(op);
                }
                if self.looks_like_index_assign() {
                    return self.parse_index_assign();
                }
                if self.looks_like_field_assign() {
                    let base_tok = self.bump();
                    let Token::Ident(base) = base_tok.kind else {
                        unreachable!();
                    };
                    self.expect_kind(Token::Dot)?;
                    let field_tok = self.bump();
                    let Token::Ident(field) = field_tok.kind else {
                        return Err(Diagnostic::error(format!(
                            "expected field name, found {}",
                            field_tok.kind
                        ))
                        .label(field_tok.span, "expected identifier"));
                    };
                    self.expect_kind(Token::Eq)?;
                    let value = self.parse_expr()?;
                    let span = base_tok.span.merge(value.span);
                    self.optional_semi();
                    return Ok(Stmt {
                        span,
                        kind: StmtKind::FieldAssign {
                            base,
                            base_span: base_tok.span,
                            field,
                            field_span: field_tok.span,
                            value,
                        },
                    });
                }
                let checkpoint = self.pos;
                let name_tok = self.bump();
                if let Token::Ident(name) = &name_tok.kind {
                    if let Some(op) = compound_op(self.peek_kind()) {
                        self.bump();
                        let rhs = self.parse_expr()?;
                        let span = name_tok.span.merge(rhs.span);
                        self.optional_semi();
                        let lhs = Expr {
                            span: name_tok.span,
                            kind: ExprKind::Ident(name.clone()),
                        };
                        let value = Expr {
                            span,
                            kind: ExprKind::Binary {
                                op,
                                left: Box::new(lhs),
                                right: Box::new(rhs),
                            },
                        };
                        return Ok(Stmt {
                            span,
                            kind: StmtKind::Assign {
                                name: name.clone(),
                                name_span: name_tok.span,
                                value,
                            },
                        });
                    }
                    if matches!(self.peek_kind(), Token::Eq) {
                        self.bump();
                        let value = self.parse_expr()?;
                        let span = name_tok.span.merge(value.span);
                        self.optional_semi();
                        return Ok(Stmt {
                            span,
                            kind: StmtKind::Assign {
                                name: name.clone(),
                                name_span: name_tok.span,
                                value,
                            },
                        });
                    }
                }
                self.pos = checkpoint;
                let expr = self.parse_expr()?;
                let span = expr.span;
                self.optional_semi();
                Ok(Stmt {
                    span,
                    kind: StmtKind::Expr(expr),
                })
            }
            _ => {
                let expr = self.parse_expr()?;
                let span = expr.span;
                self.optional_semi();
                Ok(Stmt {
                    span,
                    kind: StmtKind::Expr(expr),
                })
            }
        }
    }

    fn parse_print_stmt(&mut self) -> Result<Stmt, Diagnostic> {
        let start = self.expect_kind(Token::Print)?;
        let args = if matches!(self.peek_kind(), Token::LParen) {
            self.bump();
            let mut args = Vec::new();
            if !matches!(self.peek_kind(), Token::RParen) {
                loop {
                    args.push(self.parse_expr()?);
                    if matches!(self.peek_kind(), Token::Comma) {
                        self.bump();
                        continue;
                    }
                    break;
                }
            }
            self.expect_kind(Token::RParen)?;
            args
        } else {
            let mut args = Vec::new();
            args.push(self.parse_expr()?);
            while matches!(self.peek_kind(), Token::Comma) {
                self.bump();
                args.push(self.parse_expr()?);
            }
            args
        };
        let end = args.last().map(|e| e.span).unwrap_or(start.span);
        self.optional_semi();
        Ok(Stmt {
            span: start.span.merge(end),
            kind: StmtKind::Print(args),
        })
    }

    fn expect_ident(&mut self, expected: &str) -> Result<SpannedToken, Diagnostic> {
        let tok = self.bump();
        match &tok.kind {
            Token::Ident(s) if s == expected => Ok(tok),
            _ => Err(Diagnostic::error(format!(
                "expected `{expected}`, found {}",
                tok.kind
            ))
            .label(tok.span, format!("expected `{expected}`"))),
        }
    }

    fn looks_like_typed_var_decl(&self) -> bool {
        // `Type name =` including dotted paths: `std.math.Vec2 v = ...`
        if !matches!(self.peek_kind(), Token::Ident(_)) {
            return false;
        }
        let mut i = self.pos + 1;
        while matches!(
            self.tokens.get(i).map(|t| &t.kind),
            Some(Token::Dot)
        ) {
            i += 1;
            if !matches!(
                self.tokens.get(i).map(|t| &t.kind),
                Some(Token::Ident(_))
            ) {
                return false;
            }
            i += 1;
        }
        // optional template args `<...>`
        if matches!(self.tokens.get(i).map(|t| &t.kind), Some(Token::Lt)) {
            let mut depth = 1usize;
            i += 1;
            while let Some(tok) = self.tokens.get(i) {
                match &tok.kind {
                    Token::Lt => depth += 1,
                    Token::Gt => {
                        depth -= 1;
                        if depth == 0 {
                            i += 1;
                            break;
                        }
                    }
                    Token::Eof => return false,
                    _ => {}
                }
                i += 1;
            }
        }
        matches!(
            self.tokens.get(i).map(|t| &t.kind),
            Some(Token::Ident(_))
        ) && matches!(
            self.tokens.get(i + 1).map(|t| &t.kind),
            Some(Token::Eq | Token::LBracket)
        )
    }

    fn looks_like_index_assign(&self) -> bool {
        if !matches!(self.peek_kind(), Token::Ident(_)) {
            return false;
        }
        let mut depth = 0usize;
        let mut i = self.pos + 1;
        loop {
            let Some(tok) = self.tokens.get(i) else {
                return false;
            };
            match &tok.kind {
                Token::LBracket => depth += 1,
                Token::RBracket => {
                    if depth == 0 {
                        return false;
                    }
                    depth -= 1;
                    if depth == 0 {
                        return matches!(
                            self.tokens.get(i + 1).map(|t| &t.kind),
                            Some(Token::Eq)
                        );
                    }
                }
                Token::Semicolon | Token::RBrace | Token::Eof => return false,
                _ if depth == 0 => return false,
                _ => {}
            }
            i += 1;
        }
    }

    fn parse_index_assign(&mut self) -> Result<Stmt, Diagnostic> {
        let base_tok = self.bump();
        let Token::Ident(base) = base_tok.kind else {
            unreachable!();
        };
        self.expect_kind(Token::LBracket)?;
        let index = self.parse_expr()?;
        self.expect_kind(Token::RBracket)?;
        self.expect_kind(Token::Eq)?;
        let value = self.parse_expr()?;
        let span = base_tok.span.merge(value.span);
        self.optional_semi();
        Ok(Stmt {
            span,
            kind: StmtKind::IndexAssign {
                base,
                base_span: base_tok.span,
                index,
                value,
            },
        })
    }

    fn looks_like_field_assign(&self) -> bool {
        matches!(self.peek_kind(), Token::Ident(_))
            && matches!(
                self.tokens.get(self.pos + 1).map(|t| &t.kind),
                Some(Token::Dot)
            )
            && matches!(
                self.tokens.get(self.pos + 2).map(|t| &t.kind),
                Some(Token::Ident(_))
            )
            && matches!(
                self.tokens.get(self.pos + 3).map(|t| &t.kind),
                Some(Token::Eq)
            )
    }

    fn looks_like_compound_field_assign(&self) -> Option<BinOp> {
        if !matches!(self.peek_kind(), Token::Ident(_)) {
            return None;
        }
        if !matches!(
            self.tokens.get(self.pos + 1).map(|t| &t.kind),
            Some(Token::Dot)
        ) {
            return None;
        }
        if !matches!(
            self.tokens.get(self.pos + 2).map(|t| &t.kind),
            Some(Token::Ident(_))
        ) {
            return None;
        }
        compound_op(
            match self.tokens.get(self.pos + 3).map(|t| &t.kind) {
                Some(t) => t,
                None => return None,
            },
        )
    }

    fn parse_compound_field_assign(&mut self, op: BinOp) -> Result<Stmt, Diagnostic> {
        let base_tok = self.bump();
        let Token::Ident(base) = base_tok.kind else {
            unreachable!();
        };
        self.expect_kind(Token::Dot)?;
        let field_tok = self.bump();
        let Token::Ident(field) = field_tok.kind else {
            return Err(Diagnostic::error(format!(
                "expected field name, found {}",
                field_tok.kind
            ))
            .label(field_tok.span, "expected identifier"));
        };
        self.bump(); // compound op
        let rhs = self.parse_expr()?;
        let span = base_tok.span.merge(rhs.span);
        self.optional_semi();
        let lhs = Expr {
            span: base_tok.span.merge(field_tok.span),
            kind: ExprKind::Field {
                base: Box::new(Expr {
                    span: base_tok.span,
                    kind: ExprKind::Ident(base.clone()),
                }),
                field: field.clone(),
                field_span: field_tok.span,
            },
        };
        let value = Expr {
            span,
            kind: ExprKind::Binary {
                op,
                left: Box::new(lhs),
                right: Box::new(rhs),
            },
        };
        Ok(Stmt {
            span,
            kind: StmtKind::FieldAssign {
                base,
                base_span: base_tok.span,
                field,
                field_span: field_tok.span,
                value,
            },
        })
    }

    fn optional_semi(&mut self) {
        if matches!(self.peek_kind(), Token::Semicolon) {
            self.bump();
        }
    }

    fn parse_infer_var_decl(&mut self) -> Result<Stmt, Diagnostic> {
        let start = self.bump(); // var | let
        let name_tok = self.bump();
        let Token::Ident(name) = name_tok.kind else {
            return Err(Diagnostic::error(format!(
                "expected variable name, found {}",
                name_tok.kind
            ))
            .label(name_tok.span, "expected identifier")
            .help("variable declaration: `var x = 1`"));
        };
        let ty = if matches!(self.peek_kind(), Token::Colon) {
            self.bump();
            self.parse_type()?.0
        } else {
            Type::Infer
        };
        self.expect_kind(Token::Eq)?;
        let init = self.parse_expr()?;
        let span = start.span.merge(init.span);
        self.optional_semi();
        Ok(Stmt {
            span,
            kind: StmtKind::VarDecl {
                name,
                name_span: name_tok.span,
                ty,
                own: OwnSpec::default(),
                init,
            },
        })
    }

    fn parse_var_decl(&mut self) -> Result<Stmt, Diagnostic> {
        let (ty, ty_span) = self.parse_type()?;
        let own = ty.own_spec();
        let name_tok = self.bump();
        let Token::Ident(name) = name_tok.kind else {
            return Err(Diagnostic::error(format!(
                "expected variable name, found {}",
                name_tok.kind
            ))
            .label(name_tok.span, "expected identifier")
            .help("variable declaration: `int x = 1` or `own int x = 1`"));
        };

        if matches!(self.peek_kind(), Token::LBracket) {
            if own.is_managed() {
                return Err(Diagnostic::error(
                    "owned array declarations are not supported yet",
                )
                .label(ty_span, "remove ownership qualifier"));
            }
            self.bump();
            let len_tok = self.bump();
            let Token::IntLit(len) = len_tok.kind else {
                return Err(Diagnostic::error(format!(
                    "expected array length, found {}",
                    len_tok.kind
                ))
                .label(len_tok.span, "expected integer literal"));
            };
            if len < 0 {
                return Err(Diagnostic::error("array length must be non-negative")
                    .label(len_tok.span, "invalid array length"));
            }
            self.expect_kind(Token::RBracket)?;
            let span = ty_span.merge(len_tok.span);
            self.optional_semi();
            return Ok(Stmt {
                span,
                kind: StmtKind::ArrayDecl {
                    name,
                    name_span: name_tok.span,
                    elem: ty,
                    len,
                },
            });
        }

        self.expect_kind(Token::Eq)?;
        let init = self.parse_expr()?;
        let span = ty_span.merge(init.span);
        self.optional_semi();
        Ok(Stmt {
            span,
            kind: StmtKind::VarDecl {
                name,
                name_span: name_tok.span,
                ty,
                own,
                init,
            },
        })
    }

    fn parse_if(&mut self) -> Result<Stmt, Diagnostic> {
        let if_tok = self.expect_kind(Token::If)?;
        self.parse_if_rest(if_tok.span)
    }

    fn parse_if_rest(&mut self, start_span: Span) -> Result<Stmt, Diagnostic> {
        // Always parse a full expression so `if (a & 1) == 1` works.
        let cond = self.parse_expr()?;
        let then_body = self.parse_block()?;

        let (else_body, end_span) = if matches!(self.peek_kind(), Token::Elif) {
            let elif_tok = self.bump();
            let nested = self.parse_if_rest(elif_tok.span)?;
            let end = nested.span;
            (vec![nested], end)
        } else if matches!(self.peek_kind(), Token::Else) {
            self.bump();
            if matches!(self.peek_kind(), Token::If) {
                let nested = self.parse_if()?;
                let end = nested.span;
                (vec![nested], end)
            } else {
                let body = self.parse_block()?;
                let end = body.last().map(|s| s.span).unwrap_or(start_span);
                (body, end)
            }
        } else if matches!(self.peek_kind(), Token::While) {
            self.bump();
            let while_body = self.parse_block()?;
            let while_stmt = Stmt {
                span: cond.span.merge(
                    while_body
                        .last()
                        .map(|s| s.span)
                        .unwrap_or(cond.span),
                ),
                kind: StmtKind::While {
                    cond: cond.clone(),
                    body: while_body,
                },
            };
            let mut combined = then_body;
            let end = while_stmt.span;
            combined.push(while_stmt);
            return Ok(Stmt {
                span: start_span.merge(end),
                kind: StmtKind::If {
                    cond,
                    then_body: combined,
                    else_body: vec![],
                },
            });
        } else {
            let end = then_body.last().map(|s| s.span).unwrap_or(start_span);
            (vec![], end)
        };

        Ok(Stmt {
            span: start_span.merge(end_span),
            kind: StmtKind::If {
                cond,
                then_body,
                else_body,
            },
        })
    }

    fn parse_while(&mut self) -> Result<Stmt, Diagnostic> {
        let while_tok = self.expect_kind(Token::While)?;
        let cond = if matches!(self.peek_kind(), Token::LBrace) {
            Expr::bool_lit(while_tok.span, true)
        } else {
            self.parse_expr()?
        };
        let body = self.parse_block()?;
        let end = body.last().map(|s| s.span).unwrap_or(cond.span);
        Ok(Stmt {
            span: while_tok.span.merge(end),
            kind: StmtKind::While { cond, body },
        })
    }

    fn parse_for(&mut self) -> Result<Stmt, Diagnostic> {
        let for_tok = self.expect_kind(Token::For)?;
        let name_tok = self.bump();
        let Token::Ident(name) = name_tok.kind else {
            return Err(Diagnostic::error(format!(
                "expected loop variable, found {}",
                name_tok.kind
            ))
            .label(name_tok.span, "expected identifier")
            .help("`for i in 0..n { ... }`"));
        };
        self.expect_kind(Token::In)?;
        let start = self.parse_expr()?;
        self.expect_kind(Token::DotDot)?;
        let end_expr = self.parse_expr()?;
        let body = self.parse_block()?;
        let end = body.last().map(|s| s.span).unwrap_or(end_expr.span);
        Ok(Stmt {
            span: for_tok.span.merge(end),
            kind: StmtKind::For {
                name,
                name_span: name_tok.span,
                start,
                end: end_expr,
                body,
            },
        })
    }

    fn parse_expr(&mut self) -> Result<Expr, Diagnostic> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.parse_and()?;
        while matches!(self.peek_kind(), Token::OrOr) {
            self.bump();
            let right = self.parse_and()?;
            let span = left.span.merge(right.span);
            left = Expr {
                span,
                kind: ExprKind::Binary {
                    op: BinOp::Or,
                    left: Box::new(left),
                    right: Box::new(right),
                },
            };
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.parse_bitor()?;
        while matches!(self.peek_kind(), Token::AndAnd) {
            self.bump();
            let right = self.parse_bitor()?;
            let span = left.span.merge(right.span);
            left = Expr {
                span,
                kind: ExprKind::Binary {
                    op: BinOp::And,
                    left: Box::new(left),
                    right: Box::new(right),
                },
            };
        }
        Ok(left)
    }

    fn parse_bitor(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.parse_bitxor()?;
        while matches!(self.peek_kind(), Token::Pipe) {
            self.bump();
            let right = self.parse_bitxor()?;
            let span = left.span.merge(right.span);
            left = Expr {
                span,
                kind: ExprKind::Binary {
                    op: BinOp::BitOr,
                    left: Box::new(left),
                    right: Box::new(right),
                },
            };
        }
        Ok(left)
    }

    fn parse_bitxor(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.parse_bitand()?;
        while matches!(self.peek_kind(), Token::Caret) {
            self.bump();
            let right = self.parse_bitand()?;
            let span = left.span.merge(right.span);
            left = Expr {
                span,
                kind: ExprKind::Binary {
                    op: BinOp::BitXor,
                    left: Box::new(left),
                    right: Box::new(right),
                },
            };
        }
        Ok(left)
    }

    fn parse_bitand(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.parse_equality()?;
        while matches!(self.peek_kind(), Token::Amp) {
            self.bump();
            let right = self.parse_equality()?;
            let span = left.span.merge(right.span);
            left = Expr {
                span,
                kind: ExprKind::Binary {
                    op: BinOp::BitAnd,
                    left: Box::new(left),
                    right: Box::new(right),
                },
            };
        }
        Ok(left)
    }

    fn parse_equality(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.parse_compare()?;
        loop {
            let op = match self.peek_kind() {
                Token::EqEq => BinOp::Eq,
                Token::Ne => BinOp::Ne,
                _ => break,
            };
            self.bump();
            let right = self.parse_compare()?;
            let span = left.span.merge(right.span);
            left = Expr {
                span,
                kind: ExprKind::Binary {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                },
            };
        }
        Ok(left)
    }

    fn parse_compare(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.parse_shift()?;
        loop {
            let op = match self.peek_kind() {
                Token::Lt => BinOp::Lt,
                Token::Le => BinOp::Le,
                Token::Gt => BinOp::Gt,
                Token::Ge => BinOp::Ge,
                _ => break,
            };
            self.bump();
            let right = self.parse_shift()?;
            let span = left.span.merge(right.span);
            left = Expr {
                span,
                kind: ExprKind::Binary {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                },
            };
        }
        Ok(left)
    }

    fn parse_shift(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.parse_term()?;
        loop {
            let op = match self.peek_kind() {
                Token::Shl => BinOp::Shl,
                Token::Shr => BinOp::Shr,
                _ => break,
            };
            self.bump();
            let right = self.parse_term()?;
            let span = left.span.merge(right.span);
            left = Expr {
                span,
                kind: ExprKind::Binary {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                },
            };
        }
        Ok(left)
    }

    fn parse_term(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.parse_factor()?;
        loop {
            let op = match self.peek_kind() {
                Token::Plus => BinOp::Add,
                Token::Minus => BinOp::Sub,
                _ => break,
            };
            self.bump();
            let right = self.parse_factor()?;
            let span = left.span.merge(right.span);
            left = Expr {
                span,
                kind: ExprKind::Binary {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                },
            };
        }
        Ok(left)
    }

    fn parse_factor(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.parse_unary()?;
        loop {
            let op = match self.peek_kind() {
                Token::Star => BinOp::Mul,
                Token::Slash => BinOp::Div,
                Token::Percent => BinOp::Mod,
                _ => break,
            };
            self.bump();
            let right = self.parse_unary()?;
            let span = left.span.merge(right.span);
            left = Expr {
                span,
                kind: ExprKind::Binary {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                },
            };
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Expr, Diagnostic> {
        if matches!(self.peek_kind(), Token::Minus) {
            let op = self.bump();
            let e = self.parse_unary()?;
            let span = op.span.merge(e.span);
            return Ok(match &e.kind {
                ExprKind::FloatLit(n) => Expr::float_lit(span, -*n),
                _ => Expr {
                    span,
                    kind: ExprKind::Binary {
                        op: BinOp::Sub,
                        left: Box::new(Expr::int_lit(op.span, 0)),
                        right: Box::new(e),
                    },
                },
            });
        }
        if matches!(self.peek_kind(), Token::Bang) {
            let op = self.bump();
            let e = self.parse_unary()?;
            let span = op.span.merge(e.span);
            return Ok(Expr {
                span,
                kind: ExprKind::Binary {
                    op: BinOp::Eq,
                    left: Box::new(e),
                    right: Box::new(Expr::bool_lit(op.span, false)),
                },
            });
        }
        if matches!(self.peek_kind(), Token::Move) {
            let op = self.bump();
            let e = self.parse_unary()?;
            let span = op.span.merge(e.span);
            return Ok(Expr {
                span,
                kind: ExprKind::Unary {
                    op: UnaryOp::Move,
                    expr: Box::new(e),
                },
            });
        }
        if matches!(self.peek_kind(), Token::Amp) {
            let op = self.bump();
            let mutable = if matches!(self.peek_kind(), Token::Mut) {
                self.bump();
                true
            } else {
                false
            };
            let e = self.parse_unary()?;
            let span = op.span.merge(e.span);
            return Ok(Expr {
                span,
                kind: ExprKind::Unary {
                    op: if mutable {
                        UnaryOp::AddrOfMut
                    } else {
                        UnaryOp::AddrOf
                    },
                    expr: Box::new(e),
                },
            });
        }
        if matches!(self.peek_kind(), Token::Star) {
            let op = self.bump();
            let e = self.parse_unary()?;
            let span = op.span.merge(e.span);
            return Ok(Expr {
                span,
                kind: ExprKind::Unary {
                    op: UnaryOp::Deref,
                    expr: Box::new(e),
                },
            });
        }
        self.parse_postfix()
    }

    fn parse_postfix(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_primary()?;
        while matches!(self.peek_kind(), Token::LBracket) {
            let open = self.bump();
            let index = self.parse_expr()?;
            let close = self.expect_kind(Token::RBracket)?;
            let span = expr.span.merge(close.span);
            expr = Expr {
                span: open.span.merge(span),
                kind: ExprKind::Index {
                    base: Box::new(expr),
                    index: Box::new(index),
                },
            };
        }
        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expr, Diagnostic> {
        let tok = self.bump();
        match tok.kind {
            Token::IntLit(n) => Ok(Expr {
                span: tok.span,
                kind: ExprKind::IntLit(n),
            }),
            Token::FloatLit(n) => Ok(Expr {
                span: tok.span,
                kind: ExprKind::FloatLit(n),
            }),
            Token::StrLit(s) => Ok(Expr {
                span: tok.span,
                kind: ExprKind::StrLit(s),
            }),
            Token::True => Ok(Expr {
                span: tok.span,
                kind: ExprKind::BoolLit(true),
            }),
            Token::False => Ok(Expr {
                span: tok.span,
                kind: ExprKind::BoolLit(false),
            }),
            Token::LParen => {
                let e = self.parse_expr()?;
                let close = self.expect_kind(Token::RParen)?;
                Ok(Expr {
                    span: tok.span.merge(close.span),
                    kind: e.kind,
                })
            }
            Token::Ident(name) => {
                let type_args = if self.looks_like_type_args() {
                    self.parse_type_args()?
                } else {
                    Vec::new()
                };
                // Struct literal: `Point { ... }` / `Box<int> { ... }`
                if matches!(self.peek_kind(), Token::LBrace) && self.looks_like_struct_lit_body() {
                    return self.parse_struct_lit(name, tok.span, type_args);
                }

                let mut parts = vec![(name, tok.span)];
                let mut span = tok.span;
                while matches!(self.peek_kind(), Token::Dot) {
                    self.bump();
                    let part = self.bump();
                    if let Some(p) = path_segment_name(&part.kind) {
                        span = span.merge(part.span);
                        parts.push((p, part.span));
                    } else {
                        return Err(Diagnostic::error(format!(
                            "expected identifier after `.`, found {}",
                            part.kind
                        ))
                        .label(part.span, "expected identifier"));
                    }
                }

                if matches!(self.peek_kind(), Token::LParen) {
                    self.bump();
                    let mut args = Vec::new();
                    if !matches!(self.peek_kind(), Token::RParen) {
                        loop {
                            args.push(self.parse_expr()?);
                            if matches!(self.peek_kind(), Token::Comma) {
                                self.bump();
                                continue;
                            }
                            break;
                        }
                    }
                    let close = self.expect_kind(Token::RParen)?;
                    let span = tok.span.merge(close.span);
                    if parts.len() >= 2 && type_args.is_empty() {
                        // Method call: recv.method(args)
                        let (method, method_span) = parts.last().unwrap().clone();
                        let mut receiver = Expr {
                            span: parts[0].1,
                            kind: ExprKind::Ident(parts[0].0.clone()),
                        };
                        for (fname, fspan) in parts.iter().skip(1).take(parts.len() - 2) {
                            let es = receiver.span.merge(*fspan);
                            receiver = Expr {
                                span: es,
                                kind: ExprKind::Field {
                                    base: Box::new(receiver),
                                    field: fname.clone(),
                                    field_span: *fspan,
                                },
                            };
                        }
                        Ok(Expr {
                            span,
                            kind: ExprKind::MethodCall {
                                receiver: Box::new(receiver),
                                method,
                                method_span,
                                args,
                            },
                        })
                    } else {
                        let callee = parts
                            .iter()
                            .map(|(s, _)| s.as_str())
                            .collect::<Vec<_>>()
                            .join(".");
                        Ok(Expr {
                            span,
                            kind: ExprKind::Call {
                                callee,
                                callee_span: span,
                                type_args,
                                args,
                            },
                        })
                    }
                } else if matches!(self.peek_kind(), Token::LBrace)
                    && self.looks_like_struct_lit_body()
                {
                    let full = parts
                        .iter()
                        .map(|(s, _)| s.as_str())
                        .collect::<Vec<_>>()
                        .join(".");
                    self.parse_struct_lit(full, tok.span, type_args)
                } else if parts.len() == 1 {
                    Ok(Expr {
                        span: parts[0].1,
                        kind: ExprKind::Ident(parts[0].0.clone()),
                    })
                } else {
                    let mut expr = Expr {
                        span: parts[0].1,
                        kind: ExprKind::Ident(parts[0].0.clone()),
                    };
                    for (fname, fspan) in parts.into_iter().skip(1) {
                        let es = expr.span.merge(fspan);
                        expr = Expr {
                            span: es,
                            kind: ExprKind::Field {
                                base: Box::new(expr),
                                field: fname,
                                field_span: fspan,
                            },
                        };
                    }
                    Ok(expr)
                }
            }
            Token::Float if matches!(self.peek_kind(), Token::LParen) => {
                let callee_span = tok.span;
                self.bump();
                let mut args = Vec::new();
                if !matches!(self.peek_kind(), Token::RParen) {
                    loop {
                        args.push(self.parse_expr()?);
                        if matches!(self.peek_kind(), Token::Comma) {
                            self.bump();
                            continue;
                        }
                        break;
                    }
                }
                let close = self.expect_kind(Token::RParen)?;
                Ok(Expr {
                    span: tok.span.merge(close.span),
                    kind: ExprKind::Call {
                        callee: "float".into(),
                        callee_span,
                        type_args: Vec::new(),
                        args,
                    },
                })
            }
            other => Err(Diagnostic::error(format!(
                "expected expression, found {other}"
            ))
            .label(tok.span, "expected expression")),
        }
    }

    fn looks_like_struct_lit_body(&self) -> bool {
        // peek is `{`; look at tokens after it
        match self.tokens.get(self.pos + 1).map(|t| &t.kind) {
            Some(Token::RBrace) => true,
            Some(Token::Ident(_)) => {
                matches!(
                    self.tokens.get(self.pos + 2).map(|t| &t.kind),
                    Some(Token::Colon)
                )
            }
            _ => false,
        }
    }

    fn parse_struct_lit(
        &mut self,
        name: String,
        name_span: Span,
        type_args: Vec<Type>,
    ) -> Result<Expr, Diagnostic> {
        self.expect_kind(Token::LBrace)?;
        let mut fields = Vec::new();
        if !matches!(self.peek_kind(), Token::RBrace) {
            loop {
                let fname = self.bump();
                let Token::Ident(fnm) = fname.kind else {
                    return Err(Diagnostic::error(format!(
                        "expected field name, found {}",
                        fname.kind
                    ))
                    .label(fname.span, "expected identifier"));
                };
                self.expect_kind(Token::Colon)?;
                let value = self.parse_expr()?;
                fields.push((fnm, fname.span, value));
                if matches!(self.peek_kind(), Token::Comma) {
                    self.bump();
                    if matches!(self.peek_kind(), Token::RBrace) {
                        break;
                    }
                    continue;
                }
                break;
            }
        }
        let close = self.expect_kind(Token::RBrace)?;
        Ok(Expr {
            span: name_span.merge(close.span),
            kind: ExprKind::StructLit {
                name,
                name_span,
                type_args,
                fields,
            },
        })
    }
}

fn compound_op(tok: &Token) -> Option<BinOp> {
    match tok {
        Token::PlusEq => Some(BinOp::Add),
        Token::MinusEq => Some(BinOp::Sub),
        Token::StarEq => Some(BinOp::Mul),
        Token::SlashEq => Some(BinOp::Div),
        Token::PercentEq => Some(BinOp::Mod),
        _ => None,
    }
}

/// Allow keywords as path segments so `std.own`, `std.string` etc. work.
fn path_segment_name(tok: &Token) -> Option<String> {
    match tok {
        Token::Ident(s) => Some(s.clone()),
        Token::Own => Some("own".into()),
        Token::Share => Some("share".into()),
        Token::Joint => Some("joint".into()),
        Token::Whole => Some("whole".into()),
        Token::Exclusive => Some("exclusive".into()),
        Token::Move => Some("move".into()),
        Token::Mut => Some("mut".into()),
        Token::Ref => Some("ref".into()),
        Token::Out => Some("out".into()),
        Token::Str => Some("str".into()),
        Token::Int => Some("int".into()),
        Token::Float => Some("float".into()),
        Token::Bool => Some("bool".into()),
        Token::In => Some("in".into()),
        Token::End => Some("end".into()),
        _ => None,
    }
}

fn prefix_type(ty: &mut Type, ns: &str, locals: &[String]) {
    match ty {
        Type::Named(n) if locals.iter().any(|s| s == n) => {
            *n = format!("{ns}.{n}");
        }
        Type::Apply { name, args } => {
            if locals.iter().any(|s| s == name) {
                *name = format!("{ns}.{name}");
            }
            for a in args {
                prefix_type(a, ns, locals);
            }
        }
        Type::Ptr(inner) | Type::Owned { inner, .. } | Type::Borrowed { inner, .. } => {
            prefix_type(inner, ns, locals);
        }
        Type::Array { elem, .. } => prefix_type(elem, ns, locals),
        _ => {}
    }
}

fn prefix_types_in_function(f: &mut Function, ns: &str, locals: &[String]) {
    for p in &mut f.params {
        prefix_type(&mut p.ty, ns, locals);
    }
    if let Some(ret) = &mut f.ret_ty {
        prefix_type(ret, ns, locals);
    }
    prefix_types_in_stmts(&mut f.body, ns, locals);
}

fn prefix_types_in_struct(s: &mut StructDef, ns: &str, locals: &[String]) {
    for field in &mut s.fields {
        prefix_type(&mut field.ty, ns, locals);
    }
    for m in &mut s.methods {
        prefix_types_in_function(&mut m.func, ns, locals);
    }
}

fn prefix_types_in_stmts(stmts: &mut [Stmt], ns: &str, locals: &[String]) {
    for stmt in stmts {
        match &mut stmt.kind {
            StmtKind::VarDecl { ty, init, .. } => {
                prefix_type(ty, ns, locals);
                prefix_types_in_expr(init, ns, locals);
            }
            StmtKind::ArrayDecl { elem, .. } => prefix_type(elem, ns, locals),
            StmtKind::Assign { value, .. }
            | StmtKind::Return(Some(value))
            | StmtKind::Expr(value) => prefix_types_in_expr(value, ns, locals),
            StmtKind::IndexAssign { index, value, .. } => {
                prefix_types_in_expr(index, ns, locals);
                prefix_types_in_expr(value, ns, locals);
            }
            StmtKind::DerefAssign { ptr, value } => {
                prefix_types_in_expr(ptr, ns, locals);
                prefix_types_in_expr(value, ns, locals);
            }
            StmtKind::FieldAssign { value, .. } => prefix_types_in_expr(value, ns, locals),
            StmtKind::If {
                cond,
                then_body,
                else_body,
            } => {
                prefix_types_in_expr(cond, ns, locals);
                prefix_types_in_stmts(then_body, ns, locals);
                prefix_types_in_stmts(else_body, ns, locals);
            }
            StmtKind::While { cond, body } => {
                prefix_types_in_expr(cond, ns, locals);
                prefix_types_in_stmts(body, ns, locals);
            }
            StmtKind::For {
                start, end, body, ..
            } => {
                prefix_types_in_expr(start, ns, locals);
                prefix_types_in_expr(end, ns, locals);
                prefix_types_in_stmts(body, ns, locals);
            }
            StmtKind::Print(args) => {
                for a in args {
                    prefix_types_in_expr(a, ns, locals);
                }
            }
            _ => {}
        }
    }
}

fn prefix_types_in_expr(expr: &mut Expr, ns: &str, locals: &[String]) {
    match &mut expr.kind {
        ExprKind::Unary { expr, .. } => prefix_types_in_expr(expr, ns, locals),
        ExprKind::Binary { left, right, .. } => {
            prefix_types_in_expr(left, ns, locals);
            prefix_types_in_expr(right, ns, locals);
        }
        ExprKind::Call {
            type_args, args, ..
        } => {
            for t in type_args {
                prefix_type(t, ns, locals);
            }
            for a in args {
                prefix_types_in_expr(a, ns, locals);
            }
        }
        ExprKind::MethodCall {
            receiver, args, ..
        } => {
            prefix_types_in_expr(receiver, ns, locals);
            for a in args {
                prefix_types_in_expr(a, ns, locals);
            }
        }
        ExprKind::StructLit {
            name,
            type_args,
            fields,
            ..
        } => {
            if locals.iter().any(|s| s == name) {
                *name = format!("{ns}.{name}");
            }
            for t in type_args {
                prefix_type(t, ns, locals);
            }
            for (_, _, e) in fields {
                prefix_types_in_expr(e, ns, locals);
            }
        }
        ExprKind::Field { base, .. } => prefix_types_in_expr(base, ns, locals),
        ExprKind::Index { base, index } => {
            prefix_types_in_expr(base, ns, locals);
            prefix_types_in_expr(index, ns, locals);
        }
        _ => {}
    }
}
