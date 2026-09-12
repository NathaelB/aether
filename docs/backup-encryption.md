# What "encrypted" means for an archive

Two different mechanisms get called encryption at rest, and they protect
against different people. Writing "archives are encrypted" without saying which
one is in play is the sentence that gets quoted back during an audit, so this
page says which one this platform implements today and which one it does not.

## What is implemented

**The object store encrypts what it is given.** Every object this platform
writes asks for server side encryption, and the store refuses the write rather
than quietly storing plaintext if it cannot do it.

What that protects: the disks under the bucket, a stolen backup drive, a
misconfigured bucket listing, and anybody who obtains the raw storage without
obtaining the store's credentials.

What it does not protect against: **the provider.** The store holds the key and
decrypts on read. A provider employee with sufficient access, or anybody who
compels the provider, can read an archive. Choosing `kms:<key id>` narrows who
at the provider can do it and does not change who performs the decryption.

**Data keys are wrapped by a key manager.** A data key is generated per use,
wrapped by a key held in OpenBao, Vault or a cloud key manager, and only ever
stored wrapped. The key manager never sees archive contents, only 32 bytes.

What that buys today: rotation costs a rewrap of a few hundred bytes instead of
rewriting terabytes, and destroying the wrapping key makes everything wrapped
under it unrecoverable without touching a single object in the bucket.

**A rotation never rewrites history.** Every archive records the key version it
was wrapped under, and rotating adds a version rather than replacing one.
Archives taken before a rotation stay readable, under the version they recorded.
This is covered by a test against a real key manager, not by a convention.

## What is not implemented

**Encryption the platform cannot bypass.** An archive encrypted on the data
plane, before it leaves the cluster, under a key held in the customer's own key
manager, which this platform can ask to unwrap and can never read directly.

That is what a customer means when they ask for bring your own key, and it is
the only version where the answer to "can Aether read my backups" is no rather
than "not in practice". It needs a different write path: a logical dump
streamed through compression and per archive envelope encryption, rather than
the object store's own encryption of what it receives.

It is a separate chantier. Until it lands, the honest answer to a customer
asking whether the platform can read their archives is **yes, and here is what
would have to be true for that to happen.**

## Configuration

| | |
|---|---|
| `OBJECT_STORE_ENCRYPTION` | `managed` (the store's own key, the default), `kms:<key id>`, or `none` |
| `KEY_MANAGER_ADDRESS` | where data keys are wrapped |
| `KEY_MANAGER_KEY` | the wrapping key; empty means this installation wraps nothing, and it is logged as the decision it is |

An installation that cannot reach either one still starts and still serves.
Backups stop, authentication does not, and both say so on the first line of the
logs. The alternative to noticing there is noticing during a restore.

## Locally

`docker compose up -d rustfs openbao` brings up both. RustFS needs
`RUSTFS_SSE_S3_MASTER_KEY` set to a base64 encoded 32 bytes before it will
accept an encrypted write, which `docker-compose.yaml` does with a fixed value.
That value is the local stand in for a provider held key. It is not a secret
and it is not the key story above.

OpenBao runs in dev mode: in memory, unsealed, one known root token. Fine for a
throwaway stack and for nothing else.

```bash
make test-keys          # against the real key manager
make test-objectstore   # against the real store
```
