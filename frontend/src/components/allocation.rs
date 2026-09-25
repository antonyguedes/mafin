//! Alocação da carteira (por tipo de ativo e por ticker) em gráficos de rosca.

use leptos::prelude::*;
use shared::format::{format_brl, format_percent};
use shared::rust_decimal::Decimal;
use shared::{AllocationSlice, AssetType, Position};

use crate::components::chart::{DonutWithLegend, PALETTE, PieSlice};

/// Classe Tailwind de cada tipo de ativo (pontos em tabelas e cards).
pub fn asset_type_color(asset_type: AssetType) -> &'static str {
    match asset_type {
        AssetType::Stock => "bg-sky-500",
        AssetType::Fii => "bg-amber-500",
        AssetType::FixedIncome => "bg-violet-500",
    }
}

/// Mesma cor em hex, para os gráficos.
pub fn asset_type_hex(asset_type: AssetType) -> &'static str {
    match asset_type {
        AssetType::Stock => PALETTE[0],
        AssetType::Fii => PALETTE[1],
        AssetType::FixedIncome => PALETTE[2],
    }
}

fn caption(cost: Decimal, percent: Decimal) -> String {
    format!("{} · {}", format_brl(cost), format_percent(percent))
}

pub fn slices_by_type(allocation: &[AllocationSlice]) -> Vec<PieSlice> {
    allocation
        .iter()
        .map(|s| PieSlice {
            label: s.asset_type.plural_pt().into(),
            value: s.cost.0.to_string(),
            caption: caption(s.cost.0, s.percent),
            color: asset_type_hex(s.asset_type).into(),
        })
        .collect()
}

/// Uma fatia por ticker, do maior para o menor custo; acima de 7, o resto vira "Outros".
pub fn slices_by_ticker(positions: &[Position]) -> Vec<PieSlice> {
    let total: Decimal = positions.iter().map(|p| p.total_cost.0).sum();
    if total.is_zero() {
        return Vec::new();
    }
    let mut sorted: Vec<&Position> = positions.iter().collect();
    sorted.sort_by_key(|p| std::cmp::Reverse(p.total_cost));

    let mut items: Vec<(String, Decimal)> = sorted.iter().map(|p| (p.ticker.clone(), p.total_cost.0)).collect();
    if items.len() > PALETTE.len() {
        let rest: Decimal = items.drain(PALETTE.len() - 1..).map(|(_, c)| c).sum();
        items.push(("Outros".into(), rest));
    }
    items
        .into_iter()
        .enumerate()
        .map(|(i, (label, cost))| PieSlice {
            label,
            value: cost.to_string(),
            caption: caption(cost, cost / total * Decimal::ONE_HUNDRED),
            color: PALETTE[i % PALETTE.len()].into(),
        })
        .collect()
}

#[component]
pub fn AllocationChart(#[prop(into)] allocation: Signal<Vec<AllocationSlice>>) -> impl IntoView {
    let slices = Signal::derive(move || allocation.with(|a| slices_by_type(a)));
    view! { <DonutWithLegend slices label="Alocação por tipo de ativo" /> }
}
