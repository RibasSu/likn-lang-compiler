# Guia Web e Performance

Este guia cobre como gerar WASM e quais decisões atuais impactam performance em aplicações web.

## 1. Build web recomendado

```bash
cargo run -- --web --fast app.ikn -o app.wasm
```

Perfil `fast` aplica otimizações agressivas para reduzir tamanho e melhorar custo de execução.

## 2. Integração de saída `print`

No alvo web, `print` usa um hook de log importado:

```rust
extern "C" {
    fn likn_console_log(ptr: *const u8, len: usize);
}
```

Você precisa fornecer essa função no host JavaScript.

Exemplo mínimo:

```js
import wasmBytes from "./app.wasm?arraybuffer";

const { instance } = await WebAssembly.instantiate(wasmBytes, {
  env: {
    likn_console_log(ptr, len) {
      const view = new Uint8Array(instance.exports.memory.buffer, ptr, len);
      console.log(new TextDecoder().decode(view));
    }
  }
});

instance.exports.likn_main();
```

## 3. Estratégia de otimização atual

## 3.1 Runtime

Likn transpila para Rust e delega ao `rustc` backend LLVM.

No perfil `fast`:
- reduz overhead com `panic=abort`
- melhora inlining e qualidade de otimização com `lto=fat`
- reduz código morto/símbolos em WASM (`strip=symbols`)

## 3.2 Build time vs runtime

- `dev`: compila mais rápido para ciclo de desenvolvimento.
- `fast`: compila mais lento, mas gera artefato mais eficiente.

Recomendação:
- usar `dev` no dia a dia
- usar `fast` em benchmark, CI de release e publicação

## 4. Checklist de performance web

- Compile com `--web --fast`
- Minimize chamadas frequentes de `print` em hot paths
- Prefira lógica numérica simples e funções puras em trechos críticos
- Faça benchmark de cenários reais antes de micro-otimizar

## 5. Limitações atuais para web

- ABI de funções ainda simples (inteiros/fluxo básico)
- Sem binding automático para strings entre JS e WASM
- Sem gerador de glue code (como `wasm-bindgen`)

## 6. Próximos passos sugeridos

- camada oficial de interop JS/WASM
- serialização para troca estruturada de dados
- modo AOT dedicado para SSR/backend web
