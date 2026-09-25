//! Blocos reutilizáveis de página.

use leptos::prelude::*;

/// Título da página, com área opcional de ações à direita (filtros, botões).
#[component]
pub fn PageHeader(
    title: &'static str,
    subtitle: &'static str,
    #[prop(optional)] children: Option<Children>,
) -> impl IntoView {
    view! {
        <header class="mb-8 flex flex-wrap items-end justify-between gap-4">
            <div>
                <h1 class="text-2xl font-semibold tracking-tight">{title}</h1>
                <p class="mt-1 text-sm text-slate-500 dark:text-slate-400">{subtitle}</p>
            </div>
            {children.map(|c| c())}
        </header>
    }
}

#[component]
pub fn Card(children: Children, #[prop(optional)] class: &'static str) -> impl IntoView {
    view! {
        <section class=format!(
            "rounded-xl border border-slate-200 bg-white p-6 shadow-sm dark:border-slate-800 dark:bg-slate-900 {class}",
        )>{children()}</section>
    }
}

/// Cartão de métrica. `value`, `hint` e `value_class` podem ser reativos.
#[component]
pub fn StatCard(
    label: &'static str,
    #[prop(into)] value: Signal<String>,
    #[prop(into)] hint: Signal<String>,
    #[prop(into, default = Signal::stored(""))] value_class: Signal<&'static str>,
) -> impl IntoView {
    view! {
        <Card>
            <p class="text-sm text-slate-500 dark:text-slate-400">{label}</p>
            <p class=move || format!("tabular mt-2 text-2xl font-semibold {}", value_class.get())>{value}</p>
            <p class="mt-1 text-xs text-slate-400 dark:text-slate-500">{hint}</p>
        </Card>
    }
}

/// Faixa de erro (ex.: falha de IPC).
#[component]
pub fn ErrorBanner(#[prop(into)] message: Signal<Option<String>>) -> impl IntoView {
    move || {
        message.get().map(|msg| {
            view! {
                <div
                    role="alert"
                    class="mb-4 rounded-lg border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-700 dark:border-red-900 dark:bg-red-950 dark:text-red-300"
                >
                    {msg}
                </div>
            }
        })
    }
}
