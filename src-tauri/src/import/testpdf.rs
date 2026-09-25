//! Gerador de notas de corretagem SINTÉTICAS no layout SINACOR, para testes.
//!
//! Reproduz o que importa para a extração de texto: cada célula é um objeto de texto
//! posicionado separadamente (como nos PDFs reais) e os quadros "Resumo dos Negócios" e
//! "Resumo Financeiro" ficam lado a lado, na mesma altura. Não é uma nota real.

use lopdf::content::{Content, Operation};
use lopdf::encryption::{EncryptionState, EncryptionVersion, Permissions};
use lopdf::{Document, Object, Stream, dictionary};

pub struct SynthTrade {
    pub side: &'static str,
    pub market: &'static str,
    pub spec: &'static str,
    pub obs: &'static str,
    pub quantity: &'static str,
    pub price: &'static str,
    pub value: &'static str,
    pub dc: &'static str,
}

pub struct SynthNote {
    pub number: &'static str,
    pub page: u32,
    pub date: &'static str,
    pub broker: &'static str,
    pub trades: Vec<SynthTrade>,
    /// `None` = página intermediária de uma nota com várias folhas (sem resumo).
    pub summary: Option<SynthSummary>,
}

pub struct SynthSummary {
    pub sells: &'static str,
    pub buys: &'static str,
    pub operations: &'static str,
    /// (rótulo, valor, D/C)
    pub fees: Vec<(&'static str, &'static str, &'static str)>,
    pub irrf_base: &'static str,
    pub irrf: &'static str,
    pub net_date: &'static str,
    pub net: &'static str,
    pub net_dc: &'static str,
}

/// Texto em WinAnsi (Latin-1), para acentos como nos PDFs reais.
fn latin1(text: &str) -> Object {
    Object::string_literal(text.chars().map(|c| if (c as u32) < 256 { c as u8 } else { b'?' }).collect::<Vec<u8>>())
}

fn text(ops: &mut Vec<Operation>, x: i64, y: i64, size: i64, value: &str) {
    if value.is_empty() {
        return;
    }
    ops.push(Operation::new("BT", vec![]));
    ops.push(Operation::new("Tf", vec!["F1".into(), size.into()]));
    ops.push(Operation::new("Td", vec![x.into(), y.into()]));
    ops.push(Operation::new("Tj", vec![latin1(value)]));
    ops.push(Operation::new("ET", vec![]));
}

fn page_ops(note: &SynthNote) -> Vec<Operation> {
    let mut ops = Vec::new();
    let t = &mut ops;
    text(t, 200, 810, 12, "NOTA DE CORRETAGEM");
    text(t, 400, 790, 7, "Nr. nota");
    text(t, 460, 790, 7, "Folha");
    text(t, 500, 790, 7, "Data pregão");
    text(t, 400, 780, 8, note.number);
    text(t, 460, 780, 8, &note.page.to_string());
    text(t, 500, 780, 8, note.date);
    text(t, 30, 760, 9, note.broker);
    text(t, 30, 750, 7, "Av. Exemplo, 1000 - São Paulo - SP");
    text(t, 30, 700, 8, "Negócios realizados");
    let header = [
        (30, "Q Negociação"),
        (95, "C/V"),
        (115, "Tipo mercado"),
        (170, "Prazo"),
        (200, "Especificação do título"),
        (330, "Obs. (*)"),
        (370, "Quantidade"),
        (425, "Preço / Ajuste"),
        (485, "Valor Operação / Ajuste"),
        (565, "D/C"),
    ];
    for (x, label) in header {
        text(t, x, 690, 6, label);
    }
    let mut y = 678;
    for trade in &note.trades {
        text(t, 30, y, 7, "1-BOVESPA");
        text(t, 95, y, 7, trade.side);
        text(t, 115, y, 7, trade.market);
        text(t, 200, y, 7, trade.spec);
        text(t, 335, y, 7, trade.obs);
        text(t, 380, y, 7, trade.quantity);
        text(t, 435, y, 7, trade.price);
        text(t, 500, y, 7, trade.value);
        text(t, 567, y, 7, trade.dc);
        y -= 11;
    }

    match &note.summary {
        None => text(t, 250, 300, 8, "CONTINUA..."),
        Some(s) => {
            text(t, 30, 330, 8, "Resumo dos Negócios");
            text(t, 320, 330, 8, "Resumo Financeiro");
            let left = [
                ("Debêntures", "0,00"),
                ("Vendas à vista", s.sells),
                ("Compras à vista", s.buys),
                ("Opções - compras", "0,00"),
                ("Opções - vendas", "0,00"),
                ("Valor das operações", s.operations),
            ];
            let mut right: Vec<(String, &str, &str)> = s.fees.iter().map(|(l, v, dc)| (l.to_string(), *v, *dc)).collect();
            right.push((format!("I.R.R.F. s/ operações, base R${}", s.irrf_base), s.irrf, ""));
            let rows = left.len().max(right.len());
            let mut y = 318;
            for i in 0..rows {
                if let Some((label, value)) = left.get(i) {
                    text(t, 30, y, 7, label);
                    text(t, 250, y, 7, value);
                }
                if let Some((label, value, dc)) = right.get(i) {
                    text(t, 320, y, 7, label);
                    text(t, 520, y, 7, value);
                    text(t, 567, y, 7, dc);
                }
                y -= 10;
            }
            y -= 6;
            text(t, 320, y, 8, &format!("Líquido para {}", s.net_date));
            text(t, 520, y, 8, s.net);
            text(t, 567, y, 8, s.net_dc);
        }
    }
    ops
}

/// Um PDF com uma página por `SynthNote`; com `password`, criptografado (RC4 128 bits).
pub fn note_pdf(notes: &[SynthNote], password: Option<&str>) -> Vec<u8> {
    build(notes, password, false)
}

/// Mesma nota no formato padrão do lopdf (PDF 1.5 com xref stream), para garantir que a
/// descriptografia não depende da estrutura do arquivo.
pub fn note_pdf_xref_stream(notes: &[SynthNote], password: Option<&str>) -> Vec<u8> {
    build(notes, password, true)
}

fn build(notes: &[SynthNote], password: Option<&str>, xref_stream: bool) -> Vec<u8> {
    // Padrão: PDF 1.4 com tabela xref clássica, como as notas das corretoras.
    let mut doc = Document::with_version(if xref_stream { "1.5" } else { "1.4" });
    if !xref_stream {
        doc.reference_table.cross_reference_type = lopdf::xref::XrefType::CrossReferenceTable;
    }
    let pages_id = doc.new_object_id();
    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
        "Encoding" => "WinAnsiEncoding",
    });
    let resources_id = doc.add_object(dictionary! { "Font" => dictionary! { "F1" => font_id } });

    let mut kids = Vec::new();
    for note in notes {
        let content = Content { operations: page_ops(note) };
        let content_id = doc.add_object(Stream::new(dictionary! {}, content.encode().expect("conteúdo válido")));
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => content_id,
        });
        kids.push(page_id.into());
    }
    let count = kids.len() as i64;
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => kids,
            "Count" => count,
            "Resources" => resources_id,
            "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
        }),
    );
    let catalog_id = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    doc.trailer.set("Root", catalog_id);
    // Criptografia exige um ID de documento no trailer.
    doc.trailer.set("ID", Object::Array(vec![latin1("mafin-test-note-0"), latin1("mafin-test-note-0")]));

    if let Some(password) = password {
        let state = EncryptionState::try_from(EncryptionVersion::V2 {
            document: &doc,
            owner_password: password,
            user_password: password,
            key_length: 128,
            permissions: Permissions::all(),
        })
        .expect("estado de criptografia");
        doc.encrypt(&state).expect("criptografar");
    }

    let mut out = Vec::new();
    doc.save_to(&mut out).expect("salvar PDF");
    out
}

/// Nota típica: compra de PETR4 (nome da empresa, sem ticker), venda de FII com ticker no
/// texto e uma compra fracionária; custos 6,57 e IRRF 0,05.
pub fn sample_notes() -> Vec<SynthNote> {
    vec![SynthNote {
        number: "123456",
        page: 1,
        date: "25/09/2026",
        broker: "XP INVESTIMENTOS CCTVM S/A",
        trades: vec![
            SynthTrade { side: "C", market: "VISTA", spec: "PETROBRAS PN N2", obs: "", quantity: "100", price: "38,45", value: "3.845,00", dc: "D" },
            SynthTrade { side: "V", market: "VISTA", spec: "FII CSHG LOG HGLG11 CI", obs: "", quantity: "10", price: "165,00", value: "1.650,00", dc: "C" },
            SynthTrade { side: "C", market: "FRACIONARIO", spec: "VALE ON NM", obs: "", quantity: "7", price: "61,20", value: "428,40", dc: "D" },
        ],
        summary: Some(SynthSummary {
            sells: "1.650,00",
            buys: "4.273,40",
            operations: "5.923,40",
            fees: vec![
                ("Valor líquido das operações", "2.623,40", "D"),
                ("Taxa de liquidação", "1,48", "D"),
                ("Taxa de Registro", "0,00", "D"),
                ("Total CBLC", "2.624,88", "D"),
                ("Taxa de termo/opções", "0,00", "D"),
                ("Taxa A.N.A.", "0,00", "D"),
                ("Emolumentos", "0,19", "D"),
                ("Total Bovespa / Soma", "0,19", "D"),
                ("Taxa Operacional", "4,90", "D"),
                ("Execução", "0,00", ""),
                ("Taxa de Custódia", "0,00", ""),
                ("Impostos", "0,00", ""),
            ],
            irrf_base: "1.650,00",
            irrf: "0,08",
            net_date: "29/09/2026",
            // −2.623,40 − 6,57 de custos − 0,08 de IRRF
            net: "2.630,05",
            net_dc: "D",
        }),
    }]
}
