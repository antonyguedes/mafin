use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use shared::{Asset, AssetType, NewAsset};

use super::PortfolioState;
use crate::components::allocation::asset_type_color;
use crate::components::form::{
    DeleteButton, EditButton, INPUT, LABEL, PRIMARY_BUTTON, SECONDARY_BUTTON, SegmentOption, Segmented, form_card_class,
};
use crate::components::page::ErrorBanner;
use crate::components::table::{ROW, ROW_EDITING, TABLE_CARD, TD, TH};
use crate::ipc;

const COMMON_BROKERS: &[&str] = &["BTG Pactual", "Clear", "Inter", "Itaú", "NuInvest", "Rico", "XP"];

#[component]
pub(super) fn AssetsTab(state: PortfolioState) -> impl IntoView {
    let editing = RwSignal::new(None::<Asset>);
    let action_error = RwSignal::new(None::<String>);

    view! {
        <ErrorBanner message=state.load_error />
        <AssetForm state editing />

        <section class=TABLE_CARD>
            <div class="border-b border-slate-200 px-4 py-3 dark:border-slate-800">
                <h2 class="text-sm font-semibold">
                    "Ativos cadastrados "
                    <span class="font-normal text-slate-500">{move || format!("· {}", state.assets.with(Vec::len))}</span>
                </h2>
            </div>
            <div class="px-4 pt-4 empty:hidden">
                <ErrorBanner message=action_error />
            </div>
            <table class="w-full text-sm">
                <thead class="bg-slate-50 dark:bg-slate-950/40">
                    <tr>
                        <th class=TH>"Ticker"</th>
                        <th class=TH>"Tipo"</th>
                        <th class=TH>"Corretora"</th>
                        <th class=format!("{TH} text-right")>"Ordens"</th>
                        <th class=format!("{TH} w-40 text-right")><span class="sr-only">"Ações"</span></th>
                    </tr>
                </thead>
                <tbody class="divide-y divide-slate-100 dark:divide-slate-800">
                    <For
                        each=move || state.assets.get()
                        key=|a| a.clone()
                        children=move |asset| view! { <AssetRow asset state editing action_error /> }
                    />
                </tbody>
            </table>
            <Show when=move || state.assets.with(Vec::is_empty)>
                <p class="px-4 py-10 text-center text-sm text-slate-500">
                    "Nenhum ativo cadastrado. Comece pelo formulário acima (ex.: PETR4 na XP)."
                </p>
            </Show>
        </section>
    }
}

#[component]
fn AssetForm(state: PortfolioState, editing: RwSignal<Option<Asset>>) -> impl IntoView {
    let ticker = RwSignal::new(String::new());
    let asset_type = RwSignal::new(AssetType::Stock);
    let broker = RwSignal::new(String::new());
    let error = RwSignal::new(None::<String>);
    let saving = RwSignal::new(false);

    let brokers = move || {
        let mut all: Vec<String> = COMMON_BROKERS.iter().map(|b| b.to_string()).collect();
        for asset in state.assets.get() {
            if !all.iter().any(|b| b.eq_ignore_ascii_case(&asset.broker)) {
                all.push(asset.broker);
            }
        }
        all.sort_by_key(|b| b.to_lowercase());
        all
    };

    // Mantém tipo e corretora: é comum cadastrar vários ativos da mesma corretora.
    let clear = move || {
        ticker.set(String::new());
        error.set(None);
    };

    Effect::new(move |_| {
        if let Some(asset) = editing.get() {
            ticker.set(asset.ticker);
            asset_type.set(asset.asset_type);
            broker.set(asset.broker);
            error.set(None);
        }
    });

    let submit = move |ev: SubmitEvent| {
        ev.prevent_default();
        if saving.get_untracked() {
            return;
        }
        let input = NewAsset { ticker: ticker.get_untracked(), asset_type: asset_type.get_untracked(), broker: broker.get_untracked() };
        let input = match input.validated() {
            Ok(input) => input,
            Err(e) => return error.set(Some(e.message)),
        };
        saving.set(true);
        error.set(None);
        let editing_id = editing.get_untracked().map(|a| a.id);
        spawn_local(async move {
            let result = match editing_id {
                Some(id) => ipc::update_asset(id, input).await,
                None => ipc::create_asset(input).await,
            };
            saving.set(false);
            match result {
                Ok(_) => {
                    editing.set(None);
                    clear();
                    state.reload();
                }
                Err(msg) => error.set(Some(msg)),
            }
        });
    };

    let is_editing = move || editing.with(Option::is_some);

    view! {
        <form on:submit=submit class=move || form_card_class(is_editing())>
            <h2 class="mb-4 text-sm font-semibold">{move || if is_editing() { "Editando ativo" } else { "Novo ativo" }}</h2>
            <ErrorBanner message=error />
            <div class="grid grid-cols-2 gap-3 lg:grid-cols-12">
                <label class="lg:col-span-3">
                    <span class=LABEL>"Ticker"</span>
                    <input class=format!("{INPUT} uppercase") placeholder="Ex.: PETR4" autocomplete="off" bind:value=ticker />
                </label>
                <div class="lg:col-span-5">
                    <span class=LABEL>"Tipo"</span>
                    <Segmented
                        value=asset_type
                        options=AssetType::ALL.into_iter().map(|t| SegmentOption::new(t, t.label_pt())).collect()
                    />
                </div>
                <label class="col-span-2 lg:col-span-4">
                    <span class=LABEL>"Corretora"</span>
                    <input class=INPUT list="brokers" placeholder="Ex.: XP" autocomplete="off" bind:value=broker />
                    <datalist id="brokers">
                        {move || brokers().into_iter().map(|b| view! { <option value=b></option> }).collect_view()}
                    </datalist>
                </label>
            </div>
            <div class="mt-4 flex justify-end gap-2">
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
                        (false, false) => "Cadastrar ativo",
                    }}
                </button>
            </div>
        </form>
    }
}

#[component]
fn AssetRow(
    asset: Asset,
    state: PortfolioState,
    editing: RwSignal<Option<Asset>>,
    action_error: RwSignal<Option<String>>,
) -> impl IntoView {
    let id = asset.id;
    let is_editing = move || editing.with(|e| e.as_ref().is_some_and(|a| a.id == id));
    let order_count = move || state.orders.with(|os| os.iter().filter(|o| o.asset_id == id).count());

    let edit = {
        let asset = asset.clone();
        move |()| editing.set(Some(asset.clone()))
    };
    let delete = move |()| {
        spawn_local(async move {
            match ipc::delete_asset(id).await {
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
            <td class=format!("{TD} font-medium")>{asset.ticker.clone()}</td>
            <td class=TD>
                <span class="flex items-center gap-2">
                    <span class=format!("size-2 rounded-full {}", asset_type_color(asset.asset_type))></span>
                    {asset.asset_type.label_pt()}
                </span>
            </td>
            <td class=format!("{TD} text-slate-600 dark:text-slate-400")>{asset.broker.clone()}</td>
            <td class=format!("{TD} tabular text-right text-slate-600 dark:text-slate-400")>{order_count}</td>
            <td class=format!("{TD} text-right whitespace-nowrap")>
                <EditButton on_click=edit />
                <DeleteButton on_confirm=delete />
            </td>
        </tr>
    }
}
