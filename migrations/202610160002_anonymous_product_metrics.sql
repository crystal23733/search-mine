CREATE TABLE product_metric_aggregates (
    metric text NOT NULL CHECK (metric IN ('weekly_nsm','d7')),
    period date NOT NULL CHECK (period BETWEEN DATE '1970-01-01' AND DATE '9999-12-31'),
    computed_at bigint NOT NULL CHECK (computed_at BETWEEN 0 AND 253402300799),
    observed_through bigint NOT NULL CHECK (observed_through BETWEEN 0 AND computed_at),
    expires_at bigint NOT NULL CHECK (expires_at BETWEEN computed_at+1 AND 253402300799),
    complete boolean NOT NULL,
    participants integer,
    repeat_accounts integer,
    human_participations integer,
    bot_participations integer,
    sample_size integer,
    retained integer,
    PRIMARY KEY(metric,period),
    CHECK (
        (metric='weekly_nsm' AND participants IS NOT NULL AND participants BETWEEN 0 AND 10000
         AND repeat_accounts IS NOT NULL AND repeat_accounts BETWEEN 0 AND participants
         AND human_participations IS NOT NULL AND human_participations BETWEEN 0 AND 10000
         AND bot_participations IS NOT NULL AND bot_participations BETWEEN 0 AND 10000
         AND human_participations+bot_participations BETWEEN participants+repeat_accounts AND 10000
         AND (repeat_accounts<>0 OR human_participations+bot_participations=participants)
         AND (participants<>0 OR human_participations+bot_participations=0)
         AND sample_size IS NULL AND retained IS NULL)
        OR
        (metric='d7' AND participants IS NULL AND repeat_accounts IS NULL AND human_participations IS NULL AND bot_participations IS NULL
         AND sample_size IS NOT NULL AND sample_size BETWEEN 0 AND 10000
         AND ((complete AND retained IS NOT NULL AND retained BETWEEN 0 AND sample_size) OR (NOT complete AND retained IS NULL)))
    )
);
CREATE INDEX product_metric_expiry ON product_metric_aggregates(expires_at);
CREATE FUNCTION preserve_product_metric_period() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.metric IS DISTINCT FROM OLD.metric OR NEW.period IS DISTINCT FROM OLD.period OR NEW.expires_at IS DISTINCT FROM OLD.expires_at
       OR NEW.computed_at < OLD.computed_at OR NEW.observed_through < OLD.observed_through OR (OLD.complete AND NOT NEW.complete)
       OR (NEW.computed_at=OLD.computed_at AND NEW IS DISTINCT FROM OLD) THEN
        RAISE EXCEPTION 'Metric period and freshness cannot regress' USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER product_metric_period_immutable BEFORE UPDATE ON product_metric_aggregates
    FOR EACH ROW EXECUTE FUNCTION preserve_product_metric_period();
