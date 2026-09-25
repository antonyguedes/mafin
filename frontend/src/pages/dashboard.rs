use leptos::prelude::*;
use shared::format::{format_brl, format_date_br};
use shared::{TransactionFilter, TransactionSummary};

use crate::components::allocation::AllocationBar;
use crate::components::page::{Card, ErrorBanner, PageHeader, StatCard};
use crate::ipc;
use crate::util::current_month;

#[component]
pub fn Dashboard() -> impl IntoView {
    let month = current_month();
    let transactions = LocalResource::new(move || {
        ipc::list_transactions(TransactionFilter { period: Some(month), kind: None })
    });
    let portfolio = LocalResource::new(ipc::get_portfolio);
    let tax = LocalResource::new(ipc::get_tax_report);

    let balance = move || {
        transactions
            .get()
            .and_then(Result::ok)
            .map(|rows| format_brl(TransactionSummary::of(&rows).balance().0))
            .unwrap_or_else(|| "—".into())
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
        Some(Some(m)) if m.stock.exempt && m.fii.sales_total.is_zero() => "Vendas de ações dentro da isenção".into(),
        _ => "Nada a pagar sobre as vendas do mês".into(),
    };
    let allocation = Signal::derive(move || portfolio.get().and_then(Result::ok).map(|p| p.allocation).unwrap_or_default());
    let error = Signal::derive(move || {
        transactions
            .get()
            .and_then(Result::err)
            .or_else(|| portfolio.get().and_then(Result::err))
            .or_else(|| tax.get().and_then(Result::err))
    });

    view! {
        <PageHeader title="Dashboard" subtitle="Visão geral das suas finanças e investimentos." />
        <ErrorBanner message=error />
        <div class="mb-6 grid grid-cols-1 gap-4 md:grid-cols-3">
            <StatCard label="Saldo do mês" value=balance hint=month.label_pt() />
            <StatCard label="Patrimônio investido" value=invested hint="Custódia a preço médio" />
            <StatCard label="IR sobre as vendas do mês" value=tax_value hint=tax_hint />
        </div>
        <Show when=move || !allocation.with(Vec::is_empty)>
            <Card class="mb-6">
                <h2 class="mb-4 text-sm font-semibold">"Alocação da carteira"</h2>
                <AllocationBar slices=allocation />
            </Card>
        </Show>

    }
}
