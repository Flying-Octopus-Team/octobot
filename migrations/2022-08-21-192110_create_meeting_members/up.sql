CREATE TABLE public.meeting_members
(
    -- Include the id column in the table definition, to satisfy Diesel's requirements.
    id uuid NOT NULL DEFAULT gen_random_uuid (),
    member_id uuid NOT NULL,
    meeting_id uuid NOT NULL,
    PRIMARY KEY (id),
    CONSTRAINT "FK_member" FOREIGN KEY (member_id)
        REFERENCES public.member (id) MATCH FULL
        ON UPDATE NO ACTION
        ON DELETE NO ACTION,
    CONSTRAINT "FK_meeting" FOREIGN KEY (meeting_id)
        REFERENCES public.meeting (id) MATCH FULL
        ON UPDATE NO ACTION
        ON DELETE NO ACTION
);
