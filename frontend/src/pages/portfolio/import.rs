//! Carteira › Importar notas: PDF de nota de corretagem (SINACOR) → prévia editável → ordens.

use leptos::html;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos::web_sys::HtmlInputElement;
use shared::format::{format_brl, format_date_br, format_quantity};
use shared::import::{
    FeeLine, ImportNote, ImportRequest, ImportSummary, ImportTrade, NotePreview, PASSWORD_REQUIRED, ParseResult,
    TickerSource, WRONG_PASSWORD,
};
use shared::chrono::NaiveDate;
use shared::rust_decimal::Decimal;
use shared::{AssetType, Money, OrderKind, Quantity};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

use super::PortfolioState;
use crate::components::form::{DeleteButton, INPUT, PRIMARY_BUTTON, SECONDARY_BUTTON};
use crate::components::page::{Card, ErrorBanner};
use crate::components::table::{ROW, TABLE_CARD, TD, TH};
use crate::ipc;

#[wasm_bindgen(module = "/js/files.js")]
extern "C" {
    #[wasm_bindgen(js_name = readFileBase64, catch)]
    fn read_file_base64(input: &HtmlInputElement) -> Result<js_sys::Promise, JsValue>;
    #[wasm_bindgen(js_name = fileName)]
    fn file_name_of(input: &HtmlInputElement) -> String;
}

async fn read_file(input: &HtmlInputElement) -> Result<String, String> {
    let promise = read_file_base64(input).map_err(|e| format!("{e:?}"))?;
    JsFuture::from(promise)
        .await
        .map(|v| v.as_string().unwrap_or_default())
        .map_err(|e| e.as_string().unwrap_or_else(|| "Não foi possível ler o arquivo".into()))
}

/// Negócio da prévia com os campos editáveis em signals próprios (editar não redesenha a prévia).
#[derive(Clone)]
struct TradeDraft {
    spec: String,
    market: String,
    side: OrderKind,
    quantity: Quantity,
    price: Money,
    value: Money,
    fees: Money,
    source: TickerSource,
    ticker: RwSignal<String>,
    asset_type: RwSignal<AssetType>,
}

#[derive(Clone)]
struct NoteDraft {
    number: String,
    trade_date: NaiveDate,
    irrf: Money,
    total_costs: Money,
    net: Option<Money>,
    fee_lines: Vec<FeeLine>,
    warnings: Vec<String>,
    already_imported: bool,
    include: RwSignal<bool>,
    broker: RwSignal<String>,
    trades: Vec<TradeDraft>,
}

impl NoteDraft {
    fn from_preview(note: NotePreview) -> Self {
        Self {
            include: RwSignal::new(!note.already_imported && !note.trades.is_empty()),
            broker: RwSignal::new(note.broker.clone().unwrap_or_default()),
            trades: note
                .trades
                .into_iter()
                .map(|t| TradeDraft {
                    ticker: RwSignal::new(t.ticker.clone().unwrap_or_default()),
                    asset_type: RwSignal::new(t.asset_type),
                    spec: t.spec,
                    market: t.market,
                    side: t.side,
                    quantity: t.quantity,
                    price: t.price,
                    value: t.value,
                    fees: t.fees,
                    source: t.ticker_source,
                })
                .collect(),
            number: note.number,
            trade_date: note.trade_date,
            irrf: note.irrf,
            total_costs: note.total_costs,
            net: note.net,
            fee_lines: note.fee_lines,
            warnings: note.warnings,
            already_imported: note.already_imported,
        }
    }

    fn to_request(&self) -> Result<ImportNote, String> {
        let broker = self.broker.get_untracked().trim().to_owned();
        if broker.is_empty() {
            return Err(format!("Informe a corretora da nota {}.", self.number));
        }
        let trades = self
            .trades
            .iter()
            .map(|t| {
                let ticker = t.ticker.get_untracked().trim().to_uppercase();
                if ticker.is_empty() {
                    return Err(format!("Informe o ticker de “{}” (nota {}).", t.spec, self.number));
                }
                Ok(ImportTrade {
                    spec: t.spec.clone(),
                    ticker,
                    asset_type: t.asset_type.get_untracked(),
                    side: t.side,
                    quantity: t.quantity,
                    price: t.price,
                    fees: t.fees,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ImportNote { number: self.number.clone(), trade_date: self.trade_date, broker, irrf: self.irrf, trades })
    }
}

#[component]
pub(super) fn ImportTab(state: PortfolioState) -> impl IntoView {
    let file_ref = NodeRef::<html::Input>::new();
    let file_b64 = RwSignal::new(None::<String>);
    let file_name = RwSignal::new(String::new());
    let password = RwSignal::new(String::new());
    let need_password = RwSignal::new(false);
    let busy = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);
    let parsed = RwSignal::new(None::<ParseResult>);
    let drafts = RwSignal::new(Vec::<NoteDraft>::new());
    let done = RwSignal::new(None::<ImportSummary>);

    let imported = LocalResource::new(move || {
        state.version.track();
        ipc::list_imported_notes()
    });

    let parse = move || {
        let Some(b64) = file_b64.get_untracked() else { return };
        let pw = Some(password.get_untracked()).filter(|p| !p.trim().is_empty());
        busy.set(true);
        error.set(None);
        spawn_local(async move {
            let result = ipc::parse_broker_note(b64, pw).await;
            busy.set(false);
            match result {
                Ok(result) => {
                    need_password.set(false);
                    drafts.set(result.notes.iter().cloned().map(NoteDraft::from_preview).collect());
                    parsed.set(Some(result));
                }
                Err(msg) => {
                    if msg.starts_with(PASSWORD_REQUIRED) || msg.starts_with(WRONG_PASSWORD) {
                        need_password.set(true);
                    }
                    parsed.set(None);
                    drafts.set(Vec::new());
                    error.set(Some(msg));
                }
            }
        });
    };

    let on_file = move |_| {
        let Some(input) = file_ref.get_untracked() else { return };
        let name = file_name_of(&input);
        done.set(None);
        password.set(String::new());
        need_password.set(false);
        spawn_local(async move {
            match read_file(&input).await {
                Ok(b64) => {
                    file_name.set(name);
                    file_b64.set(Some(b64));
                    parse();
                }
                Err(msg) => error.set(Some(msg)),
            }
        });
    };

    let reset = move || {
        parsed.set(None);
        drafts.set(Vec::new());
        file_b64.set(None);
        file_name.set(String::new());
        if let Some(input) = file_ref.get_untracked() {
            input.set_value("");
        }
    };

    let selected = move || drafts.with(|d| d.iter().filter(|n| n.include.get()).count());
    let do_import = move |_| {
        let notes = drafts.with_untracked(|d| {
            d.iter().filter(|n| n.include.get_untracked()).map(NoteDraft::to_request).collect::<Result<Vec<_>, _>>()
        });
        let notes = match notes {
            Ok(notes) if !notes.is_empty() => notes,
            Ok(_) => return error.set(Some("Selecione ao menos uma nota.".into())),
            Err(msg) => return error.set(Some(msg)),
        };
        busy.set(true);
        error.set(None);
        spawn_local(async move {
            let result = ipc::import_broker_notes(ImportRequest { notes }).await;
            busy.set(false);
            match result {
                Ok(summary) => {
                    done.set(Some(summary));
                    reset();
                    state.reload();
                }
                Err(msg) => error.set(Some(msg)),
            }
        });
    };

    view! {
        <ErrorBanner message=state.load_error />

        <Card class="mb-6">
            <h2 class="text-sm font-semibold">"Importar nota de corretagem (PDF)"</h2>
            <p class="mt-1 text-sm text-slate-500 dark:text-slate-400">
                "Notas no padrão SINACOR (XP, Rico, Clear, BTG, Inter e a maioria das corretoras). Nada é gravado até você confirmar a prévia."
            </p>
            <div class="mt-4 flex flex-wrap items-center gap-3">
                <label class=format!("{PRIMARY_BUTTON} cursor-pointer")>
                    "Escolher PDF…"
                    <input node_ref=file_ref type="file" accept="application/pdf,.pdf" class="sr-only" aria-label="Arquivo da nota" on:change=on_file />
                </label>
                <span class="text-sm text-slate-600 dark:text-slate-400">{move || file_name.get()}</span>
                <Show when=move || busy.get()>
                    <span class="text-sm text-slate-500">"Processando…"</span>
                </Show>
            </div>
            <Show when=move || need_password.get()>
                <form
                    class="mt-4 flex flex-wrap items-end gap-3"
                    on:submit=move |ev| {
                        ev.prevent_default();
                        parse();
                    }
                >
                    <label class="w-64">
                        <span class="mb-1 block text-xs font-medium text-slate-600 dark:text-slate-400">"Senha do PDF"</span>
                        <input class=INPUT type="password" autocomplete="off" aria-label="Senha do PDF" bind:value=password />
                    </label>
                    <button type="submit" class=PRIMARY_BUTTON>"Abrir"</button>
                </form>
            </Show>
            <div class="mt-4 empty:hidden">
                <ErrorBanner message=error />
                {move || done.get().map(|s| view! {
                    <div role="status" class="rounded-lg border border-emerald-200 bg-emerald-50 px-4 py-3 text-sm text-emerald-800 dark:border-emerald-900 dark:bg-emerald-950 dark:text-emerald-200">
                        {format!(
                            "Importado: {} nota(s), {} ordem(ns).{}",
                            s.notes,
                            s.orders,
                            if s.assets_created.is_empty() { String::new() } else { format!(" Ativos criados: {}.", s.assets_created.join(", ")) },
                        )}
                    </div>
                })}
            </div>
        </Card>

        {move || parsed.get().map(|result| {
            let notes = drafts.get_untracked();
            view! {
                {result.warnings.iter().map(|w| view! {
                    <div class="mb-4 rounded-lg border border-amber-200 bg-amber-50 px-4 py-3 text-sm text-amber-800 dark:border-amber-900 dark:bg-amber-950 dark:text-amber-200">{w.clone()}</div>
                }).collect_view()}
                {notes.into_iter().map(|note| view! { <NoteCard note state /> }).collect_view()}
                <Show when=move || drafts.with(|d| !d.is_empty())>
                    <div class="mb-6 flex items-center justify-end gap-2">
                        <button type="button" class=SECONDARY_BUTTON on:click=move |_| reset()>"Cancelar"</button>
                        <button type="button" class=PRIMARY_BUTTON disabled=move || busy.get() || selected() == 0 on:click=do_import>
                            {move || format!("Importar {} nota(s)", selected())}
                        </button>
                    </div>
                </Show>
                <details class="mb-6 text-sm">
                    <summary class="cursor-pointer text-slate-500 hover:text-slate-900 dark:hover:text-white">"Ver texto extraído do PDF"</summary>
                    <pre class="mt-2 max-h-80 overflow-auto rounded-lg bg-slate-100 p-3 text-xs whitespace-pre-wrap dark:bg-slate-900">{result.raw_text.clone()}</pre>
                </details>
            }
        })}

        <ImportedNotes state imported />
    }
}

#[component]
fn NoteCard(note: NoteDraft, state: PortfolioState) -> impl IntoView {
    let broker = note.broker;
    let include = note.include;
    let brokers = move || {
        let mut all: Vec<String> = state.assets.get().into_iter().map(|a| a.broker).collect();
        all.sort();
        all.dedup();
        all
    };
    let list_id = format!("brokers-note-{}", note.number);
    let total_value: Decimal = note.trades.iter().map(|t| t.value.0).sum();
    let fee_title = note.fee_lines.iter().map(|f| format!("{}: {}", f.label, format_brl(f.amount.0))).collect::<Vec<_>>().join("\n");

    view! {
        <section class=move || format!("{TABLE_CARD} mb-4 {}", if include.get() { "" } else { "opacity-60" })>
            <div class="flex flex-wrap items-center justify-between gap-3 border-b border-slate-200 px-4 py-3 dark:border-slate-800">
                <label class="flex items-center gap-3">
                    <input type="checkbox" class="size-4 accent-emerald-600" prop:checked=move || include.get() disabled=note.already_imported
                        on:change=move |ev| include.set(event_target_checked(&ev)) />
                    <span class="text-sm font-semibold">{format!("Nota {} · pregão {}", note.number, format_date_br(note.trade_date))}</span>
                    {note.already_imported.then(|| view! {
                        <span class="rounded-full bg-slate-200 px-2 py-0.5 text-xs text-slate-700 dark:bg-slate-700 dark:text-slate-200">"já importada"</span>
                    })}
                </label>
                <label class="flex items-center gap-2 text-sm">
                    <span class="text-slate-500">"Corretora"</span>
                    <input class=format!("{INPUT} w-44") list=list_id.clone() aria-label="Corretora da nota" bind:value=broker />
                    <datalist id=list_id>
                        {move || brokers().into_iter().map(|b| view! { <option value=b></option> }).collect_view()}
                    </datalist>
                </label>
            </div>

            {(!note.warnings.is_empty()).then(|| view! {
                <ul class="mx-4 mt-3 list-inside list-disc rounded-lg border border-amber-200 bg-amber-50 px-3 py-2 text-xs text-amber-800 dark:border-amber-900 dark:bg-amber-950 dark:text-amber-200">
                    {note.warnings.iter().map(|w| view! { <li>{w.clone()}</li> }).collect_view()}
                </ul>
            })}

            <table class="w-full text-sm">
                <thead class="bg-slate-50 dark:bg-slate-950/40">
                    <tr>
                        <th class=TH>"C/V"</th>
                        <th class=TH>"Especificação na nota"</th>
                        <th class=TH>"Ticker"</th>
                        <th class=TH>"Tipo"</th>
                        <th class=format!("{TH} text-right")>"Qtd."</th>
                        <th class=format!("{TH} text-right")>"Preço"</th>
                        <th class=format!("{TH} text-right")>"Valor"</th>
                        <th class=format!("{TH} text-right")>"Custos"</th>
                    </tr>
                </thead>
                <tbody class="divide-y divide-slate-100 dark:divide-slate-800">
                    {note.trades.iter().cloned().map(|t| view! { <TradeRow trade=t broker state /> }).collect_view()}
                </tbody>
            </table>

            <div class="flex flex-wrap justify-end gap-x-6 gap-y-1 border-t border-slate-200 px-4 py-3 text-xs text-slate-500 dark:border-slate-800 dark:text-slate-400">
                <span class="tabular">{format!("Operações: {}", format_brl(total_value))}</span>
                <span class="tabular" title=fee_title>
                    {format!("Custos: {} (rateados pelo valor)", format_brl(note.total_costs.0))}
                </span>
                <span class="tabular">{format!("IRRF: {} (somado ao mês)", format_brl(note.irrf.0))}</span>
                {note.net.map(|n| view! {
                    <span class="tabular font-medium text-slate-700 dark:text-slate-200">
                        {format!("Líquido: {} {}", format_brl(n.0.abs()), if n.is_sign_negative() { "D" } else { "C" })}
                    </span>
                })}
            </div>
        </section>
    }
}

#[component]
fn TradeRow(trade: TradeDraft, broker: RwSignal<String>, state: PortfolioState) -> impl IntoView {
    let ticker = trade.ticker;
    let asset_type = trade.asset_type;
    // Tipo só é pedido quando o ativo (ticker + corretora) ainda não existe.
    let existing = move || {
        let t = ticker.get().trim().to_uppercase();
        let b = broker.get().trim().to_owned();
        state.assets.with(|a| a.iter().find(|x| x.ticker == t && x.broker == b).map(|x| x.asset_type))
    };
    let source_class = match trade.source {
        TickerSource::Alias | TickerSource::InText => "text-emerald-600 dark:text-emerald-400",
        TickerSource::CompanyName => "text-amber-600 dark:text-amber-400",
        TickerSource::Unknown => "text-red-600 dark:text-red-400",
    };
    let (side_label, side_class) = match trade.side {
        OrderKind::Buy => ("C", "text-emerald-600"),
        OrderKind::Sell => ("V", "text-red-600"),
    };

    view! {
        <tr class=ROW>
            <td class=format!("{TD} font-semibold {side_class}")>{side_label}</td>
            <td class=format!("{TD} text-slate-600 dark:text-slate-400")>
                {trade.spec.clone()}
                {(trade.market == "FRACIONARIO").then(|| view! { <span class="ml-1 text-xs text-slate-400">"fracionário"</span> })}
            </td>
            <td class=TD>
                <div class="flex items-center gap-2">
                    <input
                        class=format!("{INPUT} w-28 uppercase")
                        aria-label=format!("Ticker de {}", trade.spec)
                        autocomplete="off"
                        bind:value=ticker
                    />
                    <span class=format!("text-xs {source_class}")>{trade.source.label_pt()}</span>
                </div>
            </td>
            <td class=TD>
                {move || match existing() {
                    Some(t) => view! { <span class="text-slate-500">{t.label_pt()}</span> }.into_any(),
                    None => view! {
                        <select
                            class=format!("{INPUT} w-32")
                            aria-label=format!("Tipo de {}", trade.spec)
                            on:change=move |ev| {
                                let value = event_target_value(&ev);
                                if let Some(t) = AssetType::ALL.into_iter().find(|t| format!("{t:?}") == value) {
                                    asset_type.set(t);
                                }
                            }
                        >
                            {AssetType::ALL.into_iter().map(|t| view! {
                                <option value=format!("{t:?}") selected=move || asset_type.get() == t>{t.label_pt()}</option>
                            }).collect_view()}
                        </select>
                    }.into_any(),
                }}
            </td>
            <td class=format!("{TD} tabular text-right")>{format_quantity(trade.quantity.0)}</td>
            <td class=format!("{TD} tabular text-right")>{format_brl(trade.price.0)}</td>
            <td class=format!("{TD} tabular text-right")>{format_brl(trade.value.0)}</td>
            <td class=format!("{TD} tabular text-right text-slate-500")>{format_brl(trade.fees.0)}</td>
        </tr>
    }
}

#[component]
fn ImportedNotes(
    state: PortfolioState,
    imported: LocalResource<Result<Vec<shared::import::ImportedNote>, String>>,
) -> impl IntoView {
    let action_error = RwSignal::new(None::<String>);
    let rows = move || imported.get().and_then(Result::ok).unwrap_or_default();
    view! {
        <section class=TABLE_CARD>
            <div class="border-b border-slate-200 px-4 py-3 dark:border-slate-800">
                <h2 class="text-sm font-semibold">"Notas importadas"</h2>
            </div>
            <div class="px-4 pt-4 empty:hidden">
                <ErrorBanner message=action_error />
            </div>
            <table class="w-full text-sm">
                <thead class="bg-slate-50 dark:bg-slate-950/40">
                    <tr>
                        <th class=TH>"Pregão"</th>
                        <th class=TH>"Nota"</th>
                        <th class=TH>"Corretora"</th>
                        <th class=format!("{TH} text-right")>"Ordens"</th>
                        <th class=format!("{TH} text-right")>"IRRF"</th>
                        <th class=format!("{TH} text-right")><span class="sr-only">"Ações"</span></th>
                    </tr>
                </thead>
                <tbody class="divide-y divide-slate-100 dark:divide-slate-800">
                    <For
                        each=rows
                        key=|n| n.clone()
                        children=move |n| {
                            let id = n.id;
                            let undo = move |()| {
                                spawn_local(async move {
                                    match ipc::undo_imported_note(id).await {
                                        Ok(()) => {
                                            action_error.set(None);
                                            state.reload();
                                        }
                                        Err(msg) => action_error.set(Some(msg)),
                                    }
                                });
                            };
                            view! {
                                <tr class=ROW>
                                    <td class=format!("{TD} tabular text-slate-600 dark:text-slate-400")>{format_date_br(n.trade_date)}</td>
                                    <td class=format!("{TD} font-medium")>{n.number.clone()}</td>
                                    <td class=TD>{n.broker.clone()}</td>
                                    <td class=format!("{TD} tabular text-right")>{n.orders}</td>
                                    <td class=format!("{TD} tabular text-right text-slate-500")>{format_brl(n.irrf.0)}</td>
                                    <td class=format!("{TD} text-right")><DeleteButton on_confirm=undo label="Desfazer" /></td>
                                </tr>
                            }
                        }
                    />
                </tbody>
            </table>
            <Show when=move || rows().is_empty()>
                <p class="px-4 py-8 text-center text-sm text-slate-500">"Nenhuma nota importada ainda."</p>
            </Show>
        </section>
    }
}
