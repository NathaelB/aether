variable "ferriskey_url" {
  description = "Base URL of the FerrisKey instance."
  type        = string
  default     = "http://localhost:3334"
}

variable "realm" {
  description = "Realm to create for Aether."
  type        = string
  default     = "aether"
}

variable "admin_username" {
  description = "Initial admin account, used for the bootstrap password grant."
  type        = string
  default     = "admin"
}

variable "admin_password" {
  description = <<-DESC
    Password for the bootstrap admin account.

    Defaults to the value a local FerrisKey ships with. Anything shared should
    pass it through TF_VAR_admin_password from a secrets manager, and should
    follow the provider's two-phase guide rather than using the admin account
    for day-to-day changes.
  DESC
  type        = string
  sensitive   = true
  default     = "admin"
}

variable "console_redirect_uris" {
  description = "Where the console is allowed to send a user back after login."
  type        = set(string)
  default = [
    "http://localhost:5173",
    "http://localhost:5173/*",
    "http://localhost:5556",
    "http://localhost:5556/*",
  ]
}

variable "allow_self_registration" {
  description = <<-DESC
    Whether anyone reaching the login page may create an account.

    True by default because this configuration targets a local stack, where it
    is the only way to obtain a usable account -- see the comment on
    `ferriskey_realm_settings.aether`. Set it to false for anything shared.
  DESC
  type        = bool
  default     = true
}
