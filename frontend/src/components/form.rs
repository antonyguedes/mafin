//! Controles de formulário compartilhados entre as telas.

use leptos::prelude::*;

pub const INPUT: &str = "w-full rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm shadow-xs outline-none focus:border-brand-500 focus:ring-2 focus:ring-brand-500/20 dark:border-slate-700 dark:bg-slate-950";
pub const LABEL: &str = "mb-1 block text-xs font-medium text-slate-600 dark:text-slate-400";
pub const PRIMARY_BUTTON: &str = "rounded-lg bg-brand-600 px-4 py-2 text-sm font-medium text-white shadow-sm hover:bg-brand-700 disabled:opacity-60";
pub const SECONDARY_BUTTON: &str = "rounded-lg px-4 py-2 text-sm font-medium text-slate-600 hover:bg-slate-100 dark:text-slate-300 dark:hover:bg-slate-800";

/// Classes do cartão de formulário; destaca em âmbar durante uma edição.
pub fn form_card_class(editing: bool) -> String {
    let ring = if editing { "ring-2 ring-amber-400/60" } else { "" };
    format!("mb-6 rounded-xl border border-slate-200 bg-white p-5 shadow-sm dark:border-slate-800 dark:bg-slate-900 {ring}")
}

/// Uma opção do [`Segmented`]: valor, rótulo e classes quando ativa.
pub struct SegmentOption<T> {
    pub value: T,
    pub label: &'static str,
    pub active_class: &'static str,
}

impl<T> SegmentOption<T> {
    pub fn new(value: T, label: &'static str) -> Self {
        Self { value, label, active_class: "text-slate-900 dark:text-white" }
    }

    pub fn colored(value: T, label: &'static str, active_class: &'static str) -> Self {
        Self { value, label, active_class }
    }
}

/// Controle segmentado (grupo de botões exclusivos) ligado a um signal.
#[component]
pub fn Segmented<T>(
    value: RwSignal<T>,
    options: Vec<SegmentOption<T>>,
    /// Versão compacta, para filtros em cabeçalhos de tabela.
    #[prop(optional)]
    small: bool,
    #[prop(optional)] class: &'static str,
) -> impl IntoView
where
    T: Copy + PartialEq + Send + Sync + 'static,
{
    let size = if small { "px-3 py-1 text-xs" } else { "flex-1 px-3 py-1.5 text-sm" };
    view! {
        <div class=format!("flex gap-1 rounded-lg bg-slate-100 p-1 dark:bg-slate-800 {class}")>
            {options
                .into_iter()
                .map(|opt| {
                    let SegmentOption { value: option, label, active_class } = opt;
                    view! {
                        <button
                            type="button"
                            class=move || {
                                let base = format!("rounded-md font-medium transition-colors {size}");
                                if value.get() == option {
                                    format!("{base} bg-white shadow-sm dark:bg-slate-950 {active_class}")
                                } else {
                                    format!("{base} text-slate-500 hover:text-slate-900 dark:hover:text-white")
                                }
                            }
                            aria-pressed=move || (value.get() == option).to_string()
                            on:click=move |_| value.set(option)
                        >
                            {label}
                        </button>
                    }
                })
                .collect_view()}
        </div>
    }
}

pub const ROW_ACTION: &str = "rounded-md px-2 py-1 text-xs font-medium transition-colors";

/// Exclusão em dois cliques ("Excluir" → "Confirmar"), sem diálogos nativos
/// (que não são confiáveis em todas as plataformas do Tauri).
#[component]
pub fn DeleteButton(
    #[prop(into)] on_confirm: Callback<()>,
    #[prop(default = "Excluir")] label: &'static str,
) -> impl IntoView {
    let confirming = RwSignal::new(false);
    view! {
        <button
            type="button"
            class=move || {
                if confirming.get() {
                    format!("{ROW_ACTION} bg-red-600 text-white hover:bg-red-700")
                } else {
                    format!("{ROW_ACTION} text-slate-500 hover:bg-red-50 hover:text-red-600 dark:hover:bg-red-950")
                }
            }
            on:click=move |_| {
                if confirming.get_untracked() {
                    confirming.set(false);
                    on_confirm.run(());
                } else {
                    confirming.set(true);
                }
            }
            on:blur=move |_| confirming.set(false)
        >
            {move || if confirming.get() { "Confirmar" } else { label }}
        </button>
    }
}

/// Botão "Editar" de linha de tabela.
#[component]
pub fn EditButton(#[prop(into)] on_click: Callback<()>) -> impl IntoView {
    view! {
        <button
            type="button"
            class=format!("{ROW_ACTION} text-slate-500 hover:bg-slate-100 hover:text-slate-900 dark:hover:bg-slate-800 dark:hover:text-white")
            on:click=move |_| on_click.run(())
        >
            "Editar"
        </button>
    }
}
