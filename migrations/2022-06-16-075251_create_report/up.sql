CREATE TABLE public.report
(
    id uuid NOT NULL DEFAULT gen_random_uuid (),
    member_id uuid NOT NULL,
    content text NOT NULL,
    create_date date NOT NULL DEFAULT now(),
    published boolean NOT NULL DEFAULT false,
    PRIMARY KEY (id),
    CONSTRAINT "FK_report_member" FOREIGN KEY (member_id)
        REFERENCES public.member (id) MATCH FULL
        ON UPDATE NO ACTION
        ON DELETE NO ACTION
);
