# The realm and OIDC clients Aether needs, declared rather than created.
#
# This replaces a script that drove the API by hand. The provider is FerrisKey's
# own, so it stays in step with the API, it manages the redirect URIs the script
# never created, and it converges -- which `ferris-ctl realm import` does not:
# that command creates, and returns a 500 on a realm that already exists.
#
# One thing still needs the API: at provider v0.1.0 `ferriskey_client.secret`
# holds "***", the mask the API returns in the client body, so the generated
# secret is read from the client-secret endpoint in scripts/bootstrap-ferriskey.sh.

terraform {
  required_version = ">= 1.5"

  required_providers {
    ferriskey = {
      source  = "ferriskey/ferriskey"
      version = "~> 0.1"
    }
  }
}

# Phase 1 of the provider's bootstrap guide: the password grant against the
# public admin-cli client, using the admin account the instance ships with.
#
# The guide's phase 2 -- a dedicated `terraform-runner` service account with
# scoped roles and its own state -- is what a shared instance should use. This
# configuration is for a local stack that is thrown away, where a second phase
# would be ceremony around an admin password that is already `admin`.
provider "ferriskey" {
  url       = var.ferriskey_url
  realm     = "master"
  client_id = "admin-cli"
  username  = var.admin_username
  password  = var.admin_password
}

resource "ferriskey_realm" "aether" {
  name = var.realm
}

resource "ferriskey_realm_settings" "aether" {
  realm = ferriskey_realm.aether.name

  # Short-lived access tokens are why Herald refreshes rather than holding one:
  # the control plane checks `exp`, so a fixed token dies quietly.
  access_token_lifetime  = 300
  refresh_token_lifetime = 1800

  # Self-registration, so a local stack can be signed into without a password
  # having to be provisioned out of band.
  #
  # This is not a convenience that happens to be enabled -- it is the only way
  # in. FerrisKey has no admin API for setting another user's password:
  # `/users/{id}/credentials` is read-only, and `login-actions/update-password`
  # applies to whoever holds the token. So a user created by Terraform exists
  # and cannot log in.
  #
  # Correct for a throwaway stack whose admin password is `admin`, and wrong
  # for anything shared: on a real instance this is off, and accounts come from
  # an identity provider or an invitation.
  user_registration_enabled = var.allow_self_registration
}

# The web console. Public because a browser cannot keep a secret.
resource "ferriskey_client" "console" {
  realm         = ferriskey_realm.aether.name
  client_id     = "console"
  name          = "Aether Console"
  client_type   = "public"
  public_client = true

  direct_access_grants_enabled = true

  redirect_uris = var.console_redirect_uris
}

# Herald authenticates as itself, with no user involved. The control plane
# rejects any caller whose client id does not contain "herald-service", so this
# name is a contract rather than a preference.
resource "ferriskey_client" "herald" {
  realm       = ferriskey_realm.aether.name
  client_id   = "herald-service"
  name        = "Herald"
  client_type = "confidential"

  public_client                = false
  service_account_enabled      = true
  direct_access_grants_enabled = false
}

# Marks an account as operating this installation rather than using it.
#
# Installation-wide, unlike the permissions in `aether-permission`, which say
# what a member may do inside their own organisation. Only an operator sees the
# data planes every organisation runs on.
resource "ferriskey_role" "operator" {
  realm       = ferriskey_realm.aether.name
  name        = "aether-operator"
  description = "Operates the installation: data planes, placement, capacity."
  permissions = []
}
