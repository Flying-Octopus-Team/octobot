CREATE TABLE IF NOT EXISTS public.member
(
    id uuid NOT NULL DEFAULT gen_random_uuid (),
    discord_id text,
    trello_id text,
    trello_report_card_id text,
    PRIMARY KEY (id)
);
