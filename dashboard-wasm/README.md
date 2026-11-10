# Painel Iced + WebAssembly

**http://localhost:3040/painel**

- Servidor HTTP e leitura dos logs: Rust nativo em `collector/src/bin/fortaleza-dashboard.rs`.
- Interface completa: Iced 0.14 compilado para WebAssembly, em `src/app.rs`.
- Mapa, pontos, escala térmica, seletores e textos são desenhados pelo Iced. A página contém apenas o documento mínimo para carregar o módulo; não há interface feita em HTML/CSS.
- `static/bootstrap.js` apenas inicializa os bindings gerados por wasm-bindgen. Não usa React ou Node.js.
- Os arquivos gerados ficam embutidos no executável do servidor.

O painel verifica os logs a cada cinco segundos, preserva o bairro selecionado e atualiza o mapa e os pontos das últimas 24 horas. Se perder a conexão, preserva os dados anteriores e tenta novamente. O coletor continua independente, no serviço `fortaleza-collector`.

## Compilar

Requer rustup, o alvo WASM e a versão **0.2.130** do CLI wasm-bindgen (já instalados neste PC):

```sh
~/.cargo/bin/rustup target add wasm32-unknown-unknown
./dashboard-wasm/build.sh
```

O CLI está em `~/.local/bin/wasm-bindgen`. O script compila WASM com um processo por vez para limitar o uso de memória e usa o compilador do sistema para o servidor quando disponível. O navegador precisa de WebGL2 para renderizar a interface Iced.

## Instalar ou atualizar

Na raiz do projeto:

```sh
install -m 755 collector/target/release/fortaleza-dashboard ~/.local/bin/fortaleza-dashboard.new
mv ~/.local/bin/fortaleza-dashboard.new ~/.local/bin/fortaleza-dashboard
install -m 644 collector/fortaleza-dashboard.service ~/.config/systemd/user/fortaleza-dashboard.service
systemctl --user daemon-reload
systemctl --user restart fortaleza-dashboard
```

O serviço inicia com o PC. Uso manual:

```sh
collector/target/release/fortaleza-dashboard --logs collector/logs --port 3040
```

O servidor escuta somente em `127.0.0.1` e lê os logs existentes.

## Verificar

Após o build:

```sh
RUSTC=/usr/bin/rustc /usr/bin/cargo test --manifest-path collector/Cargo.toml --features dashboard
RUSTC=/usr/bin/rustc /usr/bin/cargo test --manifest-path dashboard-wasm/Cargo.toml
```

As flags de compilador correspondem à instalação Arch Linux deste PC. Em outros sistemas, use `cargo test` com os mesmos argumentos de manifesto/features.
