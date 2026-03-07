# Todo App (Likn)

Projeto exemplo de aplicação web monolítica em Likn Lang com:

- cadastro/login
- sessão
- tarefas por usuário
- SQLite
- renderização server-side

## Estrutura

- `src/main.ikn`: bootstrap da aplicação
- `src/web/*`: servidor e rotas
- `src/controllers/*`: entrada HTTP
- `src/services/*`: regras de negócio
- `src/repositories/*`: acesso ao banco
- `src/database/*`: bootstrap/migrations
- `src/auth/*`: autenticação/sessão
- `src/views/*`: páginas HTML

## Como compilar neste repositório

No root do compilador:

```bash
./target/debug/likn-lang-compiler examples/todo_web_sqlite/src/main.ikn -o /tmp/todo_app
```

O compilador resolve dependências via `libs/lib-likn-lang` automaticamente neste workspace.
