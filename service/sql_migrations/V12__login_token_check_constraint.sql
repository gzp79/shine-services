ALTER TABLE login_tokens
    ADD CONSTRAINT chk_fingerprint CHECK (kind <> 3 OR NOT(fingerprint IS NULL OR fingerprint = ''));