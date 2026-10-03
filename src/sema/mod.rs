use crate::ast::*;
use crate::diag::Diagnostics;
use crate::span::Span;
use crate::types::*;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct LocalVarInfo {
    pub name: String,
    pub ty: Type,
    pub offset: i32, // Offset relative to FP (Frame Pointer) or SP
    pub global_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct GlobalVarInfo {
    pub name: String,
    pub ty: Type,
    pub is_static: bool,
    pub is_extern: bool,
    pub init: Option<Initializer>,
}

#[derive(Debug, Clone)]
pub struct StringLiteral {
    pub label: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypedExprKind {
    Int(i64),
    Float(f64),
    Char(char),
    StringLiteral(String), // label in .rodata
    LocalVar(i32),         // offset
    GlobalVar(String),     // name
    Binary {
        op: BinaryOp,
        lhs: Box<TypedExpr>,
        rhs: Box<TypedExpr>,
    },
    Unary {
        op: UnaryOp,
        expr: Box<TypedExpr>,
    },
    Ternary {
        cond: Box<TypedExpr>,
        then_expr: Box<TypedExpr>,
        else_expr: Box<TypedExpr>,
    },
    Call {
        callee: Box<TypedExpr>,
        args: Vec<TypedExpr>,
    },
    Member {
        expr: Box<TypedExpr>,
        offset: usize,
        bit_width: Option<usize>,
        bit_offset: Option<usize>,
    },
    Deref(Box<TypedExpr>),
    AddrOf(Box<TypedExpr>),
    AddrOfLabel(String),
    Cast {
        expr: Box<TypedExpr>,
    },
    StmtExpr(Vec<TypedStmt>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedExpr {
    pub kind: TypedExprKind,
    pub ty: Type,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypedStmt {
    Expr(TypedExpr),
    Return(Option<TypedExpr>, Span),
    Block(Vec<TypedStmt>, Span),
    If {
        cond: TypedExpr,
        then_stmt: Box<TypedStmt>,
        else_stmt: Option<Box<TypedStmt>>,
        span: Span,
    },
    While {
        cond: TypedExpr,
        body: Box<TypedStmt>,
        span: Span,
    },
    DoWhile {
        body: Box<TypedStmt>,
        cond: TypedExpr,
        span: Span,
    },
    For {
        init: Option<Box<TypedStmt>>,
        cond: Option<TypedExpr>,
        step: Option<TypedExpr>,
        body: Box<TypedStmt>,
        span: Span,
    },
    Break(Span),
    Continue(Span),
    Switch {
        expr: TypedExpr,
        body: Box<TypedStmt>,
        cases: Vec<(i64, String)>,
        default_label: Option<String>,
        break_label: String,
        span: Span,
    },
    Case {
        label: String,
        body: Box<TypedStmt>,
        span: Span,
    },
    Default {
        label: String,
        body: Box<TypedStmt>,
        span: Span,
    },
    Goto(String, Span),
    GotoExpr(TypedExpr, Span),
    Label(String, Box<TypedStmt>, Span),
    Empty(Span),
}

#[derive(Debug, Clone)]
pub struct TypedFunction {
    pub name: String,
    pub ret_type: Type,
    pub params: Vec<LocalVarInfo>,
    pub is_variadic: bool,
    pub body: Option<Vec<TypedStmt>>,
    pub is_static: bool,
    pub stack_size: usize,
}

#[derive(Debug, Clone, Default)]
pub struct TypedProgram {
    pub functions: Vec<TypedFunction>,
    pub globals: Vec<GlobalVarInfo>,
    pub string_literals: Vec<StringLiteral>,
}

pub struct Sema {
    globals: HashMap<String, GlobalVarInfo>,
    extra_globals: Vec<GlobalVarInfo>,
    function_signatures: HashMap<String, Type>,
    scopes: Vec<HashMap<String, LocalVarInfo>>,
    current_offset: i32,
    string_literals: Vec<StringLiteral>,
    str_counter: usize,
    switch_stack: Vec<SwitchContext>,
    label_counter: usize,
    static_counter: usize,
    current_func_ret_type: Option<Type>,
    current_func_name: String,
}

#[allow(dead_code)]
struct SwitchContext {
    cases: Vec<(i64, String)>,
    default_label: Option<String>,
    break_label: String,
}

impl Default for Sema {
    fn default() -> Self {
        Self::new()
    }
}

impl Sema {
    pub fn new() -> Self {
        Self {
            globals: HashMap::new(),
            extra_globals: Vec::new(),
            function_signatures: HashMap::new(),
            scopes: Vec::new(),
            current_offset: 0,
            string_literals: Vec::new(),
            str_counter: 0,
            switch_stack: Vec::new(),
            label_counter: 0,
            static_counter: 0,
            current_func_ret_type: None,
            current_func_name: String::new(),
        }
    }

    fn new_label(&mut self, prefix: &str) -> String {
        self.label_counter += 1;
        format!(".L.{}.{}", prefix, self.label_counter)
    }

    fn enter_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn exit_scope(&mut self) {
        self.scopes.pop();
    }

    fn add_local_var(&mut self, name: String, ty: Type) -> LocalVarInfo {
        let align = ty.align().max(4) as i32;
        let size = ty.size() as i32;

        self.current_offset += size;
        self.current_offset = (self.current_offset + align - 1) / align * align;

        let info = LocalVarInfo {
            name: name.clone(),
            ty,
            offset: self.current_offset,
            global_name: None,
        };

        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, info.clone());
        }

        info
    }

    fn add_static_local_var(
        &mut self,
        name: String,
        ty: Type,
        init: Option<Initializer>,
    ) -> String {
        self.static_counter += 1;
        let mangled = format!(
            "{}_{}_{}",
            self.current_func_name, name, self.static_counter
        );
        let scoped_init = init.map(|i| scope_static_init(i, &self.current_func_name));
        let g_info = GlobalVarInfo {
            name: mangled.clone(),
            ty: ty.clone(),
            is_static: true,
            is_extern: false,
            init: scoped_init,
        };
        self.globals.insert(mangled.clone(), g_info.clone());
        self.extra_globals.push(g_info);

        let info = LocalVarInfo {
            name: name.clone(),
            ty,
            offset: 0,
            global_name: Some(mangled.clone()),
        };
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, info);
        }
        mangled
    }

    fn find_var(&self, name: &str) -> Option<(Type, Option<i32>, Option<String>)> {
        for scope in self.scopes.iter().rev() {
            if let Some(info) = scope.get(name) {
                if let Some(gname) = &info.global_name {
                    return Some((info.ty.clone(), None, Some(gname.clone())));
                } else {
                    return Some((info.ty.clone(), Some(info.offset), None));
                }
            }
        }
        if let Some(g) = self.globals.get(name) {
            return Some((g.ty.clone(), None, Some(name.to_string())));
        }
        if let Some(func_ty) = self.function_signatures.get(name) {
            return Some((func_ty.clone(), None, Some(name.to_string())));
        }
        if let Some(sig) = builtin_signature(name) {
            return Some((sig, None, Some(name.to_string())));
        }
        None
    }

    pub fn analyze_program(&mut self, prog: Program, diag: &mut Diagnostics) -> TypedProgram {
        let mut typed_prog = TypedProgram::default();

        // Register all globals
        for mut g in prog.globals {
            if let Some(init) = &g.init
                && let Some(len) = infer_array_len(&g.ty, init)
                && let TypeKind::Array { elem, .. } = &g.ty.kind
            {
                g.ty = Type::new(TypeKind::Array {
                    elem: elem.clone(),
                    len: Some(len),
                });
            }

            let info = GlobalVarInfo {
                name: g.name.clone(),
                ty: g.ty.clone(),
                is_static: g.is_static,
                is_extern: g.is_extern,
                init: g.init,
            };
            self.globals.insert(g.name, info.clone());
            typed_prog.globals.push(info);
        }

        // Register function signatures
        for func in &prog.functions {
            let ty = Type::new(TypeKind::Function {
                ret: Box::new(func.ret_type.clone()),
                params: func.params.iter().map(|(t, _)| t.clone()).collect(),
                is_variadic: func.is_variadic,
            });
            self.function_signatures.insert(func.name.clone(), ty);
        }

        // Analyze functions
        for func in prog.functions {
            let typed_func = self.analyze_function(func, diag);
            typed_prog.functions.push(typed_func);
        }

        typed_prog.globals.extend(self.extra_globals.clone());

        typed_prog.string_literals = self.string_literals.clone();
        typed_prog
    }

    fn analyze_function(&mut self, func: Function, diag: &mut Diagnostics) -> TypedFunction {
        self.scopes.clear();
        self.current_offset = 0;
        self.current_func_ret_type = Some(func.ret_type.clone());
        self.current_func_name = func.name.clone();
        self.enter_scope();

        let mut params = Vec::new();
        for (ty, name) in func.params {
            if !name.is_empty() {
                let info = self.add_local_var(name, ty);
                params.push(info);
            } else {
                let info = self.add_local_var(format!("__arg_{}", params.len()), ty);
                params.push(info);
            }
        }

        let typed_body = if let Some(body) = func.body {
            let mut stmts = Vec::new();
            for stmt in body {
                stmts.push(self.analyze_stmt(stmt, diag));
            }
            Some(stmts)
        } else {
            None
        };

        self.exit_scope();
        self.current_func_ret_type = None;

        let stack_size = ((self.current_offset + 15) / 16 * 16) as usize;

        TypedFunction {
            name: func.name,
            ret_type: func.ret_type,
            params,
            is_variadic: func.is_variadic,
            body: typed_body,
            is_static: func.is_static,
            stack_size,
        }
    }

    fn analyze_stmt(&mut self, stmt: Stmt, diag: &mut Diagnostics) -> TypedStmt {
        match stmt {
            Stmt::Expr(expr) => {
                let typed_expr = self.analyze_expr(expr, diag);
                TypedStmt::Expr(typed_expr)
            }
            Stmt::Return(expr_opt, span) => {
                let typed_expr = expr_opt.map(|e| {
                    let mut te = self.analyze_expr(e, diag);
                    if let Some(ret_ty) = &self.current_func_ret_type
                        && te.ty != *ret_ty
                        && !ret_ty.is_struct()
                        && !ret_ty.is_union()
                        && !ret_ty.is_array()
                    {
                        te = TypedExpr {
                            kind: TypedExprKind::Cast { expr: Box::new(te) },
                            ty: ret_ty.clone(),
                            span,
                        };
                    }
                    te
                });
                TypedStmt::Return(typed_expr, span)
            }
            Stmt::Block(stmts, span) => {
                self.enter_scope();
                let mut typed_stmts = Vec::new();
                for s in stmts {
                    typed_stmts.push(self.analyze_stmt(s, diag));
                }
                self.exit_scope();
                TypedStmt::Block(typed_stmts, span)
            }
            Stmt::If {
                cond,
                then_stmt,
                else_stmt,
                span,
            } => {
                let typed_cond = self.analyze_expr(cond, diag);
                let typed_then = Box::new(self.analyze_stmt(*then_stmt, diag));
                let typed_else = else_stmt.map(|s| Box::new(self.analyze_stmt(*s, diag)));
                TypedStmt::If {
                    cond: typed_cond,
                    then_stmt: typed_then,
                    else_stmt: typed_else,
                    span,
                }
            }
            Stmt::While { cond, body, span } => {
                let typed_cond = self.analyze_expr(cond, diag);
                let typed_body = Box::new(self.analyze_stmt(*body, diag));
                TypedStmt::While {
                    cond: typed_cond,
                    body: typed_body,
                    span,
                }
            }
            Stmt::DoWhile { body, cond, span } => {
                let typed_body = Box::new(self.analyze_stmt(*body, diag));
                let typed_cond = self.analyze_expr(cond, diag);
                TypedStmt::DoWhile {
                    body: typed_body,
                    cond: typed_cond,
                    span,
                }
            }
            Stmt::For {
                init,
                cond,
                step,
                body,
                span,
            } => {
                self.enter_scope();
                let typed_init = match init {
                    Some(ForInit::Expr(e)) => {
                        let te = self.analyze_expr(e, diag);
                        Some(Box::new(TypedStmt::Expr(te)))
                    }
                    Some(ForInit::Decl(decls)) => {
                        let mut init_stmts = Vec::new();
                        for d in decls {
                            let info = self.add_local_var(d.name, d.ty.clone());
                            if let Some(init) = d.init {
                                let mut local_stmts =
                                    self.analyze_local_init(&info, init, d.span, diag);
                                init_stmts.append(&mut local_stmts);
                            }
                        }
                        Some(Box::new(TypedStmt::Block(init_stmts, span)))
                    }
                    None => None,
                };

                let typed_cond = cond.map(|c| self.analyze_expr(c, diag));
                let typed_step = step.map(|s| self.analyze_expr(s, diag));
                let typed_body = Box::new(self.analyze_stmt(*body, diag));
                self.exit_scope();

                TypedStmt::For {
                    init: typed_init,
                    cond: typed_cond,
                    step: typed_step,
                    body: typed_body,
                    span,
                }
            }
            Stmt::Switch { expr, body, span } => {
                let typed_expr = self.analyze_expr(expr, diag);
                let break_label = self.new_label("switch_break");

                self.switch_stack.push(SwitchContext {
                    cases: Vec::new(),
                    default_label: None,
                    break_label: break_label.clone(),
                });

                let typed_body = Box::new(self.analyze_stmt(*body, diag));
                let ctx = self.switch_stack.pop().unwrap();

                TypedStmt::Switch {
                    expr: typed_expr,
                    body: typed_body,
                    cases: ctx.cases,
                    default_label: ctx.default_label,
                    break_label,
                    span,
                }
            }
            Stmt::Case { val, body, span } => {
                let label = self.new_label("case");
                if let Some(ctx) = self.switch_stack.last_mut() {
                    ctx.cases.push((val, label.clone()));
                } else {
                    diag.error(span, "'case' statement not in switch statement");
                }
                let typed_body = Box::new(self.analyze_stmt(*body, diag));
                TypedStmt::Case {
                    label,
                    body: typed_body,
                    span,
                }
            }
            Stmt::Default { body, span } => {
                let label = self.new_label("default");
                if let Some(ctx) = self.switch_stack.last_mut() {
                    ctx.default_label = Some(label.clone());
                } else {
                    diag.error(span, "'default' statement not in switch statement");
                }
                let typed_body = Box::new(self.analyze_stmt(*body, diag));
                TypedStmt::Default {
                    label,
                    body: typed_body,
                    span,
                }
            }
            Stmt::Break(span) => TypedStmt::Break(span),
            Stmt::Continue(span) => TypedStmt::Continue(span),
            Stmt::Goto(lbl, span) => {
                let scoped = format!("{}_{}", self.current_func_name, lbl);
                TypedStmt::Goto(scoped, span)
            }
            Stmt::GotoExpr(expr, span) => {
                let texpr = self.analyze_expr(expr, diag);
                TypedStmt::GotoExpr(texpr, span)
            }
            Stmt::Label(lbl, body, span) => {
                let scoped = format!("{}_{}", self.current_func_name, lbl);
                let typed_body = Box::new(self.analyze_stmt(*body, diag));
                TypedStmt::Label(scoped, typed_body, span)
            }
            Stmt::Decl(decls) => {
                let mut stmts = Vec::new();
                for d in decls {
                    let mut ty = d.ty.clone();
                    if let Some(init) = &d.init
                        && let Some(len) = infer_array_len(&ty, init)
                        && let TypeKind::Array { elem, .. } = &ty.kind
                    {
                        ty = Type::new(TypeKind::Array {
                            elem: elem.clone(),
                            len: Some(len),
                        });
                    }

                    if d.is_static {
                        self.add_static_local_var(d.name, ty, d.init);
                    } else {
                        let info = self.add_local_var(d.name, ty);
                        if let Some(init) = d.init {
                            let mut local_stmts =
                                self.analyze_local_init(&info, init, d.span, diag);
                            stmts.append(&mut local_stmts);
                        }
                    }
                }
                if stmts.is_empty() {
                    TypedStmt::Empty(Span::dummy())
                } else if stmts.len() == 1 {
                    stmts.pop().unwrap()
                } else {
                    TypedStmt::Block(stmts, Span::dummy())
                }
            }
            Stmt::Empty(span) => TypedStmt::Empty(span),
        }
    }

    fn analyze_local_init(
        &mut self,
        info: &LocalVarInfo,
        init: Initializer,
        span: Span,
        diag: &mut Diagnostics,
    ) -> Vec<TypedStmt> {
        let mut stmts = Vec::new();
        self.emit_init_to_offset(&info.ty, info.offset, &init, span, &mut stmts, diag);
        stmts
    }

    fn emit_init_to_offset(
        &mut self,
        ty: &Type,
        base_offset: i32,
        init: &Initializer,
        span: Span,
        stmts: &mut Vec<TypedStmt>,
        diag: &mut Diagnostics,
    ) {
        match init {
            Initializer::Single(expr) => {
                if let TypeKind::Array { elem, len } = &ty.kind
                    && elem.is_char()
                    && let Expr::String(s, _) = &**expr
                {
                    let bytes = s.as_bytes();
                    let target_len = len.unwrap_or(bytes.len() + 1);
                    for i in 0..target_len {
                        let val = if i < bytes.len() {
                            bytes[i] as char
                        } else {
                            '\0'
                        };
                        let offset = base_offset - i as i32;
                        let assign = TypedExpr {
                            kind: TypedExprKind::Binary {
                                op: BinaryOp::Assign,
                                lhs: Box::new(TypedExpr {
                                    kind: TypedExprKind::LocalVar(offset),
                                    ty: *elem.clone(),
                                    span,
                                }),
                                rhs: Box::new(TypedExpr {
                                    kind: TypedExprKind::Char(val),
                                    ty: *elem.clone(),
                                    span,
                                }),
                            },
                            ty: *elem.clone(),
                            span,
                        };
                        stmts.push(TypedStmt::Expr(assign));
                    }
                    return;
                }

                let mut te = self.analyze_expr(*expr.clone(), diag);
                if te.ty != *ty && !ty.is_struct() && !ty.is_union() && !ty.is_array() {
                    te = TypedExpr {
                        kind: TypedExprKind::Cast { expr: Box::new(te) },
                        ty: ty.clone(),
                        span,
                    };
                }
                let assign = TypedExpr {
                    kind: TypedExprKind::Binary {
                        op: BinaryOp::Assign,
                        lhs: Box::new(TypedExpr {
                            kind: TypedExprKind::LocalVar(base_offset),
                            ty: ty.clone(),
                            span,
                        }),
                        rhs: Box::new(te),
                    },
                    ty: ty.clone(),
                    span,
                };
                stmts.push(TypedStmt::Expr(assign));
            }
            Initializer::List(items) => {
                if let TypeKind::Array { elem, .. } = &ty.kind {
                    let elem_size = elem.size() as i32;
                    let mut curr_idx = 0;
                    for item in items {
                        for d in &item.designators {
                            if let Designator::Index(idx) = d {
                                curr_idx = *idx as usize;
                            }
                        }
                        let offset = base_offset - (curr_idx as i32 * elem_size);
                        curr_idx += 1;
                        self.emit_init_to_offset(elem, offset, &item.init, span, stmts, diag);
                    }
                } else if let TypeKind::Struct(st_cell) = &ty.kind {
                    let st = st_cell.borrow();
                    let mut curr_idx = 0;
                    for item in items {
                        let mut target_member = None;
                        for d in &item.designators {
                            if let Designator::Field(fname) = d
                                && let Some((idx, m)) = st
                                    .members
                                    .iter()
                                    .enumerate()
                                    .find(|(_, mem)| &mem.name == fname)
                            {
                                target_member = Some(m.clone());
                                curr_idx = idx + 1;
                            }
                        }
                        if target_member.is_none() && curr_idx < st.members.len() {
                            target_member = Some(st.members[curr_idx].clone());
                            curr_idx += 1;
                        }
                        if let Some(member) = target_member {
                            let offset = base_offset - member.offset as i32;
                            self.emit_init_to_offset(
                                &member.ty, offset, &item.init, span, stmts, diag,
                            );
                        }
                    }
                } else if let TypeKind::Union(ut_cell) = &ty.kind {
                    let ut = ut_cell.borrow();
                    let mut target_member = ut.members.first().cloned();
                    for item in items {
                        for d in &item.designators {
                            if let Designator::Field(fname) = d
                                && let Some(m) = ut.members.iter().find(|mem| &mem.name == fname)
                            {
                                target_member = Some(m.clone());
                            }
                        }
                        if let Some(member) = &target_member {
                            self.emit_init_to_offset(
                                &member.ty,
                                base_offset,
                                &item.init,
                                span,
                                stmts,
                                diag,
                            );
                        }
                    }
                } else if let Some(first) = items.first() {
                    self.emit_init_to_offset(ty, base_offset, &first.init, span, stmts, diag);
                }
            }
        }
    }

    pub fn analyze_expr(&mut self, expr: Expr, diag: &mut Diagnostics) -> TypedExpr {
        match expr {
            Expr::Int(v, span) => {
                let ty = if v > i32::MAX as i64 || v < i32::MIN as i64 {
                    Type::long_ty()
                } else {
                    Type::int_ty()
                };
                TypedExpr {
                    kind: TypedExprKind::Int(v),
                    ty,
                    span,
                }
            }
            Expr::Float(v, span) => TypedExpr {
                kind: TypedExprKind::Float(v),
                ty: Type::new(TypeKind::Double),
                span,
            },
            Expr::Char(c, span) => TypedExpr {
                kind: TypedExprKind::Char(c),
                ty: Type::char_ty(),
                span,
            },
            Expr::String(s, span) => {
                self.str_counter += 1;
                let label = format!(".L.str.{}", self.str_counter);
                self.string_literals.push(StringLiteral {
                    label: label.clone(),
                    content: s,
                });
                TypedExpr {
                    kind: TypedExprKind::StringLiteral(label),
                    ty: Type::char_ty().pointer_to(),
                    span,
                }
            }
            Expr::Var(name, span) => {
                if let Some((ty, offset_opt, gname_opt)) = self.find_var(&name) {
                    let kind = if let Some(offset) = offset_opt {
                        TypedExprKind::LocalVar(offset)
                    } else if let Some(gname) = gname_opt {
                        TypedExprKind::GlobalVar(gname)
                    } else {
                        TypedExprKind::GlobalVar(name)
                    };
                    TypedExpr { kind, ty, span }
                } else {
                    diag.error(span, format!("Undeclared variable '{}'", name));
                    TypedExpr {
                        kind: TypedExprKind::Int(0),
                        ty: Type::int_ty(),
                        span,
                    }
                }
            }
            Expr::Binary { op, lhs, rhs, span } => {
                let mut tlhs = self.analyze_expr(*lhs, diag);
                let mut trhs = self.analyze_expr(*rhs, diag);

                let ty = match op {
                    BinaryOp::Assign => {
                        if tlhs.ty != trhs.ty
                            && !tlhs.ty.is_struct()
                            && !tlhs.ty.is_union()
                            && !tlhs.ty.is_array()
                        {
                            trhs = TypedExpr {
                                kind: TypedExprKind::Cast {
                                    expr: Box::new(trhs),
                                },
                                ty: tlhs.ty.clone(),
                                span,
                            };
                        }
                        tlhs.ty.clone()
                    }
                    BinaryOp::PlusAssign
                    | BinaryOp::MinusAssign
                    | BinaryOp::StarAssign
                    | BinaryOp::SlashAssign
                    | BinaryOp::PercentAssign
                    | BinaryOp::AmpAssign
                    | BinaryOp::PipeAssign
                    | BinaryOp::CaretAssign
                    | BinaryOp::ShlAssign
                    | BinaryOp::ShrAssign => {
                        if matches!(op, BinaryOp::PlusAssign | BinaryOp::MinusAssign)
                            && (tlhs.ty.is_pointer() || tlhs.ty.is_array())
                        {
                            let base_size = tlhs.ty.get_pointer_base().map_or(1, |b| b.size());
                            if base_size > 1 {
                                trhs = TypedExpr {
                                    kind: TypedExprKind::Binary {
                                        op: BinaryOp::Mul,
                                        lhs: Box::new(trhs),
                                        rhs: Box::new(TypedExpr {
                                            kind: TypedExprKind::Int(base_size as i64),
                                            ty: Type::long_ty(),
                                            span,
                                        }),
                                    },
                                    ty: Type::long_ty(),
                                    span,
                                };
                            }
                        }
                        tlhs.ty.clone()
                    }
                    BinaryOp::Eq
                    | BinaryOp::Ne
                    | BinaryOp::Lt
                    | BinaryOp::Le
                    | BinaryOp::Gt
                    | BinaryOp::Ge => {
                        if !tlhs.ty.is_pointer()
                            && !tlhs.ty.is_array()
                            && !trhs.ty.is_pointer()
                            && !trhs.ty.is_array()
                        {
                            let common = common_arithmetic_type(&tlhs.ty, &trhs.ty);
                            if tlhs.ty != common {
                                tlhs = TypedExpr {
                                    kind: TypedExprKind::Cast {
                                        expr: Box::new(tlhs),
                                    },
                                    ty: common.clone(),
                                    span,
                                };
                            }
                            if trhs.ty != common {
                                trhs = TypedExpr {
                                    kind: TypedExprKind::Cast {
                                        expr: Box::new(trhs),
                                    },
                                    ty: common,
                                    span,
                                };
                            }
                        }
                        Type::int_ty()
                    }
                    BinaryOp::LogicalAnd | BinaryOp::LogicalOr => Type::int_ty(),
                    BinaryOp::Add => {
                        if tlhs.ty.is_pointer() || tlhs.ty.is_array() {
                            let base_size = tlhs.ty.get_pointer_base().map_or(1, |b| b.size());
                            if base_size > 1 {
                                trhs = TypedExpr {
                                    kind: TypedExprKind::Binary {
                                        op: BinaryOp::Mul,
                                        lhs: Box::new(trhs),
                                        rhs: Box::new(TypedExpr {
                                            kind: TypedExprKind::Int(base_size as i64),
                                            ty: Type::long_ty(),
                                            span,
                                        }),
                                    },
                                    ty: Type::long_ty(),
                                    span,
                                };
                            }
                            if tlhs.ty.is_array() {
                                tlhs.ty.get_pointer_base().unwrap().clone().pointer_to()
                            } else {
                                tlhs.ty.clone()
                            }
                        } else if trhs.ty.is_pointer() || trhs.ty.is_array() {
                            let base_size = trhs.ty.get_pointer_base().map_or(1, |b| b.size());
                            if base_size > 1 {
                                tlhs = TypedExpr {
                                    kind: TypedExprKind::Binary {
                                        op: BinaryOp::Mul,
                                        lhs: Box::new(tlhs),
                                        rhs: Box::new(TypedExpr {
                                            kind: TypedExprKind::Int(base_size as i64),
                                            ty: Type::long_ty(),
                                            span,
                                        }),
                                    },
                                    ty: Type::long_ty(),
                                    span,
                                };
                            }
                            if trhs.ty.is_array() {
                                trhs.ty.get_pointer_base().unwrap().clone().pointer_to()
                            } else {
                                trhs.ty.clone()
                            }
                        } else {
                            let common = common_arithmetic_type(&tlhs.ty, &trhs.ty);
                            if tlhs.ty != common {
                                tlhs = TypedExpr {
                                    kind: TypedExprKind::Cast {
                                        expr: Box::new(tlhs),
                                    },
                                    ty: common.clone(),
                                    span,
                                };
                            }
                            if trhs.ty != common {
                                trhs = TypedExpr {
                                    kind: TypedExprKind::Cast {
                                        expr: Box::new(trhs),
                                    },
                                    ty: common.clone(),
                                    span,
                                };
                            }
                            common
                        }
                    }
                    BinaryOp::Sub => {
                        if (tlhs.ty.is_pointer() || tlhs.ty.is_array())
                            && (trhs.ty.is_pointer() || trhs.ty.is_array())
                        {
                            let base_size = tlhs.ty.get_pointer_base().map_or(1, |b| b.size());
                            let sub_expr = TypedExpr {
                                kind: TypedExprKind::Binary {
                                    op: BinaryOp::Sub,
                                    lhs: Box::new(tlhs),
                                    rhs: Box::new(trhs),
                                },
                                ty: Type::long_ty(),
                                span,
                            };
                            if base_size > 1 {
                                return TypedExpr {
                                    kind: TypedExprKind::Binary {
                                        op: BinaryOp::Div,
                                        lhs: Box::new(sub_expr),
                                        rhs: Box::new(TypedExpr {
                                            kind: TypedExprKind::Int(base_size as i64),
                                            ty: Type::long_ty(),
                                            span,
                                        }),
                                    },
                                    ty: Type::long_ty(),
                                    span,
                                };
                            } else {
                                return sub_expr;
                            }
                        } else if tlhs.ty.is_pointer() || tlhs.ty.is_array() {
                            let base_size = tlhs.ty.get_pointer_base().map_or(1, |b| b.size());
                            if base_size > 1 {
                                trhs = TypedExpr {
                                    kind: TypedExprKind::Binary {
                                        op: BinaryOp::Mul,
                                        lhs: Box::new(trhs),
                                        rhs: Box::new(TypedExpr {
                                            kind: TypedExprKind::Int(base_size as i64),
                                            ty: Type::long_ty(),
                                            span,
                                        }),
                                    },
                                    ty: Type::long_ty(),
                                    span,
                                };
                            }
                            if tlhs.ty.is_array() {
                                tlhs.ty.get_pointer_base().unwrap().clone().pointer_to()
                            } else {
                                tlhs.ty.clone()
                            }
                        } else {
                            let common = common_arithmetic_type(&tlhs.ty, &trhs.ty);
                            if tlhs.ty != common {
                                tlhs = TypedExpr {
                                    kind: TypedExprKind::Cast {
                                        expr: Box::new(tlhs),
                                    },
                                    ty: common.clone(),
                                    span,
                                };
                            }
                            if trhs.ty != common {
                                trhs = TypedExpr {
                                    kind: TypedExprKind::Cast {
                                        expr: Box::new(trhs),
                                    },
                                    ty: common.clone(),
                                    span,
                                };
                            }
                            common
                        }
                    }
                    BinaryOp::Shl | BinaryOp::Shr => {
                        if tlhs.ty.size() < 4 {
                            Type::int_ty()
                        } else {
                            tlhs.ty.clone()
                        }
                    }
                    BinaryOp::Comma => trhs.ty.clone(),
                    _ => {
                        let common = common_arithmetic_type(&tlhs.ty, &trhs.ty);
                        if tlhs.ty != common && !tlhs.ty.is_pointer() && !tlhs.ty.is_array() {
                            tlhs = TypedExpr {
                                kind: TypedExprKind::Cast {
                                    expr: Box::new(tlhs),
                                },
                                ty: common.clone(),
                                span,
                            };
                        }
                        if trhs.ty != common && !trhs.ty.is_pointer() && !trhs.ty.is_array() {
                            trhs = TypedExpr {
                                kind: TypedExprKind::Cast {
                                    expr: Box::new(trhs),
                                },
                                ty: common.clone(),
                                span,
                            };
                        }
                        common
                    }
                };

                TypedExpr {
                    kind: TypedExprKind::Binary {
                        op,
                        lhs: Box::new(tlhs),
                        rhs: Box::new(trhs),
                    },
                    ty,
                    span,
                }
            }
            Expr::Unary { op, expr, span } => match op {
                UnaryOp::AddrOf => {
                    let texpr = self.analyze_expr(*expr, diag);
                    let ty = texpr.ty.clone().pointer_to();
                    TypedExpr {
                        kind: TypedExprKind::AddrOf(Box::new(texpr)),
                        ty,
                        span,
                    }
                }
                UnaryOp::Deref => {
                    let texpr = self.analyze_expr(*expr, diag);
                    let base_ty = texpr.ty.get_pointer_base().cloned().unwrap_or_else(|| {
                        diag.error(span, "Cannot dereference non-pointer type");
                        Type::int_ty()
                    });
                    TypedExpr {
                        kind: TypedExprKind::Deref(Box::new(texpr)),
                        ty: base_ty,
                        span,
                    }
                }
                UnaryOp::Sizeof => {
                    let texpr = self.analyze_expr(*expr, diag);
                    TypedExpr {
                        kind: TypedExprKind::Int(texpr.ty.size() as i64),
                        ty: Type::long_ty(),
                        span,
                    }
                }
                UnaryOp::Alignof => {
                    let texpr = self.analyze_expr(*expr, diag);
                    TypedExpr {
                        kind: TypedExprKind::Int(texpr.ty.align() as i64),
                        ty: Type::long_ty(),
                        span,
                    }
                }
                UnaryOp::AddrOfLabel => {
                    let lbl = match &*expr {
                        Expr::Var(name, _) => name.clone(),
                        _ => String::new(),
                    };
                    let scoped = format!("{}_{}", self.current_func_name, lbl);
                    TypedExpr {
                        kind: TypedExprKind::AddrOfLabel(scoped),
                        ty: Type::pointer_to(Type::void()),
                        span,
                    }
                }
                UnaryOp::LogNot => {
                    let texpr = self.analyze_expr(*expr, diag);
                    TypedExpr {
                        kind: TypedExprKind::Unary {
                            op,
                            expr: Box::new(texpr),
                        },
                        ty: Type::int_ty(),
                        span,
                    }
                }
                _ => {
                    let texpr = self.analyze_expr(*expr, diag);
                    let ty = texpr.ty.clone();
                    TypedExpr {
                        kind: TypedExprKind::Unary {
                            op,
                            expr: Box::new(texpr),
                        },
                        ty,
                        span,
                    }
                }
            },
            Expr::SizeofType { target_type, span } => TypedExpr {
                kind: TypedExprKind::Int(target_type.size() as i64),
                ty: Type::long_ty(),
                span,
            },
            Expr::Ternary {
                cond,
                then_expr,
                else_expr,
                span,
            } => {
                let tcond = self.analyze_expr(*cond, diag);
                let mut tthen = self.analyze_expr(*then_expr, diag);
                let mut telse = self.analyze_expr(*else_expr, diag);
                let ty = if !tthen.ty.is_pointer()
                    && !tthen.ty.is_array()
                    && !telse.ty.is_pointer()
                    && !telse.ty.is_array()
                    && !tthen.ty.is_struct()
                    && !tthen.ty.is_union()
                    && !telse.ty.is_struct()
                    && !telse.ty.is_union()
                {
                    let common = common_arithmetic_type(&tthen.ty, &telse.ty);
                    if tthen.ty != common {
                        tthen = TypedExpr {
                            kind: TypedExprKind::Cast {
                                expr: Box::new(tthen),
                            },
                            ty: common.clone(),
                            span,
                        };
                    }
                    if telse.ty != common {
                        telse = TypedExpr {
                            kind: TypedExprKind::Cast {
                                expr: Box::new(telse),
                            },
                            ty: common.clone(),
                            span,
                        };
                    }
                    common
                } else if (tthen.ty.is_pointer() || tthen.ty.is_array()) && telse.ty.is_integer() {
                    let ty = if tthen.ty.is_array() {
                        tthen.ty.get_pointer_base().unwrap().clone().pointer_to()
                    } else {
                        tthen.ty.clone()
                    };
                    telse = TypedExpr {
                        kind: TypedExprKind::Cast {
                            expr: Box::new(telse),
                        },
                        ty: ty.clone(),
                        span,
                    };
                    ty
                } else if (telse.ty.is_pointer() || telse.ty.is_array()) && tthen.ty.is_integer() {
                    let ty = if telse.ty.is_array() {
                        telse.ty.get_pointer_base().unwrap().clone().pointer_to()
                    } else {
                        telse.ty.clone()
                    };
                    tthen = TypedExpr {
                        kind: TypedExprKind::Cast {
                            expr: Box::new(tthen),
                        },
                        ty: ty.clone(),
                        span,
                    };
                    ty
                } else {
                    tthen.ty.clone()
                };
                TypedExpr {
                    kind: TypedExprKind::Ternary {
                        cond: Box::new(tcond),
                        then_expr: Box::new(tthen),
                        else_expr: Box::new(telse),
                    },
                    ty,
                    span,
                }
            }
            Expr::Call { callee, args, span } => {
                let tcallee = match *callee {
                    Expr::Var(name, var_span) => {
                        if let Some((ty, offset_opt, gname_opt)) = self.find_var(&name) {
                            let kind = if let Some(offset) = offset_opt {
                                TypedExprKind::LocalVar(offset)
                            } else if let Some(gname) = gname_opt {
                                TypedExprKind::GlobalVar(gname)
                            } else {
                                TypedExprKind::GlobalVar(name)
                            };
                            TypedExpr {
                                kind,
                                ty,
                                span: var_span,
                            }
                        } else {
                            // Implicit function declaration
                            let func_ty = if let Some(sig) = builtin_signature(&name) {
                                sig
                            } else {
                                Type::new(TypeKind::Function {
                                    ret: Box::new(Type::int_ty()),
                                    params: Vec::new(),
                                    is_variadic: false,
                                })
                            };
                            self.function_signatures
                                .insert(name.clone(), func_ty.clone());
                            TypedExpr {
                                kind: TypedExprKind::GlobalVar(name),
                                ty: func_ty,
                                span: var_span,
                            }
                        }
                    }
                    other => self.analyze_expr(other, diag),
                };

                let (ret_ty, params_opt) = match &tcallee.ty.kind {
                    TypeKind::Function { ret, params, .. } => (*ret.clone(), Some(params.clone())),
                    TypeKind::Pointer(base) => match &base.kind {
                        TypeKind::Function { ret, params, .. } => {
                            (*ret.clone(), Some(params.clone()))
                        }
                        _ => (Type::int_ty(), None),
                    },
                    _ => (Type::int_ty(), None),
                };

                let mut targs = Vec::new();
                for (i, a) in args.into_iter().enumerate() {
                    let mut ta = self.analyze_expr(a, diag);
                    if let Some(params) = &params_opt
                        && i < params.len()
                    {
                        let target_ty = &params[i];
                        if ta.ty != *target_ty
                            && !target_ty.is_struct()
                            && !target_ty.is_union()
                            && !target_ty.is_array()
                        {
                            let span = ta.span;
                            ta = TypedExpr {
                                kind: TypedExprKind::Cast { expr: Box::new(ta) },
                                ty: target_ty.clone(),
                                span,
                            };
                        }
                    }
                    targs.push(ta);
                }

                TypedExpr {
                    kind: TypedExprKind::Call {
                        callee: Box::new(tcallee),
                        args: targs,
                    },
                    ty: ret_ty,
                    span,
                }
            }
            Expr::Index { expr, index, span } => {
                // a[b] is *(a + b)
                let add_expr = Expr::Binary {
                    op: BinaryOp::Add,
                    lhs: expr,
                    rhs: index,
                    span,
                };
                let deref_expr = Expr::Unary {
                    op: UnaryOp::Deref,
                    expr: Box::new(add_expr),
                    span,
                };
                self.analyze_expr(deref_expr, diag)
            }
            Expr::Member {
                expr,
                member,
                is_arrow,
                span,
            } => {
                let mut texpr = self.analyze_expr(*expr, diag);

                if is_arrow {
                    let base_ty = texpr.ty.get_pointer_base().cloned().unwrap_or_else(|| {
                        diag.error(span, "Arrow operator requires pointer to struct/union");
                        Type::int_ty()
                    });
                    texpr = TypedExpr {
                        kind: TypedExprKind::Deref(Box::new(texpr)),
                        ty: base_ty,
                        span,
                    };
                }

                let (offset, mty, bit_width, bit_offset) = match &texpr.ty.kind {
                    TypeKind::Struct(st_cell) => {
                        let st = st_cell.borrow();
                        if let Some(m) = st.members.iter().find(|m| m.name == member) {
                            (m.offset, m.ty.clone(), m.bit_width, m.bit_offset)
                        } else {
                            diag.error(span, format!("Struct has no member named '{}'", member));
                            (0, Type::int_ty(), None, None)
                        }
                    }
                    TypeKind::Union(ut_cell) => {
                        let ut = ut_cell.borrow();
                        if let Some(m) = ut.members.iter().find(|m| m.name == member) {
                            (m.offset, m.ty.clone(), m.bit_width, m.bit_offset)
                        } else {
                            diag.error(span, format!("Union has no member named '{}'", member));
                            (0, Type::int_ty(), None, None)
                        }
                    }
                    _ => {
                        diag.error(span, "Member access on non-struct/union type");
                        (0, Type::int_ty(), None, None)
                    }
                };

                TypedExpr {
                    kind: TypedExprKind::Member {
                        expr: Box::new(texpr),
                        offset,
                        bit_width,
                        bit_offset,
                    },
                    ty: mty,
                    span,
                }
            }
            Expr::CompoundLiteral {
                target_type,
                init,
                span,
            } => {
                let temp_info = self.add_local_var(
                    format!("__compound_{}", self.current_offset),
                    target_type.clone(),
                );
                let mut stmts = Vec::new();
                let size = target_type.size();
                let mut off = 0;
                while off + 8 <= size {
                    stmts.push(TypedStmt::Expr(TypedExpr {
                        kind: TypedExprKind::Binary {
                            op: BinaryOp::Assign,
                            lhs: Box::new(TypedExpr {
                                kind: TypedExprKind::LocalVar(temp_info.offset - off as i32),
                                ty: Type::ulonglong_ty(),
                                span,
                            }),
                            rhs: Box::new(TypedExpr {
                                kind: TypedExprKind::Int(0),
                                ty: Type::ulonglong_ty(),
                                span,
                            }),
                        },
                        ty: Type::ulonglong_ty(),
                        span,
                    }));
                    off += 8;
                }
                self.emit_init_to_offset(
                    &target_type,
                    temp_info.offset,
                    &init,
                    span,
                    &mut stmts,
                    diag,
                );
                stmts.push(TypedStmt::Expr(TypedExpr {
                    kind: TypedExprKind::LocalVar(temp_info.offset),
                    ty: target_type.clone(),
                    span,
                }));
                TypedExpr {
                    kind: TypedExprKind::StmtExpr(stmts),
                    ty: target_type,
                    span,
                }
            }
            Expr::Cast {
                target_type,
                expr,
                span,
            } => {
                let texpr = self.analyze_expr(*expr, diag);
                TypedExpr {
                    kind: TypedExprKind::Cast {
                        expr: Box::new(texpr),
                    },
                    ty: target_type,
                    span,
                }
            }
            Expr::StmtExpr { body, span } => {
                self.enter_scope();
                let mut typed_stmts = Vec::new();
                for s in body {
                    typed_stmts.push(self.analyze_stmt(s, diag));
                }
                self.exit_scope();

                let ret_ty = if let Some(TypedStmt::Expr(e)) = typed_stmts.last() {
                    e.ty.clone()
                } else {
                    Type::void()
                };

                TypedExpr {
                    kind: TypedExprKind::StmtExpr(typed_stmts),
                    ty: ret_ty,
                    span,
                }
            }
        }
    }
}

fn infer_array_len(ty: &Type, init: &Initializer) -> Option<usize> {
    if let TypeKind::Array { len: None, .. } = &ty.kind {
        match init {
            Initializer::List(items) => {
                let mut max_idx = 0;
                let mut curr = 0;
                for it in items {
                    for d in &it.designators {
                        if let Designator::Index(idx) = d {
                            curr = *idx as usize;
                        }
                    }
                    curr += 1;
                    if curr > max_idx {
                        max_idx = curr;
                    }
                }
                Some(max_idx)
            }
            Initializer::Single(expr) => {
                if let Expr::String(s, _) = &**expr {
                    Some(s.len() + 1)
                } else {
                    None
                }
            }
        }
    } else {
        None
    }
}

fn common_arithmetic_type(lhs: &Type, rhs: &Type) -> Type {
    if matches!(lhs.kind, TypeKind::Double) || matches!(rhs.kind, TypeKind::Double) {
        return Type::new(TypeKind::Double);
    }
    if matches!(lhs.kind, TypeKind::Float) || matches!(rhs.kind, TypeKind::Float) {
        return Type::new(TypeKind::Float);
    }
    let lhs_size = lhs.size();
    let rhs_size = rhs.size();
    let lhs_signed = lhs.is_signed_integer();
    let rhs_signed = rhs.is_signed_integer();

    if lhs_size == 8 && rhs_size == 8 {
        if !lhs_signed || !rhs_signed {
            Type::ulong_ty()
        } else {
            Type::long_ty()
        }
    } else if lhs_size == 8 {
        if !lhs_signed {
            Type::ulong_ty()
        } else {
            Type::long_ty()
        }
    } else if rhs_size == 8 {
        if !rhs_signed {
            Type::ulong_ty()
        } else {
            Type::long_ty()
        }
    } else {
        // Both size <= 4
        if (!lhs_signed && lhs_size == 4) || (!rhs_signed && rhs_size == 4) {
            Type::uint_ty()
        } else {
            Type::int_ty()
        }
    }
}

pub fn builtin_signature(name: &str) -> Option<Type> {
    match name {
        "__builtin_va_start" | "__builtin_va_end" => Some(Type::new(TypeKind::Function {
            ret: Box::new(Type::void()),
            params: Vec::new(),
            is_variadic: true,
        })),
        "__builtin_frame_address" | "__builtin_return_address" | "__builtin_alloca" | "alloca" => {
            Some(Type::new(TypeKind::Function {
                ret: Box::new(Type::void_ptr_ty()),
                params: Vec::new(),
                is_variadic: true,
            }))
        }
        "__builtin_inff" | "__builtin_nanf" => Some(Type::new(TypeKind::Function {
            ret: Box::new(Type::float_ty()),
            params: Vec::new(),
            is_variadic: false,
        })),
        "__builtin_inf" | "__builtin_huge_val" | "__builtin_nan" => {
            Some(Type::new(TypeKind::Function {
                ret: Box::new(Type::double_ty()),
                params: Vec::new(),
                is_variadic: false,
            }))
        }
        "__builtin_clzll" | "__builtin_ctzll" | "__builtin_bswap64" => {
            Some(Type::new(TypeKind::Function {
                ret: Box::new(Type::ulonglong_ty()),
                params: Vec::new(),
                is_variadic: false,
            }))
        }
        "__builtin_clz"
        | "__builtin_ctz"
        | "__builtin_bswap32"
        | "__builtin_constant_p"
        | "__builtin_expect" => Some(Type::new(TypeKind::Function {
            ret: Box::new(Type::int_ty()),
            params: Vec::new(),
            is_variadic: false,
        })),
        _ => None,
    }
}

fn scope_static_init(init: Initializer, func_name: &str) -> Initializer {
    match init {
        Initializer::Single(expr) => {
            Initializer::Single(Box::new(scope_static_expr(*expr, func_name)))
        }
        Initializer::List(items) => {
            let new_items = items
                .into_iter()
                .map(|item| crate::ast::InitItem {
                    designators: item.designators,
                    init: scope_static_init(item.init, func_name),
                })
                .collect();
            Initializer::List(new_items)
        }
    }
}

fn scope_static_expr(expr: Expr, func_name: &str) -> Expr {
    match expr {
        Expr::Unary {
            op: UnaryOp::AddrOfLabel,
            expr,
            span,
        } => {
            let lbl = match &*expr {
                Expr::Var(name, _) => name.clone(),
                _ => String::new(),
            };
            Expr::Var(format!(".L.user.{}_{}", func_name, lbl), span)
        }
        Expr::Unary { op, expr, span } => Expr::Unary {
            op,
            expr: Box::new(scope_static_expr(*expr, func_name)),
            span,
        },
        Expr::Binary { op, lhs, rhs, span } => Expr::Binary {
            op,
            lhs: Box::new(scope_static_expr(*lhs, func_name)),
            rhs: Box::new(scope_static_expr(*rhs, func_name)),
            span,
        },
        Expr::Cast {
            target_type,
            expr,
            span,
        } => Expr::Cast {
            target_type,
            expr: Box::new(scope_static_expr(*expr, func_name)),
            span,
        },
        Expr::Index { expr, index, span } => Expr::Index {
            expr: Box::new(scope_static_expr(*expr, func_name)),
            index: Box::new(scope_static_expr(*index, func_name)),
            span,
        },
        Expr::Member {
            expr,
            member,
            is_arrow,
            span,
        } => Expr::Member {
            expr: Box::new(scope_static_expr(*expr, func_name)),
            member,
            is_arrow,
            span,
        },
        other => other,
    }
}
