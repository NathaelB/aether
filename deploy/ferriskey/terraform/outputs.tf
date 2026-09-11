output "issuer" {
  description = "AUTH_ISSUER for the control plane and for Herald."
  value       = "${var.ferriskey_url}/realms/${ferriskey_realm.aether.name}"
}

output "herald_client_id" {
  value = ferriskey_client.herald.client_id
}

# Provider v0.1.0 stores "***" here rather than the generated secret: the API
# masks it in the client response, and the provider records the mask instead of
# reading the dedicated client-secret endpoint. Kept so the output starts
# working the day that is fixed, and `make bootstrap-auth` fetches the real one
# in the meantime.
output "herald_client_secret" {
  description = "Currently masked by a provider bug -- use make bootstrap-auth."
  value       = ferriskey_client.herald.secret
  sensitive   = true
}

output "herald_client_uuid" {
  description = "Used to read the real secret until the provider returns it."
  value       = ferriskey_client.herald.client_uuid
}

output "console_client_id" {
  value = ferriskey_client.console.client_id
}

output "operator_role_id" {
  description = "Grant it with: ferriskey_user_role, or the FerrisKey console."
  value       = ferriskey_role.operator.role_uuid
}

output "operator_client_id" {
  value = ferriskey_client.operator_cli.client_id
}

output "operator_client_uuid" {
  description = "Needed to read the secret from the client-secret endpoint."
  value       = ferriskey_client.operator_cli.client_uuid
}
