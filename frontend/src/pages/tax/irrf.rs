use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use shared::format::{decimal_to_input, format_brl};
use shared::tax::MonthlyTax;
use shared::{Money, YearMonth};

use crate::components::form::{INPUT, PRIMARY_BUTTON};
use crate::components::page::{Card, ErrorBanner};
use crate::ipc;

/// IRRF ("dedo-duro") retido no mês, digitado a partir das notas de corretagem.
#[component]
pub(super) fn IrrfForm(month: RwSignal<YearMonth>, selected: Signal<Option<MonthlyTax>>, version: RwSignal<u32>) -> impl IntoView {
    let amount = RwSignal::new(String::new());
    let error = RwSignal::new(None::<String>);
    let saved = RwSignal::new(false);
    let saving = RwSignal::new(false);

    // Carrega o valor gravado ao trocar de mês (ou quando o relatório recarrega).
    Effect::new(move |_| {
        let value = selected.get().map(|m| m.irrf.0).unwrap_or_default();
        amount.set(if value.is_zero() { String::new() } else { decimal_to_input(value) });
        error.set(None);
    });
    Effect::watch(move || month.get(), move |_, _, _| saved.set(false), false);

    let submit = move |ev: SubmitEvent| {
        ev.prevent_default();
        let text = amount.get_untracked();
        let value = if text.trim().is_empty() {
            Money::ZERO
        } else {
            match Money::parse_br(&text) {
                Ok(v) => v,
                Err(_) => return error.set(Some("Valor inválido. Ex.: 1,49".into())),
            }
        };
        saving.set(true);
        let m = month.get_untracked();
        spawn_local(async move {
            let result = ipc::set_monthly_irrf(m, value).await;
            saving.set(false);
            match result {
                Ok(()) => {
                    error.set(None);
                    saved.set(true);
                    version.update(|v| *v += 1);
                }
                Err(msg) => error.set(Some(msg)),
            }
        });
    };

    let estimate = move || selected.get().map(|m| m.irrf_estimate.0).filter(|e| !e.is_zero());
    let credit = move || selected.get().map(|m| m.irrf_credit_after.0).filter(|c| !c.is_zero());

    view! {
        <Card class="mb-6">
            <form on:submit=submit class="flex flex-wrap items-end gap-4">
                <div class="min-w-64 flex-1">
                    <h2 class="text-sm font-semibold">"IRRF retido no mês"</h2>
                    <p class="mt-1 text-xs text-slate-500 dark:text-slate-400">
                        "Some o “IRRF” das notas de corretagem do mês (0,005% nas vendas comuns, 1% no lucro de day trade). Ele é abatido do DARF."
                    </p>
                    <p class="tabular mt-1 text-xs text-slate-400 dark:text-slate-500">
                        {move || estimate().map(|e| format!("Estimativa pelas suas ordens: {}", format_brl(e)))}
                        {move || credit().map(|c| format!(" · Crédito para os próximos meses: {}", format_brl(c)))}
                    </p>
                </div>
                <label class="w-40">
                    <span class="sr-only">"IRRF (R$)"</span>
                    <input class=format!("{INPUT} tabular text-right") inputmode="decimal" placeholder="0,00" aria-label="IRRF do mês" bind:value=amount />
                </label>
                <button type="submit" class=PRIMARY_BUTTON disabled=move || saving.get()>
                    {move || if saving.get() { "Salvando…" } else if saved.get() { "Salvo ✓" } else { "Salvar IRRF" }}
                </button>
            </form>
            <div class="mt-3 empty:hidden">
                <ErrorBanner message=error />
            </div>
        </Card>
    }
}
