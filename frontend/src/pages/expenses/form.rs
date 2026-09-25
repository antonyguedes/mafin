use leptos::ev::SubmitEvent;
use leptos::html;
use leptos::prelude::*;
use leptos::task::spawn_local;
use shared::chrono::NaiveDate;
use shared::format::decimal_to_input;
use shared::{Money, NewTransaction, TransactionKind, YearMonth};

use super::ExpensesState;
use crate::components::form::{INPUT, LABEL, PRIMARY_BUTTON, SECONDARY_BUTTON, SegmentOption, Segmented, form_card_class};
use crate::components::page::ErrorBanner;
use crate::ipc;
use crate::util::today;

/// Sugestões iniciais; somam-se às categorias já usadas.
const DEFAULT_CATEGORIES: &[&str] = &[
    "Alimentação", "Educação", "Investimentos", "Lazer", "Mercado", "Moradia", "Salário", "Saúde", "Transporte",
];

/// Hoje, se estiver no mês selecionado; senão, o dia 1 do mês.
fn default_date(month: YearMonth) -> String {
    let today = today();
    let date = if month.contains(today) { Some(today) } else { month.first_day().ok() };
    date.map(|d| d.to_string()).unwrap_or_default()
}

#[component]
pub(super) fn TransactionForm(state: ExpensesState) -> impl IntoView {
    let kind = RwSignal::new(TransactionKind::Expense);
    let amount = RwSignal::new(String::new());
    let date = RwSignal::new(default_date(state.month.get_untracked()));
    let category = RwSignal::new(String::new());
    let description = RwSignal::new(String::new());
    let error = RwSignal::new(None::<String>);
    let saving = RwSignal::new(false);
    let form_ref = NodeRef::<html::Form>::new();
    let amount_ref = NodeRef::<html::Input>::new();

    let categories = LocalResource::new(move || {
        state.version.track();
        ipc::list_categories()
    });
    let suggestions = move || {
        let mut all: Vec<String> = DEFAULT_CATEGORIES.iter().map(|c| c.to_string()).collect();
        if let Some(Ok(used)) = categories.get() {
            for c in used {
                if !all.iter().any(|a| a.eq_ignore_ascii_case(&c)) {
                    all.push(c);
                }
            }
        }
        all.sort_by_key(|c| c.to_lowercase());
        all
    };

    // Mantém tipo e data para lançar vários itens seguidos rapidamente.
    let clear = move || {
        amount.set(String::new());
        category.set(String::new());
        description.set(String::new());
        error.set(None);
    };

    // Clicar em "Editar" na tabela carrega o lançamento aqui.
    Effect::new(move |_| {
        if let Some(tx) = state.editing.get() {
            kind.set(tx.kind);
            amount.set(decimal_to_input(tx.amount.0));
            date.set(tx.date.to_string());
            category.set(tx.category);
            description.set(tx.description.unwrap_or_default());
            error.set(None);
            if let Some(form) = form_ref.get_untracked() {
                form.scroll_into_view();
            }
            if let Some(input) = amount_ref.get_untracked() {
                let _ = input.focus();
            }
        }
    });

    // Trocar de mês move a data padrão para dentro do novo mês.
    Effect::watch(
        move || state.month.get(),
        move |month, _, _| {
            let inside = NaiveDate::parse_from_str(&date.get_untracked(), "%Y-%m-%d").is_ok_and(|d| month.contains(d));
            if state.editing.get_untracked().is_none() && !inside {
                date.set(default_date(*month));
            }
        },
        false,
    );

    let cancel = move |_| {
        state.editing.set(None);
        clear();
    };

    let submit = move |ev: SubmitEvent| {
        ev.prevent_default();
        if saving.get_untracked() {
            return;
        }

        let input = (|| {
            let amount = Money::parse_br(&amount.get_untracked())
                .map_err(|_| "Valor inválido. Exemplos: 59,90 ou 1.234,56".to_string())?;
            let date = NaiveDate::parse_from_str(&date.get_untracked(), "%Y-%m-%d")
                .map_err(|_| "Data inválida.".to_string())?;
            NewTransaction {
                kind: kind.get_untracked(),
                amount,
                date,
                category: category.get_untracked(),
                description: Some(description.get_untracked()),
            }
            // Mesma validação do backend, para feedback imediato.
            .validated()
            .map_err(|e| e.message)
        })();

        let input = match input {
            Ok(input) => input,
            Err(msg) => return error.set(Some(msg)),
        };

        saving.set(true);
        error.set(None);
        let editing_id = state.editing.get_untracked().map(|t| t.id);
        spawn_local(async move {
            let result = match editing_id {
                Some(id) => ipc::update_transaction(id, input).await,
                None => ipc::create_transaction(input).await,
            };
            saving.set(false);
            match result {
                Ok(saved) => {
                    state.editing.set(None);
                    clear();
                    // Lançou em outro mês? Vai para ele, para o usuário ver o registro.
                    state.month.set(YearMonth::of(saved.date));
                    state.reload();
                    if let Some(input) = amount_ref.get_untracked() {
                        let _ = input.focus();
                    }
                }
                Err(msg) => error.set(Some(msg)),
            }
        });
    };

    let is_editing = move || state.editing.with(Option::is_some);
    view! {
        <form
            node_ref=form_ref
            on:submit=submit
            class=move || form_card_class(is_editing())
        >
            <div class="mb-4 flex items-center justify-between">
                <h2 class="text-sm font-semibold">
                    {move || if is_editing() { "Editando lançamento" } else { "Novo lançamento" }}
                </h2>
            </div>

            <ErrorBanner message=error />

            <div class="grid grid-cols-2 gap-3 lg:grid-cols-12">
                <div class="col-span-2 lg:col-span-3">
                    <span class=LABEL>"Tipo"</span>
                    <Segmented
                        value=kind
                        options=vec![
                            SegmentOption::colored(TransactionKind::Expense, "Despesa", "text-red-600 dark:text-red-400"),
                            SegmentOption::colored(TransactionKind::Income, "Receita", "text-emerald-600 dark:text-emerald-400"),
                        ]
                    />
                </div>
                <label class="lg:col-span-2">
                    <span class=LABEL>"Valor (R$)"</span>
                    <input
                        node_ref=amount_ref
                        class=format!("{INPUT} tabular text-right")
                        inputmode="decimal"
                        placeholder="0,00"
                        autocomplete="off"
                        bind:value=amount
                    />
                </label>
                <label class="lg:col-span-2">
                    <span class=LABEL>"Data"</span>
                    <input class=INPUT type="date" required bind:value=date />
                </label>
                <label class="lg:col-span-2">
                    <span class=LABEL>"Categoria"</span>
                    <input class=INPUT list="expense-categories" placeholder="Ex.: Mercado" autocomplete="off" bind:value=category />
                    <datalist id="expense-categories">
                        {move || suggestions().into_iter().map(|c| view! { <option value=c></option> }).collect_view()}
                    </datalist>
                </label>
                <label class="col-span-2 lg:col-span-3">
                    <span class=LABEL>"Descrição (opcional)"</span>
                    <input class=INPUT placeholder="Ex.: Compra da semana" autocomplete="off" bind:value=description />
                </label>
            </div>

            <div class="mt-4 flex justify-end gap-2">
                <Show when=is_editing>
                    <button
                        type="button"
                        class=SECONDARY_BUTTON
                        on:click=cancel
                    >
                        "Cancelar"
                    </button>
                </Show>
                <button
                    type="submit"
                    class=PRIMARY_BUTTON
                    disabled=move || saving.get()
                >
                    {move || match (saving.get(), is_editing()) {
                        (true, _) => "Salvando…",
                        (false, true) => "Salvar alterações",
                        (false, false) => "Adicionar",
                    }}
                </button>
            </div>
        </form>
    }
}
