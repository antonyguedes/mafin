use leptos::prelude::*;
use shared::format::{format_brl, format_date_br, format_decimal_br, format_quantity};
use shared::rust_decimal::Decimal;
use shared::tax::{CategoryTax, MonthlyTax, TaxCategory};
use shared::{AssetType, SaleKind, SaleResult};

use crate::components::allocation::asset_type_color;
use crate::components::table::{ROW, TABLE_CARD, TD, TH};

fn signed_brl(value: Decimal) -> String {
    if value > Decimal::ZERO { format!("+{}", format_brl(value)) } else { format_brl(value) }
}

fn result_class(value: Decimal) -> &'static str {
    if value > Decimal::ZERO {
        "text-emerald-600 dark:text-emerald-400"
    } else if value < Decimal::ZERO {
        "text-red-600 dark:text-red-400"
    } else {
        ""
    }
}

/// Apuração do mês, lado a lado por categoria.
#[component]
pub(super) fn MonthDetail(month: MonthlyTax) -> impl IntoView {
    type Line = (&'static str, fn(&CategoryTax) -> String);
    let lines: [Line; 7] = [
        ("Vendas no mês", |c| format_brl(c.sales_total.0)),
        ("Resultado do mês", |c| signed_brl(c.result.0)),
        ("Prejuízo a compensar (início do mês)", |c| format_brl(c.loss_before.0)),
        ("Prejuízo compensado", |c| format_brl(c.loss_used.0)),
        ("Base de cálculo", |c| {
            if c.exempt && c.result.0 > Decimal::ZERO { "Isento".into() } else { format_brl(c.taxable_base.0) }
        }),
        ("Alíquota", |c| format!("{}%", format_decimal_br(c.rate_percent, 0))),
        ("Imposto", |c| format_brl(c.tax.0)),
    ];
    // Day trade só aparece se houve no mês, para não poluir a tabela.
    let cats: Vec<CategoryTax> = month
        .categories()
        .into_iter()
        .filter(|c| c.category != TaxCategory::StockDayTrade || !c.sales_total.is_zero() || !c.loss_before.is_zero())
        .cloned()
        .collect();
    let span = cats.len().to_string();

    view! {
        <section class=format!("{TABLE_CARD} mb-6")>
            <div class="border-b border-slate-200 px-4 py-3 dark:border-slate-800">
                <h2 class="text-sm font-semibold">{format!("Apuração de {}", month.month.label_pt())}</h2>
            </div>
            <table class="w-full text-sm">
                <thead class="bg-slate-50 dark:bg-slate-950/40">
                    <tr>
                        <th class=TH></th>
                        {cats.iter().map(|c| view! { <th class=format!("{TH} w-48 text-right")>{c.category.label_pt()}</th> }).collect_view()}
                    </tr>
                </thead>
                <tbody class="divide-y divide-slate-100 dark:divide-slate-800">
                    {lines
                        .into_iter()
                        .map(|(label, value)| {
                            view! {
                                <tr>
                                    <td class=format!("{TD} text-slate-600 dark:text-slate-400")>{label}</td>
                                    {cats
                                        .iter()
                                        .map(|c| {
                                            let class = if label == "Resultado do mês" { result_class(c.result.0) } else { "" };
                                            view! { <td class=format!("{TD} tabular text-right {class}")>{value(c)}</td> }
                                        })
                                        .collect_view()}
                                </tr>
                            }
                        })
                        .collect_view()}
                </tbody>
                <tfoot class="border-t-2 border-slate-200 bg-slate-50 dark:border-slate-700 dark:bg-slate-950/40">
                    <FootRow label="IR devido no mês" value=format_brl(month.tax_due.0) span=span.clone() strong=true />
                    {(!month.irrf_used.is_zero()).then(|| view! {
                        <FootRow label="(−) IRRF abatido" value=format!("−{}", format_brl(month.irrf_used.0)) span=span.clone() />
                    })}
                    {(!month.carried_in.is_zero()).then(|| view! {
                        <FootRow label="(+) Saldo abaixo de R$ 10,00 de meses anteriores" value=format_brl(month.carried_in.0) span=span.clone() />
                    })}
                    <FootRow
                        label="DARF (código 6015)"
                        value=if month.darf.is_zero() {
                            if month.carried_out.is_zero() {
                                "Nada a pagar".to_string()
                            } else {
                                format!("Acumula {} para o próximo mês", format_brl(month.carried_out.0))
                            }
                        } else {
                            format!("{} · vence {}", format_brl(month.darf.0), format_date_br(month.due_date))
                        }
                        span=span.clone()
                        strong=true
                    />
                </tfoot>
            </table>
        </section>
    }
}

#[component]
fn FootRow(label: &'static str, value: String, span: String, #[prop(optional)] strong: bool) -> impl IntoView {
    let weight = if strong { "font-semibold" } else { "text-slate-600 dark:text-slate-400" };
    view! {
        <tr>
            <td class=format!("{TD} {weight}")>{label}</td>
            <td class=format!("{TD} tabular text-right {weight}") colspan=span>{value}</td>
        </tr>
    }
}

/// Vendas do mês. Custo: PM da carteira (swing) ou compras do dia (day trade).
#[component]
pub(super) fn MonthSales(sales: Signal<Vec<SaleResult>>) -> impl IntoView {
    view! {
        <Show when=move || !sales.with(Vec::is_empty)>
            <section class=format!("{TABLE_CARD} mb-6")>
                <div class="border-b border-slate-200 px-4 py-3 dark:border-slate-800">
                    <h2 class="text-sm font-semibold">"Vendas do mês"</h2>
                </div>
                <table class="w-full text-sm">
                    <thead class="bg-slate-50 dark:bg-slate-950/40">
                        <tr>
                            <th class=TH>"Data"</th>
                            <th class=TH>"Ativo"</th>
                            <th class=format!("{TH} text-right")>"Qtd."</th>
                            <th class=format!("{TH} text-right")>"Valor da venda"</th>
                            <th class=format!("{TH} text-right")>"Custo"</th>
                            <th class=format!("{TH} text-right")>"Taxas"</th>
                            <th class=format!("{TH} text-right")>"Resultado"</th>
                        </tr>
                    </thead>
                    <tbody class="divide-y divide-slate-100 dark:divide-slate-800">
                        <For
                            each=move || sales.get()
                            key=|s| (s.order_id, s.kind, s.result)
                            children=move |s| {
                                let fixed = s.asset_type == AssetType::FixedIncome;
                                view! {
                                    <tr class=ROW>
                                        <td class=format!("{TD} tabular text-slate-600 dark:text-slate-400")>{format_date_br(s.date)}</td>
                                        <td class=TD>
                                            <span class="flex items-center gap-2 font-medium">
                                                <span class=format!("size-2 rounded-full {}", asset_type_color(s.asset_type))></span>
                                                {s.ticker.clone()}
                                                {(s.kind == SaleKind::DayTrade).then(|| view! { <span class="rounded bg-amber-100 px-1.5 text-xs font-normal text-amber-800 dark:bg-amber-950 dark:text-amber-200">"day trade"</span> })}
                                                {fixed.then(|| view! { <span class="text-xs font-normal text-slate-400">"fora da apuração"</span> })}
                                            </span>
                                        </td>
                                        <td class=format!("{TD} tabular text-right")>{format_quantity(s.quantity.0)}</td>
                                        <td class=format!("{TD} tabular text-right")>{format_brl(s.gross.0)}</td>
                                        <td class=format!("{TD} tabular text-right text-slate-600 dark:text-slate-400")>{format_brl(s.cost.0)}</td>
                                        <td class=format!("{TD} tabular text-right text-slate-500")>{format_brl(s.fees.0)}</td>
                                        <td class=format!("{TD} tabular text-right font-medium {}", result_class(s.result.0))>{signed_brl(s.result.0)}</td>
                                    </tr>
                                }
                            }
                        />
                    </tbody>
                </table>
            </section>
        </Show>
    }
}
