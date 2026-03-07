# Referência da Linguagem Likn

Este documento descreve a sintaxe suportada atualmente pelo compilador.

## 1. Estrutura do arquivo

Um arquivo `.ikn` é uma sequência de statements.

Statements suportados:
- `let`
- `const`
- expressão livre
- `if` / `else`
- `fn` / `def`
- `return`
- `print`

Exemplo:

```ikn
let x = 10
print(x)
```

## 2. Literais e identificadores

## 2.1 Números

Inteiros e floats são suportados:
- inteiros: `i8/i16/i32/i64/i128`, `u8/u16/u32/u64/u128`, `isize`, `usize`
- floats: `f32`, `f64`

```ikn
let idade = 42
```

## 2.2 Strings

Strings com aspas duplas e escapes:
- `\n`
- `\t`
- `\"`
- `\\`

```ikn
print("linha 1\nlinha 2")
```

## 2.3 Booleanos

`true` e `false` são reconhecidos como booleanos.

```ikn
if true {
  print("ok")
}
```

## 2.4 Identificadores

Regra: `[A-Za-z_][A-Za-z0-9_]*`

```ikn
let usuario_01 = 1
```

## 3. Expressões

## 3.1 Operadores binários

Ordem de precedência (menor para maior):
1. `||`
2. `&&`
3. `==`, `!=`
4. `>`, `<`, `>=`, `<=`
5. `+`, `-`
6. `*`, `/`, `%`

Exemplo:

```ikn
print 1 + 2 * 3
```

Resultado semântico: `1 + (2 * 3)`.

## 3.2 Operadores unários

- `-expr`
- `!expr`
- `not expr` (alias)

```ikn
print -10
print !false
```

## 3.3 Chamada de função

```ikn
fn soma(a, b) {
  return a + b
}

print(soma(3, 4))
```

## 3.4 Chamada com namespace

A biblioteca padrão usa chamadas com `.`:

```ikn
let texto = fs.read("dados.txt")
let nome = term.input("Nome: ")
term.println(nome)
```

## 3.5 Operadores textuais (aliases Python)

- `and` -> `&&`
- `or` -> `||`
- `not` -> `!`

Exemplo:

```ikn
if ativo and not bloqueado {
  print("ok")
}
```

## 4. Statements

## 4.1 Declaração de variável

Imutável por padrão; mutabilidade é opt-in com `mut`:

```ikn
let x = 10
let mut y: i64 = x + 20
```

Shadowing é permitido:

```ikn
let x = 1
let x = x + 1
```

## 4.1.1 Constantes

```ikn
const limite: i64 = 100
```

## 4.2 `if / else`

```ikn
if x > 5 {
  print("maior")
} else {
  print("menor")
}
```

## 4.3 Definição de função

Anotações de tipo são opcionais. O compilador faz inferência quando possível.

```ikn
fn dobro(v: i64) -> i64 {
  return v * 2
}
```

Funções sem retorno útil usam `()`.

## 4.4 `return`

```ikn
fn ident(v) {
  return v
}
```

Também existe `return;` para retorno unit.

## 4.6 Tipos e inferência

Anotação explícita:

```ikn
let x: i64 = 10
fn soma(a: i64, b: i64) -> i64 {
  return a + b
}
```

Inferência:

```ikn
let x = 10
let y = x + 2
```

O compilador rejeita combinações inválidas:
- `1 + true`
- `if 10 { ... }`
- funções com caminhos de retorno inconsistentes

## 4.5 `print`

Aceita ambos formatos:

```ikn
print "oi"
print("oi")
```

## 5. Comentários

Comentários de linha iniciam com `//`:

```ikn
// isto é ignorado
let x = 1
```

Também é aceito comentário estilo Python com `#`:

```ikn
# comentário
let x = 1
```

## 6. Delimitadores

Ponto e vírgula (`;`) é opcional para statements.

```ikn
let x = 1;
print(x);
```

Blocos continuam com `{}` (estilo JavaScript). Para legibilidade estilo Python, `:` antes de `{` é opcional:

```ikn
def soma(a: i64, b: i64) -> i64: {
  return a + b
}
```

## 7. Erros e diagnóstico

Erros léxicos/sintáticos incluem posição aproximada:
- linha
- coluna

Exemplo de mensagem:

```text
erro em linha 3, coluna 12: string não terminada
```

## 8. Limitações atuais

- Sem sistema de módulos/imports.
- Sem estruturas/arrays/objetos.
- `str` é aceito na sintaxe, mas mapeado para `String` no backend atual.

## 9. Biblioteca padrão embutida

## 9.1 `fs.*` (arquivos)

- `fs.read(path)` -> `String`
- `fs.write(path, content)` -> `()`
- `fs.append(path, content)` -> `()`
- `fs.exists(path)` -> `bool`

Exemplo:

```ikn
fs.write("saida.txt", "linha 1")
fs.append("saida.txt", "\nlinha 2")
print(fs.read("saida.txt"))
print(fs.exists("saida.txt"))
```

## 9.2 `term.*` (terminal)

- `term.print(valor)` -> imprime em `stdout`
- `term.println(valor)` -> alias de `term.print`
- `term.output(valor)` -> alias de `term.print`
- `term.eprint(valor)` -> imprime em `stderr`
- `term.eprintln(valor)` -> alias de `term.eprint`
- `term.error(valor)` -> alias de `term.eprint`
- `term.input(prompt)` -> lê uma linha de `stdin` e retorna `String`
- `panic(msg)` -> `!` (never)

Exemplo:

```ikn
let nome = term.input("Nome: ")
term.println(nome)
```
