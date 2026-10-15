CREATE TABLE metric_collection (
    singleton boolean PRIMARY KEY CHECK (singleton),
    started_at bigint NOT NULL CHECK (started_at BETWEEN 0 AND 253402300799),
    coverage_since bigint NOT NULL CHECK (coverage_since BETWEEN started_at AND 253402300799)
);
CREATE FUNCTION preserve_metric_collection_start() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.singleton IS DISTINCT FROM OLD.singleton
       OR NEW.started_at IS DISTINCT FROM OLD.started_at
       OR NEW.coverage_since < OLD.coverage_since THEN
        RAISE EXCEPTION 'Metric collection boundary cannot move backwards' USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER metric_collection_monotonic BEFORE UPDATE ON metric_collection
    FOR EACH ROW EXECUTE FUNCTION preserve_metric_collection_start();
