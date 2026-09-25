use leptos::prelude::*;
use shared::format::{format_brl, format_percent, format_quantity};
use shared::rust_decimal::Decimal;
use shared::{AllocationSlice, AssetType, Position};

use super::{PortfolioState, Tab};
use crate::components::allocation::{AllocationChart, asset_type_color, slices_by_ticker};
use crate::components::chart::DonutWithLegend;
use crate::components::page::{Card, ErrorBanner, StatCard};
use crate::components::table::{ROW, TABLE_CARD, TD, TH};

#[component]
pub(super) fn CustodyTab(state: PortfolioState) -> impl IntoView {
    let positions = Signal::derive(move || state.portfolio.get().map(|p| p.positions).unwrap_or_default());
    let allocation = Signal::derive(move || state.portfolio.get().map(|p| p.allocation).unwrap_or_default());
    let total = Signal::derive(move || state.portfolio.get().map(|p| p.total_cost.0).unwrap_or_default());

    view! {
        <ErrorBanner message=state.load_error />

        <div class="mb-6 grid grid-cols-2 gap-4 lg:grid-cols-4">
            <StatCard
                label="Total investido"
                value=move || format_brl(total.get())
                hint="Custo a preço médio"
            />
            {AssetType::ALL
                .into_iter()
                .map(|asset_type| view! { <TypeCard asset_type positions allocation /> })
                .collect_view()}
        </div>

        <Show when=move || !allocation.with(Vec::is_empty)>
            <div class="mb-6 grid grid-cols-1 gap-4 xl:grid-cols-2">
                <Card>
                    <h2 class="mb-4 text-sm font-semibold">"Alocação por tipo"</h2>
                    <AllocationChart allocation />
                </Card>
                <Card>
                    <h2 class="mb-4 text-sm font-semibold">"Alocação por ativo"</h2>
                    <DonutWithLegend
                        slices=Signal::derive(move || positions.with(|p| slices_by_ticker(p)))
                        label="Alocação por ativo"
                    />
                </Card>
            </div>
        </Show>

        <section class=TABLE_CARD>
            <div class="border-b border-slate-200 px-4 py-3 dark:border-slate-800">
                <h2 class="text-sm font-semibold">
                    "Posições "
                    <span class="font-normal text-slate-500">{move || format!("· {} ativos", positions.with(Vec::len))}</span>
                </h2>
            </div>
            <table class="w-full text-sm">
                <thead class="bg-slate-50 dark:bg-slate-950/40">
                    <tr>
                        <th class=TH>"Ticker"</th>
                        <th class=TH>"Corretoras"</th>
                        <th class=format!("{TH} text-right")>"Quantidade"</th>
                        <th class=format!("{TH} text-right")>"Preço médio"</th>
                        <th class=format!("{TH} text-right")>"Custo total"</th>
                        <th class=format!("{TH} w-28 text-right")>"% carteira"</th>
                    </tr>
                </thead>
                <tbody class="divide-y divide-slate-100 dark:divide-slate-800">
                    <For
                        each=move || positions.get()
                        key=|p| (p.ticker.clone(), p.quantity, p.total_cost)
                        children=move |p| view! { <PositionRow position=p total /> }
                    />
                </tbody>
            </table>
            <Show when=move || positions.with(Vec::is_empty)>
                <div class="px-4 py-10 text-center text-sm text-slate-500">
                    {move || {
                        if state.loading.get() {
                            view! { <p>"Carregando…"</p> }.into_any()
                        } else {
                            view! {
                                <p>"Nenhuma posição em carteira."</p>
                                <p class="mt-2">
                                    <button
                                        type="button"
                                        class="font-medium text-brand-600 hover:underline"
                                        on:click=move |_| {
                                            let tab = if state.assets.with(Vec::is_empty) { Tab::Assets } else { Tab::Orders };
                                            state.tab.set(tab);
                                        }
                                    >
                                        {move || {
                                            if state.assets.with(Vec::is_empty) {
                                                "Cadastre seu primeiro ativo"
                                            } else {
                                                "Registre uma ordem de compra"
                                            }
                                        }}
                                    </button>
                                </p>
                            }
                            .into_any()
                        }
                    }}
                </div>
            </Show>
        </section>
    }
}

#[component]
fn TypeCard(
    asset_type: AssetType,
    positions: Signal<Vec<Position>>,
    allocation: Signal<Vec<AllocationSlice>>,
) -> impl IntoView {
    let slice = move || allocation.get().into_iter().find(|s| s.asset_type == asset_type);
    let count = move || positions.with(|ps| ps.iter().filter(|p| p.asset_type == asset_type).count());

    view! {
        <Card>
            <p class="flex items-center gap-2 text-sm text-slate-500 dark:text-slate-400">
                <span class=format!("size-2.5 rounded-full {}", asset_type_color(asset_type))></span>
                {asset_type.plural_pt()}
            </p>
            <p class="tabular mt-2 text-2xl font-semibold">
                {move || format_brl(slice().map(|s| s.cost.0).unwrap_or_default())}
            </p>
            <p class="tabular mt-1 text-xs text-slate-400 dark:text-slate-500">
                {move || {
                    let n = count();
                    let percent = slice().map(|s| s.percent).unwrap_or(Decimal::ZERO);
                    let noun = if n == 1 { "ativo" } else { "ativos" };
                    format!("{} da carteira · {n} {noun}", format_percent(percent))
                }}
            </p>
        </Card>
    }
}

#[component]
fn PositionRow(position: Position, total: Signal<Decimal>) -> impl IntoView {
    let cost = position.total_cost.0;
    let share = move || {
        let total = total.get();
        if total.is_zero() { Decimal::ZERO } else { cost / total * Decimal::ONE_HUNDRED }
    };

    view! {
        <tr class=ROW>
            <td class=TD>
                <span class="flex items-center gap-2 font-medium">
                    <span class=format!("size-2 rounded-full {}", asset_type_color(position.asset_type))></span>
                    {position.ticker.clone()}
                    <span class="text-xs font-normal text-slate-400">{position.asset_type.label_pt()}</span>
                </span>
            </td>
            <td class=format!("{TD} text-slate-600 dark:text-slate-400")>{position.brokers.join(", ")}</td>
            <td class=format!("{TD} tabular text-right")>{format_quantity(position.quantity.0)}</td>
            <td class=format!("{TD} tabular text-right")>{format_brl(position.average_price.0)}</td>
            <td class=format!("{TD} tabular text-right font-medium")>{format_brl(cost)}</td>
            <td class=format!("{TD} tabular text-right text-slate-600 dark:text-slate-400")>{move || format_percent(share())}</td>
        </tr>
    }
}
