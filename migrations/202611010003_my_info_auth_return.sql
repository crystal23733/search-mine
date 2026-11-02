-- Only fixed local tokens are persisted; arbitrary redirect URLs remain forbidden.
ALTER TABLE auth_transactions DROP CONSTRAINT auth_transactions_return_path_check;
ALTER TABLE auth_transactions ADD CONSTRAINT auth_transactions_return_path_check
    CHECK (return_path IN ('home', 'daily', 'friends', 'settings', 'me', 'account'));
