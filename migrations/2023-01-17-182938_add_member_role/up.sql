ALTER TABLE
    IF EXISTS public.member
ADD
    COLUMN role integer NOT NULL DEFAULT 0;

UPDATE
    public.member
SET
    role = 1
WHERE
    is_apprentice = true;

ALTER TABLE
    IF EXISTS public.member DROP COLUMN IF EXISTS is_apprentice;
