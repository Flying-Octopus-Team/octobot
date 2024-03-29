CREATE TABLE IF NOT EXISTS public.member
(
    id uuid NOT NULL DEFAULT gen_random_uuid (),
    display_name text NOT NULL,
    discord_id text UNIQUE,
    trello_id text UNIQUE,
    trello_report_card_id text UNIQUE,
    PRIMARY KEY (id)
);
