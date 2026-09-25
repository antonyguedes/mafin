use leptos::prelude::*;
use leptos_router::components::A;
use shared::{PingRequest, rust_decimal::Decimal};

use crate::app::routes;
use crate::components::icons::{Icon, IconKind};
use crate::ipc;

struct NavItem {
    href: &'static str,
    label: &'static str,
    icon: IconKind,
}

const NAV_ITEMS: [NavItem; 4] = [
    NavItem { href: routes::DASHBOARD, label: "Dashboard", icon: IconKind::Dashboard },
    NavItem { href: routes::EXPENSES, label: "Gastos", icon: IconKind::Expenses },
    NavItem { href: routes::PORTFOLIO, label: "Carteira", icon: IconKind::Portfolio },
    NavItem { href: routes::TAX, label: "Imposto de Renda", icon: IconKind::Tax },
];

/// Casca da aplicação: sidebar fixa à esquerda + área de conteúdo com rolagem própria.
#[component]
pub fn Layout(children: Children) -> impl IntoView {
    view! {
        <div class="flex h-full">
            <Sidebar />
            <main class="min-w-0 flex-1 overflow-y-auto">
                <div class="mx-auto max-w-6xl px-8 py-8">{children()}</div>
            </main>
        </div>
    }
}

#[component]
fn Sidebar() -> impl IntoView {
    view! {
        <aside class="flex w-60 shrink-0 flex-col border-r border-slate-800 bg-slate-900 text-slate-300">
            <div class="flex items-center gap-2.5 px-5 py-5">
                <span class="grid size-8 place-items-center rounded-lg bg-brand-600 text-white">
                    <Icon kind=IconKind::Logo class="size-5" />
                </span>
                <span class="text-lg font-semibold tracking-tight text-white">"Mafin"</span>
            </div>

            <nav class="flex-1 space-y-1 px-3" aria-label="Navegação principal">
                {NAV_ITEMS
                    .iter()
                    .map(|item| {
                        view! {
                            // `<A>` marca o link ativo com aria-current="page"; o estilo sai daí.
                            // `exact` só no Dashboard: "/" é prefixo de todas as rotas.
                            <A
                                href=item.href
                                exact=item.href == routes::DASHBOARD
                                attr:class="flex items-center gap-3 rounded-lg px-3 py-2 text-sm font-medium transition-colors hover:bg-slate-800 hover:text-white aria-[current=page]:bg-brand-600 aria-[current=page]:text-white"
                            >
                                <Icon kind=item.icon />
                                <span>{item.label}</span>
                            </A>
                        }
                    })
                    .collect_view()}
            </nav>

            <BackendStatus />
        </aside>
    }
}

/// Rodapé da sidebar: faz um `ping` no backend para mostrar se o IPC está de pé.
#[component]
fn BackendStatus() -> impl IntoView {
    let status = LocalResource::new(|| {
        ipc::ping(PingRequest { message: "status".into(), amount: Decimal::ZERO })
    });

    view! {
        <div class="border-t border-slate-800 px-5 py-4 text-xs">
            {move || match status.get() {
                None => view! { <span class="text-slate-500">"Conectando…"</span> }.into_any(),
                Some(Ok(resp)) => view! {
                    <span class="flex items-center gap-2 text-slate-400">
                        <span class="size-2 rounded-full bg-brand-500"></span>
                        {format!("Offline · v{}", resp.backend_version)}
                    </span>
                }
                .into_any(),
                Some(Err(err)) => view! {
                    <span class="flex items-center gap-2 text-red-400" title=err>
                        <span class="size-2 rounded-full bg-red-500"></span>
                        "Backend indisponível"
                    </span>
                }
                .into_any(),
            }}
        </div>
    }
}
