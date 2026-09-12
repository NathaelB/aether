-- Who let this person in.
--
-- Nullable, and it will stay nullable: the owner joined by creating the
-- organisation, so there is nobody to name. A default of "themselves" would
-- read as somebody having invited themselves, which is a different fact.
--
-- SET NULL rather than CASCADE: the person who invited somebody leaving does
-- not unmake the membership, and losing the record of who invited them is a
-- smaller loss than losing the member.
ALTER TABLE members
    ADD COLUMN invited_by UUID NULL REFERENCES users(id) ON DELETE SET NULL;

CREATE INDEX idx_members_invited_by ON members (invited_by) WHERE invited_by IS NOT NULL;
