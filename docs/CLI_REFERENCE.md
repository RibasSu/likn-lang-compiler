# Referência da CLI

## Comando base

```bash
likn-lang-compiler [FLAGS] <arquivo.ikn>
```

Em desenvolvimento local:

```bash
cargo run -- [FLAGS] <arquivo.ikn>
```

## Flags

## `--help`, `-h`

Mostra ajuda resumida.

## `--target <native|web>`

Define o tipo de artefato gerado.

- `native`: executável local (padrão)
- `web`: módulo `.wasm` (`wasm32-unknown-unknown`)

Exemplo:

```bash
cargo run -- --target web app.ikn
```

## `--web`

Atalho para `--target web`.

## `--profile <dev|fast>`

Define flags de otimização do `rustc`.

- `dev`: build mais simples para iteração.
- `fast`: build agressivo para performance.

### Flags usadas em `fast` (native)

- `-C opt-level=3`
- `-C lto=fat`
- `-C codegen-units=1`
- `-C panic=abort`
- `-C target-cpu=native`

### Flags usadas em `fast` (web)

- `-C opt-level=z`
- `-C lto=fat`
- `-C codegen-units=1`
- `-C panic=abort`
- `-C strip=symbols`

## `--fast`

Atalho para `--profile fast`.

## `--output <path>`, `-o <path>`

Define caminho final do artefato.

Exemplo:

```bash
cargo run -- app.ikn --output ./build/app-bin
cargo run -- --web app.ikn --output ./build/app.wasm
```

## `--no-emit-rust`

Não mantém o arquivo Rust intermediário (`.rs`) após compilação bem-sucedida.

## Arquivos gerados

Padrão (`native`):
- Rust intermediário: `<stem>.rs`
- Artefato final: `<stem>`

Padrão (`web`):
- Rust intermediário: `<stem>.rs`
- Artefato final: `<stem>.wasm`

`<stem>` é o nome do arquivo `.ikn` sem extensão.

## Códigos de saída

- `0`: compilação concluída
- `1`: erro de CLI, parsing, I/O ou `rustc`

## Diagnóstico comum

Se build web falhar com erro de target ausente, instale:

```bash
rustup target add wasm32-unknown-unknown
```
