# Gráficos (ECharts via JS + Wasm)

Os gráficos usam **Apache ECharts 6.1.0** (Apache-2.0), versionado em
`frontend/vendor/echarts.min.js` (idêntico ao pacote npm `echarts@6.1.0`, integridade
`sha512-q0yaFPggC9FUdsWH4blavRWFmxdrIodbkoKNAjJudAI6CA9gNPxHtV2RcZNEepZVlk4yvBYkOkbk6HIVpIyHZA==`).
Nada é baixado em tempo de execução: o app é offline.

## Onde estão

| Tela | Gráfico | Dados (Rust) |
|---|---|---|
| Dashboard | Barras receitas × despesas, 12 meses | `shared::monthly_summaries` |
| Dashboard, Carteira | Rosca de alocação por tipo | `Portfolio::allocation` → `allocation::slices_by_type` |
| Carteira | Rosca de alocação por ativo | `Portfolio::positions` → `allocation::slices_by_ticker` |
| Dashboard, Gastos | Rosca de despesas por categoria | `shared::expenses_by_category` |

## Arquitetura

```
index.html ── <script data-trunk src="vendor/echarts.min.js">   (window.echarts)
js/charts.js ── renderPie / renderBars / dispose                 (cola JS, sem regra de negócio)
src/components/chart.rs ── #[wasm_bindgen(module = "/js/charts.js")]
                           PieChart, BarChart, DonutWithLegend   (componentes Leptos)
```

- O `wasm-bindgen` empacota `js/charts.js` como *snippet* e o Trunk o copia para `dist/`.
- `chart_view` cria o `<div>`, redesenha num `Effect` quando os dados mudam e chama
  `dispose` no `on_cleanup` (libera canvas e `ResizeObserver`).
- O tema claro/escuro segue `prefers-color-scheme`, inclusive mudando em tempo real.

## Regras

1. **Sem float nas contas.** Valores vão ao JS como string decimal (`Decimal::to_string()`)
   e viram `Number` só para desenhar. Totais, percentuais e rótulos ("R$ 1.234,56 · 75,00%")
   são calculados e formatados em Rust.
2. **A legenda é HTML** (`DonutWithLegend`), gerada pelo Rust: acessível, legível por leitor
   de tela e visível mesmo se o canvas falhar. O canvas tem `role="img"` com `aria-label`.
3. **Cores** vêm de `chart::PALETTE` (hex). Os tipos de ativo usam as mesmas cores dos pontos
   Tailwind das tabelas (`asset_type_color` ↔ `asset_type_hex`).

## Adicionar um gráfico

1. Uma função pura em `shared` que monte os dados (com teste).
2. Converter para `PieSlice`/`BarData` no componente, com `caption` já formatado.
3. Se for um tipo novo de gráfico, uma função em `js/charts.js` (via `draw(el, render)`)
   e o binding correspondente em `chart.rs`.
4. No E2E, `t.hasChart('<aria-label>')` confere que o canvas foi desenhado.

## Atualizar o ECharts

Baixe `dist/echarts.min.js` e `LICENSE` da versão desejada, confira contra o tarball do npm
(`npm view echarts@<v> dist.integrity`) e atualize a versão no comentário do `index.html` e
aqui. Para um bundle menor, é possível gerar um build customizado só com `PieChart`,
`BarChart`, `TooltipComponent`, `LegendComponent`, `GridComponent` e o renderer canvas.
