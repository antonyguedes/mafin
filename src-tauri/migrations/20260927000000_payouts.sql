-- Proventos (dividendos, JCP, rendimentos de FII). Mesmas convenções do esquema inicial:
-- valores em TEXT (Decimal canônico), datas ISO, enums em snake_case.
CREATE TABLE payouts (
    id          INTEGER PRIMARY KEY,
    -- NO ACTION: um ativo com proventos não pode ser excluído.
    asset_id    INTEGER NOT NULL REFERENCES assets (id),
    kind        TEXT NOT NULL CHECK (kind IN ('dividend', 'jcp', 'fii_income', 'other')),
    -- Data de pagamento (competência na declaração).
    date        TEXT NOT NULL CHECK (date IS date(date)),
    gross       TEXT NOT NULL,
    withheld    TEXT NOT NULL DEFAULT '0',
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX idx_payouts_date ON payouts (date);
CREATE INDEX idx_payouts_asset ON payouts (asset_id);
