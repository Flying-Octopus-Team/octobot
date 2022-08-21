CREATE TABLE public.summary
(
    id uuid NOT NULL DEFAULT gen_random_uuid (),
    content text NOT NULL,
    create_date date NOT NULL DEFAULT now(),
    PRIMARY KEY (id)
);
