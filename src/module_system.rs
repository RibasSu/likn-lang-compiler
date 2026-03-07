use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::ast::{Expr, ExprKind, Span, Stmt, StmtKind};
use crate::error::CompileError;
use crate::parser::parse_source;

const ENTRY_MODULE_ID: &str = "__entry__";

#[derive(Debug, Clone)]
pub struct ResolvedProgram {
    pub ast: Vec<Stmt>,
    pub entry_source: String,
    pub has_imports: bool,
}

#[derive(Debug, Clone)]
struct ImportDecl {
    path: String,
    span: Span,
}

#[derive(Debug, Clone)]
struct ModuleUnit {
    id: String,
    file_path: PathBuf,
    source: String,
    ast: Vec<Stmt>,
    imports: Vec<ImportDecl>,
    exported_funcs: HashSet<String>,
    canonical_funcs: HashMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VisitState {
    Visiting,
    Visited,
}

#[derive(Debug, Clone)]
struct NamespaceMap {
    modules: HashMap<String, String>,
}

struct ModuleLoader {
    root_dir: PathBuf,
    modules: HashMap<String, ModuleUnit>,
    states: HashMap<String, VisitState>,
    load_order: Vec<String>,
}

pub fn resolve_program(entry_file: &Path) -> Result<ResolvedProgram, CompileError> {
    let root_dir = entry_file
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let mut loader = ModuleLoader::new(root_dir);
    loader.load_entry(entry_file)?;
    loader.build_program()
}

impl ModuleLoader {
    fn new(root_dir: PathBuf) -> Self {
        Self {
            root_dir,
            modules: HashMap::new(),
            states: HashMap::new(),
            load_order: Vec::new(),
        }
    }

    fn load_entry(&mut self, entry_file: &Path) -> Result<(), CompileError> {
        self.load_module(ENTRY_MODULE_ID.to_string(), entry_file.to_path_buf())
    }

    fn load_module(&mut self, module_id: String, file_path: PathBuf) -> Result<(), CompileError> {
        match self.states.get(&module_id) {
            Some(VisitState::Visited) => return Ok(()),
            Some(VisitState::Visiting) => {
                return Err(CompileError::new(
                    format!("ciclo de import detectado envolvendo '{module_id}'"),
                    1,
                    1,
                ));
            }
            None => {}
        }

        let source = fs::read_to_string(&file_path).map_err(|err| {
            CompileError::new(
                format!("falha ao ler módulo {}: {err}", file_path.display()),
                1,
                1,
            )
        })?;
        let file_display = file_path.display().to_string();
        let ast = parse_source(&source)
            .map_err(|err| err.with_source_context(file_display.clone(), source.clone()))?;

        let mut imports = Vec::new();
        let mut local_funcs = HashSet::new();
        let mut exported_funcs = HashSet::new();

        for stmt in &ast {
            match &stmt.kind {
                StmtKind::Import(path) => imports.push(ImportDecl {
                    path: path.clone(),
                    span: stmt.span,
                }),
                StmtKind::Func { name, .. } => {
                    if !local_funcs.insert(name.clone()) {
                        return Err(self
                            .error_with_context(
                                &file_display,
                                &source,
                                stmt.span,
                                format!("função '{name}' já foi declarada neste módulo"),
                            )
                            .with_help("renomeie uma das funções"));
                    }
                }
                StmtKind::Export(inner) => {
                    if let StmtKind::Func { name, .. } = &inner.kind {
                        if !local_funcs.insert(name.clone()) {
                            return Err(self
                                .error_with_context(
                                    &file_display,
                                    &source,
                                    inner.span,
                                    format!("função '{name}' já foi declarada neste módulo"),
                                )
                                .with_help("renomeie uma das funções"));
                        }
                        exported_funcs.insert(name.clone());
                    }
                }
                _ => {}
            }
        }

        let canonical_funcs = local_funcs
            .iter()
            .map(|name| (name.clone(), canonical_function_name(&module_id, name)))
            .collect::<HashMap<_, _>>();

        let module = ModuleUnit {
            id: module_id.clone(),
            file_path: file_path.clone(),
            source: source.clone(),
            ast,
            imports: imports.clone(),
            exported_funcs,
            canonical_funcs,
        };

        self.states.insert(module_id.clone(), VisitState::Visiting);
        self.modules.insert(module_id.clone(), module);

        for import in imports {
            let dependency_path = self.path_for_module(&import.path);
            if !dependency_path.is_file() {
                let current = self
                    .modules
                    .get(&module_id)
                    .expect("módulo carregado antes da resolução de imports");
                return Err(self
                    .module_error(
                        current,
                        import.span,
                        format!("módulo '{}' não encontrado", import.path),
                    )
                    .with_help(format!(
                        "esperado em {}",
                        dependency_path.as_path().display()
                    )));
            }

            if matches!(self.states.get(&import.path), Some(VisitState::Visiting)) {
                let current = self
                    .modules
                    .get(&module_id)
                    .expect("módulo carregado antes da resolução de imports");
                return Err(self
                    .module_error(
                        current,
                        import.span,
                        format!(
                            "ciclo de import detectado entre '{}' e '{}'",
                            module_id, import.path
                        ),
                    )
                    .with_help("remova a dependência circular entre os módulos"));
            }

            self.load_module(import.path, dependency_path)?;
        }

        self.states.insert(module_id.clone(), VisitState::Visited);
        self.load_order.push(module_id);
        Ok(())
    }

    fn build_program(&self) -> Result<ResolvedProgram, CompileError> {
        let mut final_ast = Vec::new();

        for module_id in &self.load_order {
            let module = self
                .modules
                .get(module_id)
                .expect("módulo deve existir no mapa");
            let namespaces = self.build_namespaces(module)?;
            let is_entry = module.id == ENTRY_MODULE_ID;

            for stmt in &module.ast {
                let Some(rewritten) = self.rewrite_stmt(module, &namespaces, stmt)? else {
                    continue;
                };

                let is_function = matches!(rewritten.kind, StmtKind::Func { .. });
                if !is_entry && !is_function {
                    return Err(self
                        .module_error(
                            module,
                            stmt.span,
                            "módulos importados só podem conter declarações de função",
                        )
                        .with_help("mova código executável para o módulo de entrada"));
                }
                final_ast.push(rewritten);
            }
        }

        let entry = self
            .modules
            .get(ENTRY_MODULE_ID)
            .expect("módulo de entrada deve existir");
        Ok(ResolvedProgram {
            ast: final_ast,
            entry_source: entry.source.clone(),
            has_imports: !entry.imports.is_empty(),
        })
    }

    fn build_namespaces(&self, module: &ModuleUnit) -> Result<NamespaceMap, CompileError> {
        let mut namespaces = HashMap::new();

        for import in &module.imports {
            let target = import.path.clone();

            namespaces.insert(import.path.clone(), target.clone());

            let alias = import
                .path
                .rsplit('.')
                .next()
                .expect("import path não vazio")
                .to_string();

            if let Some(previous) = namespaces.get(&alias) {
                if previous != &target {
                    return Err(self
                        .module_error(
                            module,
                            import.span,
                            format!("namespace '{alias}' é ambíguo em imports"),
                        )
                        .with_help(format!(
                            "os módulos '{}' e '{}' colidem no mesmo namespace local",
                            previous, target
                        )));
                }
            } else {
                namespaces.insert(alias, target);
            }
        }

        Ok(NamespaceMap {
            modules: namespaces,
        })
    }

    fn rewrite_stmt(
        &self,
        module: &ModuleUnit,
        namespaces: &NamespaceMap,
        stmt: &Stmt,
    ) -> Result<Option<Stmt>, CompileError> {
        match &stmt.kind {
            StmtKind::Import(_) => Ok(None),
            StmtKind::Export(inner) => self.rewrite_stmt(module, namespaces, inner),
            StmtKind::Let {
                name,
                mutable,
                ty,
                expr,
            } => Ok(Some(Stmt {
                span: stmt.span,
                kind: StmtKind::Let {
                    name: name.clone(),
                    mutable: *mutable,
                    ty: ty.clone(),
                    expr: self.rewrite_expr(module, namespaces, expr)?,
                },
            })),
            StmtKind::Const { name, ty, expr } => Ok(Some(Stmt {
                span: stmt.span,
                kind: StmtKind::Const {
                    name: name.clone(),
                    ty: ty.clone(),
                    expr: self.rewrite_expr(module, namespaces, expr)?,
                },
            })),
            StmtKind::Expr(expr) => Ok(Some(Stmt {
                span: stmt.span,
                kind: StmtKind::Expr(self.rewrite_expr(module, namespaces, expr)?),
            })),
            StmtKind::Print(expr) => Ok(Some(Stmt {
                span: stmt.span,
                kind: StmtKind::Print(self.rewrite_expr(module, namespaces, expr)?),
            })),
            StmtKind::Return(value) => Ok(Some(Stmt {
                span: stmt.span,
                kind: StmtKind::Return(match value {
                    Some(expr) => Some(self.rewrite_expr(module, namespaces, expr)?),
                    None => None,
                }),
            })),
            StmtKind::If {
                cond,
                then_block,
                else_block,
            } => Ok(Some(Stmt {
                span: stmt.span,
                kind: StmtKind::If {
                    cond: self.rewrite_expr(module, namespaces, cond)?,
                    then_block: self.rewrite_block(module, namespaces, then_block)?,
                    else_block: self.rewrite_block(module, namespaces, else_block)?,
                },
            })),
            StmtKind::Func {
                name,
                params,
                return_type,
                body,
            } => {
                let renamed = module.canonical_funcs.get(name).cloned().ok_or_else(|| {
                    self.module_error(
                        module,
                        stmt.span,
                        format!("falha interna ao resolver função '{name}'"),
                    )
                })?;
                Ok(Some(Stmt {
                    span: stmt.span,
                    kind: StmtKind::Func {
                        name: renamed,
                        params: params.clone(),
                        return_type: return_type.clone(),
                        body: self.rewrite_block(module, namespaces, body)?,
                    },
                }))
            }
        }
    }

    fn rewrite_block(
        &self,
        module: &ModuleUnit,
        namespaces: &NamespaceMap,
        block: &[Stmt],
    ) -> Result<Vec<Stmt>, CompileError> {
        let mut out = Vec::new();
        for stmt in block {
            if let Some(rewritten) = self.rewrite_stmt(module, namespaces, stmt)? {
                out.push(rewritten);
            }
        }
        Ok(out)
    }

    fn rewrite_expr(
        &self,
        module: &ModuleUnit,
        namespaces: &NamespaceMap,
        expr: &Expr,
    ) -> Result<Expr, CompileError> {
        match &expr.kind {
            ExprKind::Int(_)
            | ExprKind::Float(_)
            | ExprKind::Bool(_)
            | ExprKind::Char(_)
            | ExprKind::String(_)
            | ExprKind::Var(_) => Ok(expr.clone()),
            ExprKind::UnaryOp(op, value) => Ok(Expr {
                span: expr.span,
                kind: ExprKind::UnaryOp(
                    op.clone(),
                    Box::new(self.rewrite_expr(module, namespaces, value)?),
                ),
            }),
            ExprKind::BinaryOp(lhs, op, rhs) => Ok(Expr {
                span: expr.span,
                kind: ExprKind::BinaryOp(
                    Box::new(self.rewrite_expr(module, namespaces, lhs)?),
                    op.clone(),
                    Box::new(self.rewrite_expr(module, namespaces, rhs)?),
                ),
            }),
            ExprKind::Call(name, args) => {
                let rewritten_args = args
                    .iter()
                    .map(|arg| self.rewrite_expr(module, namespaces, arg))
                    .collect::<Result<Vec<_>, _>>()?;
                let rewritten_name = self.rewrite_call_name(module, namespaces, expr.span, name)?;

                Ok(Expr {
                    span: expr.span,
                    kind: ExprKind::Call(rewritten_name, rewritten_args),
                })
            }
        }
    }

    fn rewrite_call_name(
        &self,
        module: &ModuleUnit,
        namespaces: &NamespaceMap,
        span: Span,
        name: &str,
    ) -> Result<String, CompileError> {
        if name == "panic"
            || name.starts_with("term.")
            || name.starts_with("fs.")
            || name.starts_with("str.")
        {
            return Ok(name.to_string());
        }

        if let Some(local) = module.canonical_funcs.get(name) {
            return Ok(local.clone());
        }

        let Some((namespace, function_name)) = name.rsplit_once('.') else {
            return Ok(name.to_string());
        };

        let target_module_id = namespaces.modules.get(namespace).ok_or_else(|| {
            self.module_error(module, span, format!("módulo '{namespace}' não foi importado"))
                .with_help(format!("adicione `import {namespace}` no topo do arquivo"))
        })?;
        let target_module = self.modules.get(target_module_id).ok_or_else(|| {
            self.module_error(
                module,
                span,
                format!("módulo '{target_module_id}' não foi carregado"),
            )
        })?;

        if !target_module.exported_funcs.contains(function_name) {
            return Err(self
                .module_error(
                    module,
                    span,
                    format!(
                        "função '{function_name}' não é exportada por '{}'",
                        target_module_id
                    ),
                )
                .with_help(format!(
                    "marque com `export fn {function_name}(...)` em '{}'",
                    target_module_id
                )));
        }

        target_module
            .canonical_funcs
            .get(function_name)
            .cloned()
            .ok_or_else(|| {
                self.module_error(
                    module,
                    span,
                    format!("falha interna ao resolver '{}.{}'", namespace, function_name),
                )
            })
    }

    fn module_error(&self, module: &ModuleUnit, span: Span, message: impl Into<String>) -> CompileError {
        self.error_with_context(
            &module.file_path.display().to_string(),
            &module.source,
            span,
            message,
        )
    }

    fn error_with_context(
        &self,
        file: &str,
        source: &str,
        span: Span,
        message: impl Into<String>,
    ) -> CompileError {
        CompileError::new(message, span.line, span.column)
            .with_span(span.len)
            .with_source_context(file.to_string(), source.to_string())
    }

    fn path_for_module(&self, module_path: &str) -> PathBuf {
        let relative = module_path.replace('.', "/");
        self.root_dir.join(format!("{relative}.ikn"))
    }
}

fn canonical_function_name(module_id: &str, name: &str) -> String {
    let module_ns = if module_id == ENTRY_MODULE_ID {
        "entry".to_string()
    } else {
        module_id.replace('.', "__")
    };
    format!("__likn_{module_ns}__{name}")
}
