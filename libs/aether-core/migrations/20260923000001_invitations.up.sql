-- How somebody who has never signed in gets into an organisation.
--
-- They have no user row until their first authenticated request, so there is
-- nothing to attach a membership to. An invitation is a promise of one,
-- addressed to an email rather than to a person the platform knows.
CREATE TABLE invitations (
    id UUID PRIMARY KEY,
    organisation_id UUID NOT NULL REFERENCES organisations(id) ON DELETE CASCADE,

    -- Stored lowercased, because that is how it is matched against the email
    -- of whoever turns up holding the link. Comparing two spellings of the
    -- same address and calling them different is how an invitation stops
    -- working for the person it was written for.
    email TEXT NOT NULL,

    -- The hash, never the secret. A readable invitations table is a set of
    -- live credentials, which is the same reason a passwords table holds
    -- hashes -- except these grant entry to somebody else's organisation.
    token_hash TEXT NOT NULL UNIQUE,

    expires_at TIMESTAMPTZ NOT NULL,
    invited_by UUID NULL REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    -- Both nullable and both kept. An accepted invitation is the record of
    -- how somebody got in, and deleting it on acceptance would lose that.
    accepted_at TIMESTAMPTZ NULL,
    revoked_at TIMESTAMPTZ NULL
);

-- One outstanding invitation per address per organisation. Two live links to
-- the same inbox is two ways in, and revoking the one somebody remembers
-- leaves the other working.
CREATE UNIQUE INDEX idx_invitations_one_outstanding
    ON invitations (organisation_id, email)
    WHERE accepted_at IS NULL AND revoked_at IS NULL;

CREATE INDEX idx_invitations_organisation ON invitations (organisation_id);

-- What they will hold once they are in. Cascades with the role, so deleting a
-- role does not leave an invitation promising something that no longer exists.
CREATE TABLE invitation_roles (
    invitation_id UUID NOT NULL REFERENCES invitations(id) ON DELETE CASCADE,
    role_id UUID NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
    PRIMARY KEY (invitation_id, role_id)
);

CREATE INDEX idx_invitation_roles_role ON invitation_roles (role_id);

ALTER TABLE invitations
    ADD CONSTRAINT invitations_email_is_lowercase CHECK (email = lower(email)),
    ADD CONSTRAINT invitations_email_looks_like_one CHECK (email ~ '^[^@[:space:]]+@[^@[:space:]]+$');
