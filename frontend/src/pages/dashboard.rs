use leptos::prelude::*;
use shared::format::{format_brl, format_date_br};
use shared::payout::PayoutTotals;
use shared::{Money, Transaction, TransactionFilter, TransactionSummary, monthly_summaries};

use crate::components::allocation::AllocationChart;
use crate::components::chart::{BarChart, BarData, BarSeries};
use crate::components::page::{Card, ErrorBanner, PageHeader, StatCard};
use crate::ipc;
use crate::pages::expenses::ExpensesByCategory;
use crate::util::current_month;

/// Meses exibidos no gráfico de receitas × despesas.
const HISTORY_MONTHS: usize = 12;

fn income_expense_bars(rows: &[Transaction]) -> BarData {
    let months = current_month().last_n(HISTORY_MONTHS);
    let summaries = monthly_summaries(rows, &months);
    let series = |name: &str, color: &str, pick: fn(&TransactionSummary) -> Money| BarSeries {
        name: name.into(),
        color: color.into(),
        values: summaries.iter().map(|(_, s)| pick(s).0.to_string()).collect(),
        captions: summaries.iter().map(|(_, s)| format_brl(pick(s).0)).collect(),
    };
    BarData {
        categories: months.iter().map(|m| m.short_label_pt()).collect(),
        series: vec![series("Receitas", "#10b981", |s| s.income), series("Despesas", "#f43f5e", |s| s.expense)],
    }
}

#[component]
pub fn Dashboard() -> impl IntoView {
    let month = current_month();
    // Histórico completo: o mês atual e o gráfico de 12 meses saem da mesma consulta.
    let transactions = LocalResource::new(move || ipc::list_transactions(TransactionFilter::default()));
    let portfolio = LocalResource::new(ipc::get_portfolio);
    let tax = LocalResource::new(ipc::get_tax_report);
    let payouts = LocalResource::new(ipc::list_payouts);

    let all_rows = Signal::derive(move || transactions.get().and_then(Result::ok).unwrap_or_default());
    let month_rows =
        Signal::derive(move || all_rows.with(|r| r.iter().filter(|t| month.contains(t.date)).cloned().collect::<Vec<_>>()));
    let loaded = move || transactions.get().is_some_and(|r| r.is_ok());

    let balance = move || {
        if loaded() { format_brl(month_rows.with(|r| TransactionSummary::of(r)).balance().0) } else { "—".into() }
    };
    let invested = move || {
        portfolio.get().and_then(Result::ok).map(|p| format_brl(p.total_cost.0)).unwrap_or_else(|| "—".into())
    };
    // Imposto sobre as vendas deste mês (o DARF vence no mês seguinte).
    let this_month_tax = move || tax.get().and_then(Result::ok).map(|r| r.month(month).cloned());
    let tax_value = move || match this_month_tax() {
        Some(Some(m)) => format_brl(m.darf.0),
        Some(None) => format_brl(Default::default()),
        None => "—".into(),
    };
    let tax_hint = move || match this_month_tax() {
        Some(Some(m)) if !m.darf.is_zero() => format!("DARF 6015 · vence {}", format_date_br(m.due_date)),
        _ => "Nada a pagar sobre as vendas do mês".into(),
    };
    let month_payouts = move || {
        payouts.get().and_then(Result::ok).map(|ps| PayoutTotals::of(ps.iter().filter(|p| month.contains(p.date))))
    };
    let payouts_value = move || month_payouts().map(|t| format_brl(t.net().0)).unwrap_or_else(|| "—".into());
    let payouts_hint = move || {
        month_payouts()
            .filter(|t| !t.withheld.is_zero())
            .map(|t| format!("Líquido · {} de IR retido", format_brl(t.withheld.0)))
            .unwrap_or_else(|| "Dividendos, JCP e rendimentos (líquido)".into())
    };
    let allocation = Signal::derive(move || portfolio.get().and_then(Result::ok).map(|p| p.allocation).unwrap_or_default());
    let bars = Signal::derive(move || all_rows.with(|r| income_expense_bars(r)));
    let error = Signal::derive(move || {
        transactions
            .get()
            .and_then(Result::err)
            .or_else(|| portfolio.get().and_then(Result::err))
            .or_else(|| tax.get().and_then(Result::err))
            .or_else(|| payouts.get().and_then(Result::err))
    });

    view! {
        <PageHeader title="Dashboard" subtitle="Visão geral das suas finanças e investimentos." />
        <ErrorBanner message=error />
        <div class="mb-6 grid grid-cols-1 gap-4 md:grid-cols-2 xl:grid-cols-4">
            <StatCard label="Saldo do mês" value=balance hint=month.label_pt() />
            <StatCard label="Patrimônio investido" value=invested hint="Custódia a preço médio" />
            <StatCard label="Proventos do mês" value=payouts_value hint=payouts_hint />
            <StatCard label="IR sobre as vendas do mês" value=tax_value hint=tax_hint />
        </div>

        <Card class="mb-6">
            <h2 class="mb-2 text-sm font-semibold">"Receitas × despesas · últimos 12 meses"</h2>
            <BarChart data=bars label="Receitas e despesas dos últimos 12 meses" />
        </Card>

        <div class="grid grid-cols-1 gap-4 xl:grid-cols-2">
            <Card>
                <h2 class="mb-4 text-sm font-semibold">"Alocação da carteira"</h2>
                <Show
                    when=move || !allocation.with(Vec::is_empty)
                    fallback=|| view! { <p class="text-sm text-slate-500">"Sem posições em carteira."</p> }
                >
                    <AllocationChart allocation />
                </Show>
            </Card>
            <Card>
                <h2 class="mb-4 text-sm font-semibold">{format!("Despesas de {}", month.label_pt())}</h2>
                <ExpensesByCategory rows=month_rows />
            </Card>
        </div>
    }
}
