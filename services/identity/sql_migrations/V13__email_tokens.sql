ALTER TABLE login_tokens
    ADD COLUMN email VARCHAR(255) NULL,
    -- for EmailVerify and EmailUpdate tokens, an email is required
    ADD CONSTRAINT chk_email CHECK (kind not in(4, 5) OR NOT(email IS NULL OR email = ''));