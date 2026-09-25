use leptos::prelude::*;
use shared::YearMonth;

use crate::util::current_month;

/// Navegador de mês: ‹ Setembro de 2026 › [Mês atual]. Reutilizado na apuração de IR.
#[component]
pub fn MonthPicker(month: RwSignal<YearMonth>) -> impl IntoView {
    let is_current = move || month.get() == current_month();
    let arrow = "grid size-9 place-items-center rounded-lg text-slate-500 hover:bg-slate-100 hover:text-slate-900 dark:hover:bg-slate-800 dark:hover:text-white";

    view! {
        <div class="flex items-center gap-1">
            <button type="button" class=arrow aria-label="Mês anterior" on:click=move |_| month.update(|m| *m = m.prev())>
                "‹"
            </button>
            <span class="min-w-44 text-center text-sm font-medium">{move || month.get().label_pt()}</span>
            <button type="button" class=arrow aria-label="Próximo mês" on:click=move |_| month.update(|m| *m = m.next())>
                "›"
            </button>
            <button
                type="button"
                class="ml-2 rounded-lg border border-slate-200 px-3 py-1.5 text-xs font-medium text-slate-600 hover:bg-slate-100 disabled:opacity-40 disabled:hover:bg-transparent dark:border-slate-700 dark:text-slate-300 dark:hover:bg-slate-800"
                disabled=is_current
                on:click=move |_| month.set(current_month())
            >
                "Mês atual"
            </button>
        </div>
    }
}
