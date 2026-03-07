# Arquitetura do Compilador

## Pipeline

O compilador executa estas fases:

1. Leitura do arquivo `.ikn`
2. Lexing (`Lexer`)
3. Parsing (`Parser`) para AST
4. Geração de Rust (`compile_program`)
5. Build via `rustc`

## 1. Lexer

Responsável por converter texto em tokens com posição (`line`, `column`).

Tokens suportados:
- `Number`
- `String`
- `Ident`
- `Symbol`
- `Eof`

Recursos principais:
- suporte a comentários `//`
- suporte a escapes em string
- operadores de 1 e 2 caracteres (`+`, `==`, `>=`, `&&`, ...)

## 2. Parser

Parser descendente recursivo com precedência (Pratt-style simplificado).

Níveis importantes:
- statements (`let`, `fn`, `if`, `return`, `print`, expressão)
- blocos delimitados por `{}`
- expressões com precedência (`||`, `&&`, comparações, soma, multiplicação)
- operadores unários (`-`, `!`)

Erros de parsing retornam `CompileError` com localização.

## 3. AST

### Expressões (`Expr`)
- `Number(i64)`
- `Bool(bool)`
- `String(String)`
- `Var(String)`
- `UnaryOp(op, expr)`
- `BinaryOp(lhs, op, rhs)`
- `Call(name, args)`

### Statements (`Stmt`)
- `Let(name, expr)`
- `Expr(expr)`
- `If(cond, then_block, else_block)`
- `Func(name, params, body)`
- `Return(expr)`
- `Print(expr)`

## 4. Codegen

O codegen é string-based e gera Rust legível.

Estratégia:
- funções Likn viram funções Rust globais
- bloco principal vira função de entrada (`__likn_entry` ou `likn_main`)
- `print` mapeia para `println!` (native) ou `likn_print` (web)

## 5. Alvos de build

## 5.1 Native

- saída padrão: executável local
- entrypoint final: `fn main()`

## 5.2 Web

- target: `wasm32-unknown-unknown`
- crate-type: `cdylib`
- entrypoint exportado: `pub extern "C" fn likn_main() -> i64`

## 6. Testes

- unit tests em `src/main.rs`
- integração de CLI em `tests/cli.rs`

Cobertura atual inclui:
- precedência de operadores
- parsing de funções/chamadas
- build native e web
- diagnóstico de erro de parsing

## 7. Pontos de extensão

- sistema de tipos real na AST
- tabela de símbolos + validação semântica
- backend intermediário (IR) antes de Rust
- múltiplos arquivos e imports
