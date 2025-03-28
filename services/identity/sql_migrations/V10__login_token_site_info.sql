ALTER TABLE login_tokens
ADD agent TEXT NOT NULL default '',
ADD country TEXT default NULL,
ADD region TEXT default NULL,
ADD city TEXT default NULL;