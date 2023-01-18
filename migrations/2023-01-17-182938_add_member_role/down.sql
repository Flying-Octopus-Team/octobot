ALTER TABLE
    IF EXISTS public.member
ADD
    COLUMN is_apprentice boolean NOT NULL DEFAULT false;

UPDATE
    public.member
SET
    is_apprentice = true
WHERE
    role = 1;

ALTER TABLE
    IF EXISTS public.member DROP COLUMN IF EXISTS role;