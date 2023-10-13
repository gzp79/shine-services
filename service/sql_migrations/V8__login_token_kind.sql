ALTER TABLE login_tokens
    ADD fingerprint TEXT,
    ADD kind SMALLINT NOT NULL default 3;