# The realm and OIDC client Autharie needs on its dedicated Ferriskey,
# declared rather than clicked together by hand. Mirrors
# deploy/ferriskey/terraform/, adapted for a shared production instance
# rather than a throwaway local one: self-registration is off, the console
# client cannot use the direct password grant, and there is no default admin
# password to fall back on.
#
# What this does NOT create: a static "herald-service" client. Autharie's
# control plane mints one client per data plane itself, using the
# REALM_ADMIN_* credentials in charts/aether-control-plane/values-production.yaml
# -- there is no shared Herald identity to pre-provision.

terraform {
  required_version = ">= 1.5"

  required_providers {
    ferriskey = {
      source  = "ferriskey/ferriskey"
      version = "~> 0.1"
    }
  }
}

provider "ferriskey" {
  url       = var.ferriskey_url
  realm     = "master"
  client_id = "admin-cli"
  username  = var.admin_username
  password  = var.admin_password
}

resource "ferriskey_realm" "autharie" {
  name = var.realm
}

resource "ferriskey_realm_settings" "autharie" {
  realm = ferriskey_realm.autharie.name

  # Short-lived access tokens are why the console and Herald both refresh
  # rather than holding one: the control plane checks `exp`, so a fixed
  # token dies quietly.
  access_token_lifetime  = 300
  refresh_token_lifetime = 1800

  # Off, unlike the local dev realm: accounts on a shared instance come from
  # an invitation, not from anyone reaching the login page.
  user_registration_enabled = false
}

# The web console. Public because a browser cannot keep a secret, and no
# direct password grant: the console only ever uses the Authorization Code
# flow (see apps/console/src/lib/auth/user-manager.ts), so there is no
# reason to also accept a password posted straight to the token endpoint.
resource "ferriskey_client" "console" {
  realm         = ferriskey_realm.autharie.name
  client_id     = "autharie-console"
  name          = "Autharie Console"
  client_type   = "public"
  public_client = true

  direct_access_grants_enabled = false

  redirect_uris = toset([
    var.console_origin,
    "${var.console_origin}/",
  ])
}
