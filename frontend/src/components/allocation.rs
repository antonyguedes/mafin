//! Visualização da alocação da carteira por tipo de ativo.
//!
//! Hoje: barra empilhada em HTML/Tailwind + legenda (zero dependências).
//! Futuro: gráfico de pizza/disco com uma biblioteca JS. Este componente é o ponto
//! de troca: a assinatura (`slices`) já é o dado do gráfico. Veja `docs/graficos.md`.

use leptos::prelude::*;
use shared::format::{format_brl, format_percent};
use shared::{AllocationSlice, AssetType};

/// Cor de cada tipo de ativo, usada em barra, legenda e badges.
pub fn asset_type_color(asset_type: AssetType) -> &'static str {
    match asset_type {
        AssetType::Stock => "bg-sky-500",
        AssetType::Fii => "bg-amber-500",
        AssetType::FixedIncome => "bg-violet-500",
    }
}

#[component]
pub fn AllocationBar(#[prop(into)] slices: Signal<Vec<AllocationSlice>>) -> impl IntoView {
    view! {
        <div>
            <div class="flex h-3 overflow-hidden rounded-full bg-slate-100 dark:bg-slate-800" role="img" aria-label="Alocação por tipo de ativo">
                {move || {
                    slices
                        .get()
                        .into_iter()
                        .map(|s| {
                            // O percentual vira texto só para o CSS; nenhuma conta é feita em float.
                            let width = format!("width: {}%", s.percent.round_dp(4));
                            view! { <div class=asset_type_color(s.asset_type) style=width></div> }
                        })
                        .collect_view()
                }}
            </div>
            <ul class="mt-3 flex flex-wrap gap-x-6 gap-y-2 text-sm">
                {move || {
                    slices
                        .get()
                        .into_iter()
                        .map(|s| {
                            view! {
                                <li class="flex items-center gap-2">
                                    <span class=format!("size-2.5 rounded-full {}", asset_type_color(s.asset_type))></span>
                                    <span class="font-medium">{s.asset_type.plural_pt()}</span>
                                    <span class="tabular text-slate-500 dark:text-slate-400">
                                        {format!("{} · {}", format_percent(s.percent), format_brl(s.cost.0))}
                                    </span>
                                </li>
                            }
                        })
                        .collect_view()
                }}
            </ul>
        </div>
    }
}
