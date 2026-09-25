//! Ponte IPC com o backend Tauri via `window.__TAURI__.core.invoke`
//! (disponível porque `app.withGlobalTauri = true` no tauri.conf.json).

use serde::{Serialize, de::DeserializeOwned};
use shared::{
    Asset, NewAsset, NewOrder, NewTransaction, Order, OrderFilter, PingRequest, PingResponse, Portfolio, Transaction,
    TransactionFilter, commands,
};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

#[wasm_bindgen]
extern "C" {
    // Binding síncrono que devolve a Promise. `catch` converte exceções JS em `Err`
    // em vez de derrubar o Wasm (ex.: `window.__TAURI__` ausente fora do Tauri).
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"], js_name = invoke, catch)]
    fn tauri_invoke(cmd: &str, args: JsValue) -> Result<js_sys::Promise, JsValue>;
}

/// `true` quando a página roda dentro da janela do Tauri (e não num navegador comum).
pub fn is_tauri() -> bool {
    js_sys::Reflect::get(&js_sys::global(), &JsValue::from_str("__TAURI__"))
        .is_ok_and(|v| !v.is_undefined())
}

/// Invoca um `#[tauri::command]` serializando `args` e desserializando a resposta.
/// Erros do comando (`Err(String)`/`AppError` no Rust) chegam aqui como `Err`.
pub async fn invoke<A: Serialize, R: DeserializeOwned>(cmd: &str, args: &A) -> Result<R, String> {
    if !is_tauri() {
        return Err("Fora do Tauri: rode com `cargo tauri dev`".into());
    }

    // `serialize_maps_as_objects`: o Tauri espera um objeto JS simples, não um `Map`.
    let serializer = serde_wasm_bindgen::Serializer::new().serialize_maps_as_objects(true);
    let args = args.serialize(&serializer).map_err(|e| e.to_string())?;

    let result = match tauri_invoke(cmd, args) {
        Ok(promise) => JsFuture::from(promise).await,
        Err(err) => Err(err),
    };

    match result {
        Ok(value) => serde_wasm_bindgen::from_value(value).map_err(|e| e.to_string()),
        Err(err) => Err(err.as_string().unwrap_or_else(|| format!("{err:?}"))),
    }
}

#[derive(Serialize)]
struct PingArgs {
    req: PingRequest,
}

pub async fn ping(req: PingRequest) -> Result<PingResponse, String> {
    invoke(commands::PING, &PingArgs { req }).await
}

// ---- Lançamentos (receitas/despesas) -------------------------------------------------

#[derive(Serialize)]
struct NoArgs {}

#[derive(Serialize)]
struct IdArgs {
    id: i64,
}

#[derive(Serialize)]
struct InputArgs<T> {
    input: T,
}

#[derive(Serialize)]
struct UpdateArgs<T> {
    id: i64,
    input: T,
}

#[derive(Serialize)]
struct FilterArgs<T> {
    filter: T,
}

pub async fn list_transactions(filter: TransactionFilter) -> Result<Vec<Transaction>, String> {
    invoke(commands::LIST_TRANSACTIONS, &FilterArgs { filter }).await
}

pub async fn create_transaction(input: NewTransaction) -> Result<Transaction, String> {
    invoke(commands::CREATE_TRANSACTION, &InputArgs { input }).await
}

pub async fn update_transaction(id: i64, input: NewTransaction) -> Result<Transaction, String> {
    invoke(commands::UPDATE_TRANSACTION, &UpdateArgs { id, input }).await
}

pub async fn delete_transaction(id: i64) -> Result<(), String> {
    invoke(commands::DELETE_TRANSACTION, &IdArgs { id }).await
}

pub async fn list_categories() -> Result<Vec<String>, String> {
    invoke(commands::LIST_CATEGORIES, &NoArgs {}).await
}

// ---- Carteira: ativos, ordens e custódia --------------------------------------------

pub async fn list_assets() -> Result<Vec<Asset>, String> {
    invoke(commands::LIST_ASSETS, &NoArgs {}).await
}

pub async fn create_asset(input: NewAsset) -> Result<Asset, String> {
    invoke(commands::CREATE_ASSET, &InputArgs { input }).await
}

pub async fn update_asset(id: i64, input: NewAsset) -> Result<Asset, String> {
    invoke(commands::UPDATE_ASSET, &UpdateArgs { id, input }).await
}

pub async fn delete_asset(id: i64) -> Result<(), String> {
    invoke(commands::DELETE_ASSET, &IdArgs { id }).await
}

pub async fn list_orders() -> Result<Vec<Order>, String> {
    invoke(commands::LIST_ORDERS, &FilterArgs { filter: OrderFilter::default() }).await
}

pub async fn create_order(input: NewOrder) -> Result<Order, String> {
    invoke(commands::CREATE_ORDER, &InputArgs { input }).await
}

pub async fn update_order(id: i64, input: NewOrder) -> Result<Order, String> {
    invoke(commands::UPDATE_ORDER, &UpdateArgs { id, input }).await
}

pub async fn delete_order(id: i64) -> Result<(), String> {
    invoke(commands::DELETE_ORDER, &IdArgs { id }).await
}

pub async fn get_portfolio() -> Result<Portfolio, String> {
    invoke(commands::GET_PORTFOLIO, &NoArgs {}).await
}

// ---- Imposto de Renda ----------------------------------------------------------------

pub async fn get_tax_report() -> Result<shared::tax::TaxReport, String> {
    invoke(commands::GET_TAX_REPORT, &NoArgs {}).await
}
