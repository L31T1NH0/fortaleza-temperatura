# Coletor de temperatura de Fortaleza

Programa Rust contínuo, com coordenadas dos 121 bairros incluídas no binário. Usa Open-Meteo sem chave, para uso não comercial.

- Coleta ao iniciar e, depois, nas horas cheias (horário de Fortaleza, UTC−3).
- Um arquivo `logs/fortaleza-AAAA-MM-DD.jsonl` por dia, de 00h a 23h59. O primeiro e último dia podem ser parciais.
- Cada linha JSON contém uma coleta completa dos 121 bairros. Campos: `hora` (janela da coleta), `coletado_em`, `fonte`, `leituras`; cada leitura inclui bairro, coordenadas, `temperatura_c` e `horario_dado` da API.
- A gravação é atômica, sincronizada no disco e sem duplicar uma hora após reinício. O bloqueio impede duas instâncias no mesmo diretório.
- Se a API falhar, tenta até três vezes por hora, com intervalo de cinco minutos. Uma coleta incompleta não é gravada. Falhas ficam no journal, sem temperaturas inventadas. Em condições normais são 2.904 localizações consultadas/dia; até 8.712 com três tentativas completas.
- Sem limite automático de um mês: continua até ser parado. Arquivos anteriores são preservados.

## Uso manual

```sh
cargo build --release
./target/release/fortaleza-collector --output ./logs
# Fazer só uma coleta (também respeita a deduplicação):
./target/release/fortaleza-collector --once --output ./logs
```

## Serviço no Linux

Os caminhos abaixo correspondem à instalação neste PC:

```sh
mkdir -p ~/.local/bin ~/.config/systemd/user
install -m 755 target/release/fortaleza-collector ~/.local/bin/fortaleza-collector
install -m 644 fortaleza-collector.service ~/.config/systemd/user/fortaleza-collector.service
systemctl --user daemon-reload
systemctl --user enable --now fortaleza-collector.service
# Permitir serviço do usuário mesmo sem login:
loginctl enable-linger "$USER"
```

```sh
systemctl --user status fortaleza-collector
journalctl --user -u fortaleza-collector -f
systemctl --user stop fortaleza-collector
systemctl --user start fortaleza-collector
# Parar e remover a inicialização automática:
systemctl --user disable --now fortaleza-collector
```

Logs: `~/Projetos/fortaleza-temperatura/collector/logs/`.

O PC precisa ficar ligado, conectado e sem suspensão. O serviço volta após reinício, mas não recompõe horas em que esteve desligado ou sem internet. O dado é uma estimativa de modelo a 2 m do solo, não uma medição independente por bairro. Uma consulta às 15h pode conter, por exemplo, o dado do modelo de 14h45; os dois horários ficam registrados.

Verificação: `cargo test` e `cargo clippy --all-targets -- -D warnings`.

## Painel de acompanhamento

Abra **http://localhost:3040/painel** neste PC. A tela contém o mapa colorido pela temperatura da última coleta e um gráfico de pontos com todos os bairros nas últimas 24 horas. Cada ponto representa uma leitura de um bairro. Valores idênticos podem se sobrepor; selecione um bairro no mapa ou no seletor para destacá-lo. O histórico atravessa os arquivos diários e não inventa registros ausentes.

O painel usa tema escuro e ocupa uma tela, com os elementos lado a lado na janela de desktop. Os tons da paleta são aproximações para tela, sem equivalência Pantone certificada.

O serviço `fortaleza-dashboard` inicia com o PC e atualiza os dados a cada cinco segundos, sem novas consultas meteorológicas:

```sh
systemctl --user status fortaleza-dashboard
journalctl --user -u fortaleza-dashboard -f
```

Para reconstruir após mudanças, na raiz do projeto:

```sh
./dashboard-wasm/build.sh
install -m 755 collector/target/release/fortaleza-dashboard ~/.local/bin/fortaleza-dashboard.new
mv ~/.local/bin/fortaleza-dashboard.new ~/.local/bin/fortaleza-dashboard
systemctl --user restart fortaleza-dashboard
```

Uso manual, a partir da raiz: `collector/target/release/fortaleza-dashboard --logs collector/logs`. O painel é servido por Rust e sua interface Iced roda em WebAssembly; não depende de Node.js. Veja [instruções do painel](../dashboard-wasm/README.md). O servidor escuta somente em `127.0.0.1:3040`; o site hospedado na nuvem não acessa os arquivos deste PC.
