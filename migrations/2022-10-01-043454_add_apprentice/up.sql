ALTER TABLE IF EXISTS public.member
    ADD COLUMN is_apprentice boolean NOT NULL DEFAULT false;
