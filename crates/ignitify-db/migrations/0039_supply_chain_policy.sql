CREATE TABLE supply_chain_policy (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    enforcement TEXT NOT NULL DEFAULT 'warning'
        CHECK (enforcement IN ('warning', 'require-provenance')),
    updated_at TEXT NOT NULL
);

INSERT INTO supply_chain_policy (id, enforcement, updated_at)
VALUES (1, 'warning', CURRENT_TIMESTAMP);
