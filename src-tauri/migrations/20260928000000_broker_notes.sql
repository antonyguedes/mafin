-- Importação de notas de corretagem.

-- Especificação do título (normalizada, ex.: "PETROBRAS PN N2") -> ticker confirmado pelo
-- usuário. Reaproveitada nas próximas importações.
CREATE TABLE security_aliases (
    spec    TEXT PRIMARY KEY,
    ticker  TEXT NOT NULL CHECK (length(ticker) > 0)
);

-- Notas já importadas: evita importar a mesma nota duas vezes e permite desfazer.
CREATE TABLE imported_notes (
    id           INTEGER PRIMARY KEY,
    broker       TEXT NOT NULL CHECK (length(broker) > 0),
    number       TEXT NOT NULL,
    trade_date   TEXT NOT NULL CHECK (trade_date IS date(trade_date)),
    -- IRRF da nota, somado ao IRRF do mês na importação (e subtraído ao desfazer).
    irrf         TEXT NOT NULL DEFAULT '0',
    imported_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    UNIQUE (broker, number, trade_date)
);

-- Ordens criadas por uma importação apontam para a nota (NULL = cadastro manual).
ALTER TABLE orders ADD COLUMN note_id INTEGER REFERENCES imported_notes (id);
CREATE INDEX idx_orders_note ON orders (note_id);
