use leptos::prelude::*;
use leptos::task::spawn_local;
use shared::format::{format_brl, format_date_br};
use shared::{Transaction, TransactionKind};

use super::ExpensesState;
use crate::components::form::{DeleteButton, EditButton, SegmentOption, Segmented};
use crate::components::page::ErrorBanner;
use crate::components::table::{ROW, ROW_EDITING, TABLE_CARD, TD, TH};
use crate::ipc;

#[component]
pub(super) fn TransactionTable(
    state: ExpensesState,
    /// Linhas já filtradas por tipo.
    rows: Signal<Vec<Transaction>>,
    /// Total de lançamentos do mês (sem filtro de tipo).
    total: Signal<usize>,
    kind_filter: RwSignal<Option<TransactionKind>>,
    loading: Signal<bool>,
    load_error: Signal<Option<String>>,
) -> impl IntoView {
    let action_error = RwSignal::new(None::<String>);

    view! {
        <section class=TABLE_CARD>
            <div class="flex items-center justify-between border-b border-slate-200 px-4 py-3 dark:border-slate-800">
                <h2 class="text-sm font-semibold">
                    "Lançamentos "
                    <span class="font-normal text-slate-500">{move || format!("· {} no mês", total.get())}</span>
                </h2>
                <Segmented
                    value=kind_filter
                    small=true
                    options=vec![
                        SegmentOption::new(None, "Todos"),
                        SegmentOption::new(Some(TransactionKind::Income), "Receitas"),
                        SegmentOption::new(Some(TransactionKind::Expense), "Despesas"),
                    ]
                />
            </div>

            <div class="px-4 pt-4 empty:hidden">
                <ErrorBanner message=load_error />
                <ErrorBanner message=action_error />
            </div>

            <table class="w-full text-sm">
                <thead class="bg-slate-50 dark:bg-slate-950/40">
                    <tr>
                        <th class=format!("{TH} w-28")>"Data"</th>
                        <th class=TH>"Descrição"</th>
                        <th class=format!("{TH} w-40")>"Categoria"</th>
                        <th class=format!("{TH} w-40 text-right")>"Valor"</th>
                        <th class=format!("{TH} w-40 text-right")><span class="sr-only">"Ações"</span></th>
                    </tr>
                </thead>
                <tbody class="divide-y divide-slate-100 dark:divide-slate-800">
                    <For
                        each=move || rows.get()
                        key=|tx| tx.clone()
                        children=move |tx| view! { <Row tx state action_error /> }
                    />
                </tbody>
            </table>

            <Show when=move || rows.with(Vec::is_empty)>
                <p class="px-4 py-10 text-center text-sm text-slate-500">
                    {move || {
                        if loading.get() {
                            "Carregando…".to_string()
                        } else if total.get() > 0 {
                            "Nenhum lançamento deste tipo no mês.".to_string()
                        } else {
                            format!("Nenhum lançamento em {}.", state.month.get().label_pt())
                        }
                    }}
                </p>
            </Show>
        </section>
    }
}

#[component]
fn Row(tx: Transaction, state: ExpensesState, action_error: RwSignal<Option<String>>) -> impl IntoView {
    let id = tx.id;
    let is_editing = move || state.editing.with(|e| e.as_ref().is_some_and(|t| t.id == id));

    let (sign, amount_class) = match tx.kind {
        TransactionKind::Income => ("+ ", "text-emerald-600 dark:text-emerald-400"),
        TransactionKind::Expense => ("− ", "text-red-600 dark:text-red-400"),
    };

    let edit = {
        let tx = tx.clone();
        move |()| state.editing.set(Some(tx.clone()))
    };

    let delete = move |()| {
        spawn_local(async move {
            match ipc::delete_transaction(id).await {
                Ok(()) => {
                    action_error.set(None);
                    if is_editing() {
                        state.editing.set(None);
                    }
                    state.reload();
                }
                Err(msg) => action_error.set(Some(msg)),
            }
        });
    };

    view! {
        <tr class=move || if is_editing() { ROW_EDITING } else { ROW }>
            <td class=format!("{TD} tabular text-slate-600 dark:text-slate-400")>{format_date_br(tx.date)}</td>
            <td class=TD>
                {match tx.description.clone() {
                    Some(text) => view! { <span>{text}</span> }.into_any(),
                    None => view! { <span class="text-slate-400">"—"</span> }.into_any(),
                }}
            </td>
            <td class=TD>
                <span class="rounded-full bg-slate-100 px-2 py-0.5 text-xs text-slate-700 dark:bg-slate-800 dark:text-slate-300">
                    {tx.category.clone()}
                </span>
            </td>
            <td class=format!("{TD} tabular text-right font-medium {amount_class}")>{sign}{format_brl(tx.amount.0)}</td>
            <td class=format!("{TD} text-right whitespace-nowrap")>
                <EditButton on_click=edit />
                <DeleteButton on_confirm=delete />
            </td>
        </tr>
    }
}
