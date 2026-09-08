# dev-methodology preferences (user answers — change only when the user does)
commit: claude-allowed          # feature branches only, never main
create-pr: claude-allowed
merge-pr: human-only            # prepare everything, hand off the button
feature-merge: squash           # feature -> main
workstream-merge: squash        # squash everywhere
tracking: chantier-issue + sub-issues
reviewers: none                 # no CODEOWNERS, single-maintainer repo
assignee: @me
labels: bug, documentation, duplicate, enhancement, good first issue, help wanted, invalid, question, wontfix
model-strategy: split           # orchestration = Opus 5, implementation sub-agents = fast model
attribution: none               # NEVER add Co-Authored-By: Claude or any AI attribution
