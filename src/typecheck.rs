use std::collections::HashMap;

use crate::ast::{Expr, ExprKind, Span, Stmt, StmtKind, TypeRef, TypeRefKind};
use crate::error::CompileError;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Type {
    I8,
    I16,
    I32,
    I64,
    I128,
    U8,
    U16,
    U32,
    U64,
    U128,
    Isize,
    Usize,
    F32,
    F64,
    Bool,
    Char,
    Str,
    String,
    Unit,
    Never,
    Infer(u32),
}

impl Type {
    pub fn rust_type_name(&self) -> &'static str {
        match self {
            Type::I8 => "i8",
            Type::I16 => "i16",
            Type::I32 => "i32",
            Type::I64 => "i64",
            Type::I128 => "i128",
            Type::U8 => "u8",
            Type::U16 => "u16",
            Type::U32 => "u32",
            Type::U64 => "u64",
            Type::U128 => "u128",
            Type::Isize => "isize",
            Type::Usize => "usize",
            Type::F32 => "f32",
            Type::F64 => "f64",
            Type::Bool => "bool",
            Type::Char => "char",
            Type::Str => "String",
            Type::String => "String",
            Type::Unit => "()",
            Type::Never => "!",
            Type::Infer(_) => "_",
        }
    }

    fn display_name(&self) -> &'static str {
        match self {
            Type::I8 => "i8",
            Type::I16 => "i16",
            Type::I32 => "i32",
            Type::I64 => "i64",
            Type::I128 => "i128",
            Type::U8 => "u8",
            Type::U16 => "u16",
            Type::U32 => "u32",
            Type::U64 => "u64",
            Type::U128 => "u128",
            Type::Isize => "isize",
            Type::Usize => "usize",
            Type::F32 => "f32",
            Type::F64 => "f64",
            Type::Bool => "bool",
            Type::Char => "char",
            Type::Str => "str",
            Type::String => "String",
            Type::Unit => "()",
            Type::Never => "!",
            Type::Infer(_) => "<infer>",
        }
    }

    fn is_integer(&self) -> bool {
        matches!(
            self,
            Type::I8
                | Type::I16
                | Type::I32
                | Type::I64
                | Type::I128
                | Type::U8
                | Type::U16
                | Type::U32
                | Type::U64
                | Type::U128
                | Type::Isize
                | Type::Usize
        )
    }

    fn is_float(&self) -> bool {
        matches!(self, Type::F32 | Type::F64)
    }

    fn is_number(&self) -> bool {
        self.is_integer() || self.is_float()
    }

    fn is_orderable(&self) -> bool {
        self.is_number() || matches!(self, Type::Char)
    }

    fn is_unsigned_integer(&self) -> bool {
        matches!(
            self,
            Type::U8 | Type::U16 | Type::U32 | Type::U64 | Type::U128 | Type::Usize
        )
    }
}

#[derive(Debug, Clone)]
pub struct FunctionSig {
    pub params: Vec<Type>,
    pub ret: Type,
}

#[derive(Debug, Clone)]
pub struct TypeInfo {
    pub function_sigs: HashMap<String, FunctionSig>,
}

#[derive(Debug, Clone, Copy)]
enum InferKind {
    Any,
    Integer,
    Float,
    Number,
}

#[derive(Debug, Clone)]
struct Binding {
    ty: Type,
}

#[derive(Debug, Default)]
struct Env {
    scopes: Vec<HashMap<String, Binding>>,
}

impl Env {
    fn new() -> Self {
        let mut env = Self { scopes: Vec::new() };
        env.push();
        env
    }

    fn push(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop(&mut self) {
        let _ = self.scopes.pop();
    }

    fn insert(&mut self, name: String, ty: Type) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, Binding { ty });
        }
    }

    fn get(&self, name: &str) -> Option<Type> {
        for scope in self.scopes.iter().rev() {
            if let Some(binding) = scope.get(name) {
                return Some(binding.ty.clone());
            }
        }
        None
    }
}

#[derive(Debug, Clone, Copy)]
struct BlockCheck {
    always_returns: bool,
    saw_return: bool,
}

pub fn check_program(stmts: &[Stmt]) -> Result<TypeInfo, CompileError> {
    let mut checker = TypeChecker::new();
    checker.collect_functions(stmts)?;
    checker.check_functions(stmts)?;
    checker.check_top_level(stmts)?;
    checker.finalize()
}

struct TypeChecker {
    functions: HashMap<String, FunctionSig>,
    infer_kinds: HashMap<u32, InferKind>,
    substitutions: HashMap<u32, Type>,
    next_infer_id: u32,
}

impl TypeChecker {
    fn new() -> Self {
        Self {
            functions: HashMap::new(),
            infer_kinds: HashMap::new(),
            substitutions: HashMap::new(),
            next_infer_id: 0,
        }
    }

    fn collect_functions(&mut self, stmts: &[Stmt]) -> Result<(), CompileError> {
        for stmt in stmts {
            let func_stmt = match &stmt.kind {
                StmtKind::Func { .. } => stmt,
                StmtKind::Export(inner) => inner,
                _ => continue,
            };

            let StmtKind::Func {
                name,
                params,
                return_type,
                ..
            } = &func_stmt.kind
            else {
                continue;
            };

            if self.functions.contains_key(name) {
                return Err(
                    self.err_at_span(func_stmt.span, format!("função '{name}' já foi declarada"))
                );
            }

            let inherited_param_type = if let Some(type_ref) = return_type {
                Some(self.type_from_ref(type_ref)?)
            } else {
                None
            };

            if inherited_param_type.is_none() && params.iter().any(|param| param.ty.is_none()) {
                let missing = params
                    .iter()
                    .find(|param| param.ty.is_none())
                    .expect("at least one param without type");
                return Err(self
                    .err_at_span(
                        missing.span,
                        format!(
                            "parâmetro '{}' sem tipo explícito exige retorno com '-> Tipo'",
                            missing.name
                        ),
                    )
                    .with_help(
                        "adicione '-> Tipo' na função ou anote o tipo de todos os parâmetros",
                    ));
            }

            let mut param_types = Vec::with_capacity(params.len());
            for param in params {
                let ty = if let Some(type_ref) = &param.ty {
                    self.type_from_ref(type_ref)?
                } else if let Some(default_ty) = &inherited_param_type {
                    default_ty.clone()
                } else {
                    self.fresh_infer(InferKind::Any)
                };
                param_types.push(ty);
            }

            let ret = if let Some(default_ty) = inherited_param_type {
                default_ty
            } else {
                self.fresh_infer(InferKind::Any)
            };

            self.functions.insert(
                name.clone(),
                FunctionSig {
                    params: param_types,
                    ret,
                },
            );
        }

        Ok(())
    }

    fn check_functions(&mut self, stmts: &[Stmt]) -> Result<(), CompileError> {
        for stmt in stmts {
            let func_stmt = match &stmt.kind {
                StmtKind::Func { .. } => stmt,
                StmtKind::Export(inner) => inner,
                _ => continue,
            };

            let StmtKind::Func {
                name, params, body, ..
            } = &func_stmt.kind
            else {
                continue;
            };

            let sig = self.functions.get(name).cloned().ok_or_else(|| {
                self.err_at_span(func_stmt.span, "assinatura de função não encontrada")
            })?;

            let mut env = Env::new();
            for (index, param) in params.iter().enumerate() {
                env.insert(param.name.clone(), sig.params[index].clone());
            }

            let block = self.check_block(body, &mut env, Some(sig.ret.clone()), true, true)?;

            if !block.saw_return {
                if block.always_returns {
                    if matches!(self.resolve_type(&sig.ret), Type::Infer(_)) {
                        self.unify(
                            sig.ret.clone(),
                            Type::Never,
                            func_stmt.span,
                            "função diverge e não retorna normalmente",
                        )?;
                    }
                } else {
                    self.unify(
                        sig.ret.clone(),
                        Type::Unit,
                        func_stmt.span,
                        "função sem retorno explícito deve retornar ()",
                    )?;
                }
            }

            let resolved_ret = self.resolve_with_defaults(sig.ret.clone());
            if !block.always_returns && !matches!(resolved_ret, Type::Unit) {
                return Err(self
                    .err_at_span(func_stmt.span, "nem todos os caminhos retornam um valor")
                    .with_help(format!(
                        "a função '{}' foi inferida/declarada com retorno '{}'",
                        name,
                        resolved_ret.display_name()
                    )));
            }

            if matches!(resolved_ret, Type::Never) && !block.always_returns {
                return Err(self
                    .err_at_span(
                        func_stmt.span,
                        "função com retorno '!' não pode terminar normalmente",
                    )
                    .with_help("garanta que todos os caminhos divirjam (ex.: panic)"));
            }
        }

        Ok(())
    }

    fn check_top_level(&mut self, stmts: &[Stmt]) -> Result<(), CompileError> {
        let mut env = Env::new();
        let block = self.check_block(stmts, &mut env, None, false, false)?;
        if block.always_returns {
            return Err(self.err_at_span(
                stmts
                    .first()
                    .map(|stmt| stmt.span)
                    .unwrap_or(Span::new(1, 1, 1)),
                "retorno no escopo global não é permitido",
            ));
        }
        Ok(())
    }

    fn check_block(
        &mut self,
        stmts: &[Stmt],
        env: &mut Env,
        expected_return: Option<Type>,
        in_function: bool,
        allow_tail_expr_return: bool,
    ) -> Result<BlockCheck, CompileError> {
        env.push();

        let mut saw_return = false;
        let mut always_returns = false;

        for (index, stmt) in stmts.iter().enumerate() {
            let is_last = index + 1 == stmts.len();
            if allow_tail_expr_return && in_function && is_last {
                if let StmtKind::Expr(expr) = &stmt.kind {
                    let expected = expected_return.clone().ok_or_else(|| {
                        self.err_at_span(stmt.span, "retorno implícito sem função de contexto")
                    })?;
                    let got = self.infer_expr(expr, env)?;
                    self.unify(expected, got, stmt.span, "tipo de retorno incompatível")?;
                    saw_return = true;
                    always_returns = true;
                    break;
                }
            }

            let check = self.check_stmt(stmt, env, expected_return.clone(), in_function)?;
            saw_return |= check.saw_return;
            if check.always_returns {
                always_returns = true;
                break;
            }
        }

        env.pop();

        Ok(BlockCheck {
            always_returns,
            saw_return,
        })
    }

    fn check_stmt(
        &mut self,
        stmt: &Stmt,
        env: &mut Env,
        expected_return: Option<Type>,
        in_function: bool,
    ) -> Result<BlockCheck, CompileError> {
        match &stmt.kind {
            StmtKind::Import(_) => {
                if in_function {
                    return Err(self
                        .err_at_span(stmt.span, "import só é permitido no escopo global")
                        .with_help("mova o import para o topo do arquivo"));
                }
                Ok(BlockCheck {
                    always_returns: false,
                    saw_return: false,
                })
            }
            StmtKind::Export(inner) => {
                if in_function {
                    return Err(self
                        .err_at_span(stmt.span, "export só é permitido no escopo global")
                        .with_help("mova o export para o topo do arquivo"));
                }
                self.check_stmt(inner, env, expected_return, in_function)
            }
            StmtKind::Let {
                name,
                mutable: _,
                ty,
                expr,
            } => {
                let expr_ty = self.infer_expr(expr, env)?;
                let resolved_ty = if let Some(type_ref) = ty {
                    let annotated = self.type_from_ref(type_ref)?;
                    self.unify(
                        annotated.clone(),
                        expr_ty,
                        expr.span,
                        "tipo do valor não é compatível com anotação",
                    )?;
                    annotated
                } else {
                    expr_ty
                };
                env.insert(name.clone(), resolved_ty);
                Ok(BlockCheck {
                    always_returns: false,
                    saw_return: false,
                })
            }
            StmtKind::Const { name, ty, expr } => {
                let expr_ty = self.infer_expr(expr, env)?;
                let resolved_ty = if let Some(type_ref) = ty {
                    let annotated = self.type_from_ref(type_ref)?;
                    self.unify(
                        annotated.clone(),
                        expr_ty,
                        expr.span,
                        "tipo do valor não é compatível com anotação de const",
                    )?;
                    annotated
                } else {
                    expr_ty
                };
                env.insert(name.clone(), resolved_ty);
                Ok(BlockCheck {
                    always_returns: false,
                    saw_return: false,
                })
            }
            StmtKind::Expr(expr) => {
                let ty = self.infer_expr(expr, env)?;
                Ok(BlockCheck {
                    always_returns: matches!(self.resolve_type(&ty), Type::Never),
                    saw_return: false,
                })
            }
            StmtKind::Print(expr) => {
                let _ = self.infer_expr(expr, env)?;
                Ok(BlockCheck {
                    always_returns: false,
                    saw_return: false,
                })
            }
            StmtKind::If {
                cond,
                then_block,
                else_block,
            } => {
                let cond_ty = self.infer_expr(cond, env)?;
                self.unify(
                    Type::Bool,
                    cond_ty,
                    cond.span,
                    "condição de if deve ser bool",
                )?;

                let then_check =
                    self.check_block(then_block, env, expected_return.clone(), in_function, false)?;
                let else_check = if else_block.is_empty() {
                    BlockCheck {
                        always_returns: false,
                        saw_return: false,
                    }
                } else {
                    self.check_block(else_block, env, expected_return, in_function, false)?
                };

                Ok(BlockCheck {
                    always_returns: !else_block.is_empty()
                        && then_check.always_returns
                        && else_check.always_returns,
                    saw_return: then_check.saw_return || else_check.saw_return,
                })
            }
            StmtKind::Func { .. } => {
                if in_function {
                    return Err(self
                        .err_at_span(
                            stmt.span,
                            "declaração de função dentro de função não é suportada",
                        )
                        .with_help("mova a função para o escopo global"));
                }

                Ok(BlockCheck {
                    always_returns: false,
                    saw_return: false,
                })
            }
            StmtKind::Return(value) => {
                if !in_function {
                    return Err(
                        self.err_at_span(stmt.span, "'return' só é permitido dentro de função")
                    );
                }
                let Some(expected) = expected_return else {
                    return Err(self.err_at_span(stmt.span, "retorno sem função de contexto"));
                };

                let got = if let Some(expr) = value {
                    self.infer_expr(expr, env)?
                } else {
                    Type::Unit
                };

                self.unify(expected, got, stmt.span, "tipo de retorno incompatível")?;

                Ok(BlockCheck {
                    always_returns: true,
                    saw_return: true,
                })
            }
        }
    }

    fn infer_expr(&mut self, expr: &Expr, env: &mut Env) -> Result<Type, CompileError> {
        match &expr.kind {
            ExprKind::Int(_) => Ok(self.fresh_infer(InferKind::Integer)),
            ExprKind::Float(_) => Ok(self.fresh_infer(InferKind::Float)),
            ExprKind::Bool(_) => Ok(Type::Bool),
            ExprKind::Char(_) => Ok(Type::Char),
            ExprKind::String(_) => Ok(Type::String),
            ExprKind::Var(name) => env.get(name).ok_or_else(|| {
                self.err_at_span(expr.span, format!("variável '{name}' não foi declarada"))
            }),
            ExprKind::UnaryOp(op, value) => {
                let value_ty = self.infer_expr(value, env)?;
                match op.as_str() {
                    "-" => {
                        self.ensure_number(
                            value_ty.clone(),
                            value.span,
                            "operador '-' exige número",
                        )?;
                        let resolved = self.resolve_type(&value_ty);
                        if resolved.is_unsigned_integer() {
                            return Err(self
                                .err_at_span(
                                    value.span,
                                    "operador '-' não pode ser aplicado a tipo unsigned",
                                )
                                .with_help("use um tipo assinado (ex.: i64)"));
                        }
                        Ok(value_ty)
                    }
                    "!" => {
                        self.unify(Type::Bool, value_ty, expr.span, "operador '!' exige bool")?;
                        Ok(Type::Bool)
                    }
                    _ => {
                        Err(self
                            .err_at_span(expr.span, format!("operador unário desconhecido: {op}")))
                    }
                }
            }
            ExprKind::BinaryOp(lhs, op, rhs) => {
                let left_ty = self.infer_expr(lhs, env)?;
                let right_ty = self.infer_expr(rhs, env)?;

                match op.as_str() {
                    "+" | "-" | "*" | "/" | "%" => {
                        let unified = self.unify(
                            left_ty,
                            right_ty,
                            expr.span,
                            "operandos incompatíveis para operação aritmética",
                        )?;
                        self.ensure_number(
                            unified.clone(),
                            expr.span,
                            "operação aritmética exige tipos numéricos",
                        )?;
                        Ok(unified)
                    }
                    "==" | "!=" => {
                        let _ = self.unify(
                            left_ty,
                            right_ty,
                            expr.span,
                            "comparação exige tipos compatíveis",
                        )?;
                        Ok(Type::Bool)
                    }
                    ">" | "<" | ">=" | "<=" => {
                        let unified = self.unify(
                            left_ty,
                            right_ty,
                            expr.span,
                            "comparação exige tipos compatíveis",
                        )?;
                        self.ensure_orderable(
                            unified,
                            expr.span,
                            "comparação relacional exige número ou char",
                        )?;
                        Ok(Type::Bool)
                    }
                    "&&" | "||" => {
                        self.unify(Type::Bool, left_ty, lhs.span, "operador lógico exige bool")?;
                        self.unify(Type::Bool, right_ty, rhs.span, "operador lógico exige bool")?;
                        Ok(Type::Bool)
                    }
                    _ => Err(self.err_at_span(expr.span, format!("operador desconhecido: {op}"))),
                }
            }
            ExprKind::Call(name, args) => self.infer_call(expr.span, name, args, env),
        }
    }

    fn infer_call(
        &mut self,
        span: Span,
        name: &str,
        args: &[Expr],
        env: &mut Env,
    ) -> Result<Type, CompileError> {
        match name {
            "term.print" | "term.println" | "term.output" | "term.eprint" | "term.eprintln"
            | "term.error" => {
                self.expect_arity(name, args, 1, span)?;
                let _ = self.infer_expr(&args[0], env)?;
                Ok(Type::Unit)
            }
            "term.input" => {
                self.expect_arity(name, args, 1, span)?;
                let prompt = self.infer_expr(&args[0], env)?;
                self.expect_string_like(prompt, args[0].span, "term.input espera prompt string")?;
                Ok(Type::String)
            }
            "fs.read" => {
                self.expect_arity(name, args, 1, span)?;
                let path = self.infer_expr(&args[0], env)?;
                self.expect_string_like(path, args[0].span, "fs.read espera caminho string")?;
                Ok(Type::String)
            }
            "fs.write" | "fs.append" => {
                self.expect_arity(name, args, 2, span)?;
                let path = self.infer_expr(&args[0], env)?;
                self.expect_string_like(path, args[0].span, "o caminho deve ser string")?;
                let content = self.infer_expr(&args[1], env)?;
                self.expect_string_like(content, args[1].span, "o conteúdo deve ser string")?;
                Ok(Type::Unit)
            }
            "fs.exists" => {
                self.expect_arity(name, args, 1, span)?;
                let path = self.infer_expr(&args[0], env)?;
                self.expect_string_like(path, args[0].span, "fs.exists espera caminho string")?;
                Ok(Type::Bool)
            }
            "str.upper" | "str.lower" => {
                self.expect_arity(name, args, 1, span)?;
                let value = self.infer_expr(&args[0], env)?;
                self.expect_string_like(value, args[0].span, "função de texto espera string")?;
                Ok(Type::String)
            }
            "str.contains" => {
                self.expect_arity(name, args, 2, span)?;
                let value = self.infer_expr(&args[0], env)?;
                self.expect_string_like(value, args[0].span, "o texto base deve ser string")?;
                let needle = self.infer_expr(&args[1], env)?;
                self.expect_string_like(needle, args[1].span, "o trecho buscado deve ser string")?;
                Ok(Type::Bool)
            }
            "str.len" => {
                self.expect_arity(name, args, 1, span)?;
                let value = self.infer_expr(&args[0], env)?;
                self.expect_string_like(value, args[0].span, "str.len espera string")?;
                Ok(Type::Usize)
            }
            "str.concat" => {
                self.expect_arity(name, args, 2, span)?;
                let left = self.infer_expr(&args[0], env)?;
                self.expect_string_like(left, args[0].span, "str.concat espera string no primeiro argumento")?;
                let right = self.infer_expr(&args[1], env)?;
                self.expect_string_like(right, args[1].span, "str.concat espera string no segundo argumento")?;
                Ok(Type::String)
            }
            "str.from_int" => {
                self.expect_arity(name, args, 1, span)?;
                let value = self.infer_expr(&args[0], env)?;
                self.expect_integer(value, args[0].span, "str.from_int espera tipo inteiro")?;
                Ok(Type::String)
            }
            "str.from_bool" => {
                self.expect_arity(name, args, 1, span)?;
                let value = self.infer_expr(&args[0], env)?;
                self.unify(Type::Bool, value, args[0].span, "str.from_bool espera bool")?;
                Ok(Type::String)
            }
            "sqlite.exec" => {
                self.expect_arity(name, args, 2, span)?;
                let db_path = self.infer_expr(&args[0], env)?;
                self.expect_string_like(db_path, args[0].span, "sqlite.exec espera caminho string")?;
                let sql = self.infer_expr(&args[1], env)?;
                self.expect_string_like(sql, args[1].span, "sqlite.exec espera SQL string")?;
                Ok(Type::Unit)
            }
            "sqlite.query" => {
                self.expect_arity(name, args, 2, span)?;
                let db_path = self.infer_expr(&args[0], env)?;
                self.expect_string_like(db_path, args[0].span, "sqlite.query espera caminho string")?;
                let sql = self.infer_expr(&args[1], env)?;
                self.expect_string_like(sql, args[1].span, "sqlite.query espera SQL string")?;
                Ok(Type::String)
            }
            "crypto.sha256" => {
                self.expect_arity(name, args, 1, span)?;
                let value = self.infer_expr(&args[0], env)?;
                self.expect_string_like(value, args[0].span, "crypto.sha256 espera string")?;
                Ok(Type::String)
            }
            "crypto.verify_sha256" => {
                self.expect_arity(name, args, 2, span)?;
                let value = self.infer_expr(&args[0], env)?;
                self.expect_string_like(value, args[0].span, "crypto.verify_sha256 espera string no primeiro argumento")?;
                let hash = self.infer_expr(&args[1], env)?;
                self.expect_string_like(hash, args[1].span, "crypto.verify_sha256 espera hash string no segundo argumento")?;
                Ok(Type::Bool)
            }
            "crypto.random_token" => {
                self.expect_arity(name, args, 0, span)?;
                Ok(Type::String)
            }
            "html.escape" => {
                self.expect_arity(name, args, 1, span)?;
                let value = self.infer_expr(&args[0], env)?;
                self.expect_string_like(value, args[0].span, "html.escape espera string")?;
                Ok(Type::String)
            }
            "html.page" => {
                self.expect_arity(name, args, 2, span)?;
                let title = self.infer_expr(&args[0], env)?;
                self.expect_string_like(title, args[0].span, "html.page espera título string")?;
                let body = self.infer_expr(&args[1], env)?;
                self.expect_string_like(body, args[1].span, "html.page espera body string")?;
                Ok(Type::String)
            }
            "web.start" => {
                self.expect_arity(name, args, 1, span)?;
                let port = self.infer_expr(&args[0], env)?;
                self.expect_integer(port, args[0].span, "web.start espera porta inteira")?;
                Ok(Type::Unit)
            }
            "web.get" | "web.post" => {
                self.expect_arity(name, args, 2, span)?;
                let path = self.infer_expr(&args[0], env)?;
                self.expect_string_like(path, args[0].span, "rota deve ser string")?;
                let handler = self.infer_expr(&args[1], env)?;
                self.expect_string_like(handler, args[1].span, "handler deve ser string")?;
                Ok(Type::Unit)
            }
            "web.run" => {
                self.expect_arity(name, args, 0, span)?;
                Ok(Type::Unit)
            }
            "web.html" | "web.redirect" => {
                self.expect_arity(name, args, 1, span)?;
                let value = self.infer_expr(&args[0], env)?;
                self.expect_string_like(value, args[0].span, "web resposta espera string")?;
                Ok(Type::String)
            }
            "panic" => {
                self.expect_arity(name, args, 1, span)?;
                let _ = self.infer_expr(&args[0], env)?;
                Ok(Type::Never)
            }
            _ => {
                if name.contains('.') {
                    return Err(self.err_at_span(
                        span,
                        format!("função de biblioteca padrão desconhecida: {name}"),
                    ));
                }

                let sig = self.functions.get(name).cloned().ok_or_else(|| {
                    self.err_at_span(span, format!("função '{name}' não foi declarada"))
                })?;

                if sig.params.len() != args.len() {
                    return Err(self.err_at_span(
                        span,
                        format!(
                            "função '{name}' espera {} argumento(s), recebeu {}",
                            sig.params.len(),
                            args.len()
                        ),
                    ));
                }

                for (index, arg) in args.iter().enumerate() {
                    let arg_ty = self.infer_expr(arg, env)?;
                    self.unify(
                        sig.params[index].clone(),
                        arg_ty,
                        arg.span,
                        format!("tipo inválido no argumento {} de '{}'", index + 1, name),
                    )?;
                }

                Ok(sig.ret)
            }
        }
    }

    fn expect_arity(
        &self,
        name: &str,
        args: &[Expr],
        expected: usize,
        span: Span,
    ) -> Result<(), CompileError> {
        if args.len() != expected {
            return Err(self.err_at_span(
                span,
                format!(
                    "'{name}' espera {expected} argumento(s), recebeu {}",
                    args.len()
                ),
            ));
        }
        Ok(())
    }

    fn expect_string_like(
        &mut self,
        ty: Type,
        span: Span,
        message: &str,
    ) -> Result<(), CompileError> {
        let resolved = self.resolve_type(&ty);
        match resolved {
            Type::String | Type::Str => Ok(()),
            Type::Infer(_) => {
                self.unify(ty, Type::String, span, message.to_string())?;
                Ok(())
            }
            _ => Err(self
                .err_at_span(span, message)
                .with_help(format!("tipo recebido: {}", resolved.display_name()))),
        }
    }

    fn expect_integer(&mut self, ty: Type, span: Span, message: &str) -> Result<(), CompileError> {
        let resolved = self.resolve_type(&ty);
        match resolved {
            concrete if concrete.is_integer() => Ok(()),
            Type::Infer(_) => {
                let integer_ty = self.fresh_infer(InferKind::Integer);
                self.unify(ty, integer_ty, span, message.to_string())?;
                Ok(())
            }
            other => Err(self
                .err_at_span(span, message)
                .with_help(format!("tipo recebido: {}", other.display_name()))),
        }
    }

    fn ensure_number(&mut self, ty: Type, span: Span, message: &str) -> Result<(), CompileError> {
        let resolved = self.resolve_type(&ty);
        match resolved {
            Type::Infer(_) => {
                let number_ty = self.fresh_infer(InferKind::Number);
                self.unify(ty, number_ty, span, message.to_string())?;
                Ok(())
            }
            concrete if concrete.is_number() => Ok(()),
            other => Err(self
                .err_at_span(span, message)
                .with_help(format!("tipo recebido: {}", other.display_name()))),
        }
    }

    fn ensure_orderable(
        &mut self,
        ty: Type,
        span: Span,
        message: &str,
    ) -> Result<(), CompileError> {
        let resolved = self.resolve_type(&ty);
        match resolved {
            concrete if concrete.is_orderable() => Ok(()),
            Type::Infer(_) => self.ensure_number(ty, span, message),
            other => Err(self
                .err_at_span(span, message)
                .with_help(format!("tipo recebido: {}", other.display_name()))),
        }
    }

    fn type_from_ref(&self, type_ref: &TypeRef) -> Result<Type, CompileError> {
        match &type_ref.kind {
            TypeRefKind::Unit => Ok(Type::Unit),
            TypeRefKind::Never => Ok(Type::Never),
            TypeRefKind::Named(name) => match name.as_str() {
                "i8" => Ok(Type::I8),
                "i16" => Ok(Type::I16),
                "i32" => Ok(Type::I32),
                "i64" => Ok(Type::I64),
                "i128" => Ok(Type::I128),
                "u8" => Ok(Type::U8),
                "u16" => Ok(Type::U16),
                "u32" => Ok(Type::U32),
                "u64" => Ok(Type::U64),
                "u128" => Ok(Type::U128),
                "isize" => Ok(Type::Isize),
                "usize" => Ok(Type::Usize),
                "f32" => Ok(Type::F32),
                "f64" => Ok(Type::F64),
                "int" => Ok(Type::I64),
                "float" => Ok(Type::F64),
                "bool" => Ok(Type::Bool),
                "char" => Ok(Type::Char),
                "str" => Ok(Type::Str),
                "String" => Ok(Type::String),
                _ => Err(self
                    .err_at_span(type_ref.span, format!("tipo desconhecido: {name}"))
                    .with_help(
                        "tipos suportados: int, float, i8/i16/i32/i64/i128, u8/u16/u32/u64/u128, isize, usize, f32, f64, bool, char, str, String, (), !",
                    )),
            },
        }
    }

    fn fresh_infer(&mut self, kind: InferKind) -> Type {
        let id = self.next_infer_id;
        self.next_infer_id += 1;
        self.infer_kinds.insert(id, kind);
        Type::Infer(id)
    }

    fn resolve_type(&mut self, ty: &Type) -> Type {
        match ty {
            Type::Infer(id) => {
                let next = self.substitutions.get(id).cloned();
                if let Some(next_ty) = next {
                    let resolved = self.resolve_type(&next_ty);
                    self.substitutions.insert(*id, resolved.clone());
                    resolved
                } else {
                    Type::Infer(*id)
                }
            }
            other => other.clone(),
        }
    }

    fn resolve_with_defaults(&mut self, ty: Type) -> Type {
        let resolved = self.resolve_type(&ty);
        match resolved {
            Type::Infer(id) => {
                let kind = self.infer_kinds.get(&id).copied().unwrap_or(InferKind::Any);
                let default = match kind {
                    InferKind::Float => Type::F64,
                    InferKind::Integer | InferKind::Number | InferKind::Any => Type::I64,
                };
                self.substitutions.insert(id, default.clone());
                default
            }
            other => other,
        }
    }

    fn set_infer_type(
        &mut self,
        id: u32,
        ty: Type,
        span: Span,
        context: &str,
    ) -> Result<(), CompileError> {
        if let Type::Infer(other_id) = ty {
            if id == other_id {
                return Ok(());
            }
        }

        let kind = self.infer_kinds.get(&id).copied().unwrap_or(InferKind::Any);
        if !self.type_fits_kind(&ty, kind) {
            return Err(self.err_at_span(span, context).with_help(format!(
                "esperado tipo compatível com inferência {:?}, recebeu {}",
                kind,
                ty.display_name()
            )));
        }

        self.substitutions.insert(id, ty);
        Ok(())
    }

    fn type_fits_kind(&self, ty: &Type, kind: InferKind) -> bool {
        match kind {
            InferKind::Any => true,
            InferKind::Integer => ty.is_integer() || matches!(ty, Type::Infer(_)),
            InferKind::Float => ty.is_float() || matches!(ty, Type::Infer(_)),
            InferKind::Number => ty.is_number() || matches!(ty, Type::Infer(_)),
        }
    }

    fn unify(
        &mut self,
        left: Type,
        right: Type,
        span: Span,
        context: impl Into<String>,
    ) -> Result<Type, CompileError> {
        let context = context.into();
        let left = self.resolve_type(&left);
        let right = self.resolve_type(&right);

        if left == right {
            return Ok(left);
        }

        match (left.clone(), right.clone()) {
            (Type::Infer(id), ty) => {
                self.set_infer_type(id, ty.clone(), span, &context)?;
                Ok(ty)
            }
            (ty, Type::Infer(id)) => {
                self.set_infer_type(id, ty.clone(), span, &context)?;
                Ok(ty)
            }
            (Type::String, Type::Str) | (Type::Str, Type::String) => Ok(Type::String),
            (l, r) => Err(self
                .err_at_span(
                    span,
                    format!(
                        "{}: '{}' vs '{}'",
                        context,
                        l.display_name(),
                        r.display_name()
                    ),
                )
                .with_help("adicione anotação explícita de tipo para remover ambiguidade")),
        }
    }

    fn err_at_span(&self, span: Span, message: impl Into<String>) -> CompileError {
        CompileError::new(message, span.line, span.column).with_span(span.len)
    }

    fn finalize(&mut self) -> Result<TypeInfo, CompileError> {
        let mut out = HashMap::new();
        for (name, sig) in self.functions.clone() {
            let params = sig
                .params
                .into_iter()
                .map(|ty| self.resolve_with_defaults(ty))
                .collect::<Vec<_>>();
            let ret = self.resolve_with_defaults(sig.ret);
            out.insert(name, FunctionSig { params, ret });
        }
        Ok(TypeInfo { function_sigs: out })
    }
}
