# Likn Language Compiler

Compilador da linguagem **Likn** implementado em Rust.

Ele lê arquivos `.ikn`, transforma em código Rust intermediário e gera:
- binário nativo (`target=native`)
- módulo WebAssembly (`target=web`)

## Estado do projeto

- Lexer/parser reescritos com precedência de operadores e erros com linha/coluna.
- Geração de código Rust separando funções globais e ponto de entrada.
- Perfis de build (`dev` e `fast`) com foco em performance.
- Biblioteca padrão embutida com `fs.*` e `term.*`.
- Sistema de tipos estático com inferência, anotação explícita, `const`, `mut`, `()`, `!` e checagem de coerência de retorno.
- Diagnósticos de erro com trecho de código e marcador de coluna (estilo rustc).
- Testes unitários e testes de integração de CLI.

## Instalação

Pré-requisitos:
- Rust toolchain (`cargo`, `rustc`)

Opcional para WebAssembly:
- `rustup target add wasm32-unknown-unknown`

## Uso rápido

Compilar para binário nativo:

```bash
cargo run -- exemplo.ikn
```

Compilar para WebAssembly otimizado:

```bash
cargo run -- --web --fast exemplo.ikn -o exemplo.wasm
```

Executar testes:

```bash
cargo test
```

## Opções de CLI

```text
Likn Lang Compiler

Uso:
  likn-lang-compiler [FLAGS] <arquivo.ikn>

Flags:
  --help, -h            Exibe esta ajuda
  --target <native|web> Define o alvo de build
  --web                 Atalho para --target web
  --profile <dev|fast>  Define o perfil de otimização
  --fast                Atalho para --profile fast
  --output, -o <path>   Define o arquivo de saída
  --no-emit-rust        Não mantém o .rs gerado
```

## Documentação

- [Visão geral da linguagem](docs/LANGUAGE_REFERENCE.md)
- [Referência da CLI](docs/CLI_REFERENCE.md)
- [Guia de performance para web](docs/WEB_GUIDE.md)
- [Arquitetura do compilador](docs/COMPILER_ARCHITECTURE.md)

## Exemplo de Likn

```ikn
let x = 10

fn soma(a, b) {
  return a + b
}

if x > 5 {
  print("Maior que 5")
} else {
  print("Menor ou igual a 5")
}

print(soma(3, 4))
```

Exemplo com stdlib:

```ikn
fs.write("log.txt", "hello")
let conteudo = fs.read("log.txt")
print(conteudo)
let nome = term.input("Nome: ")
term.println(nome)
```

## Roadmap sugerido

- Módulos/arquivos múltiplos
- Macros/funções nativas para integração web
- Gerador de documentação automatizado (site)
