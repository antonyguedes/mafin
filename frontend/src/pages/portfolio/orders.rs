use leptos::ev::SubmitEvent;
use leptos::html;
use leptos::prelude::*;
use leptos::task::spawn_local;
use shared::chrono::NaiveDate;
use shared::format::{decimal_to_input, format_brl, format_date_br, format_quantity};
use shared::rust_decimal::Decimal;
use shared::{Asset, Money, NewOrder, Order, OrderKind, Quantity};

use super::{PortfolioState, Tab};
use crate::components::form::{
    DeleteButton, EditButton, INPUT, LABEL, PRIMARY_BUTTON, SECONDARY_BUTTON, SegmentOption, Segmented, form_card_class,
};
use crate::components::page::ErrorBanner;
use crate::components::table::{ROW, ROW_EDITING, TABLE_CARD, TD, TH};
use crate::ipc;
use crate::util::today;

fn asset_label(asset: &Asset) -> String {
    format!("{} · {}", asset.ticker, asset.broker)
}

/// Valor financeiro da ordem: compra = qtd × preço + taxas; venda = qtd × preço − taxas.
fn order_total(kind: OrderKind, quantity: Decimal, price: Decimal, fees: Decimal) -> Decimal {
    match kind {
        OrderKind::Buy => quantity * price + fees,
        OrderKind::Sell => quantity * price - fees,
    }
}

#[component]
pub(super) fn OrdersTab(state: PortfolioState) -> impl IntoView {
    let editing = RwSignal::new(None::<Order>);
    let action_error = RwSignal::new(None::<String>);
    // "" = todos os ativos; senão, o id do ativo.
    let asset_filter = RwSignal::new(String::new());

    let rows = Signal::derive(move || {
        let filter = asset_filter.get().parse::<i64>().ok();
        state.orders.with(|os| os.iter().filter(|o| filter.is_none_or(|id| o.asset_id == id)).cloned().collect::<Vec<_>>())
    });

    view! {
        <ErrorBanner message=state.load_error />

        <Show
            when=move || !state.assets.with(Vec::is_empty)
            fallback=move || {
                view! {
                    <div class="mb-6 rounded-xl border border-dashed border-slate-300 p-8 text-center text-sm text-slate-600 dark:border-slate-700 dark:text-slate-400">
                        <p>"Para registrar ordens, cadastre primeiro o ativo (ticker e corretora)."</p>
                        <button type="button" class=format!("{PRIMARY_BUTTON} mt-4") on:click=move |_| state.tab.set(Tab::Assets)>
                            "Cadastrar ativo"
                        </button>
                    </div>
                }
            }
        >
            <OrderForm state editing />
        </Show>

        <section class=TABLE_CARD>
            <div class="flex items-center justify-between border-b border-slate-200 px-4 py-3 dark:border-slate-800">
                <h2 class="text-sm font-semibold">
                    "Ordens " <span class="font-normal text-slate-500">{move || format!("· {}", rows.with(Vec::len))}</span>
                </h2>
                <select
                    class="rounded-lg border border-slate-300 bg-white px-2 py-1 text-xs dark:border-slate-700 dark:bg-slate-950"
                    aria-label="Filtrar por ativo"
                    bind:value=asset_filter
                >
                    <option value="">"Todos os ativos"</option>
                    {move || {
                        state
                            .assets
                            .get()
                            .into_iter()
                            .map(|a| view! { <option value=a.id.to_string()>{asset_label(&a)}</option> })
                            .collect_view()
                    }}
                </select>
            </div>
            <div class="px-4 pt-4 empty:hidden">
                <ErrorBanner message=action_error />
            </div>
            <table class="w-full text-sm">
                <thead class="bg-slate-50 dark:bg-slate-950/40">
                    <tr>
                        <th class=TH>"Data"</th>
                        <th class=TH>"Ativo"</th>
                        <th class=TH>"Tipo"</th>
                        <th class=format!("{TH} text-right")>"Qtd."</th>
                        <th class=format!("{TH} text-right")>"Preço"</th>
                        <th class=format!("{TH} text-right")>"Taxas"</th>
                        <th class=format!("{TH} text-right")>"Total"</th>
                        <th class=format!("{TH} text-right")>"Resultado"</th>
                        <th class=format!("{TH} text-right")><span class="sr-only">"Ações"</span></th>
                    </tr>
                </thead>
                <tbody class="divide-y divide-slate-100 dark:divide-slate-800">
                    <For
                        each=move || rows.get()
                        key=|o| (o.id, o.asset_id, o.kind, o.quantity, o.price, o.fees, o.date)
                        children=move |order| view! { <OrderRow order state editing action_error /> }
                    />
                </tbody>
            </table>
            <Show when=move || rows.with(Vec::is_empty)>
                <p class="px-4 py-10 text-center text-sm text-slate-500">"Nenhuma ordem registrada."</p>
            </Show>
        </section>
    }
}

#[component]
fn OrderForm(state: PortfolioState, editing: RwSignal<Option<Order>>) -> impl IntoView {
    let asset_id = RwSignal::new(String::new());
    let kind = RwSignal::new(OrderKind::Buy);
    let quantity = RwSignal::new(String::new());
    let price = RwSignal::new(String::new());
    let fees = RwSignal::new(String::from("0"));
    let date = RwSignal::new(today().to_string());
    let error = RwSignal::new(None::<String>);
    let saving = RwSignal::new(false);
    let form_ref = NodeRef::<html::Form>::new();

    // Pré-seleciona o primeiro ativo quando a lista chega.
    Effect::new(move |_| {
        let assets = state.assets.get();
        let current = asset_id.get_untracked();
        if !assets.iter().any(|a| a.id.to_string() == current)
            && let Some(first) = assets.first()
        {
            asset_id.set(first.id.to_string());
        }
    });

    Effect::new(move |_| {
        if let Some(order) = editing.get() {
            asset_id.set(order.asset_id.to_string());
            kind.set(order.kind);
            quantity.set(decimal_to_input(order.quantity.0));
            price.set(decimal_to_input(order.price.0));
            fees.set(decimal_to_input(order.fees.0));
            date.set(order.date.to_string());
            error.set(None);
            if let Some(form) = form_ref.get_untracked() {
                form.scroll_into_view();
            }
        }
    });

    // Mantém ativo, tipo e data para lançar notas com várias ordens.
    let clear = move || {
        quantity.set(String::new());
        price.set(String::new());
        fees.set(String::from("0"));
        error.set(None);
    };

    let parse = move || -> Result<NewOrder, String> {
        let asset_id = asset_id.get().parse::<i64>().map_err(|_| "Selecione um ativo.".to_string())?;
        let quantity = Quantity::parse_br(&quantity.get()).map_err(|_| "Quantidade inválida.".to_string())?;
        let price = Money::parse_br(&price.get()).map_err(|_| "Preço inválido. Ex.: 38,45".to_string())?;
        let fees_text = fees.get();
        let fees = if fees_text.trim().is_empty() {
            Money::ZERO
        } else {
            Money::parse_br(&fees_text).map_err(|_| "Taxas inválidas. Ex.: 4,90".to_string())?
        };
        let date = NaiveDate::parse_from_str(&date.get(), "%Y-%m-%d").map_err(|_| "Data inválida.".to_string())?;
        NewOrder { asset_id, kind: kind.get(), quantity, price, fees, date }.validated().map_err(|e| e.message)
    };

    // Prévia ao vivo do valor da operação.
    let preview = move || {
        parse().ok().map(|o| {
            let label = match o.kind {
                OrderKind::Buy => "Custo total",
                OrderKind::Sell => "Valor líquido",
            };
            format!("{label}: {}", format_brl(order_total(o.kind, o.quantity.0, o.price.0, o.fees.0)))
        })
    };

    let submit = move |ev: SubmitEvent| {
        ev.prevent_default();
        if saving.get_untracked() {
            return;
        }
        let input = match untrack(parse) {
            Ok(input) => input,
            Err(msg) => return error.set(Some(msg)),
        };
        saving.set(true);
        error.set(None);
        let editing_id = editing.get_untracked().map(|o| o.id);
        spawn_local(async move {
            let result = match editing_id {
                Some(id) => ipc::update_order(id, input).await,
                None => ipc::create_order(input).await,
            };
            saving.set(false);
            match result {
                Ok(_) => {
                    editing.set(None);
                    clear();
                    state.reload();
                }
                // Ex.: "Venda de 100 PETR4 em 06/09/2026 excede a posição de 50"
                Err(msg) => error.set(Some(msg)),
            }
        });
    };

    let is_editing = move || editing.with(Option::is_some);

    view! {
        <form node_ref=form_ref on:submit=submit class=move || form_card_class(is_editing())>
            <h2 class="mb-4 text-sm font-semibold">{move || if is_editing() { "Editando ordem" } else { "Nova ordem" }}</h2>
            <ErrorBanner message=error />
            <div class="grid grid-cols-2 gap-3 lg:grid-cols-12">
                <label class="col-span-2 lg:col-span-3">
                    <span class=LABEL>"Ativo"</span>
                    <select class=INPUT bind:value=asset_id>
                        {move || {
                            state
                                .assets
                                .get()
                                .into_iter()
                                .map(|a| view! { <option value=a.id.to_string()>{asset_label(&a)}</option> })
                                .collect_view()
                        }}
                    </select>
                </label>
                <div class="col-span-2 lg:col-span-2">
                    <span class=LABEL>"Operação"</span>
                    <Segmented
                        value=kind
                        options=vec![
                            SegmentOption::colored(OrderKind::Buy, "Compra", "text-emerald-600 dark:text-emerald-400"),
                            SegmentOption::colored(OrderKind::Sell, "Venda", "text-red-600 dark:text-red-400"),
                        ]
                    />
                </div>
                <label class="lg:col-span-2">
                    <span class=LABEL>"Quantidade"</span>
                    <input class=format!("{INPUT} tabular text-right") inputmode="decimal" placeholder="100" autocomplete="off" bind:value=quantity />
                </label>
                <label class="lg:col-span-2">
                    <span class=LABEL>"Preço unitário (R$)"</span>
                    <input class=format!("{INPUT} tabular text-right") inputmode="decimal" placeholder="0,00" autocomplete="off" bind:value=price />
                </label>
                <label class="lg:col-span-1">
                    <span class=LABEL>"Taxas (R$)"</span>
                    <input class=format!("{INPUT} tabular text-right") inputmode="decimal" autocomplete="off" bind:value=fees />
                </label>
                <label class="lg:col-span-2">
                    <span class=LABEL>"Data"</span>
                    <input class=INPUT type="date" required bind:value=date />
                </label>
            </div>
            <div class="mt-4 flex items-center justify-between gap-2">
                <p class="tabular text-sm text-slate-500 dark:text-slate-400">{preview}</p>
                <div class="flex gap-2">
                    <Show when=is_editing>
                        <button
                            type="button"
                            class=SECONDARY_BUTTON
                            on:click=move |_| {
                                editing.set(None);
                                clear();
                            }
                        >
                            "Cancelar"
                        </button>
                    </Show>
                    <button type="submit" class=PRIMARY_BUTTON disabled=move || saving.get()>
                        {move || match (saving.get(), is_editing()) {
                            (true, _) => "Salvando…",
                            (false, true) => "Salvar alterações",
                            (false, false) => "Registrar ordem",
                        }}
                    </button>
                </div>
            </div>
        </form>
    }
}

#[component]
fn OrderRow(
    order: Order,
    state: PortfolioState,
    editing: RwSignal<Option<Order>>,
    action_error: RwSignal<Option<String>>,
) -> impl IntoView {
    let id = order.id;
    let is_editing = move || editing.with(|e| e.as_ref().is_some_and(|o| o.id == id));
    let asset = move || state.asset(order.asset_id);
    let total = order_total(order.kind, order.quantity.0, order.price.0, order.fees.0);
    // Resultado realizado, calculado no backend pelo motor de posição.
    let result = move || {
        state.portfolio.with(|p| p.as_ref().and_then(|p| p.sales.iter().find(|s| s.order_id == id).map(|s| s.result.0)))
    };

    let (kind_class, kind_label) = match order.kind {
        OrderKind::Buy => ("bg-emerald-50 text-emerald-700 dark:bg-emerald-950 dark:text-emerald-300", "Compra"),
        OrderKind::Sell => ("bg-red-50 text-red-700 dark:bg-red-950 dark:text-red-300", "Venda"),
    };

    let edit = {
        let order = order.clone();
        move |()| editing.set(Some(order.clone()))
    };
    let delete = move |()| {
        spawn_local(async move {
            match ipc::delete_order(id).await {
                Ok(()) => {
                    action_error.set(None);
                    if is_editing() {
                        editing.set(None);
                    }
                    state.reload();
                }
                Err(msg) => action_error.set(Some(msg)),
            }
        });
    };

    view! {
        <tr class=move || if is_editing() { ROW_EDITING } else { ROW }>
            <td class=format!("{TD} tabular text-slate-600 dark:text-slate-400")>{format_date_br(order.date)}</td>
            <td class=TD>
                {move || match asset() {
                    Some(a) => view! {
                        <span class="font-medium">{a.ticker}</span>
                        <span class="ml-1 text-xs text-slate-400">{a.broker}</span>
                    }
                    .into_any(),
                    None => view! { <span class="text-slate-400">"—"</span> }.into_any(),
                }}
            </td>
            <td class=TD>
                <span class=format!("rounded-full px-2 py-0.5 text-xs font-medium {kind_class}")>{kind_label}</span>
            </td>
            <td class=format!("{TD} tabular text-right")>{format_quantity(order.quantity.0)}</td>
            <td class=format!("{TD} tabular text-right")>{format_brl(order.price.0)}</td>
            <td class=format!("{TD} tabular text-right text-slate-500")>{format_brl(order.fees.0)}</td>
            <td class=format!("{TD} tabular text-right font-medium")>{format_brl(total)}</td>
            <td class=format!("{TD} tabular text-right")>
                {move || match result() {
                    Some(r) if r.is_sign_negative() && !r.is_zero() => {
                        view! { <span class="text-red-600 dark:text-red-400">{format_brl(r)}</span> }.into_any()
                    }
                    Some(r) => view! { <span class="text-emerald-600 dark:text-emerald-400">{format!("+{}", format_brl(r))}</span> }.into_any(),
                    None => view! { <span class="text-slate-300 dark:text-slate-600">"—"</span> }.into_any(),
                }}
            </td>
            <td class=format!("{TD} text-right whitespace-nowrap")>
                <EditButton on_click=edit />
                <DeleteButton on_confirm=delete />
            </td>
        </tr>
    }
}
