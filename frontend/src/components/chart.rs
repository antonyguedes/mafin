//! Gráficos (ECharts via `js/charts.js`). Ver `docs/graficos.md`.
//!
//! Os componentes recebem dados já formatados em Rust (`caption`) e valores como string
//! decimal: nenhuma conta financeira é feita no JS.

use leptos::html;
use leptos::prelude::*;
use leptos::web_sys::HtmlElement;
use serde::Serialize;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(module = "/js/charts.js")]
extern "C" {
    #[wasm_bindgen(js_name = renderPie)]
    fn render_pie(el: &HtmlElement, slices: JsValue) -> bool;
    #[wasm_bindgen(js_name = renderBars)]
    fn render_bars(el: &HtmlElement, data: JsValue) -> bool;
    fn dispose(el: &HtmlElement);
}

/// Paleta categórica (Tailwind 500), usada em gráficos e legendas.
pub const PALETTE: [&str; 8] = ["#0ea5e9", "#f59e0b", "#8b5cf6", "#10b981", "#f43f5e", "#14b8a6", "#6366f1", "#84cc16"];

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PieSlice {
    pub label: String,
    /// Decimal como texto.
    pub value: String,
    /// Texto do tooltip, já formatado ("R$ 1.234,56 · 75,00%").
    pub caption: String,
    /// Cor hex.
    pub color: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BarSeries {
    pub name: String,
    pub color: String,
    pub values: Vec<String>,
    pub captions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BarData {
    pub categories: Vec<String>,
    pub series: Vec<BarSeries>,
}

/// Monta o `<div>` do gráfico e redesenha quando `data` muda; libera o canvas ao desmontar.
fn chart_view<T>(data: Signal<T>, class: &'static str, label: &'static str, render: fn(&HtmlElement, JsValue) -> bool) -> impl IntoView
where
    T: Serialize + Clone + Send + Sync + 'static,
{
    let el = NodeRef::<html::Div>::new();
    Effect::new(move |_| {
        let value = data.get();
        if let Some(div) = el.get() {
            match serde_wasm_bindgen::to_value(&value) {
                Ok(js) => {
                    render(&div, js);
                }
                Err(err) => leptos::logging::error!("gráfico: {err}"),
            }
        }
    });
    on_cleanup(move || {
        if let Some(div) = el.get_untracked() {
            dispose(&div);
        }
    });
    view! { <div node_ref=el class=class role="img" aria-label=label></div> }
}

#[component]
pub fn PieChart(
    #[prop(into)] slices: Signal<Vec<PieSlice>>,
    #[prop(default = "h-48 w-48")] class: &'static str,
    #[prop(default = "Gráfico de rosca")] label: &'static str,
) -> impl IntoView {
    chart_view(slices, class, label, render_pie)
}

#[component]
pub fn BarChart(
    #[prop(into)] data: Signal<BarData>,
    #[prop(default = "h-64 w-full")] class: &'static str,
    #[prop(default = "Gráfico de barras")] label: &'static str,
) -> impl IntoView {
    chart_view(data, class, label, render_bars)
}

/// Rosca + legenda HTML (acessível e sempre legível, mesmo sem o canvas).
#[component]
pub fn DonutWithLegend(
    #[prop(into)] slices: Signal<Vec<PieSlice>>,
    #[prop(default = "Gráfico de rosca")] label: &'static str,
) -> impl IntoView {
    view! {
        <div class="flex flex-wrap items-center gap-6">
            <PieChart slices label />
            <ul class="min-w-48 flex-1 space-y-2 text-sm">
                {move || {
                    slices
                        .get()
                        .into_iter()
                        .map(|s| {
                            view! {
                                <li class="flex items-center justify-between gap-4">
                                    <span class="flex items-center gap-2">
                                        <span class="size-2.5 shrink-0 rounded-full" style=format!("background-color: {}", s.color)></span>
                                        <span class="font-medium">{s.label}</span>
                                    </span>
                                    <span class="tabular text-right whitespace-nowrap text-slate-500 dark:text-slate-400">{s.caption}</span>
                                </li>
                            }
                        })
                        .collect_view()
                }}
            </ul>
        </div>
    }
}
