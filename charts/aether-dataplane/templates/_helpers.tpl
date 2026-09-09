{{- define "aether-dataplane.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{- define "aether-dataplane.fullname" -}}
{{- if .Values.fullnameOverride }}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- printf "%s-%s" .Release.Name (include "aether-dataplane.name" .) | trunc 63 | trimSuffix "-" }}
{{- end }}
{{- end }}

{{- define "aether-dataplane.labels" -}}
helm.sh/chart: {{ printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
app.kubernetes.io/name: {{ include "aether-dataplane.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
app.kubernetes.io/part-of: aether
{{- end }}

{{- define "aether-dataplane.image" -}}
{{- $tag := .root.Values.image.tag | default .root.Chart.AppVersion -}}
{{- printf "%s/%s/aether-%s:%s" .root.Values.image.registry .root.Values.image.repository .component $tag -}}
{{- end }}

{{/*
The broker URL every component talks to. An external broker wins over the
in-chart one, so pointing at a real cluster does not require disabling
anything twice.
*/}}
{{- define "aether-dataplane.amqpUrl" -}}
{{- if .Values.rabbitmq.externalUrl -}}
{{ .Values.rabbitmq.externalUrl }}
{{- else -}}
{{ printf "amqp://%s:%s@%s-rabbitmq:5672" .Values.rabbitmq.auth.username .Values.rabbitmq.auth.password (include "aether-dataplane.fullname" .) }}
{{- end -}}
{{- end }}

{{/*
Fails the render rather than installing something that cannot work. A data
plane with no id or no control plane URL starts, logs an error every cycle and
does nothing -- which looks like a bug in Herald rather than a missing value.
*/}}
{{- define "aether-dataplane.validate" -}}
{{- if and .Values.herald.enabled (not .Values.dataplane.id) -}}
{{- fail "dataplane.id is required when herald is enabled: it is the id the control plane registered for this cluster" -}}
{{- end -}}
{{- if and .Values.herald.enabled (not .Values.controlPlane.url) -}}
{{- fail "controlPlane.url is required when herald is enabled" -}}
{{- end -}}
{{- $hasClientCredentials := and .Values.controlPlane.auth.issuer (or .Values.controlPlane.auth.clientSecret .Values.controlPlane.existingSecret) -}}
{{- $hasStaticToken := or .Values.controlPlane.token .Values.controlPlane.existingSecret -}}
{{- if and .Values.herald.enabled (not (or $hasClientCredentials $hasStaticToken)) -}}
{{- fail "herald cannot authenticate: set controlPlane.auth.issuer with a client secret, or controlPlane.token" -}}
{{- end -}}
{{- if and .Values.controlPlane.auth.issuer .Values.controlPlane.token -}}
{{- fail "set controlPlane.auth.issuer or controlPlane.token, not both" -}}
{{- end -}}
{{- if and .Values.controlPlane.token .Values.controlPlane.existingSecret -}}
{{- fail "set controlPlane.token or controlPlane.existingSecret, not both" -}}
{{- end -}}
{{- end }}
