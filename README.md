# Fortaleza · Temperatura

Next.js, React e TypeScript. Mapa SVG dos 121 bairros oficiais, com temperatura estimada pelo Open-Meteo. Interação por mouse, toque e teclado.

```sh
npm install
npm run dev
```

Abra http://localhost:3000. Para verificar: `npm test` e `npm run build`.

Os limites oficiais estão incluídos no projeto. `node scripts/prepare-map.mjs` regenera os caminhos SVG e escolhe um ponto interno por bairro.

A página consulta a API em quatro lotes, mantém cache local por uma hora e atualiza enquanto estiver aberta. Sem banco ou coleta em segundo plano. As cotas são contadas por localização e multiplicam pelo número de navegadores. Para muitos usuários, migrar a coleta para servidor com cache compartilhado.

Dados estimados, não sensores em cada bairro; bairros próximos podem compartilhar a mesma célula do modelo. Uso gratuito do Open-Meteo restrito a projetos não comerciais. Para uso comercial, contratar o serviço e proteger a chave num servidor.

Fontes: https://open-meteo.com/ e https://mapas.fortaleza.ce.gov.br/mapa/21/bairros-de-fortaleza (IPLANFOR).
