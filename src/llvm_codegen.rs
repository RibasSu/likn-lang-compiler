use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::ast::{Expr, ExprKind, Span, Stmt, StmtKind, TypeRef, TypeRefKind};
use crate::typecheck::{FunctionSig, Type, TypeInfo};

#[derive(Debug, Clone)]
struct Value {
    ty: Type,
    operand: String,
}

#[derive(Debug, Clone)]
struct LocalBinding {
    ptr: String,
    ty: Type,
}

#[derive(Debug, Clone)]
struct GlobalConst {
    ty: Type,
    symbol: String,
}

#[derive(Debug, Clone)]
struct StringGlobal {
    name: String,
    bytes: Vec<u8>,
}

#[derive(Debug, Clone)]
enum ConstValue {
    Int(i128),
    Float(f64),
    Bool(bool),
    Char(char),
    Str(String),
}

#[derive(Debug, Clone)]
struct FunctionState {
    name: String,
    ret_ty: Type,
    entry_allocas: Vec<String>,
    body: Vec<String>,
    reg_counter: usize,
    label_counter: usize,
    scopes: Vec<HashMap<String, LocalBinding>>,
    terminated: bool,
}

impl FunctionState {
    fn new(name: impl Into<String>, ret_ty: Type) -> Self {
        Self {
            name: name.into(),
            ret_ty,
            entry_allocas: Vec::new(),
            body: Vec::new(),
            reg_counter: 0,
            label_counter: 0,
            scopes: vec![HashMap::new()],
            terminated: false,
        }
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        let _ = self.scopes.pop();
    }

    fn insert_local(&mut self, name: String, binding: LocalBinding) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, binding);
        }
    }

    fn lookup_local(&self, name: &str) -> Option<LocalBinding> {
        for scope in self.scopes.iter().rev() {
            if let Some(binding) = scope.get(name) {
                return Some(binding.clone());
            }
        }
        None
    }

    fn next_reg(&mut self, prefix: &str) -> String {
        self.reg_counter += 1;
        format!("%{}.{}", sanitize_ident(prefix), self.reg_counter)
    }

    fn next_label(&mut self, prefix: &str) -> String {
        self.label_counter += 1;
        format!("{}.{}", sanitize_ident(prefix), self.label_counter)
    }

    fn emit_instr(&mut self, line: impl Into<String>) {
        self.body.push(format!("  {}", line.into()));
    }

    fn emit_label(&mut self, label: &str) {
        self.body.push(format!("{}:", label));
        self.terminated = false;
    }

    fn alloc_local(&mut self, hint: &str, ty: &Type) -> String {
        let ptr = self.next_reg(&format!("{}.addr", hint));
        self.entry_allocas
            .push(format!("{} = alloca {}", ptr, llvm_type(ty)));
        ptr
    }

    fn render(self, params_ir: &str) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "define {} @{}({}) {{\n",
            llvm_type(&self.ret_ty),
            sanitize_ident(&self.name),
            params_ir
        ));
        out.push_str("entry:\n");
        for line in self.entry_allocas {
            out.push_str("  ");
            out.push_str(&line);
            out.push('\n');
        }
        for line in self.body {
            out.push_str(&line);
            out.push('\n');
        }
        out.push_str("}\n");
        out
    }
}

struct LlvmGenerator<'a> {
    source: String,
    type_info: &'a TypeInfo,
    externs: BTreeSet<String>,
    string_pool: BTreeMap<Vec<u8>, StringGlobal>,
    string_order: Vec<Vec<u8>>,
    format_pool: HashMap<String, String>,
    globals: Vec<String>,
    functions: Vec<String>,
    global_consts: HashMap<String, GlobalConst>,
    const_values: HashMap<String, ConstValue>,
    errors: Vec<String>,
    fixmes: Vec<String>,
}

impl<'a> LlvmGenerator<'a> {
    fn new(type_info: &'a TypeInfo, source: &str) -> Self {
        Self {
            source: source.to_string(),
            type_info,
            externs: BTreeSet::new(),
            string_pool: BTreeMap::new(),
            string_order: Vec::new(),
            format_pool: HashMap::new(),
            globals: Vec::new(),
            functions: Vec::new(),
            global_consts: HashMap::new(),
            const_values: HashMap::new(),
            errors: Vec::new(),
            fixmes: Vec::new(),
        }
    }

    fn compile(mut self, ast: &[Stmt]) -> String {
        let mut function_stmts = Vec::new();
        let mut entry_stmts = Vec::new();

        for stmt in ast {
            match &stmt.kind {
                StmtKind::Func { .. } => function_stmts.push(stmt.clone()),
                StmtKind::Export(inner) if matches!(inner.kind, StmtKind::Func { .. }) => {
                    function_stmts.push((**inner).clone())
                }
                StmtKind::Const { .. } => self.emit_global_const(stmt),
                StmtKind::Export(inner) if matches!(inner.kind, StmtKind::Const { .. }) => {
                    self.emit_global_const(inner)
                }
                StmtKind::Import(_) => {}
                _ => entry_stmts.push(stmt.clone()),
            }
        }

        for stmt in &function_stmts {
            let function_ir = self.compile_function(stmt);
            self.functions.push(function_ir);
        }

        let entry_ir = self.compile_entry_point(&entry_stmts);
        self.functions.push(entry_ir);
        self.render_module()
    }

    fn render_module(&self) -> String {
        let mut out = String::new();

        for err in &self.errors {
            out.push_str(&format!("; ERROR: {}\n", err));
        }
        for fix in &self.fixmes {
            out.push_str(&format!("; FIXME: {}\n", fix));
        }

        out.push_str("; === Likn Compiled Module ===\n");
        out.push_str(&format!("; Source: {}\n\n", self.source));

        out.push_str("; --- External Declarations ---\n");
        for decl in &self.externs {
            out.push_str(decl);
            out.push('\n');
        }
        out.push('\n');

        out.push_str("; --- Global Constants ---\n");
        for key in &self.string_order {
            if let Some(global) = self.string_pool.get(key) {
                out.push_str(&format!(
                    "@{} = private unnamed_addr constant [{} x i8] c\"{}\"\n",
                    global.name,
                    global.bytes.len(),
                    encode_llvm_cstring(&global.bytes)
                ));
            }
        }
        for global in &self.globals {
            out.push_str(global);
            out.push('\n');
        }
        out.push('\n');

        out.push_str("; --- Function Definitions ---\n");
        for function in &self.functions {
            if function.starts_with("define i64 @likn_main") {
                continue;
            }
            out.push_str(function);
            out.push('\n');
        }

        out.push_str("; --- Entry Point ---\n");
        if let Some(entry) = self
            .functions
            .iter()
            .find(|function| function.starts_with("define i64 @likn_main"))
        {
            out.push_str(entry);
        }

        out
    }

    fn compile_function(&mut self, stmt: &Stmt) -> String {
        let StmtKind::Func {
            name, params, body, ..
        } = &stmt.kind
        else {
            return String::new();
        };

        let signature = self
            .type_info
            .function_sigs
            .get(name)
            .cloned()
            .unwrap_or_else(|| FunctionSig {
                params: params
                    .iter()
                    .map(|param| {
                        param
                            .ty
                            .as_ref()
                            .and_then(type_from_ref)
                            .unwrap_or(Type::I64)
                    })
                    .collect(),
                ret: Type::I64,
            });

        let mut state = FunctionState::new(name.clone(), signature.ret.clone());

        let mut params_ir = Vec::new();
        for (index, param) in params.iter().enumerate() {
            let param_ty = signature.params.get(index).cloned().unwrap_or(Type::I64);
            let param_name = sanitize_ident(&param.name);
            params_ir.push(format!("{} %{}", llvm_type(&param_ty), param_name));

            let ptr = state.alloc_local(&param.name, &param_ty);
            state.emit_instr(format!(
                "store {} %{}, {}* {}",
                llvm_type(&param_ty),
                param_name,
                llvm_type(&param_ty),
                ptr
            ));
            state.insert_local(
                param.name.clone(),
                LocalBinding {
                    ptr,
                    ty: param_ty,
                },
            );
        }

        self.emit_block(&mut state, body, true);

        if !state.terminated {
            match state.ret_ty {
                Type::Unit => {
                    state.emit_instr("ret void");
                    state.terminated = true;
                }
                Type::Never => {
                    state.emit_instr("unreachable");
                    state.terminated = true;
                }
                _ => {
                    self.fixmes.push(format!(
                        "função '{}' terminou sem retorno explícito, retornando valor padrão",
                        name
                    ));
                    state.emit_instr(format!(
                        "ret {} {}",
                        llvm_type(&state.ret_ty),
                        default_value(&state.ret_ty)
                    ));
                    state.terminated = true;
                }
            }
        }

        state.render(&params_ir.join(", "))
    }

    fn compile_entry_point(&mut self, stmts: &[Stmt]) -> String {
        let mut state = FunctionState::new("likn_main", Type::I64);
        self.emit_block(&mut state, stmts, false);

        if !state.terminated {
            state.emit_instr("ret i64 0");
            state.terminated = true;
        }

        state.render("")
    }

    fn emit_block(&mut self, state: &mut FunctionState, stmts: &[Stmt], allow_tail_expr_return: bool) {
        state.push_scope();

        for (index, stmt) in stmts.iter().enumerate() {
            if state.terminated {
                break;
            }

            if allow_tail_expr_return
                && index + 1 == stmts.len()
                && !matches!(state.ret_ty, Type::Unit | Type::Never)
                && matches!(stmt.kind, StmtKind::Expr(_))
            {
                if let StmtKind::Expr(expr) = &stmt.kind {
                    let ret_ty = state.ret_ty.clone();
                    let value = self.emit_expr(state, expr, Some(&ret_ty));
                    let coerced = self.coerce_value(state, value, &ret_ty, stmt.span);
                    state.emit_instr(format!(
                        "ret {} {}",
                        llvm_type(&ret_ty),
                        coerced.operand
                    ));
                    state.terminated = true;
                    break;
                }
            }

            self.emit_stmt(state, stmt);
        }

        state.pop_scope();
    }

    fn emit_stmt(&mut self, state: &mut FunctionState, stmt: &Stmt) {
        match &stmt.kind {
            StmtKind::Import(_) => {}
            StmtKind::Export(inner) => self.emit_stmt(state, inner),
            StmtKind::Let {
                name,
                mutable: _,
                ty,
                expr,
            } => {
                let expected = ty.as_ref().and_then(type_from_ref);
                let inferred = expected.clone().unwrap_or_else(|| self.infer_expr_type(state, expr));
                let value = self.emit_expr(state, expr, Some(&inferred));
                let value = self.coerce_value(state, value, &inferred, stmt.span);
                let ptr = state.alloc_local(name, &inferred);
                state.emit_instr(format!(
                    "store {} {}, {}* {}",
                    llvm_type(&inferred),
                    value.operand,
                    llvm_type(&inferred),
                    ptr
                ));
                state.insert_local(
                    name.clone(),
                    LocalBinding {
                        ptr,
                        ty: inferred,
                    },
                );
            }
            StmtKind::Const { name, ty, expr } => {
                let expected = ty.as_ref().and_then(type_from_ref);
                let inferred = expected.clone().unwrap_or_else(|| self.infer_expr_type(state, expr));
                let value = self.emit_expr(state, expr, Some(&inferred));
                let value = self.coerce_value(state, value, &inferred, stmt.span);
                let ptr = state.alloc_local(name, &inferred);
                state.emit_instr(format!(
                    "store {} {}, {}* {}",
                    llvm_type(&inferred),
                    value.operand,
                    llvm_type(&inferred),
                    ptr
                ));
                state.insert_local(
                    name.clone(),
                    LocalBinding {
                        ptr,
                        ty: inferred,
                    },
                );
            }
            StmtKind::Expr(expr) => {
                let _ = self.emit_expr(state, expr, None);
            }
            StmtKind::Print(expr) => {
                self.emit_print(state, expr);
            }
            StmtKind::Return(value) => {
                match value {
                    Some(expr) => {
                        let target_ty = state.ret_ty.clone();
                        let value = self.emit_expr(state, expr, Some(&target_ty));
                        let value = self.coerce_value(state, value, &target_ty, stmt.span);
                        state.emit_instr(format!(
                            "ret {} {}",
                            llvm_type(&target_ty),
                            value.operand
                        ));
                    }
                    None => {
                        state.emit_instr("ret void");
                    }
                }
                state.terminated = true;
            }
            StmtKind::If {
                cond,
                then_block,
                else_block,
            } => {
                self.emit_if(state, cond, then_block, else_block);
            }
            StmtKind::Func { .. } => {
                self.fixmes
                    .push("função local ignorada (não suportada em LLVM backend)".to_string());
            }
        }
    }

    fn emit_if(
        &mut self,
        state: &mut FunctionState,
        cond: &Expr,
        then_block: &[Stmt],
        else_block: &[Stmt],
    ) {
        let cond_value = self.emit_expr(state, cond, Some(&Type::Bool));
        let cond_value = self.coerce_value(state, cond_value, &Type::Bool, cond.span);

        let then_label = state.next_label("if.then");
        let else_label = state.next_label("if.else");
        let merge_label = state.next_label("if.merge");

        if else_block.is_empty() {
            state.emit_instr(format!(
                "br i1 {}, label %{}, label %{}",
                cond_value.operand, then_label, merge_label
            ));

            state.emit_label(&then_label);
            self.emit_block(state, then_block, false);
            let then_terminated = state.terminated;
            if !then_terminated {
                state.emit_instr(format!("br label %{}", merge_label));
            }

            state.emit_label(&merge_label);
            return;
        }

        state.emit_instr(format!(
            "br i1 {}, label %{}, label %{}",
            cond_value.operand, then_label, else_label
        ));

        state.emit_label(&then_label);
        self.emit_block(state, then_block, false);
        let then_terminated = state.terminated;
        if !then_terminated {
            state.emit_instr(format!("br label %{}", merge_label));
        }

        state.emit_label(&else_label);
        self.emit_block(state, else_block, false);
        let else_terminated = state.terminated;
        if !else_terminated {
            state.emit_instr(format!("br label %{}", merge_label));
        }

        if then_terminated && else_terminated {
            state.terminated = true;
        } else {
            state.emit_label(&merge_label);
            state.terminated = false;
        }
    }

    fn emit_print(&mut self, state: &mut FunctionState, expr: &Expr) {
        let value = self.emit_expr(state, expr, None);
        self.externs
            .insert("declare void @likn_print(i8*)".to_string());

        if matches!(value.ty, Type::Str | Type::String) {
            state.emit_instr(format!("call void @likn_print(i8* {})", value.operand));
            return;
        }

        self.externs
            .insert("declare i32 @sprintf(i8*, i8*, ...)".to_string());

        let fmt = match value.ty {
            Type::Bool => "%d\\0",
            Type::F32 | Type::F64 => "%f\\0",
            Type::Char => "%c\\0",
            _ => "%ld\\0",
        };

        let fmt_name = self.get_or_create_format_global(fmt);
        let fmt_ptr = state.next_reg("fmt.ptr");
        let buf = state.next_reg("print.buf");
        let buf_ptr = state.next_reg("print.buf.ptr");

        state.emit_instr(format!("{} = alloca [64 x i8]", buf));
        state.emit_instr(format!(
            "{} = getelementptr inbounds [64 x i8], [64 x i8]* {}, i32 0, i32 0",
            buf_ptr, buf
        ));
        state.emit_instr(format!(
            "{} = getelementptr inbounds [{} x i8], [{} x i8]* @{}, i32 0, i32 0",
            fmt_ptr,
            fmt_bytes_len(fmt),
            fmt_bytes_len(fmt),
            fmt_name
        ));

        let formatted_arg = self.prepare_printf_arg(state, value);
        state.emit_instr(format!(
            "call i32 (i8*, i8*, ...) @sprintf(i8* {}, i8* {}, {} {})",
            buf_ptr,
            fmt_ptr,
            llvm_type(&formatted_arg.ty),
            formatted_arg.operand
        ));
        state.emit_instr(format!("call void @likn_print(i8* {})", buf_ptr));
    }

    fn prepare_printf_arg(&mut self, state: &mut FunctionState, value: Value) -> Value {
        match value.ty {
            Type::Bool => {
                let reg = state.next_reg("bool.zext");
                state.emit_instr(format!("{} = zext i1 {} to i32", reg, value.operand));
                Value {
                    ty: Type::I32,
                    operand: reg,
                }
            }
            Type::I8 | Type::I16 | Type::I32 | Type::U8 | Type::U16 | Type::U32 | Type::Char => {
                let reg = state.next_reg("int.ext");
                state.emit_instr(format!(
                    "{} = sext {} {} to i64",
                    reg,
                    llvm_type(&value.ty),
                    value.operand
                ));
                Value {
                    ty: Type::I64,
                    operand: reg,
                }
            }
            Type::U64 | Type::U128 | Type::Usize => {
                let reg = state.next_reg("uint.cast");
                state.emit_instr(format!(
                    "{} = trunc {} {} to i64",
                    reg,
                    llvm_type(&value.ty),
                    value.operand
                ));
                Value {
                    ty: Type::I64,
                    operand: reg,
                }
            }
            Type::I128 => {
                let reg = state.next_reg("i128.trunc");
                state.emit_instr(format!("{} = trunc i128 {} to i64", reg, value.operand));
                self.fixmes
                    .push("print de i128 usa truncamento para i64".to_string());
                Value {
                    ty: Type::I64,
                    operand: reg,
                }
            }
            Type::F32 => {
                let reg = state.next_reg("f32.ext");
                state.emit_instr(format!("{} = fpext float {} to double", reg, value.operand));
                Value {
                    ty: Type::F64,
                    operand: reg,
                }
            }
            _ => value,
        }
    }

    fn emit_expr(&mut self, state: &mut FunctionState, expr: &Expr, expected: Option<&Type>) -> Value {
        match &expr.kind {
            ExprKind::Int(value) => {
                let ty = expected
                    .filter(|ty| is_integer_type(ty))
                    .cloned()
                    .unwrap_or(Type::I64);
                Value {
                    ty,
                    operand: value.to_string(),
                }
            }
            ExprKind::Float(value) => {
                let ty = expected
                    .filter(|ty| matches!(ty, Type::F32 | Type::F64))
                    .cloned()
                    .unwrap_or(Type::F64);
                Value {
                    ty,
                    operand: format_float(*value),
                }
            }
            ExprKind::Bool(value) => Value {
                ty: Type::Bool,
                operand: if *value { "1".to_string() } else { "0".to_string() },
            },
            ExprKind::Char(ch) => Value {
                ty: Type::Char,
                operand: (*ch as u32).to_string(),
            },
            ExprKind::String(value) => {
                let global = self.get_or_create_string_global(value);
                let reg = state.next_reg("str.ptr");
                state.emit_instr(format!(
                    "{} = getelementptr inbounds [{} x i8], [{} x i8]* @{}, i32 0, i32 0",
                    reg,
                    global.bytes.len(),
                    global.bytes.len(),
                    global.name
                ));
                Value {
                    ty: Type::String,
                    operand: reg,
                }
            }
            ExprKind::Var(name) => {
                if let Some(local) = state.lookup_local(name) {
                    let reg = state.next_reg(name);
                    state.emit_instr(format!(
                        "{} = load {}, {}* {}",
                        reg,
                        llvm_type(&local.ty),
                        llvm_type(&local.ty),
                        local.ptr
                    ));
                    return Value {
                        ty: local.ty,
                        operand: reg,
                    };
                }

                if let Some(global) = self.global_consts.get(name) {
                    let reg = state.next_reg(name);
                    state.emit_instr(format!(
                        "{} = load {}, {}* @{}",
                        reg,
                        llvm_type(&global.ty),
                        llvm_type(&global.ty),
                        global.symbol
                    ));
                    return Value {
                        ty: global.ty.clone(),
                        operand: reg,
                    };
                }

                self.errors.push(format!(
                    "variável '{}' não encontrada na geração LLVM (linha {}, coluna {})",
                    name, expr.span.line, expr.span.column
                ));
                Value {
                    ty: Type::I64,
                    operand: "0".to_string(),
                }
            }
            ExprKind::UnaryOp(op, value) => {
                let value = self.emit_expr(state, value, expected);
                match op.as_str() {
                    "!" => {
                        let value = self.coerce_value(state, value, &Type::Bool, expr.span);
                        let reg = state.next_reg("not");
                        state.emit_instr(format!("{} = xor i1 {}, true", reg, value.operand));
                        Value {
                            ty: Type::Bool,
                            operand: reg,
                        }
                    }
                    "-" => {
                        if matches!(value.ty, Type::F32 | Type::F64) {
                            let reg = state.next_reg("negf");
                            state.emit_instr(format!(
                                "{} = fneg {} {}",
                                reg,
                                llvm_type(&value.ty),
                                value.operand
                            ));
                            Value {
                                ty: value.ty,
                                operand: reg,
                            }
                        } else {
                            let target_ty = if is_integer_type(&value.ty) {
                                value.ty.clone()
                            } else {
                                Type::I64
                            };
                            let value = self.coerce_value(state, value, &target_ty, expr.span);
                            let reg = state.next_reg("negi");
                            state.emit_instr(format!(
                                "{} = sub {} 0, {}",
                                reg,
                                llvm_type(&target_ty),
                                value.operand
                            ));
                            Value {
                                ty: target_ty,
                                operand: reg,
                            }
                        }
                    }
                    _ => {
                        self.errors.push(format!("operador unário desconhecido: {}", op));
                        Value {
                            ty: Type::I64,
                            operand: "0".to_string(),
                        }
                    }
                }
            }
            ExprKind::BinaryOp(lhs, op, rhs) => self.emit_binary_expr(state, lhs, op, rhs, expr.span),
            ExprKind::Call(name, args) => self.emit_call_expr(state, name, args, expr.span),
        }
    }

    fn emit_binary_expr(
        &mut self,
        state: &mut FunctionState,
        lhs: &Expr,
        op: &str,
        rhs: &Expr,
        span: Span,
    ) -> Value {
        match op {
            "&&" | "||" => {
                let left_raw = self.emit_expr(state, lhs, Some(&Type::Bool));
                let left = self.coerce_value(state, left_raw, &Type::Bool, span);
                let right_raw = self.emit_expr(state, rhs, Some(&Type::Bool));
                let right = self.coerce_value(state, right_raw, &Type::Bool, span);
                let reg = state.next_reg(if op == "&&" { "and" } else { "or" });
                let inst = if op == "&&" { "and" } else { "or" };
                state.emit_instr(format!("{} = {} i1 {}, {}", reg, inst, left.operand, right.operand));
                Value {
                    ty: Type::Bool,
                    operand: reg,
                }
            }
            "==" | "!=" | ">" | "<" | ">=" | "<=" => {
                let left = self.emit_expr(state, lhs, None);
                let right = self.emit_expr(state, rhs, Some(&left.ty));
                let common_ty = self.common_numeric_or_same_type(&left.ty, &right.ty);
                let left = self.coerce_value(state, left, &common_ty, span);
                let right = self.coerce_value(state, right, &common_ty, span);

                let reg = state.next_reg("cmp");
                if matches!(common_ty, Type::F32 | Type::F64) {
                    let pred = match op {
                        "==" => "oeq",
                        "!=" => "one",
                        ">" => "ogt",
                        "<" => "olt",
                        ">=" => "oge",
                        "<=" => "ole",
                        _ => "oeq",
                    };
                    state.emit_instr(format!(
                        "{} = fcmp {} {} {}, {}",
                        reg,
                        pred,
                        llvm_type(&common_ty),
                        left.operand,
                        right.operand
                    ));
                } else {
                    let pred = match op {
                        "==" => "eq",
                        "!=" => "ne",
                        ">" => "sgt",
                        "<" => "slt",
                        ">=" => "sge",
                        "<=" => "sle",
                        _ => "eq",
                    };
                    state.emit_instr(format!(
                        "{} = icmp {} {} {}, {}",
                        reg,
                        pred,
                        llvm_type(&common_ty),
                        left.operand,
                        right.operand
                    ));
                }

                Value {
                    ty: Type::Bool,
                    operand: reg,
                }
            }
            "+" | "-" | "*" | "/" | "%" => {
                let left = self.emit_expr(state, lhs, None);
                let right = self.emit_expr(state, rhs, Some(&left.ty));
                let common_ty = self.common_numeric_or_same_type(&left.ty, &right.ty);
                let left = self.coerce_value(state, left, &common_ty, span);
                let right = self.coerce_value(state, right, &common_ty, span);
                let reg = state.next_reg("arith");

                let inst = if matches!(common_ty, Type::F32 | Type::F64) {
                    match op {
                        "+" => "fadd",
                        "-" => "fsub",
                        "*" => "fmul",
                        "/" => "fdiv",
                        "%" => {
                            self.fixmes.push(
                                "operador '%' em float não suportado diretamente; usando frem"
                                    .to_string(),
                            );
                            "frem"
                        }
                        _ => "fadd",
                    }
                } else {
                    match op {
                        "+" => "add",
                        "-" => "sub",
                        "*" => "mul",
                        "/" => "sdiv",
                        "%" => "srem",
                        _ => "add",
                    }
                };

                state.emit_instr(format!(
                    "{} = {} {} {}, {}",
                    reg,
                    inst,
                    llvm_type(&common_ty),
                    left.operand,
                    right.operand
                ));
                Value {
                    ty: common_ty,
                    operand: reg,
                }
            }
            _ => {
                self.errors
                    .push(format!("operador binário desconhecido: {}", op));
                Value {
                    ty: Type::I64,
                    operand: "0".to_string(),
                }
            }
        }
    }

    fn emit_call_expr(
        &mut self,
        state: &mut FunctionState,
        name: &str,
        args: &[Expr],
        span: Span,
    ) -> Value {
        if let Some(value) = self.emit_builtin_call(state, name, args, span) {
            return value;
        }

        let Some(sig) = self.type_info.function_sigs.get(name).cloned() else {
            if name.contains('.') {
                self.errors.push(format!(
                    "função de biblioteca padrão desconhecida em LLVM backend: {}",
                    name
                ));
            } else {
                self.errors
                    .push(format!("função '{}' não encontrada para geração LLVM", name));
            }
            return Value {
                ty: Type::I64,
                operand: "0".to_string(),
            };
        };

        let mut args_ir = Vec::new();
        for (index, arg) in args.iter().enumerate() {
            let expected = sig.params.get(index).cloned().unwrap_or(Type::I64);
            let value = self.emit_expr(state, arg, Some(&expected));
            let value = self.coerce_value(state, value, &expected, arg.span);
            args_ir.push(format!("{} {}", llvm_type(&expected), value.operand));
        }

        if matches!(sig.ret, Type::Unit) {
            state.emit_instr(format!(
                "call void @{}({})",
                sanitize_ident(name),
                args_ir.join(", ")
            ));
            Value {
                ty: Type::Unit,
                operand: String::new(),
            }
        } else {
            let reg = state.next_reg("call");
            state.emit_instr(format!(
                "{} = call {} @{}({})",
                reg,
                llvm_type(&sig.ret),
                sanitize_ident(name),
                args_ir.join(", ")
            ));
            Value {
                ty: sig.ret,
                operand: reg,
            }
        }
    }

    fn emit_builtin_call(
        &mut self,
        state: &mut FunctionState,
        name: &str,
        args: &[Expr],
        span: Span,
    ) -> Option<Value> {
        match name {
            "term.print" | "term.println" | "term.output" => {
                if let Some(arg) = args.first() {
                    self.emit_print(state, arg);
                }
                Some(Value {
                    ty: Type::Unit,
                    operand: String::new(),
                })
            }
            "term.eprint" | "term.eprintln" | "term.error" => {
                self.externs
                    .insert("declare void @likn_term_eprint(i8*)".to_string());
                if let Some(arg) = args.first() {
                    let value = self.emit_expr(state, arg, Some(&Type::String));
                    let value = self.coerce_value(state, value, &Type::String, span);
                    state.emit_instr(format!("call void @likn_term_eprint(i8* {})", value.operand));
                }
                Some(Value {
                    ty: Type::Unit,
                    operand: String::new(),
                })
            }
            "term.input" => {
                self.externs
                    .insert("declare i8* @likn_term_input(i8*)".to_string());
                let prompt = args
                    .first()
                    .map(|arg| {
                        let value = self.emit_expr(state, arg, Some(&Type::String));
                        self.coerce_value(state, value, &Type::String, span)
                    })
                    .unwrap_or(Value {
                        ty: Type::String,
                        operand: "null".to_string(),
                    });

                let reg = state.next_reg("term.input");
                state.emit_instr(format!(
                    "{} = call i8* @likn_term_input(i8* {})",
                    reg, prompt.operand
                ));
                Some(Value {
                    ty: Type::String,
                    operand: reg,
                })
            }
            "fs.read" => {
                self.externs
                    .insert("declare i8* @likn_fs_read(i8*)".to_string());
                let path = args
                    .first()
                    .map(|arg| {
                        let value = self.emit_expr(state, arg, Some(&Type::String));
                        self.coerce_value(state, value, &Type::String, span)
                    })
                    .unwrap_or(Value {
                        ty: Type::String,
                        operand: "null".to_string(),
                    });

                let reg = state.next_reg("fs.read");
                state.emit_instr(format!("{} = call i8* @likn_fs_read(i8* {})", reg, path.operand));
                Some(Value {
                    ty: Type::String,
                    operand: reg,
                })
            }
            "fs.write" => {
                self.externs
                    .insert("declare void @likn_fs_write(i8*, i8*)".to_string());
                let path = args
                    .first()
                    .map(|arg| {
                        let value = self.emit_expr(state, arg, Some(&Type::String));
                        self.coerce_value(state, value, &Type::String, span)
                    })
                    .unwrap_or(Value {
                        ty: Type::String,
                        operand: "null".to_string(),
                    });
                let content = args
                    .get(1)
                    .map(|arg| {
                        let value = self.emit_expr(state, arg, Some(&Type::String));
                        self.coerce_value(state, value, &Type::String, span)
                    })
                    .unwrap_or(Value {
                        ty: Type::String,
                        operand: "null".to_string(),
                    });
                state.emit_instr(format!(
                    "call void @likn_fs_write(i8* {}, i8* {})",
                    path.operand, content.operand
                ));
                Some(Value {
                    ty: Type::Unit,
                    operand: String::new(),
                })
            }
            "fs.append" => {
                self.externs
                    .insert("declare void @likn_fs_append(i8*, i8*)".to_string());
                let path = args
                    .first()
                    .map(|arg| {
                        let value = self.emit_expr(state, arg, Some(&Type::String));
                        self.coerce_value(state, value, &Type::String, span)
                    })
                    .unwrap_or(Value {
                        ty: Type::String,
                        operand: "null".to_string(),
                    });
                let content = args
                    .get(1)
                    .map(|arg| {
                        let value = self.emit_expr(state, arg, Some(&Type::String));
                        self.coerce_value(state, value, &Type::String, span)
                    })
                    .unwrap_or(Value {
                        ty: Type::String,
                        operand: "null".to_string(),
                    });
                state.emit_instr(format!(
                    "call void @likn_fs_append(i8* {}, i8* {})",
                    path.operand, content.operand
                ));
                Some(Value {
                    ty: Type::Unit,
                    operand: String::new(),
                })
            }
            "fs.exists" => {
                self.externs
                    .insert("declare i1 @likn_fs_exists(i8*)".to_string());
                let path = args
                    .first()
                    .map(|arg| {
                        let value = self.emit_expr(state, arg, Some(&Type::String));
                        self.coerce_value(state, value, &Type::String, span)
                    })
                    .unwrap_or(Value {
                        ty: Type::String,
                        operand: "null".to_string(),
                    });
                let reg = state.next_reg("fs.exists");
                state.emit_instr(format!("{} = call i1 @likn_fs_exists(i8* {})", reg, path.operand));
                Some(Value {
                    ty: Type::Bool,
                    operand: reg,
                })
            }
            "panic" => {
                self.externs
                    .insert("declare void @likn_panic(i8*)".to_string());
                let msg = args
                    .first()
                    .map(|arg| {
                        let value = self.emit_expr(state, arg, Some(&Type::String));
                        self.coerce_value(state, value, &Type::String, span)
                    })
                    .unwrap_or(Value {
                        ty: Type::String,
                        operand: "null".to_string(),
                    });
                state.emit_instr(format!("call void @likn_panic(i8* {})", msg.operand));
                state.emit_instr("unreachable");
                state.terminated = true;
                Some(Value {
                    ty: Type::Never,
                    operand: "0".to_string(),
                })
            }
            "str.upper"
            | "str.lower"
            | "str.contains"
            | "str.len"
            | "str.concat"
            | "str.from_int"
            | "str.from_bool"
            | "sqlite.exec"
            | "sqlite.query"
            | "crypto.sha256"
            | "crypto.verify_sha256"
            | "crypto.random_token"
            | "html.escape"
            | "html.page"
            | "web.start"
            | "web.get"
            | "web.post"
            | "web.run"
            | "web.html"
            | "web.redirect" => {
                self.emit_generic_stdlib_call(state, name, args, span)
            }
            _ => None,
        }
    }

    fn emit_generic_stdlib_call(
        &mut self,
        state: &mut FunctionState,
        name: &str,
        args: &[Expr],
        span: Span,
    ) -> Option<Value> {
        let ret_ty = builtin_return_type(name)?;
        let params = builtin_param_types(name, args.len())?;

        let mut args_ir = Vec::new();
        for (index, arg) in args.iter().enumerate() {
            let param_ty = params.get(index).cloned().unwrap_or(Type::String);
            let value = self.emit_expr(state, arg, Some(&param_ty));
            let value = self.coerce_value(state, value, &param_ty, span);
            args_ir.push(format!("{} {}", llvm_type(&param_ty), value.operand));
        }

        let extern_name = format!("likn_{}", name.replace('.', "_"));
        let sig = if matches!(ret_ty, Type::Unit) {
            format!(
                "declare void @{}({})",
                sanitize_ident(&extern_name),
                params
                    .iter()
                    .map(llvm_type)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        } else {
            format!(
                "declare {} @{}({})",
                llvm_type(&ret_ty),
                sanitize_ident(&extern_name),
                params
                    .iter()
                    .map(llvm_type)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };
        self.externs.insert(sig);

        if matches!(ret_ty, Type::Unit) {
            state.emit_instr(format!(
                "call void @{}({})",
                sanitize_ident(&extern_name),
                args_ir.join(", ")
            ));
            Some(Value {
                ty: Type::Unit,
                operand: String::new(),
            })
        } else {
            let reg = state.next_reg("stdlib.call");
            state.emit_instr(format!(
                "{} = call {} @{}({})",
                reg,
                llvm_type(&ret_ty),
                sanitize_ident(&extern_name),
                args_ir.join(", ")
            ));
            Some(Value {
                ty: ret_ty,
                operand: reg,
            })
        }
    }

    fn infer_expr_type(&self, state: &FunctionState, expr: &Expr) -> Type {
        match &expr.kind {
            ExprKind::Int(_) => Type::I64,
            ExprKind::Float(_) => Type::F64,
            ExprKind::Bool(_) => Type::Bool,
            ExprKind::Char(_) => Type::Char,
            ExprKind::String(_) => Type::String,
            ExprKind::Var(name) => state
                .lookup_local(name)
                .map(|binding| binding.ty)
                .or_else(|| self.global_consts.get(name).map(|global| global.ty.clone()))
                .unwrap_or(Type::I64),
            ExprKind::UnaryOp(op, value) => {
                if op == "!" {
                    Type::Bool
                } else {
                    self.infer_expr_type(state, value)
                }
            }
            ExprKind::BinaryOp(lhs, op, rhs) => match op.as_str() {
                "==" | "!=" | ">" | "<" | ">=" | "<=" | "&&" | "||" => Type::Bool,
                _ => {
                    let left = self.infer_expr_type(state, lhs);
                    let right = self.infer_expr_type(state, rhs);
                    self.common_numeric_or_same_type(&left, &right)
                }
            },
            ExprKind::Call(name, _) => {
                if let Some(ret) = builtin_return_type(name) {
                    return ret;
                }
                self.type_info
                    .function_sigs
                    .get(name)
                    .map(|sig| sig.ret.clone())
                    .unwrap_or(Type::I64)
            }
        }
    }

    fn common_numeric_or_same_type(&self, left: &Type, right: &Type) -> Type {
        if left == right {
            return left.clone();
        }
        if matches!(left, Type::F64) || matches!(right, Type::F64) {
            return Type::F64;
        }
        if matches!(left, Type::F32) || matches!(right, Type::F32) {
            return Type::F32;
        }
        if is_integer_type(left) && is_integer_type(right) {
            if integer_bits(left) >= integer_bits(right) {
                return left.clone();
            }
            return right.clone();
        }
        left.clone()
    }

    fn coerce_value(
        &mut self,
        state: &mut FunctionState,
        value: Value,
        target: &Type,
        span: Span,
    ) -> Value {
        if &value.ty == target {
            return value;
        }

        if matches!(target, Type::String | Type::Str) && matches!(value.ty, Type::String | Type::Str) {
            return Value {
                ty: Type::String,
                operand: value.operand,
            };
        }

        if is_integer_type(&value.ty) && is_integer_type(target) {
            let src_bits = integer_bits(&value.ty);
            let dst_bits = integer_bits(target);
            if src_bits == dst_bits {
                return Value {
                    ty: target.clone(),
                    operand: value.operand,
                };
            }
            let reg = state.next_reg("int.cast");
            let cast = if src_bits > dst_bits {
                "trunc"
            } else if is_unsigned_integer_type(&value.ty) {
                "zext"
            } else {
                "sext"
            };
            state.emit_instr(format!(
                "{} = {} {} {} to {}",
                reg,
                cast,
                llvm_type(&value.ty),
                value.operand,
                llvm_type(target)
            ));
            return Value {
                ty: target.clone(),
                operand: reg,
            };
        }

        if matches!(value.ty, Type::Bool) && is_integer_type(target) {
            let reg = state.next_reg("bool.cast");
            state.emit_instr(format!(
                "{} = zext i1 {} to {}",
                reg,
                value.operand,
                llvm_type(target)
            ));
            return Value {
                ty: target.clone(),
                operand: reg,
            };
        }

        if is_integer_type(&value.ty) && matches!(target, Type::Bool) {
            let reg = state.next_reg("to.bool");
            state.emit_instr(format!(
                "{} = icmp ne {} {}, 0",
                reg,
                llvm_type(&value.ty),
                value.operand
            ));
            return Value {
                ty: Type::Bool,
                operand: reg,
            };
        }

        if matches!(value.ty, Type::F32 | Type::F64) && matches!(target, Type::F32 | Type::F64) {
            let reg = state.next_reg("float.cast");
            let cast = match (&value.ty, target) {
                (Type::F32, Type::F64) => "fpext",
                (Type::F64, Type::F32) => "fptrunc",
                _ => {
                    return Value {
                        ty: target.clone(),
                        operand: value.operand,
                    }
                }
            };
            state.emit_instr(format!(
                "{} = {} {} {} to {}",
                reg,
                cast,
                llvm_type(&value.ty),
                value.operand,
                llvm_type(target)
            ));
            return Value {
                ty: target.clone(),
                operand: reg,
            };
        }

        if is_integer_type(&value.ty) && matches!(target, Type::F32 | Type::F64) {
            let reg = state.next_reg("int.to.float");
            let cast = if is_unsigned_integer_type(&value.ty) {
                "uitofp"
            } else {
                "sitofp"
            };
            state.emit_instr(format!(
                "{} = {} {} {} to {}",
                reg,
                cast,
                llvm_type(&value.ty),
                value.operand,
                llvm_type(target)
            ));
            return Value {
                ty: target.clone(),
                operand: reg,
            };
        }

        if matches!(value.ty, Type::F32 | Type::F64) && is_integer_type(target) {
            let reg = state.next_reg("float.to.int");
            let cast = if is_unsigned_integer_type(target) {
                "fptoui"
            } else {
                "fptosi"
            };
            state.emit_instr(format!(
                "{} = {} {} {} to {}",
                reg,
                cast,
                llvm_type(&value.ty),
                value.operand,
                llvm_type(target)
            ));
            return Value {
                ty: target.clone(),
                operand: reg,
            };
        }

        self.fixmes.push(format!(
            "coerção não suportada de '{}' para '{}' na linha {}",
            type_display_name(&value.ty),
            type_display_name(target),
            span.line
        ));
        Value {
            ty: target.clone(),
            operand: default_value(target).to_string(),
        }
    }

    fn emit_global_const(&mut self, stmt: &Stmt) {
        let StmtKind::Const { name, ty, expr } = &stmt.kind else {
            return;
        };

        let resolved = match self.eval_const_expr(expr) {
            Some(value) => value,
            None => {
                self.errors.push(format!(
                    "const '{}' não pôde ser avaliada em tempo de compilação para LLVM",
                    name
                ));
                return;
            }
        };

        let target_ty = ty
            .as_ref()
            .and_then(type_from_ref)
            .unwrap_or_else(|| type_from_const_value(&resolved));

        let symbol = sanitize_ident(name);
        match (&target_ty, &resolved) {
            (Type::String | Type::Str, ConstValue::Str(value)) => {
                let global = self.get_or_create_string_global(value);
                self.globals.push(format!(
                    "@{} = constant i8* getelementptr inbounds ([{} x i8], [{} x i8]* @{}, i32 0, i32 0)",
                    symbol,
                    global.bytes.len(),
                    global.bytes.len(),
                    global.name
                ));
                self.global_consts.insert(
                    name.clone(),
                    GlobalConst {
                        ty: Type::String,
                        symbol,
                    },
                );
                self.const_values.insert(name.clone(), resolved);
            }
            _ => {
                let value = const_to_llvm_value(&resolved, &target_ty).unwrap_or_else(|| {
                    self.fixmes.push(format!(
                        "const '{}' não compatível com tipo '{}', usando valor padrão",
                        name,
                        type_display_name(&target_ty)
                    ));
                    default_value(&target_ty).to_string()
                });
                self.globals.push(format!(
                    "@{} = constant {} {}",
                    symbol,
                    llvm_type(&target_ty),
                    value
                ));
                self.global_consts.insert(
                    name.clone(),
                    GlobalConst {
                        ty: target_ty,
                        symbol,
                    },
                );
                self.const_values.insert(name.clone(), resolved);
            }
        }
    }

    fn eval_const_expr(&self, expr: &Expr) -> Option<ConstValue> {
        match &expr.kind {
            ExprKind::Int(v) => Some(ConstValue::Int(*v)),
            ExprKind::Float(v) => Some(ConstValue::Float(*v)),
            ExprKind::Bool(v) => Some(ConstValue::Bool(*v)),
            ExprKind::Char(v) => Some(ConstValue::Char(*v)),
            ExprKind::String(v) => Some(ConstValue::Str(v.clone())),
            ExprKind::Var(name) => self.const_values.get(name).cloned(),
            ExprKind::UnaryOp(op, value) => {
                let value = self.eval_const_expr(value)?;
                match (op.as_str(), value) {
                    ("-", ConstValue::Int(v)) => Some(ConstValue::Int(-v)),
                    ("-", ConstValue::Float(v)) => Some(ConstValue::Float(-v)),
                    ("!", ConstValue::Bool(v)) => Some(ConstValue::Bool(!v)),
                    _ => None,
                }
            }
            ExprKind::BinaryOp(lhs, op, rhs) => {
                let left = self.eval_const_expr(lhs)?;
                let right = self.eval_const_expr(rhs)?;
                eval_const_binary(left, op, right)
            }
            ExprKind::Call(_, _) => None,
        }
    }

    fn get_or_create_string_global(&mut self, value: &str) -> StringGlobal {
        let mut bytes = value.as_bytes().to_vec();
        bytes.push(0);
        if let Some(existing) = self.string_pool.get(&bytes) {
            return existing.clone();
        }

        let name = format!("str.{}", self.string_pool.len());
        let global = StringGlobal {
            name,
            bytes: bytes.clone(),
        };
        self.string_order.push(bytes.clone());
        self.string_pool.insert(bytes, global.clone());
        global
    }

    fn get_or_create_format_global(&mut self, fmt_escaped: &str) -> String {
        if let Some(existing) = self.format_pool.get(fmt_escaped) {
            return existing.clone();
        }

        let raw = decode_escaped_string(fmt_escaped);
        let name = format!("fmt.{}", self.format_pool.len());
        self.string_order.push(raw.clone());
        self.string_pool.insert(
            raw,
            StringGlobal {
                name: name.clone(),
                bytes: decode_escaped_string(fmt_escaped),
            },
        );
        self.format_pool
            .insert(fmt_escaped.to_string(), name.clone());
        name
    }
}

fn sanitize_ident(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '$' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        "anon".to_string()
    } else {
        out
    }
}

fn llvm_type(ty: &Type) -> &'static str {
    match ty {
        Type::I8 | Type::U8 => "i8",
        Type::I16 | Type::U16 => "i16",
        Type::I32 | Type::U32 => "i32",
        Type::I64 | Type::U64 | Type::Isize | Type::Usize => "i64",
        Type::I128 | Type::U128 => "i128",
        Type::F32 => "float",
        Type::F64 => "double",
        Type::Bool => "i1",
        Type::Char => "i32",
        Type::Str | Type::String => "i8*",
        Type::Unit | Type::Never => "void",
        Type::Infer(_) => "i64",
    }
}

fn integer_bits(ty: &Type) -> usize {
    match ty {
        Type::I8 | Type::U8 => 8,
        Type::I16 | Type::U16 => 16,
        Type::I32 | Type::U32 => 32,
        Type::I64 | Type::U64 | Type::Isize | Type::Usize => 64,
        Type::I128 | Type::U128 => 128,
        Type::Bool => 1,
        Type::Char => 32,
        _ => 64,
    }
}

fn is_integer_type(ty: &Type) -> bool {
    matches!(
        ty,
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
            | Type::Char
            | Type::Bool
    )
}

fn is_unsigned_integer_type(ty: &Type) -> bool {
    matches!(
        ty,
        Type::U8 | Type::U16 | Type::U32 | Type::U64 | Type::U128 | Type::Usize | Type::Bool
    )
}

fn type_display_name(ty: &Type) -> &'static str {
    match ty {
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

fn default_value(ty: &Type) -> &'static str {
    match ty {
        Type::Str | Type::String => "null",
        Type::F32 | Type::F64 => "0.0",
        Type::Bool => "0",
        Type::Unit | Type::Never => "",
        _ => "0",
    }
}

fn type_from_ref(type_ref: &TypeRef) -> Option<Type> {
    match &type_ref.kind {
        TypeRefKind::Unit => Some(Type::Unit),
        TypeRefKind::Never => Some(Type::Never),
        TypeRefKind::Named(name) => match name.as_str() {
            "i8" => Some(Type::I8),
            "i16" => Some(Type::I16),
            "i32" => Some(Type::I32),
            "i64" | "int" => Some(Type::I64),
            "i128" => Some(Type::I128),
            "u8" => Some(Type::U8),
            "u16" => Some(Type::U16),
            "u32" => Some(Type::U32),
            "u64" => Some(Type::U64),
            "u128" => Some(Type::U128),
            "isize" => Some(Type::Isize),
            "usize" => Some(Type::Usize),
            "f32" | "float" => Some(Type::F32),
            "f64" => Some(Type::F64),
            "bool" => Some(Type::Bool),
            "char" => Some(Type::Char),
            "str" => Some(Type::Str),
            "String" => Some(Type::String),
            _ => None,
        },
    }
}

fn type_from_const_value(value: &ConstValue) -> Type {
    match value {
        ConstValue::Int(_) => Type::I64,
        ConstValue::Float(_) => Type::F64,
        ConstValue::Bool(_) => Type::Bool,
        ConstValue::Char(_) => Type::Char,
        ConstValue::Str(_) => Type::String,
    }
}

fn const_to_llvm_value(value: &ConstValue, ty: &Type) -> Option<String> {
    match (value, ty) {
        (ConstValue::Int(v), ty) if is_integer_type(ty) => Some(v.to_string()),
        (ConstValue::Float(v), Type::F32 | Type::F64) => Some(format_float(*v)),
        (ConstValue::Bool(v), Type::Bool) => Some(if *v { "1" } else { "0" }.to_string()),
        (ConstValue::Char(ch), Type::Char) => Some((*ch as u32).to_string()),
        (ConstValue::Str(_), Type::String | Type::Str) => None,
        _ => None,
    }
}

fn eval_const_binary(left: ConstValue, op: &str, right: ConstValue) -> Option<ConstValue> {
    match (left, right) {
        (ConstValue::Int(a), ConstValue::Int(b)) => match op {
            "+" => Some(ConstValue::Int(a + b)),
            "-" => Some(ConstValue::Int(a - b)),
            "*" => Some(ConstValue::Int(a * b)),
            "/" => Some(ConstValue::Int(a / b)),
            "%" => Some(ConstValue::Int(a % b)),
            "==" => Some(ConstValue::Bool(a == b)),
            "!=" => Some(ConstValue::Bool(a != b)),
            ">" => Some(ConstValue::Bool(a > b)),
            "<" => Some(ConstValue::Bool(a < b)),
            ">=" => Some(ConstValue::Bool(a >= b)),
            "<=" => Some(ConstValue::Bool(a <= b)),
            _ => None,
        },
        (ConstValue::Float(a), ConstValue::Float(b)) => match op {
            "+" => Some(ConstValue::Float(a + b)),
            "-" => Some(ConstValue::Float(a - b)),
            "*" => Some(ConstValue::Float(a * b)),
            "/" => Some(ConstValue::Float(a / b)),
            "==" => Some(ConstValue::Bool((a - b).abs() < f64::EPSILON)),
            "!=" => Some(ConstValue::Bool((a - b).abs() >= f64::EPSILON)),
            ">" => Some(ConstValue::Bool(a > b)),
            "<" => Some(ConstValue::Bool(a < b)),
            ">=" => Some(ConstValue::Bool(a >= b)),
            "<=" => Some(ConstValue::Bool(a <= b)),
            _ => None,
        },
        (ConstValue::Bool(a), ConstValue::Bool(b)) => match op {
            "&&" => Some(ConstValue::Bool(a && b)),
            "||" => Some(ConstValue::Bool(a || b)),
            "==" => Some(ConstValue::Bool(a == b)),
            "!=" => Some(ConstValue::Bool(a != b)),
            _ => None,
        },
        (ConstValue::Char(a), ConstValue::Char(b)) => match op {
            "==" => Some(ConstValue::Bool(a == b)),
            "!=" => Some(ConstValue::Bool(a != b)),
            ">" => Some(ConstValue::Bool(a > b)),
            "<" => Some(ConstValue::Bool(a < b)),
            ">=" => Some(ConstValue::Bool(a >= b)),
            "<=" => Some(ConstValue::Bool(a <= b)),
            _ => None,
        },
        _ => None,
    }
}

fn format_float(value: f64) -> String {
    if value.is_nan() {
        "0x7FF8000000000000".to_string()
    } else if value.is_infinite() {
        if value.is_sign_positive() {
            "0x7FF0000000000000".to_string()
        } else {
            "0xFFF0000000000000".to_string()
        }
    } else {
        format!("{:.17}", value)
    }
}

fn encode_llvm_cstring(bytes: &[u8]) -> String {
    let mut out = String::new();
    for &byte in bytes {
        match byte {
            b'\\' => out.push_str("\\5C"),
            b'"' => out.push_str("\\22"),
            0x20..=0x7E => out.push(byte as char),
            _ => out.push_str(&format!("\\{:02X}", byte)),
        }
    }
    out
}

fn decode_escaped_string(value: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let mut chars = value.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if let Some(next) = chars.next() {
                match next {
                    '0' => out.push(0),
                    'n' => out.push(b'\n'),
                    't' => out.push(b'\t'),
                    '\\' => out.push(b'\\'),
                    '"' => out.push(b'"'),
                    _ => {
                        out.push(next as u8);
                    }
                }
            }
        } else {
            out.push(ch as u8);
        }
    }
    out
}

fn fmt_bytes_len(fmt: &str) -> usize {
    decode_escaped_string(fmt).len()
}

fn builtin_return_type(name: &str) -> Option<Type> {
    match name {
        "str.upper" | "str.lower" | "str.concat" | "str.from_int" | "str.from_bool" => {
            Some(Type::String)
        }
        "str.contains" | "crypto.verify_sha256" | "fs.exists" => Some(Type::Bool),
        "str.len" => Some(Type::Usize),
        "sqlite.exec" | "web.start" | "web.get" | "web.post" | "web.run" => Some(Type::Unit),
        "sqlite.query" | "crypto.sha256" | "crypto.random_token" | "html.escape" | "html.page"
        | "web.html" | "web.redirect" => Some(Type::String),
        _ => None,
    }
}

fn builtin_param_types(name: &str, arity: usize) -> Option<Vec<Type>> {
    let params = match name {
        "str.upper" | "str.lower" => vec![Type::String],
        "str.contains" => vec![Type::String, Type::String],
        "str.len" => vec![Type::String],
        "str.concat" => vec![Type::String, Type::String],
        "str.from_int" => vec![Type::I64],
        "str.from_bool" => vec![Type::Bool],
        "sqlite.exec" | "sqlite.query" => vec![Type::String, Type::String],
        "crypto.sha256" => vec![Type::String],
        "crypto.verify_sha256" => vec![Type::String, Type::String],
        "crypto.random_token" => vec![],
        "html.escape" => vec![Type::String],
        "html.page" => vec![Type::String, Type::String],
        "web.start" => vec![Type::I64],
        "web.get" | "web.post" => vec![Type::String, Type::String],
        "web.run" => vec![],
        "web.html" | "web.redirect" => vec![Type::String],
        _ => return None,
    };

    if params.len() == arity {
        Some(params)
    } else {
        None
    }
}

pub fn compile_program_to_llvm_ir(ast: &[Stmt], type_info: &TypeInfo, source: &str) -> String {
    LlvmGenerator::new(type_info, source).compile(ast)
}

#[cfg(test)]
mod tests {
    use crate::parser::parse_source;
    use crate::typecheck::check_program;

    use super::compile_program_to_llvm_ir;

    #[test]
    fn emits_basic_function_and_entrypoint() {
        let src = r#"
fn dobro(v: i64) -> i64 {
  return v * 2
}

let x = dobro(21)
print(x)
"#;
        let ast = parse_source(src).expect("parse");
        let types = check_program(&ast).expect("typecheck");
        let ir = compile_program_to_llvm_ir(&ast, &types, "demo.ikn");

        assert!(ir.contains("; === Likn Compiled Module ==="));
        assert!(ir.contains("define i64 @dobro(i64 %v)"));
        assert!(ir.contains("define i64 @likn_main()"));
        assert!(ir.contains("declare void @likn_print(i8*)"));
    }
}
