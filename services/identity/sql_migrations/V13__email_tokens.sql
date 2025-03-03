ALTER TABLE login_tokens
ADD COLUMN email VARCHAR(255) NULL,
-- for EmailAccess token, a bound email is required
ADD CONSTRAINT chk_email CHECK (
    kind not in (4)
    OR NOT (
        email IS NULL
        OR email = ''
    )
);