-- Ensure only one active EmailAccess token per user
-- This prevents multiple concurrent email confirmation/change tokens
-- Kind 4 = EmailAccess token
CREATE UNIQUE INDEX idx_one_email_token_per_user ON login_tokens (user_id) WHERE kind = 4;
