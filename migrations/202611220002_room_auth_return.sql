ALTER TABLE auth_transactions DROP CONSTRAINT auth_transactions_return_path_check;
ALTER TABLE auth_transactions ADD CONSTRAINT auth_transactions_return_path_check
    CHECK (return_path IN ('home', 'daily', 'friends', 'rooms', 'settings', 'me', 'account'));

ALTER TABLE auth_transactions DROP CONSTRAINT auth_transactions_invitation_check;
ALTER TABLE auth_transactions ADD CONSTRAINT auth_transactions_invitation_check CHECK (
    invite_code IS NULL OR (
        intent = 'login' AND return_path IN ('friends', 'rooms')
        AND octet_length(invite_code) = 8
        AND invite_code ~ '^[ABCDEFGHJKLMNPQRSTUVWXYZ23456789]{8}$'
    )
);
