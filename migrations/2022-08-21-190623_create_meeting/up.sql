CREATE TABLE public.meeting
(
    id uuid NOT NULL DEFAULT gen_random_uuid (),
    start_date timestamp without time zone NOT NULL DEFAULT now(),
    end_date timestamp without time zone,
    summary_id uuid NOT NULL,
    channel_id text NOT NULL,
    scheduled_cron text NOT NULL,
    PRIMARY KEY (id),
    CONSTRAINT "FK_meeting_summary" FOREIGN KEY (summary_id)
        REFERENCES public.summary (id) MATCH FULL
        ON UPDATE NO ACTION
        ON DELETE NO ACTION
);
