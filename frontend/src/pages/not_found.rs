use leptos::prelude::*;
use leptos_router::components::A;

use crate::app::routes;
use crate::components::page::PageHeader;

#[component]
pub fn NotFound() -> impl IntoView {
    view! {
        <PageHeader title="Página não encontrada" subtitle="O endereço acessado não existe." />
        <A href=routes::DASHBOARD attr:class="text-sm font-medium text-brand-600 hover:underline">
            "Voltar ao Dashboard"
        </A>
    }
}
