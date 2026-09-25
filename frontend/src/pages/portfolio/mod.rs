//! Carteira: custódia atual, ordens de compra/venda e cadastro de ativos.

mod assets;
mod custody;
mod orders;

use leptos::prelude::*;
use shared::{Asset, Order, Portfolio};

use crate::components::form::{SegmentOption, Segmented};
use crate::components::page::PageHeader;
use crate::ipc;
use assets::AssetsTab;
use custody::CustodyTab;
use orders::OrdersTab;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tab {
    Custody,
    Orders,
    Assets,
}

/// Estado compartilhado entre as abas. `Copy`: só contém signals/recursos.
#[derive(Clone, Copy)]
pub(crate) struct PortfolioState {
    pub tab: RwSignal<Tab>,
    /// Incrementado após qualquer escrita, para recarregar ativos, ordens e custódia.
    pub version: RwSignal<u32>,
    pub assets: Signal<Vec<Asset>>,
    pub orders: Signal<Vec<Order>>,
    pub portfolio: Signal<Option<Portfolio>>,
    /// Erro de carga de qualquer um dos três recursos.
    pub load_error: Signal<Option<String>>,
    pub loading: Signal<bool>,
}

impl PortfolioState {
    pub fn reload(self) {
        self.version.update(|v| *v += 1);
    }

    pub fn asset(self, id: i64) -> Option<Asset> {
        self.assets.with(|all| all.iter().find(|a| a.id == id).cloned())
    }
}

#[component]
pub fn Portfolio() -> impl IntoView {
    let version = RwSignal::new(0u32);
    let assets_res = LocalResource::new(move || {
        version.track();
        ipc::list_assets()
    });
    let orders_res = LocalResource::new(move || {
        version.track();
        ipc::list_orders()
    });
    let portfolio_res = LocalResource::new(move || {
        version.track();
        ipc::get_portfolio()
    });

    let state = PortfolioState {
        tab: RwSignal::new(Tab::Custody),
        version,
        assets: Signal::derive(move || assets_res.get().and_then(Result::ok).unwrap_or_default()),
        orders: Signal::derive(move || orders_res.get().and_then(Result::ok).unwrap_or_default()),
        portfolio: Signal::derive(move || portfolio_res.get().and_then(Result::ok)),
        load_error: Signal::derive(move || {
            [assets_res.get().and_then(Result::err), orders_res.get().and_then(Result::err), portfolio_res.get().and_then(Result::err)]
                .into_iter()
                .flatten()
                .next()
        }),
        loading: Signal::derive(move || portfolio_res.get().is_none()),
    };

    view! {
        <PageHeader title="Carteira" subtitle="Custódia a preço médio, ordens de compra/venda e ativos.">
            <Segmented
                value=state.tab
                class="w-80"
                options=vec![
                    SegmentOption::new(Tab::Custody, "Custódia"),
                    SegmentOption::new(Tab::Orders, "Ordens"),
                    SegmentOption::new(Tab::Assets, "Ativos"),
                ]
            />
        </PageHeader>

        {move || match state.tab.get() {
            Tab::Custody => view! { <CustodyTab state /> }.into_any(),
            Tab::Orders => view! { <OrdersTab state /> }.into_any(),
            Tab::Assets => view! { <AssetsTab state /> }.into_any(),
        }}
    }
}
