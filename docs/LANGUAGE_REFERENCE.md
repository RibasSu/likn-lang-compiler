# Referência da Linguagem Likn

Este documento descreve a sintaxe suportada atualmente pelo compilador.

## 1. Estrutura do arquivo

Um arquivo `.ikn` é uma sequência de statements.

Statements suportados:
- `let`
- expressão livre
- `if` / `else`
- `fn`
- `return`
- `print`

Exemplo:

```ikn
let x = 10
print(x)
```

## 2. Literais e identificadores

## 2.1 Números

Apenas inteiros de 64 bits (`i64`).

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

## 4. Statements

## 4.1 Declaração de variável

```ikn
let x = 10
let y = x + 20
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

Todas as funções são geradas como `fn nome(...params: i64) -> i64`.

```ikn
fn dobro(v) {
  return v * 2
}
```

Observação: o compilador injeta `0` no fim da função se não houver retorno explícito no último caminho.

## 4.4 `return`

```ikn
fn ident(v) {
  return v
}
```

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

## 6. Delimitadores

Ponto e vírgula (`;`) é opcional para statements.

```ikn
let x = 1;
print(x);
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

- Tipagem fixa focada em `i64` para assinaturas de função.
- Sem sistema de módulos/imports.
- Sem estruturas/arrays/objetos.
- Sem inferência de tipos no nível da linguagem Likn.
