//! Módulo de Gastos: resumo do mês, formulário de lançamento e planilha.

mod form;
mod table;

use leptos::prelude::*;
use shared::format::format_brl;
use shared::{Transaction, TransactionFilter, TransactionKind, TransactionSummary, YearMonth};

use crate::components::month_picker::MonthPicker;
use crate::components::page::{PageHeader, StatCard};
use crate::ipc;
use crate::util::current_month;
use form::TransactionForm;
use table::TransactionTable;

/// Estado compartilhado entre formulário e tabela. `Copy`: só contém signals.
#[derive(Clone, Copy)]
pub(crate) struct ExpensesState {
    pub month: RwSignal<YearMonth>,
    /// Incrementado após qualquer escrita, para recarregar a lista.
    pub version: RwSignal<u32>,
    /// Lançamento carregado no formulário para edição.
    pub editing: RwSignal<Option<Transaction>>,
}

impl ExpensesState {
    pub fn reload(self) {
        self.version.update(|v| *v += 1);
    }
}

#[component]
pub fn Expenses() -> impl IntoView {
    let state = ExpensesState {
        month: RwSignal::new(current_month()),
        version: RwSignal::new(0),
        editing: RwSignal::new(None),
    };

    // Busca o mês inteiro; o filtro por tipo é aplicado na tela para o resumo
    // continuar mostrando receitas e despesas.
    let rows = LocalResource::new(move || {
        let month = state.month.get();
        state.version.track();
        ipc::list_transactions(TransactionFilter { period: Some(month), kind: None })
    });
    let loaded = Memo::new(move |_| rows.get());
    let all_rows = Signal::derive(move || loaded.get().and_then(Result::ok).unwrap_or_default());
    let load_error = Signal::derive(move || loaded.get().and_then(Result::err));

    let summary = Memo::new(move |_| all_rows.with(|rows| TransactionSummary::of(rows)));
    let kind_filter = RwSignal::new(None::<TransactionKind>);
    let visible_rows = Signal::derive(move || {
        let kind = kind_filter.get();
        all_rows.with(|rows| rows.iter().filter(|t| kind.is_none_or(|k| t.kind == k)).cloned().collect::<Vec<_>>())
    });

    // Sair do mês cancela a edição em andamento de um lançamento que deixou de estar visível.
    Effect::watch(
        move || state.month.get(),
        move |month, _, _| {
            if state.editing.with_untracked(|e| e.as_ref().is_some_and(|t| !month.contains(t.date))) {
                state.editing.set(None);
            }
        },
        false,
    );

    view! {
        <PageHeader title="Gastos" subtitle="Receitas e despesas do mês, em formato de planilha.">
            <MonthPicker month=state.month />
        </PageHeader>

        <div class="mb-6 grid grid-cols-1 gap-4 md:grid-cols-3">
            <StatCard
                label="Receitas"
                value=move || format_brl(summary.get().income.0)
                hint="Entradas do mês"
                value_class="text-emerald-600 dark:text-emerald-400"
            />
            <StatCard
                label="Despesas"
                value=move || format_brl(summary.get().expense.0)
                hint="Saídas do mês"
                value_class="text-red-600 dark:text-red-400"
            />
            <StatCard
                label="Saldo"
                value=move || format_brl(summary.get().balance().0)
                hint="Receitas − despesas"
                value_class=move || {
                    if summary.get().balance().is_sign_negative() {
                        "text-red-600 dark:text-red-400"
                    } else {
                        ""
                    }
                }
            />
        </div>

        <TransactionForm state />

        <TransactionTable
            state
            rows=visible_rows
            total=Signal::derive(move || summary.get().count)
            kind_filter
            loading=Signal::derive(move || loaded.get().is_none())
            load_error
        />
    }
}
