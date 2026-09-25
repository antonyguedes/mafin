# Gráficos (pizza/disco) via JS + Wasm — guia de integração

Estado atual: a alocação da carteira é desenhada por `frontend/src/components/allocation.rs`
(`AllocationBar`: barra empilhada em HTML/Tailwind, sem dependências). Esse componente é o
**ponto de troca**: ele já recebe o dado do gráfico (`Vec<AllocationSlice>`), então trocar a
barra por um gráfico de pizza não muda nenhuma tela.

## Biblioteca recomendada

| Opção | Licença | Tamanho (min) | Por quê |
|---|---|---|---|
| **Apache ECharts** (recomendada) | Apache-2.0 | ~1 MB completo, ~300 KB com build só de pie/line | Pizza, rosca, linhas e barras; tooltip, legenda e tema escuro prontos. Serve também para a "evolução mensal". |
| Chart.js | MIT | ~200 KB | Mais leve; menos recursos (tema escuro manual). |

Qualquer uma funciona com o mesmo padrão abaixo.

> Alternativa sem JS: se só precisarmos da rosca de alocação, um `<svg>` com
> `stroke-dasharray` em Rust puro tem ~40 linhas e zero dependências. A biblioteca JS se paga
> quando entrarem gráficos de linha com tooltip/zoom (evolução patrimonial, gastos por mês).

## Regras do projeto

1. **Offline-first: nada de CDN.** O arquivo da biblioteca fica versionado em
   `frontend/vendor/` e é copiado pelo Trunk.
2. **Sem float nas contas.** Valores vão ao JS como **string decimal** (`Decimal::to_string()`),
   e o JS só faz `Number(v)` para desenhar pixels. Somas, percentuais e rótulos formatados
   ("R$ 1.234,56", "75,00%") são calculados em Rust (`shared::format`) e enviados prontos.
3. **O JS não tem estado de negócio.** Ele recebe `(elemento, dados)` e desenha; quem decide
   o que mostrar é o componente Leptos.

## Passo a passo

### 1. Vendorizar a biblioteca

```
frontend/vendor/echarts.min.js   ← baixado uma vez de https://echarts.apache.org/en/download.html
```

Em `frontend/index.html`, antes do `<link data-trunk rel="rust" …>`:

```html
<script data-trunk src="vendor/echarts.min.js"></script>
```

### 2. Cola JS mínima (`frontend/js/charts.js`)

O `wasm-bindgen` empacota arquivos referenciados por `#[wasm_bindgen(module = …)]` como
*snippets*, e o Trunk os copia automaticamente para o `dist/`.

```js
// Recebe fatias já formatadas em Rust: { label, value: "1234.56", caption: "R$ 1.234,56 · 75,00%", color }
export function renderPie(el, slices, dark) {
  const chart = echarts.getInstanceByDom(el) ?? echarts.init(el, dark ? 'dark' : null);
  chart.setOption({
    backgroundColor: 'transparent',
    tooltip: { trigger: 'item', formatter: (p) => `${p.name}<br>${p.data.caption}` },
    series: [{
      type: 'pie',
      radius: ['55%', '80%'], // rosca ("disco")
      label: { show: false },
      data: slices.map((s) => ({ name: s.label, value: Number(s.value), caption: s.caption, itemStyle: { color: s.color } })),
    }],
  });
  return chart;
}

export function resize(el) { echarts.getInstanceByDom(el)?.resize(); }
export function dispose(el) { echarts.getInstanceByDom(el)?.dispose(); }
```

### 3. Binding Rust (`frontend/src/components/chart.rs`)

Adicionar ao `frontend/Cargo.toml`:
`web-sys = { version = "0.3", features = ["HtmlElement", "MediaQueryList"] }`.

```rust
use leptos::{html, prelude::*};
use serde::Serialize;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(module = "/js/charts.js")]
extern "C" {
    #[wasm_bindgen(js_name = renderPie)]
    fn render_pie(el: &web_sys::HtmlElement, slices: JsValue, dark: bool);
    fn dispose(el: &web_sys::HtmlElement);
}

#[derive(Serialize, Clone)]
pub struct PieSlice {
    pub label: String,
    pub value: String,   // Decimal como texto
    pub caption: String, // já formatado em Rust
    pub color: String,   // hex; manter em sincronia com asset_type_color()
}

#[component]
pub fn PieChart(#[prop(into)] slices: Signal<Vec<PieSlice>>) -> impl IntoView {
    let el = NodeRef::<html::Div>::new();

    // Redesenha quando os dados mudam (o elemento já existe quando o Effect roda).
    Effect::new(move |_| {
        let data = slices.get();
        if let Some(div) = el.get() {
            let dark = window().match_media("(prefers-color-scheme: dark)").ok().flatten().is_some_and(|m| m.matches());
            render_pie(&div, serde_wasm_bindgen::to_value(&data).unwrap(), dark);
        }
    });

    // Libera o canvas ao sair da página.
    on_cleanup(move || {
        if let Some(div) = el.get_untracked() {
            dispose(&div);
        }
    });

    view! { <div node_ref=el class="h-64 w-full"></div> }
}
```

### 4. Usar no lugar da barra

Em `AllocationBar` (ou num novo `AllocationPie`), converter `AllocationSlice` → `PieSlice`:

```rust
PieSlice {
    label: s.asset_type.plural_pt().into(),
    value: s.cost.0.to_string(),
    caption: format!("{} · {}", format_brl(s.cost.0), format_percent(s.percent)),
    color: "#0ea5e9".into(), // sky-500 para ações, etc.
}
```

## Cuidados

- **Redimensionamento:** a janela do Tauri muda de tamanho; registrar um `ResizeObserver`
  (ou `window.onresize`) que chame `resize(el)`.
- **Tema:** o app segue `prefers-color-scheme`. Para trocar o tema em tempo real, escutar
  `matchMedia(...).addEventListener('change', …)` e recriar o gráfico (`dispose` + `render`).
- **CSP:** hoje `app.security.csp` é `null`. Se for ativada, `script-src 'self'` basta, pois
  tudo é servido localmente; nunca adicionar domínios externos.
- **Tamanho do bundle:** preferir o build customizado do ECharts (só `PieChart`/`LineChart`,
  `TooltipComponent`, `LegendComponent`, renderer `canvas`).
- **Fora do Tauri** (`trunk serve` no navegador) os gráficos funcionam igual; só o IPC falha.
