use leptos::prelude::*;
use shared::YearMonth;
use shared::format::{MONTHS_PT, format_brl, format_date_br};
use shared::rust_decimal::Decimal;
use shared::tax::{MonthlyTax, TaxReport};

use crate::components::table::{ROW, ROW_EDITING, TABLE_CARD, TD, TH};

/// Meses com vendas no ano selecionado; clicar numa linha seleciona o mês.
#[component]
pub(super) fn YearSummary(month: RwSignal<YearMonth>, report: Signal<Option<TaxReport>>) -> impl IntoView {
    let year = move || month.get().year;
    let months = move || {
        let y = year();
        report.with(|r| {
            r.as_ref().map(|r| r.months.iter().filter(|m| m.month.year == y).cloned().collect::<Vec<MonthlyTax>>()).unwrap_or_default()
        })
    };
    let total_darf = move || months().iter().map(|m| m.darf.0).sum::<Decimal>();

    view! {
        <section class=TABLE_CARD>
            <div class="flex items-center justify-between border-b border-slate-200 px-4 py-3 dark:border-slate-800">
                <h2 class="text-sm font-semibold">{move || format!("Resumo de {}", year())}</h2>
                <span class="tabular text-sm text-slate-500">{move || format!("DARFs no ano: {}", format_brl(total_darf()))}</span>
            </div>
            <table class="w-full text-sm">
                <thead class="bg-slate-50 dark:bg-slate-950/40">
                    <tr>
                        <th class=TH>"Mês"</th>
                        <th class=format!("{TH} text-right")>"Vendas de ações"</th>
                        <th class=format!("{TH} text-right")>"Ações (comuns)"</th>
                        <th class=format!("{TH} text-right")>"Day trade"</th>
                        <th class=format!("{TH} text-right")>"Resultado FIIs"</th>
                        <th class=format!("{TH} text-right")>"IR devido"</th>
                        <th class=format!("{TH} text-right")>"DARF"</th>
                        <th class=format!("{TH} text-right")>"Vencimento"</th>
                    </tr>
                </thead>
                <tbody class="divide-y divide-slate-100 dark:divide-slate-800">
                    <For
                        each=months
                        key=|m| (m.month, m.darf, m.tax_due, m.stock.result, m.fii.result)
                        children=move |m| {
                            let ym = m.month;
                            let name = MONTHS_PT[(ym.month - 1) as usize];
                            view! {
                                <tr
                                    class=move || format!("cursor-pointer {}", if month.get() == ym { ROW_EDITING } else { ROW })
                                    on:click=move |_| month.set(ym)
                                >
                                    <td class=format!("{TD} font-medium")>{name}</td>
                                    <td class=format!("{TD} tabular text-right")>
                                        {format_brl(m.stock.sales_total.0)}
                                        {m.stock.exempt.then(|| view! { <span class="ml-1 text-xs text-brand-600">"isento"</span> })}
                                    </td>
                                    <td class=format!("{TD} tabular text-right")>{format_brl(m.stock.result.0)}</td>
                                    <td class=format!("{TD} tabular text-right")>{format_brl(m.day_trade.result.0)}</td>
                                    <td class=format!("{TD} tabular text-right")>{format_brl(m.fii.result.0)}</td>
                                    <td class=format!("{TD} tabular text-right")>{format_brl(m.tax_due.0)}</td>
                                    <td class=format!("{TD} tabular text-right font-medium")>{format_brl(m.darf.0)}</td>
                                    <td class=format!("{TD} tabular text-right text-slate-600 dark:text-slate-400")>
                                        {if m.darf.is_zero() { "—".to_string() } else { format_date_br(m.due_date) }}
                                    </td>
                                </tr>
                            }
                        }
                    />
                </tbody>
            </table>
            <Show when=move || months().is_empty()>
                <p class="px-4 py-8 text-center text-sm text-slate-500">{move || format!("Nenhuma venda em {}.", year())}</p>
            </Show>
        </section>
    }
}
