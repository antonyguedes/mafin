//! Imposto de Renda: apuração mensal (isenção de R$ 20 mil, prejuízo acumulado, DARF).

mod detail;
mod year;

use leptos::prelude::*;
use shared::format::{format_brl, format_date_br, format_percent};
use shared::rust_decimal::Decimal;
use shared::tax::{DARF_CODE, MonthlyTax, STOCK_EXEMPTION_LIMIT, TaxReport};
use shared::{Money, SaleResult, YearMonth};

use crate::components::month_picker::MonthPicker;
use crate::components::page::{Card, ErrorBanner, PageHeader};
use crate::ipc;
use crate::util::current_month;
use detail::{MonthDetail, MonthSales};
use year::YearSummary;

#[component]
pub fn Tax() -> impl IntoView {
    let month = RwSignal::new(current_month());
    let report = LocalResource::new(ipc::get_tax_report);
    let portfolio = LocalResource::new(ipc::get_portfolio);

    let report_value = Signal::derive(move || report.get().and_then(Result::ok));
    let error = Signal::derive(move || {
        report.get().and_then(Result::err).or_else(|| portfolio.get().and_then(Result::err))
    });
    let selected = Signal::derive(move || {
        let m = month.get();
        report_value.with(|r| r.as_ref().and_then(|r| r.month(m).cloned()))
    });
    let sales = Signal::derive(move || {
        let m = month.get();
        portfolio
            .get()
            .and_then(Result::ok)
            .map(|p| p.sales.into_iter().filter(|s| m.contains(s.date)).collect::<Vec<SaleResult>>())
            .unwrap_or_default()
    });

    view! {
        <PageHeader title="Imposto de Renda" subtitle="Apuração mensal de ações e FIIs (swing trade), com prejuízo a compensar.">
            <MonthPicker month />
        </PageHeader>

        <ErrorBanner message=error />
        <DayTradeWarning selected />

        <SummaryCards month selected report=report_value />

        {move || match selected.get() {
            Some(m) => view! { <MonthDetail month=m /> }.into_any(),
            None => view! {
                <Card class="mb-6">
                    <p class="text-sm text-slate-500">
                        {move || format!("Sem vendas de ações ou FIIs em {}. Nada a apurar neste mês.", month.get().label_pt())}
                    </p>
                </Card>
            }
            .into_any(),
        }}

        <MonthSales sales />
        <YearSummary month report=report_value />
        <Notes />
    }
}

#[component]
fn DayTradeWarning(selected: Signal<Option<MonthlyTax>>) -> impl IntoView {
    let tickers = move || selected.get().map(|m| m.day_trade_tickers).filter(|t| !t.is_empty());
    move || {
        tickers().map(|t| {
            view! {
                <div role="alert" class="mb-4 rounded-lg border border-amber-200 bg-amber-50 px-4 py-3 text-sm text-amber-800 dark:border-amber-900 dark:bg-amber-950 dark:text-amber-200">
                    <strong>"Possível day trade: "</strong>
                    {t.join(", ")}
                    ". Houve compra e venda do mesmo ativo na mesma data. Day trade tem alíquota de 20% e apuração
                    separada, que o Mafin ainda não calcula: essas vendas foram tratadas como operações comuns."
                </div>
            }
        })
    }
}

#[component]
fn SummaryCards(
    month: RwSignal<YearMonth>,
    selected: Signal<Option<MonthlyTax>>,
    report: Signal<Option<TaxReport>>,
) -> impl IntoView {
    let stock_sales = move || selected.get().map(|m| m.stock.sales_total.0).unwrap_or_default();
    let exempt = move || stock_sales() <= STOCK_EXEMPTION_LIMIT;
    let usage = move || (stock_sales() / STOCK_EXEMPTION_LIMIT * Decimal::ONE_HUNDRED).min(Decimal::ONE_HUNDRED);
    let losses = move || {
        let m = month.get();
        report.with(|r| r.as_ref().map(|r| r.losses_at(m)).unwrap_or((Money::ZERO, Money::ZERO)))
    };

    let card_label = "text-sm text-slate-500 dark:text-slate-400";
    let card_value = "tabular mt-2 text-2xl font-semibold";
    let card_hint = "tabular mt-1 text-xs text-slate-400 dark:text-slate-500";

    view! {
        <div class="mb-6 grid grid-cols-1 gap-4 md:grid-cols-2 xl:grid-cols-4">
            <Card>
                <p class=card_label>"Vendas de ações no mês"</p>
                <p class=card_value>{move || format_brl(stock_sales())}</p>
                <div class="mt-3 h-2 overflow-hidden rounded-full bg-slate-100 dark:bg-slate-800" role="progressbar"
                    aria-label="Uso do limite de isenção" aria-valuemin="0" aria-valuemax="100"
                    aria-valuenow=move || usage().round().to_string()>
                    <div
                        class=move || if exempt() { "h-full rounded-full bg-brand-500" } else { "h-full rounded-full bg-red-500" }
                        style=move || format!("width: {}%", usage().round_dp(2))
                    ></div>
                </div>
                <p class=move || {
                    format!("{card_hint} {}", if exempt() { "" } else { "text-red-600! dark:text-red-400!" })
                }>
                    {move || {
                        if exempt() {
                            format!("Isento · limite de {} ({} usado)", format_brl(STOCK_EXEMPTION_LIMIT), format_percent(usage()))
                        } else {
                            format!("Acima de {}: lucro tributável", format_brl(STOCK_EXEMPTION_LIMIT))
                        }
                    }}
                </p>
            </Card>

            <Card>
                <p class=card_label>"Lucro tributável"</p>
                <p class=card_value>{move || format_brl(selected.get().map(|m| m.taxable_base().0).unwrap_or_default())}</p>
                <p class=card_hint>
                    {move || {
                        let (s, f) = selected.get().map(|m| (m.stock.taxable_base.0, m.fii.taxable_base.0)).unwrap_or_default();
                        format!("Ações {} · FIIs {}", format_brl(s), format_brl(f))
                    }}
                </p>
            </Card>

            <Card>
                <p class=card_label>"Prejuízo a compensar"</p>
                <p class=card_value>{move || { let (s, f) = losses(); format_brl(s.0 + f.0) }}</p>
                <p class=card_hint>
                    {move || { let (s, f) = losses(); format!("Ações {} · FIIs {}", format_brl(s.0), format_brl(f.0)) }}
                </p>
            </Card>

            <Card class="border-brand-200! dark:border-brand-900!">
                <p class=card_label>"DARF a pagar"</p>
                <p class=card_value>{move || format_brl(selected.get().map(|m| m.darf.0).unwrap_or_default())}</p>
                <p class=card_hint>
                    {move || match selected.get() {
                        Some(m) if !m.darf.is_zero() => {
                            format!("Código {DARF_CODE} · vence em {}", format_date_br(m.due_date))
                        }
                        Some(m) if !m.carried_out.is_zero() => {
                            format!("{} abaixo do mínimo de R$ 10,00: soma no próximo mês", format_brl(m.carried_out.0))
                        }
                        _ => "Nada a pagar".to_string(),
                    }}
                </p>
            </Card>
        </div>
    }
}

#[component]
fn Notes() -> impl IntoView {
    view! {
        <Card class="mt-6">
            <h2 class="text-sm font-semibold">"Como o cálculo é feito"</h2>
            <ul class="mt-3 list-inside list-disc space-y-1.5 text-sm text-slate-600 dark:text-slate-400">
                <li>"Preço médio por ticker, somando todas as corretoras; taxas de compra entram no custo e taxas de venda reduzem o valor recebido."</li>
                <li>"Ações: isenção quando o total vendido no mês é de até R$ 20.000,00; acima disso, 15% sobre o lucro. Prejuízo em mês isento também fica para compensar."</li>
                <li>"FIIs: 20% sobre o lucro, sem isenção. Prejuízos de ações e de FIIs são compensados separadamente."</li>
                <li>"DARF código 6015, com vencimento no último dia útil do mês seguinte (feriados não considerados). Valores abaixo de R$ 10,00 acumulam."</li>
                <li>"Não considerados: IRRF “dedo-duro” (deduza o valor da nota de corretagem), day trade, renda fixa (tributada na fonte), proventos."</li>
                <li>"Estimativa para organização pessoal; confira com a nota de corretagem e, se preciso, com um contador."</li>
            </ul>
        </Card>
    }
}
