ALTER TABLE external_logins
DROP CONSTRAINT fkey_user_id;

ALTER TABLE external_logins
ADD CONSTRAINT fkey_user_id
FOREIGN KEY (user_id)
REFERENCES identities(user_id)
ON DELETE CASCADE;