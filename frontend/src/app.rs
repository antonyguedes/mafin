use leptos::prelude::*;
use leptos_router::components::{Route, Router, Routes};
use leptos_router::path;

use crate::components::layout::Layout;
use crate::pages::{Dashboard, Expenses, NotFound, Portfolio, Tax};

/// Rotas da aplicação. Mantenha em sincronia com `components::layout::NAV_ITEMS`.
pub mod routes {
    pub const DASHBOARD: &str = "/";
    pub const EXPENSES: &str = "/gastos";
    pub const PORTFOLIO: &str = "/carteira";
    pub const TAX: &str = "/imposto-de-renda";
}

#[component]
pub fn App() -> impl IntoView {
    view! {
        // Roteamento por histórico (URLs limpas). Funciona no `trunk serve` (fallback SPA)
        // e no build do Tauri, que devolve index.html para caminhos desconhecidos.
        <Router>
            <Layout>
                <Routes fallback=NotFound>
                    <Route path=path!("/") view=Dashboard />
                    <Route path=path!("/gastos") view=Expenses />
                    <Route path=path!("/carteira") view=Portfolio />
                    <Route path=path!("/imposto-de-renda") view=Tax />
                </Routes>
            </Layout>
        </Router>
    }
}
