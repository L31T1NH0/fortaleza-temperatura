# Fortaleza Temperatura

Mapa interativo dos 121 bairros de Fortaleza com temperaturas estimadas pelo [Open-Meteo](https://open-meteo.com/). Feito com Next.js, React e TypeScript.

## Executar

Requer Node.js 20.9 ou superior e npm.

```sh
npm ci
npm run dev
```

Abra http://localhost:3000. Para testar: `npm test`. Para gerar o site estático em `out/`: `npm run build`.

Inclui um [coletor horário em Rust](collector/README.md) e um [painel local em Iced/WebAssembly](dashboard-wasm/README.md) para acompanhar o histórico.

Os dados são estimativas de modelo, não medições de sensores em cada bairro. O uso gratuito do Open-Meteo é destinado a projetos não comerciais. Limites dos bairros: [IPLANFOR](https://mapas.fortaleza.ce.gov.br/mapa/21/bairros-de-fortaleza).
