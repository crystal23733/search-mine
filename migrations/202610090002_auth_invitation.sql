ALTER TABLE auth_transactions ADD COLUMN invite_code text;
ALTER TABLE auth_transactions ADD CONSTRAINT auth_transactions_invitation_check CHECK (
    invite_code IS NULL OR (
        intent = 'login' AND return_path = 'friends'
        AND octet_length(invite_code) = 8
        AND invite_code ~ '^[ABCDEFGHJKLMNPQRSTUVWXYZ23456789]{8}$'
    )
);
