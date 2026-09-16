variable "ferriskey_url" {
  description = <<-DESC
    Base URL of the dedicated Ferriskey instance's API.

    The /api suffix matters: unlike the local dev stack (which runs the raw
    ferriskey-api image with no SERVER_ROOT_PATH set), the production chart
    sets api.server.rootPath to /api, and the console+HTTPRoute path-split
    only forwards that prefix to the API -- everything else goes to the
    webapp's nginx, which answers a POST here with 405.
  DESC
  type    = string
  default = "https://id.autharie.fr/api"
}

variable "realm" {
  description = "Realm autharie's own users and data planes' Heralds live in."
  type        = string
  default     = "autharie"
}

variable "admin_username" {
  description = "Admin account used for the bootstrap password grant."
  type        = string
  default     = "admin"
}

variable "admin_password" {
  description = <<-DESC
    Password for the bootstrap admin account.

    No default: this targets a shared, production instance, so the value must
    come from TF_VAR_admin_password rather than being typed anywhere a
    default could leak it. See docs/production-deployment.md.
  DESC
  type        = string
  sensitive   = true
}

variable "console_origin" {
  description = <<-DESC
    Where the console is served from.

    Expanded into the exact redirect URIs Ferriskey will match. It compares
    them as strings -- the `*` in a registered URI is stored and never
    interpreted -- so both the bare origin and the trailing slash the console
    actually redirects to (see apps/console/src/lib/auth/user-manager.ts) are
    registered rather than relying on a wildcard that does nothing.
  DESC
  type    = string
  default = "https://app.autharie.fr"
}
