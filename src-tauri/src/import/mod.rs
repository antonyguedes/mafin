//! Importação de notas de corretagem em PDF (layout SINACOR).
//!
//! * [`preview`]: extrai o texto, interpreta as notas e sugere tickers. Não grava nada.
//! * [`import`]: grava as notas confirmadas numa única transação (ativos que faltam, ordens
//!   com taxas rateadas, associações de ticker, IRRF do mês) e só confirma se a carteira
//!   continuar consistente.
//! * [`undo`]: desfaz uma importação (remove as ordens da nota e o IRRF somado).

pub mod resolve;
pub mod sinacor;
#[cfg(any(test, feature = "test-support"))]
pub mod testpdf;

use base64::Engine;
use shared::import::{
    FeeLine, ImportRequest, ImportSummary, ImportedNote, NotePreview, PASSWORD_REQUIRED, ParseResult, TradePreview,
    WRONG_PASSWORD, allocate_cents,
};
use shared::{Money, NewAsset, NewOrder, Quantity, YearMonth};
use sqlx::SqlitePool;

use crate::error::{AppError, AppResult};
use crate::ledger;
use crate::repo::{assets, irrf, notes, orders};
use resolve::{ResolveContext, normalize_spec};
use sinacor::{RawNote, parse_pages};

/// Limite de tamanho do PDF (notas reais têm poucas dezenas de KB).
const MAX_PDF_BYTES: usize = 20 * 1024 * 1024;

fn unreadable(err: impl std::fmt::Display) -> AppError {
    AppError::Conflict(format!("Não foi possível ler o PDF: {err}"))
}

/// Abre um PDF protegido com a senha e devolve uma cópia sem criptografia.
///
/// Feito aqui com o lopdf porque o `pdf-extract` 0.12 (com lopdf 0.42) carrega o documento
/// sem senha e só depois descriptografa, e aí os objetos cifrados não são carregados: o
/// texto sai vazio.
fn decrypt_to_plain(pdf: &[u8], password: &str) -> Result<Vec<u8>, lopdf::Error> {
    let mut doc = lopdf::Document::load_mem_with_options(pdf, lopdf::LoadOptions::with_password(password))?;
    if doc.is_encrypted() {
        doc.decrypt(password)?;
    }
    doc.trailer.remove(b"Encrypt");
    let mut out = Vec::new();
    doc.save_to(&mut out)?;
    Ok(out)
}

/// Texto de cada página. PDF protegido: usa a senha (ou tenta senha vazia) e, sem ela, pede.
pub fn extract_pages(pdf: &[u8], password: Option<&str>) -> AppResult<Vec<String>> {
    let password = password.map(str::trim).filter(|p| !p.is_empty());
    let encrypted = lopdf::Document::load_mem(pdf).map_err(unreadable)?.is_encrypted();

    let plain;
    let bytes = if encrypted {
        plain = decrypt_to_plain(pdf, password.unwrap_or("")).map_err(|_| {
            let prefix = if password.is_some() { WRONG_PASSWORD } else { PASSWORD_REQUIRED };
            AppError::Conflict(format!("{prefix}. Informe a senha (em geral, os primeiros dígitos do CPF)."))
        })?;
        &plain[..]
    } else {
        pdf
    };

    let pages = pdf_extract::extract_text_from_mem_by_pages(bytes).map_err(unreadable)?;
    if pages.iter().all(|p| p.trim().is_empty()) {
        let hint = if encrypted {
            "Não foi possível extrair o texto deste PDF protegido. Salve uma cópia sem senha (ex.: “Imprimir como PDF”) e importe-a."
        } else {
            "O PDF não tem texto selecionável (pode ser uma imagem escaneada). Use o PDF original enviado pela corretora."
        };
        return Err(AppError::Conflict(hint.into()));
    }
    Ok(pages)
}

fn to_preview(raw: RawNote, ctx: &ResolveContext, already_imported: bool) -> NotePreview {
    let (costs, mut warnings) = raw.costs();
    warnings.splice(0..0, raw.warnings.iter().cloned());
    let values: Vec<Money> = raw.trades.iter().map(|t| Money(t.value)).collect();
    let fees = allocate_cents(Money(costs), &values);

    let trades: Vec<TradePreview> = raw
        .trades
        .iter()
        .zip(fees)
        .map(|(t, fees)| {
            let (ticker, ticker_source, asset_type) = ctx.resolve(&t.spec);
            TradePreview {
                side: t.side,
                market: t.market.clone(),
                spec: t.spec.clone(),
                quantity: Quantity(t.quantity),
                price: Money(t.price),
                value: Money(t.value),
                fees,
                ticker,
                ticker_source,
                asset_type,
            }
        })
        .collect();
    if trades.is_empty() {
        warnings.push("Nenhum negócio à vista encontrado nesta nota.".into());
    }

    NotePreview {
        number: raw.number,
        trade_date: raw.trade_date,
        broker: raw.broker,
        trades,
        fee_lines: raw.fee_lines.into_iter().map(|(label, amount)| FeeLine { label, amount: Money(amount) }).collect(),
        total_costs: Money(costs),
        irrf: Money(raw.irrf),
        net: raw.net.map(Money),
        already_imported,
        warnings,
    }
}

fn decode_pdf(pdf_base64: &str) -> AppResult<Vec<u8>> {
    let pdf = base64::engine::general_purpose::STANDARD
        .decode(pdf_base64.trim())
        .map_err(|_| AppError::Conflict("Arquivo inválido (base64)".into()))?;
    if pdf.len() > MAX_PDF_BYTES {
        return Err(AppError::Conflict("Arquivo grande demais para uma nota de corretagem".into()));
    }
    Ok(pdf)
}

/// Parte pura da prévia: páginas de texto + contexto → notas. `already_imported` consulta o
/// banco (ou devolve `false`, nos testes/fixtures).
pub fn build_preview(pages: &[String], ctx: &ResolveContext, already_imported: impl Fn(&RawNote) -> bool) -> ParseResult {
    let notes: Vec<NotePreview> =
        parse_pages(pages).into_iter().map(|raw| {
            let already = already_imported(&raw);
            to_preview(raw, ctx, already)
        }).collect();
    let mut warnings = Vec::new();
    if notes.is_empty() {
        warnings.push(
            "Nenhuma nota no padrão SINACOR foi reconhecida neste PDF. Veja o texto extraído abaixo; se a nota for de outro layout, cadastre as ordens manualmente."
                .into(),
        );
    }
    ParseResult { notes, warnings, raw_text: pages.join("\n\n--- página ---\n\n") }
}

/// Interpreta o PDF (em base64) e devolve a prévia das notas encontradas.
pub async fn preview(pool: &SqlitePool, pdf_base64: &str, password: Option<&str>) -> AppResult<ParseResult> {
    let pages = extract_pages(&decode_pdf(pdf_base64)?, password)?;
    let ctx = ResolveContext { aliases: notes::aliases(pool).await?, assets: assets::list(pool).await? };

    let mut existing = Vec::new();
    for raw in parse_pages(&pages) {
        if notes::exists(pool, raw.broker.as_deref(), &raw.number, raw.trade_date).await? {
            existing.push((raw.number.clone(), raw.trade_date));
        }
    }
    Ok(build_preview(&pages, &ctx, |raw| existing.contains(&(raw.number.clone(), raw.trade_date))))
}

/// Prévia sem banco (sem associações salvas nem checagem de duplicidade). Para fixtures.
#[cfg(feature = "test-support")]
pub fn preview_offline(pdf_base64: &str, password: Option<&str>) -> AppResult<ParseResult> {
    let pages = extract_pages(&decode_pdf(pdf_base64)?, password)?;
    Ok(build_preview(&pages, &ResolveContext::default(), |_| false))
}

/// Grava as notas confirmadas. Tudo ou nada.
pub async fn import(pool: &SqlitePool, request: ImportRequest) -> AppResult<ImportSummary> {
    if request.notes.is_empty() {
        return Err(AppError::Conflict("Nenhuma nota selecionada".into()));
    }
    let mut tx = pool.begin().await?;
    let mut summary = ImportSummary::default();

    for note in &request.notes {
        let broker = note.broker.trim();
        if broker.is_empty() {
            return Err(AppError::Conflict(format!("Informe a corretora da nota {}", note.number)));
        }
        if note.trades.is_empty() {
            return Err(AppError::Conflict(format!("A nota {} não tem negócios", note.number)));
        }
        let note_id = notes::insert(&mut *tx, broker, &note.number, note.trade_date, note.irrf).await?;

        for trade in &note.trades {
            let ticker = trade.ticker.trim().to_uppercase();
            if ticker.is_empty() {
                return Err(AppError::Conflict(format!("Informe o ticker de \"{}\" (nota {})", trade.spec, note.number)));
            }
            let existing = assets::list(&mut *tx).await?.into_iter().find(|a| a.ticker == ticker && a.broker == broker);
            let asset = match existing {
                Some(asset) => asset,
                None => {
                    let created =
                        assets::create(&mut *tx, NewAsset { ticker: ticker.clone(), asset_type: trade.asset_type, broker: broker.into() })
                            .await?;
                    summary.assets_created.push(format!("{} · {}", created.ticker, created.broker));
                    created
                }
            };
            let order = orders::create(
                &mut *tx,
                NewOrder {
                    asset_id: asset.id,
                    kind: trade.side,
                    quantity: trade.quantity,
                    price: trade.price,
                    fees: trade.fees,
                    date: note.trade_date,
                },
            )
            .await?;
            notes::link_order(&mut *tx, order.id, note_id).await?;
            notes::save_alias(&mut *tx, &normalize_spec(&trade.spec), &asset.ticker).await?;
            summary.orders += 1;
        }

        if !note.irrf.is_zero() {
            irrf::add(&mut tx, YearMonth::of(note.trade_date), note.irrf).await?;
        }
        summary.notes += 1;
    }

    ledger::validate(&mut tx, Some("Esta importação")).await?;
    tx.commit().await?;
    Ok(summary)
}

pub async fn list(pool: &SqlitePool) -> AppResult<Vec<ImportedNote>> {
    notes::list(pool).await
}

/// Desfaz uma importação: remove as ordens da nota e subtrai o IRRF que ela somou.
pub async fn undo(pool: &SqlitePool, id: i64) -> AppResult<()> {
    let mut tx = pool.begin().await?;
    let note = notes::get(&mut *tx, id).await?;
    notes::delete_with_orders(&mut tx, id).await?;
    if !note.irrf.is_zero() {
        irrf::add(&mut tx, YearMonth::of(note.trade_date), Money(-note.irrf.0)).await?;
    }
    ledger::validate(&mut tx, Some("Desfazer esta importação")).await?;
    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connect_in_memory;
    use crate::error::AppError;
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;
    use shared::import::{ImportNote, ImportTrade, TickerSource};
    use shared::{AssetType, OrderFilter, OrderKind};

    /// Soma das taxas rateadas.
    fn total_fees(preview: &NotePreview) -> Decimal {
        preview.trades.iter().map(|t| t.fees.0).sum()
    }

    fn sample_b64(password: Option<&str>) -> String {
        base64::engine::general_purpose::STANDARD.encode(testpdf::note_pdf(&testpdf::sample_notes(), password))
    }

    fn request_from(preview: &ParseResult) -> ImportRequest {
        ImportRequest {
            notes: preview
                .notes
                .iter()
                .map(|n| ImportNote {
                    number: n.number.clone(),
                    trade_date: n.trade_date,
                    broker: n.broker.clone().unwrap(),
                    irrf: n.irrf,
                    trades: n
                        .trades
                        .iter()
                        .map(|t| ImportTrade {
                            spec: t.spec.clone(),
                            ticker: t.ticker.clone().unwrap(),
                            asset_type: t.asset_type,
                            side: t.side,
                            quantity: t.quantity,
                            price: t.price,
                            fees: t.fees,
                        })
                        .collect(),
                })
                .collect(),
        }
    }

    #[tokio::test]
    async fn preview_from_real_pdf_bytes() {
        let pool = connect_in_memory().await;
        let result = preview(&pool, &sample_b64(None), None).await.unwrap();
        assert!(result.warnings.is_empty(), "{:?}", result.warnings);
        let note = &result.notes[0];
        assert_eq!((note.number.as_str(), note.broker.as_deref()), ("123456", Some("XP")));
        assert_eq!(note.total_costs, Money(dec!(6.57)));
        assert_eq!(total_fees(note), dec!(6.57), "rateio fecha nos centavos");
        assert_eq!(note.irrf, Money(dec!(0.08)));
        assert!(note.warnings.is_empty(), "{:?}", note.warnings);
        let tickers: Vec<_> = note.trades.iter().map(|t| (t.ticker.as_deref(), t.ticker_source, t.asset_type)).collect();
        assert_eq!(
            tickers,
            [
                (Some("PETR4"), TickerSource::CompanyName, AssetType::Stock),
                (Some("HGLG11"), TickerSource::InText, AssetType::Fii),
                (Some("VALE3"), TickerSource::CompanyName, AssetType::Stock),
            ]
        );
    }

    #[tokio::test]
    async fn password_protected_pdf() {
        let pool = connect_in_memory().await;
        let b64 = sample_b64(Some("123"));
        let err = preview(&pool, &b64, None).await.unwrap_err().to_string();
        assert!(err.starts_with(PASSWORD_REQUIRED), "{err}");
        let err = preview(&pool, &b64, Some("999")).await.unwrap_err().to_string();
        assert!(err.starts_with(WRONG_PASSWORD), "{err}");
        let ok = preview(&pool, &b64, Some("123")).await.unwrap();
        assert_eq!(ok.notes[0].trades.len(), 3);
    }

    #[tokio::test]
    async fn import_is_atomic_idempotent_and_undoable() {
        let pool = connect_in_memory().await;
        // A nota vende 10 HGLG11: precisa haver posição antes.
        let hglg = assets::create(&pool, NewAsset { ticker: "HGLG11".into(), asset_type: AssetType::Fii, broker: "XP".into() })
            .await
            .unwrap();
        let buy = NewOrder {
            asset_id: hglg.id,
            kind: OrderKind::Buy,
            quantity: Quantity(dec!(10)),
            price: Money(dec!(160)),
            fees: Money::ZERO,
            date: chrono::NaiveDate::from_ymd_opt(2026, 9, 1).unwrap(),
        };

        let parsed = preview(&pool, &sample_b64(None), None).await.unwrap();
        // Sem a compra anterior, a venda de HGLG11 excede a posição: nada é gravado.
        let err = import(&pool, request_from(&parsed)).await.unwrap_err();
        assert!(err.to_string().contains("Esta importação deixaria a carteira inconsistente"), "{err}");
        assert!(notes::list(&pool).await.unwrap().is_empty());
        assert_eq!(assets::list(&pool).await.unwrap().len(), 1, "ativos criados foram desfeitos");

        ledger::create_order(&pool, buy).await.unwrap();
        let summary = import(&pool, request_from(&parsed)).await.unwrap();
        assert_eq!((summary.notes, summary.orders), (1, 3));
        assert_eq!(summary.assets_created, ["PETR4 · XP", "VALE3 · XP"]);

        let all = orders::list(&pool, OrderFilter::default()).await.unwrap();
        let fees: Decimal = all.iter().map(|o| o.fees.0).sum();
        assert_eq!((all.len(), fees), (4, dec!(6.57)));
        let month = YearMonth::new(2026, 9).unwrap();
        assert_eq!(irrf::all(&pool).await.unwrap()[&month], Money(dec!(0.08)));

        // Associações salvas: a próxima prévia usa "usado antes".
        let again = preview(&pool, &sample_b64(None), None).await.unwrap();
        assert!(again.notes[0].already_imported);
        assert!(again.notes[0].trades.iter().all(|t| t.ticker_source == TickerSource::Alias));
        // Importar de novo: recusado.
        assert!(matches!(import(&pool, request_from(&again)).await, Err(AppError::Conflict(msg)) if msg.contains("já foi importada")));

        let listed = list(&pool).await.unwrap();
        assert_eq!(listed[0].orders, 3);
        undo(&pool, listed[0].id).await.unwrap();
        assert_eq!(orders::list(&pool, OrderFilter::default()).await.unwrap().len(), 1);
        assert!(irrf::all(&pool).await.unwrap().is_empty());
        assert!(list(&pool).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn rejects_missing_ticker_or_broker() {
        let pool = connect_in_memory().await;
        let parsed = preview(&pool, &sample_b64(None), None).await.unwrap();
        let mut request = request_from(&parsed);
        request.notes[0].trades[0].ticker = " ".into();
        assert!(import(&pool, request).await.unwrap_err().to_string().contains("Informe o ticker"));
        let mut request = request_from(&parsed);
        request.notes[0].broker = "".into();
        assert!(import(&pool, request).await.unwrap_err().to_string().contains("Informe a corretora"));
    }

    #[test]
    fn encrypted_pdf_with_xref_stream() {
        let pdf = testpdf::note_pdf_xref_stream(&testpdf::sample_notes(), Some("abc"));
        let pages = extract_pages(&pdf, Some("abc")).unwrap();
        assert_eq!(parse_pages(&pages)[0].trades.len(), 3);
    }

    #[test]
    fn empty_text_pdf_is_reported() {
        let pdf = testpdf::note_pdf(&[], None);
        let err = extract_pages(&pdf, None).unwrap_err().to_string();
        assert!(err.contains("não tem texto selecionável"), "{err}");
    }

    #[test]
    fn garbage_is_not_a_pdf() {
        assert!(extract_pages(b"not a pdf", None).unwrap_err().to_string().contains("Não foi possível ler o PDF"));
    }
}
