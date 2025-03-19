CREATE TABLE roles (
    user_id UUID NOT NULL,
    role TEXT NOT NULL,
    CONSTRAINT fkey_user_id FOREIGN KEY (user_id) REFERENCES identities (user_id) ON DELETE CASCADE
);

CREATE INDEX roles_idx_user_id ON roles (user_id);

CREATE UNIQUE INDEX roles_idx_user_id_role ON roles (user_id, role);

CREATE INDEX roles_idx_role ON roles (role);