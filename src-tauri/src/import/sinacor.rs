//! Leitura do texto extraído de notas de corretagem no layout SINACOR.
//!
//! Funciona sobre texto (uma `String` por página), sem depender do PDF: é o que permite
//! testar com fixtures. Tolerante a variações: cada informação é procurada por rótulo em
//! qualquer ponto da linha, porque os quadros "Resumo dos Negócios" e "Resumo Financeiro"
//! ficam lado a lado e a extração os junta na mesma linha.

use std::sync::LazyLock;

use chrono::NaiveDate;
use regex::Regex;
use rust_decimal::Decimal;
use shared::OrderKind;

/// Um negócio como aparece na nota (antes de resolver o ticker).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawTrade {
    pub side: OrderKind,
    pub market: String,
    pub spec: String,
    pub quantity: Decimal,
    pub price: Decimal,
    pub value: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawNote {
    pub number: String,
    pub trade_date: NaiveDate,
    pub broker: Option<String>,
    pub trades: Vec<RawTrade>,
    /// (rótulo, valor) das taxas que compõem o custo.
    pub fee_lines: Vec<(String, Decimal)>,
    pub irrf: Decimal,
    /// Líquido com sinal (+ crédito, − débito para o cliente).
    pub net: Option<Decimal>,
    pub summary_sells: Option<Decimal>,
    pub summary_buys: Option<Decimal>,
    pub warnings: Vec<String>,
}

const MONEY: &str = r"-?\d{1,3}(?:\.\d{3})*,\d{2}";

static TRADE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)
        (?:\d-)?(?:BOVESPA|B3\s*RV\s*LISTADO|B3RV)\s+
        (?P<side>[CV])\s+
        (?P<market>VISTA|FRACION[AÁ]RIO|OP[CÇ][AÃ]O\s+DE\s+COMPRA|OP[CÇ][AÃ]O\s+DE\s+VENDA|EXERC\s+OPC\s+COMPRA|EXERC\s+OPC\s+VENDA|TERMO)\s+
        (?:\d{2}/\d{2}\s+)?            # prazo (opções/termo)
        (?P<spec>.+?)\s+
        (?P<qty>\d{1,3}(?:\.\d{3})*)\s+
        (?P<price>\d{1,3}(?:\.\d{3})*,\d{2,8})\s+
        (?P<value>\d{1,3}(?:\.\d{3})*,\d{2})\s+
        (?P<dc>[DC])\b",
    )
    .unwrap()
});
static HEADER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?is)Nr\.?\s*nota.*?Data\s+preg\S*\s+(?P<number>\d+)\s+\d+\s+(?P<date>\d{2}/\d{2}/\d{4})").unwrap()
});
static NUMBER_FALLBACK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)Nr\.?\s*nota\D{0,40}?(\d{3,})").unwrap());
static DATE_FALLBACK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)Data\s+preg\S*\D{0,60}?(\d{2}/\d{2}/\d{4})").unwrap());
static NET: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(r"(?i)L[ií]quido\s+para\s+\d{{2}}/\d{{2}}/\d{{4}}\s+(?P<value>{MONEY})\s*(?P<dc>[DC])?")).unwrap()
});
static IRRF: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"(?i)I\.?\s?R\.?\s?R\.?\s?F\.?\s*s/\s*opera\S*(?:\s*,?\s*base\s*R\$\s*{MONEY})?\s+(?P<value>{MONEY})"
    ))
    .unwrap()
});

/// Taxas que compõem o custo da nota (os "Total …" são subtotais e ficam de fora).
const FEE_LABELS: [(&str, &str); 11] = [
    (r"Taxa\s+de\s+liquida\S*", "Taxa de liquidação"),
    (r"Taxa\s+de\s+registro", "Taxa de registro"),
    (r"Taxa\s+de\s+termo/op\S*", "Taxa de termo/opções"),
    (r"Taxa\s+A\.?N\.?A\.?", "Taxa A.N.A."),
    (r"Emolumentos", "Emolumentos"),
    (r"Taxa\s+Operacional|Corretagem", "Taxa operacional"),
    (r"Execu\S*o(?:\s+casa)?", "Execução"),
    (r"Taxa\s+de\s+cust\S*dia", "Taxa de custódia"),
    (r"Impostos|I\.?S\.?S\.?(?:\s*\(SÃO PAULO\))?", "Impostos (ISS)"),
    (r"Outros", "Outros"),
    (r"Taxa\s+de\s+transfer\S*\s+de\s+ativos", "Taxa de transferência de ativos"),
];
static FEES: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    FEE_LABELS
        .iter()
        .map(|(pattern, label)| (Regex::new(&format!(r"(?i)\b(?:{pattern})\s+(?P<value>{MONEY})")).unwrap(), *label))
        .collect()
});
static SELLS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!(r"(?i)Vendas\s+[àa]\s+vista\s+(?P<value>{MONEY})")).unwrap());
static BUYS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!(r"(?i)Compras\s+[àa]\s+vista\s+(?P<value>{MONEY})")).unwrap());

/// Corretoras reconhecidas no cabeçalho → nome curto usado no app.
const BROKERS: [(&str, &str); 17] = [
    ("XP INVESTIMENTOS", "XP"),
    ("RICO INVESTIMENTOS", "Rico"),
    ("CLEAR CORRETORA", "Clear"),
    ("BTG PACTUAL", "BTG Pactual"),
    ("INTER DISTRIBUIDORA", "Inter"),
    ("INTER DTVM", "Inter"),
    ("NU INVEST", "NuInvest"),
    ("NUINVEST", "NuInvest"),
    ("EASYNVEST", "NuInvest"),
    ("ITAU CORRETORA", "Itaú"),
    ("ITAÚ CORRETORA", "Itaú"),
    ("GENIAL", "Genial"),
    ("MODAL", "Modal"),
    ("TORO ", "Toro"),
    ("ÁGORA", "Ágora"),
    ("SAFRA", "Safra"),
    ("C6 CTVM", "C6"),
];

/// "1.234,56" → 1234.56
pub fn br_decimal(text: &str) -> Option<Decimal> {
    text.trim().replace('.', "").replace(',', ".").parse().ok()
}

/// "1.000" → 1000 (quantidade: ponto é separador de milhar).
fn br_integer(text: &str) -> Option<Decimal> {
    text.trim().replace('.', "").parse().ok()
}

fn first_capture(re: &Regex, text: &str) -> Option<Decimal> {
    re.captures(text).and_then(|c| br_decimal(&c["value"]))
}

/// Remove da especificação as marcações de "Obs." (#, D, F, 2…), que vêm antes da quantidade.
fn clean_spec(spec: &str) -> String {
    let mut tokens: Vec<&str> = spec.split_whitespace().collect();
    while tokens.len() > 1 && tokens.last().is_some_and(|t| t.starts_with('#') || t.chars().count() == 1) {
        tokens.pop();
    }
    tokens.join(" ")
}

fn detect_broker(text: &str) -> Option<String> {
    let upper = text.to_uppercase();
    BROKERS.iter().find(|(needle, _)| upper.contains(needle)).map(|(_, name)| name.to_string())
}

/// Lê uma página. `None` se não houver cabeçalho de nota nem negócios.
fn parse_page(text: &str) -> Option<RawNote> {
    let mut warnings = Vec::new();
    let (number, date) = match HEADER.captures(text) {
        Some(c) => (c["number"].to_string(), c["date"].to_string()),
        None => {
            let number = NUMBER_FALLBACK.captures(text).map(|c| c[1].to_string());
            let date = DATE_FALLBACK.captures(text).map(|c| c[1].to_string());
            match (number, date) {
                (Some(n), Some(d)) => (n, d),
                _ => return None,
            }
        }
    };
    let trade_date = NaiveDate::parse_from_str(&date, "%d/%m/%Y").ok()?;

    let mut trades = Vec::new();
    for line in text.lines() {
        let Some(c) = TRADE.captures(line) else { continue };
        let market = c["market"].to_uppercase().replace('Á', "A");
        let spec = clean_spec(&c["spec"]);
        if market != "VISTA" && market != "FRACIONARIO" {
            warnings.push(format!("Ignorado (mercado {market}): {spec}"));
            continue;
        }
        let (Some(quantity), Some(price), Some(value)) = (br_integer(&c["qty"]), br_decimal(&c["price"]), br_decimal(&c["value"]))
        else {
            warnings.push(format!("Linha não entendida: {}", line.trim()));
            continue;
        };
        if (quantity * price - value).abs() > Decimal::new(1, 2) {
            warnings.push(format!("{spec}: quantidade × preço difere do valor da operação"));
        }
        let side = if &c["side"] == "C" { OrderKind::Buy } else { OrderKind::Sell };
        trades.push(RawTrade { side, market, spec, quantity, price, value });
    }

    let fee_lines = FEES
        .iter()
        .filter_map(|(re, label)| first_capture(re, text).map(|v| (label.to_string(), v)))
        .filter(|(_, v)| !v.is_zero())
        .collect();

    let net = NET.captures(text).and_then(|c| {
        let value = br_decimal(&c["value"])?;
        Some(if c.name("dc").is_some_and(|m| m.as_str() == "D") { -value } else { value })
    });

    Some(RawNote {
        number,
        trade_date,
        broker: detect_broker(text),
        trades,
        fee_lines,
        irrf: first_capture(&IRRF, text).unwrap_or_default(),
        net,
        summary_sells: first_capture(&SELLS, text),
        summary_buys: first_capture(&BUYS, text),
        warnings,
    })
}

/// Lê todas as páginas e junta as folhas de uma mesma nota (mesmo número e data).
pub fn parse_pages(pages: &[String]) -> Vec<RawNote> {
    let mut notes: Vec<RawNote> = Vec::new();
    for page in pages {
        let Some(page_note) = parse_page(page) else { continue };
        match notes.iter_mut().find(|n| n.number == page_note.number && n.trade_date == page_note.trade_date) {
            Some(note) => {
                note.trades.extend(page_note.trades);
                note.warnings.extend(page_note.warnings);
                note.broker = note.broker.take().or(page_note.broker);
                // O resumo fica na última folha.
                if page_note.net.is_some() {
                    note.fee_lines = page_note.fee_lines;
                    note.irrf = page_note.irrf;
                    note.net = page_note.net;
                    note.summary_sells = page_note.summary_sells;
                    note.summary_buys = page_note.summary_buys;
                }
            }
            None => notes.push(page_note),
        }
    }
    notes
}

impl RawNote {
    pub fn sells(&self) -> Decimal {
        self.trades.iter().filter(|t| t.side == OrderKind::Sell).map(|t| t.value).sum()
    }

    pub fn buys(&self) -> Decimal {
        self.trades.iter().filter(|t| t.side == OrderKind::Buy).map(|t| t.value).sum()
    }

    /// Custos da nota (sem IRRF). Preferência: deduzido do líquido, que é o valor que de fato
    /// saiu/entrou na conta; a soma das taxas discriminadas serve de conferência.
    pub fn costs(&self) -> (Decimal, Vec<String>) {
        let mut warnings = Vec::new();
        let from_lines: Decimal = self.fee_lines.iter().map(|(_, v)| *v).sum();
        let costs = match self.net {
            Some(net) => {
                let from_net = self.sells() - self.buys() - net - self.irrf;
                if (from_net - from_lines).abs() > Decimal::new(1, 2) && !self.fee_lines.is_empty() {
                    warnings.push(format!(
                        "Custos pelo líquido ({}) diferem da soma das taxas ({}); usado o valor pelo líquido",
                        shared::format::format_brl(from_net),
                        shared::format::format_brl(from_lines),
                    ));
                }
                from_net
            }
            None => {
                warnings.push("Linha \"Líquido para\" não encontrada: custos pela soma das taxas".into());
                from_lines
            }
        };
        if costs.is_sign_negative() {
            warnings.push("Custos calculados negativos; considerados zero. Confira a nota.".into());
            return (Decimal::ZERO, warnings);
        }
        for (label, summary, parsed) in [("Vendas", self.summary_sells, self.sells()), ("Compras", self.summary_buys, self.buys())] {
            if let Some(summary) = summary
                && summary != parsed
            {
                warnings.push(format!(
                    "{label} à vista no resumo ({}) diferem da soma dos negócios lidos ({})",
                    shared::format::format_brl(summary),
                    shared::format::format_brl(parsed),
                ));
            }
        }
        (costs, warnings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    /// Texto exatamente como o `pdf-extract` devolve para a nota sintética (ver testpdf).
    const PAGE: &str = "
NOTA DE CORRETAGEM
 Nr. nota Folha Data pregão
123456 1 25/09/2026

XP INVESTIMENTOS CCTVM S/A
Negócios realizados
Q Negociação C/V Tipo mercado Prazo Especificação do título Obs. (*) Quantidade Preço / Ajuste Valor Operação / Ajuste D/C
1-BOVESPA C VISTA PETROBRAS PN N2 100 38,45 3.845,00 D
1-BOVESPA V VISTA FII CSHG LOG HGLG11 CI 10 165,00 1.650,00 C
1-BOVESPA C FRACIONARIO VALE ON NM 7 61,20 428,40 D
Resumo dos Negócios Resumo Financeiro
Debêntures 0,00 Valor líquido das operações 2.623,40 D
Vendas à vista 1.650,00 Taxa de liquidação 1,48 D
Compras à vista 4.273,40 Taxa de Registro 0,00 D
Opções - compras 0,00 Total CBLC 2.624,88 D
Valor das operações 5.923,40 Taxa A.N.A. 0,00 D
Emolumentos 0,19 D
Total Bovespa / Soma 0,19 D
Taxa Operacional 4,90 D
Impostos 0,00
I.R.R.F. s/ operações, base R$1.650,00 0,08
Líquido para 29/09/2026 2.630,05 D
";

    #[test]
    fn parses_header_trades_and_summary() {
        let notes = parse_pages(&[PAGE.to_string()]);
        assert_eq!(notes.len(), 1);
        let n = &notes[0];
        assert_eq!((n.number.as_str(), n.trade_date.to_string().as_str()), ("123456", "2026-09-25"));
        assert_eq!(n.broker.as_deref(), Some("XP"));
        let trades: Vec<_> = n.trades.iter().map(|t| (t.side, t.spec.as_str(), t.quantity, t.price, t.value)).collect();
        assert_eq!(
            trades,
            [
                (OrderKind::Buy, "PETROBRAS PN N2", dec!(100), dec!(38.45), dec!(3845)),
                (OrderKind::Sell, "FII CSHG LOG HGLG11 CI", dec!(10), dec!(165), dec!(1650)),
                (OrderKind::Buy, "VALE ON NM", dec!(7), dec!(61.20), dec!(428.40)),
            ]
        );
        assert_eq!(n.irrf, dec!(0.08));
        assert_eq!(n.net, Some(dec!(-2630.05)));
        let (costs, warnings) = n.costs();
        assert_eq!(costs, dec!(6.57));
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(
            n.fee_lines,
            [("Taxa de liquidação".into(), dec!(1.48)), ("Emolumentos".into(), dec!(0.19)), ("Taxa operacional".into(), dec!(4.90))]
        );
    }

    #[test]
    fn obs_markers_thousands_and_options() {
        let page = "Nr. nota Folha Data pregão\n777 1 02/03/2026\n\
            1-BOVESPA C VISTA ITAUUNIBANCO PN N1 D # 1.000 32,10 32.100,00 D\n\
            1-BOVESPA V OPCAO DE COMPRA 03/26 PETRC400 PN 100 1,20 120,00 C\n\
            Líquido para 04/03/2026 32.110,00 D\n";
        let n = &parse_pages(&[page.to_string()])[0];
        assert_eq!(n.trades.len(), 1);
        assert_eq!(n.trades[0].spec, "ITAUUNIBANCO PN N1");
        assert_eq!(n.trades[0].quantity, dec!(1000));
        assert!(n.warnings.iter().any(|w| w.contains("OPCAO DE COMPRA")), "{:?}", n.warnings);
        // Sem taxas discriminadas: custos pelo líquido.
        assert_eq!(n.costs().0, dec!(10));
    }

    #[test]
    fn merges_pages_of_the_same_note() {
        let first = "Nr. nota Folha Data pregão\n900 1 10/08/2026\n1-BOVESPA C VISTA WEG ON NM 10 40,00 400,00 D\nCONTINUA...";
        let last = "Nr. nota Folha Data pregão\n900 2 10/08/2026\n1-BOVESPA C VISTA WEG ON NM 5 41,00 205,00 D\nLíquido para 12/08/2026 605,50 D";
        let notes = parse_pages(&[first.to_string(), last.to_string()]);
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].trades.len(), 2);
        assert_eq!(notes[0].costs().0, dec!(0.50));
    }

    #[test]
    fn warns_when_net_is_missing_or_inconsistent() {
        let page = "Nr. nota Folha Data pregão\n1 1 10/08/2026\n1-BOVESPA C VISTA WEG ON NM 10 40,00 400,00 D\nEmolumentos 0,05 D\n";
        let (costs, warnings) = parse_pages(&[page.to_string()])[0].costs();
        assert_eq!(costs, dec!(0.05));
        assert!(warnings[0].contains("Líquido para"));

        let page = format!("{page}Líquido para 12/08/2026 401,00 D\n");
        let (costs, warnings) = parse_pages(&[page])[0].costs();
        assert_eq!(costs, dec!(1));
        assert!(warnings[0].contains("diferem da soma das taxas"), "{warnings:?}");
    }

    #[test]
    fn ignores_pages_without_note() {
        assert!(parse_pages(&["Página em branco".to_string()]).is_empty());
    }
}
