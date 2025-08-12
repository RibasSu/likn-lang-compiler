# Likn Lang Compiler

Um compilador simples para a linguagem experimental **Likn**, escrito em Rust. Ele converte arquivos `.ikn` em código Rust, compila e gera um binário executável.

## Como funciona

- O compilador lê um arquivo `.ikn` contendo código na linguagem Likn.
- Faz o parsing do código, gera um arquivo Rust equivalente (`.rs`).
- Compila o arquivo Rust usando o `rustc`, gerando um binário executável com o mesmo nome do arquivo de entrada (sem extensão).
- O binário pode ser executado normalmente no terminal.

## Exemplo de uso

Dado o arquivo `exemplo.ikn`:

```ikn
let x = 10
if x > 5 {
  print("Maior que 5")
} else {
  print("Menor ou igual a 5")
}

fn soma(a, b) {
  return a + b
}
print(soma(3, 4))
```

Compile e execute:

```sh
cargo run -- exemplo.ikn
# Saída esperada:
# Binário gerado: exemplo
# Executando binario (se tiver):
# Maior que 5
# 7
```

## Sintaxe suportada

- Declaração de variáveis: `let nome = valor`
- Estruturas condicionais: `if ... { ... } else { ... }`
- Funções: `fn nome(param1, param2) { ... }`
- Retorno de função: `return valor`
- Impressão: `print valor` ou `print(valor)`
- Expressões aritméticas e booleanas simples

## Estrutura do projeto

- `src/main.rs`: código-fonte do compilador.
- `exemplo.ikn`: exemplo de código na linguagem Likn.
- `run.sh`: script para rodar o compilador e o binário gerado.
- `Cargo.toml`: configuração do projeto Rust.

## Requisitos

- Rust (toolchain e `rustc` instalados)

## Como rodar

1. Clone o repositório.
2. Escreva seu código `.ikn`.
3. Execute:

   ```sh
   cargo run -- seu_arquivo.ikn
   ```

4. O binário será gerado com o mesmo nome do arquivo (sem extensão).
5. Execute o binário:

   ```sh
   ./seu_arquivo
   ```

## Observações

- Todos os parâmetros de função são tratados como `i64` (inteiro).
- Strings são suportadas apenas para impressão.
- O parser é simples e pode ser expandido para mais recursos.
