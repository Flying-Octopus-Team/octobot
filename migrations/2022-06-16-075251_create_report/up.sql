CREATE TABLE public.report
(
    id uuid NOT NULL DEFAULT gen_random_uuid (),
    member_uuid uuid NOT NULL,
    content text NOT NULL,
    create_date date NOT NULL DEFAULT now(),
    PRIMARY KEY (id),
    CONSTRAINT "FK_report_member" FOREIGN KEY (member_uuid)
        REFERENCES public.member (id) MATCH FULL
        ON UPDATE NO ACTION
        ON DELETE NO ACTION
);
