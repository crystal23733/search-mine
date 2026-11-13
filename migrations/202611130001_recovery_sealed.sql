-- A sealed source is permanently fenced by the managed recovery workflow.
ALTER TABLE service_recovery_control DROP CONSTRAINT service_recovery_control_state_check;
ALTER TABLE service_recovery_control ADD CHECK(state IN ('active','quarantined','sealed'));
CREATE FUNCTION reject_sealed_rights_write() RETURNS trigger
LANGUAGE plpgsql SET search_path=pg_catalog AS $$
DECLARE blocked boolean;
BEGIN
    EXECUTE format('SELECT EXISTS(SELECT 1 FROM %I.service_recovery_control WHERE singleton AND state=''sealed'')', TG_TABLE_SCHEMA) INTO blocked;
    IF blocked THEN RAISE EXCEPTION 'sealed recovery source' USING ERRCODE='55000'; END IF;
    RETURN NULL;
END $$;
CREATE TRIGGER sealed_accounts BEFORE INSERT OR UPDATE OR DELETE OR TRUNCATE ON auth_accounts FOR EACH STATEMENT EXECUTE FUNCTION reject_sealed_rights_write();
CREATE TRIGGER sealed_identities BEFORE INSERT OR UPDATE OR DELETE OR TRUNCATE ON auth_identities FOR EACH STATEMENT EXECUTE FUNCTION reject_sealed_rights_write();
CREATE TRIGGER sealed_account_rights BEFORE INSERT OR UPDATE OR DELETE OR TRUNCATE ON auth_deletion_tombstones FOR EACH STATEMENT EXECUTE FUNCTION reject_sealed_rights_write();
CREATE TRIGGER sealed_identity_rights BEFORE INSERT OR UPDATE OR DELETE OR TRUNCATE ON auth_identity_tombstones FOR EACH STATEMENT EXECUTE FUNCTION reject_sealed_rights_write();
CREATE FUNCTION protect_sealed_recovery_gate() RETURNS trigger
LANGUAGE plpgsql SET search_path=pg_catalog AS $$
BEGIN
    IF OLD.state='sealed' THEN RAISE EXCEPTION 'sealed recovery source' USING ERRCODE='55000'; END IF;
    IF TG_OP='DELETE' THEN RETURN OLD; END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER sealed_gate BEFORE UPDATE OR DELETE ON service_recovery_control FOR EACH ROW EXECUTE FUNCTION protect_sealed_recovery_gate();
CREATE TRIGGER sealed_gate_truncate BEFORE TRUNCATE ON service_recovery_control FOR EACH STATEMENT EXECUTE FUNCTION reject_sealed_rights_write();
