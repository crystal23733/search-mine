-- Bind UI locale to the one-time server transaction, preserving migrated records.
ALTER TABLE auth_transactions ADD COLUMN locale varchar(5) NOT NULL DEFAULT 'en'
    CHECK (locale IN ('en','ko','ja','zh-CN','es','pt-BR','de','fr'));
