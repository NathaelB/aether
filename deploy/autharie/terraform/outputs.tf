output "issuer" {
  description = "auth.issuer / oidc.issuerUrl for values-production.yaml."
  value       = "${var.ferriskey_url}/realms/${ferriskey_realm.autharie.name}"
}

output "console_client_id" {
  description = "oidc.clientId for values-production.yaml."
  value       = ferriskey_client.console.client_id
}
