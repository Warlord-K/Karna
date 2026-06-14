{{/*
Expand the name of the chart.
*/}}
{{- define "karna.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Create a default fully qualified app name.
*/}}
{{- define "karna.fullname" -}}
{{- if .Values.fullnameOverride }}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- $name := default .Chart.Name .Values.nameOverride }}
{{- if contains $name .Release.Name }}
{{- .Release.Name | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- printf "%s-%s" .Release.Name $name | trunc 63 | trimSuffix "-" }}
{{- end }}
{{- end }}
{{- end }}

{{/*
Create chart name and version as used by the chart label.
*/}}
{{- define "karna.chart" -}}
{{- printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Common labels.
*/}}
{{- define "karna.labels" -}}
helm.sh/chart: {{ include "karna.chart" . }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
app.kubernetes.io/part-of: karna
{{- if .Chart.AppVersion }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
{{- end }}
{{- end }}

{{/*
Component labels — call with (dict "context" . "component" "api").
*/}}
{{- define "karna.componentLabels" -}}
{{ include "karna.labels" .context }}
app.kubernetes.io/name: {{ include "karna.name" .context }}
app.kubernetes.io/instance: {{ .context.Release.Name }}
app.kubernetes.io/component: {{ .component }}
{{- end }}

{{/*
Selector labels for a component — call with (dict "context" . "component" "api").
*/}}
{{- define "karna.selectorLabels" -}}
app.kubernetes.io/name: {{ include "karna.name" .context }}
app.kubernetes.io/instance: {{ .context.Release.Name }}
app.kubernetes.io/component: {{ .component }}
{{- end }}

{{/*
Construct the full image reference for a component.
Usage: {{ include "karna.image" (dict "image" .Values.api.image "global" .Values.global "appVersion" .Chart.AppVersion) }}
*/}}
{{- define "karna.image" -}}
{{- $registry := .image.registry | default .global.imageRegistry -}}
{{- $tag := .image.tag | default .appVersion -}}
{{- if $registry -}}
{{- printf "%s/%s:%s" $registry .image.repository $tag -}}
{{- else -}}
{{- printf "%s:%s" .image.repository $tag -}}
{{- end -}}
{{- end }}

{{/*
Database URL — builds postgres:// from subchart or external config.
*/}}
{{- define "karna.databaseUrl" -}}
{{- if .Values.postgresql.enabled -}}
{{- $host := printf "%s-postgresql" (include "karna.fullname" .) -}}
{{- $user := .Values.postgresql.auth.username -}}
{{- $db := .Values.postgresql.auth.database -}}
{{- printf "postgres://%s:$(POSTGRES_PASSWORD)@%s:5432/%s" $user $host $db -}}
{{- else -}}
{{- $host := .Values.postgresql.external.host -}}
{{- $port := .Values.postgresql.external.port | default 5432 -}}
{{- $user := .Values.postgresql.external.username -}}
{{- $db := .Values.postgresql.external.database -}}
{{- printf "postgres://%s:$(POSTGRES_PASSWORD)@%s:%v/%s" $user $host $port $db -}}
{{- end -}}
{{- end }}

{{/*
Redis URL — builds redis:// from subchart or external config.
*/}}
{{- define "karna.redisUrl" -}}
{{- if .Values.redis.enabled -}}
{{- printf "redis://%s-redis-master:6379" (include "karna.fullname" .) -}}
{{- else -}}
{{- $host := .Values.redis.external.host -}}
{{- $port := .Values.redis.external.port | default 6379 -}}
{{- if .Values.redis.external.password -}}
{{- printf "redis://:%s@%s:%v" "$(REDIS_PASSWORD)" $host $port -}}
{{- else -}}
{{- printf "redis://%s:%v" $host $port -}}
{{- end -}}
{{- end -}}
{{- end }}

{{/*
PostgreSQL host — for migration job.
*/}}
{{- define "karna.postgresHost" -}}
{{- if .Values.postgresql.enabled -}}
{{- printf "%s-postgresql" (include "karna.fullname" .) -}}
{{- else -}}
{{- .Values.postgresql.external.host -}}
{{- end -}}
{{- end }}

{{/*
PostgreSQL port.
*/}}
{{- define "karna.postgresPort" -}}
{{- if .Values.postgresql.enabled -}}
{{- 5432 -}}
{{- else -}}
{{- .Values.postgresql.external.port | default 5432 -}}
{{- end -}}
{{- end }}

{{/*
PostgreSQL user.
*/}}
{{- define "karna.postgresUser" -}}
{{- if .Values.postgresql.enabled -}}
{{- .Values.postgresql.auth.username -}}
{{- else -}}
{{- .Values.postgresql.external.username -}}
{{- end -}}
{{- end }}

{{/*
PostgreSQL database.
*/}}
{{- define "karna.postgresDatabase" -}}
{{- if .Values.postgresql.enabled -}}
{{- .Values.postgresql.auth.database -}}
{{- else -}}
{{- .Values.postgresql.external.database -}}
{{- end -}}
{{- end }}

{{/*
Name of the secret containing all Karna credentials.
*/}}
{{- define "karna.secretName" -}}
{{- printf "%s-secrets" (include "karna.fullname" .) -}}
{{- end }}

{{/*
Name of the config ConfigMap.
*/}}
{{- define "karna.configMapName" -}}
{{- if .Values.config.existingConfigMap -}}
{{- .Values.config.existingConfigMap -}}
{{- else -}}
{{- printf "%s-config" (include "karna.fullname" .) -}}
{{- end -}}
{{- end }}

{{/*
Service account name.
*/}}
{{- define "karna.serviceAccountName" -}}
{{- if .Values.serviceAccount.create -}}
{{- default (include "karna.fullname" .) .Values.serviceAccount.name -}}
{{- else -}}
{{- default "default" .Values.serviceAccount.name -}}
{{- end -}}
{{- end }}

{{/*
Frontend AUTH_URL — explicit auth.url wins; otherwise derived from ingress host
when chart-managed ingress is enabled.
*/}}
{{- define "karna.authUrl" -}}
{{- if .Values.auth.url -}}
{{- .Values.auth.url -}}
{{- else if .Values.ingress.enabled -}}
{{- $host := (index .Values.ingress.hosts 0).host -}}
{{- if .Values.ingress.tls -}}
{{- printf "https://%s" $host -}}
{{- else -}}
{{- printf "http://%s" $host -}}
{{- end -}}
{{- end -}}
{{- end }}

{{/*
Memory URL injected into config.yaml. Prefers explicit config.memory.url.
When mem0 is enabled and url is omitted, defaults to the in-cluster mem0 service.
*/}}
{{- define "karna.memoryUrl" -}}
{{- if .Values.config.memory.url -}}
{{- .Values.config.memory.url -}}
{{- else if .Values.mem0.enabled -}}
{{- printf "http://%s-mem0:8000" (include "karna.fullname" .) -}}
{{- else -}}
{{- "http://localhost:8888" -}}
{{- end -}}
{{- end }}

{{/*
Secret name for mem0 provider API key (llm/embedder).
Precedence:
1) mem0.existingSecret + mem0.existingSecretApiKeyKey
2) inline mem0.apiKey
3) mem0 OpenRouter override (existingSecret or inline)
4) global openrouter settings
5) legacy mem0.openaiApiKey inline
*/}}
{{- define "karna.mem0ApiKeySecretName" -}}
{{- if .Values.mem0.existingSecret -}}
{{- .Values.mem0.existingSecret -}}
{{- else if .Values.mem0.apiKey -}}
{{- include "karna.secretName" . -}}
{{- else if .Values.mem0.openrouterExistingSecret -}}
{{- .Values.mem0.openrouterExistingSecret -}}
{{- else if .Values.mem0.openrouterApiKey -}}
{{- include "karna.secretName" . -}}
{{- else if .Values.openrouter.existingSecret -}}
{{- .Values.openrouter.existingSecret -}}
{{- else if .Values.mem0.openaiApiKey -}}
{{- include "karna.secretName" . -}}
{{- else -}}
{{- include "karna.secretName" . -}}
{{- end -}}
{{- end }}

{{/*
Secret key for mem0 provider API key (llm/embedder).
*/}}
{{- define "karna.mem0ApiKeySecretKey" -}}
{{- if .Values.mem0.existingSecret -}}
{{- .Values.mem0.existingSecretApiKeyKey | default "api-key" -}}
{{- else if .Values.mem0.apiKey -}}
{{- "mem0-api-key" -}}
{{- else if .Values.mem0.openrouterExistingSecret -}}
{{- .Values.mem0.openrouterExistingSecretApiKeyKey | default "openrouter-api-key" -}}
{{- else if .Values.mem0.openrouterApiKey -}}
{{- "mem0-openrouter-api-key" -}}
{{- else if .Values.openrouter.existingSecret -}}
{{- .Values.openrouter.existingSecretApiKeyKey | default "openrouter-api-key" -}}
{{- else if .Values.mem0.openaiApiKey -}}
{{- "mem0-openai-api-key" -}}
{{- else -}}
{{- "openrouter-api-key" -}}
{{- end -}}
{{- end }}

{{/*
Name of the secret holding the PostgreSQL password.
Returns the Bitnami-generated secret name or the user-provided existingSecret.
*/}}
{{- define "karna.postgresPasswordSecret" -}}
{{- if .Values.postgresql.enabled -}}
  {{- if .Values.postgresql.auth.existingSecret -}}
    {{- .Values.postgresql.auth.existingSecret -}}
  {{- else -}}
    {{- printf "%s-postgresql" (include "karna.fullname" .) -}}
  {{- end -}}
{{- else -}}
  {{- if .Values.postgresql.external.existingSecret -}}
    {{- .Values.postgresql.external.existingSecret -}}
  {{- else -}}
    {{- include "karna.secretName" . -}}
  {{- end -}}
{{- end -}}
{{- end }}

{{/*
Key in the PostgreSQL password secret.
*/}}
{{- define "karna.postgresPasswordSecretKey" -}}
{{- if .Values.postgresql.enabled -}}
  {{- if .Values.postgresql.auth.existingSecret -}}
    {{- "password" -}}
  {{- else -}}
    {{- "password" -}}
  {{- end -}}
{{- else -}}
  {{- if .Values.postgresql.external.existingSecret -}}
    {{- .Values.postgresql.external.existingSecretPasswordKey | default "password" -}}
  {{- else -}}
    {{- "postgres-password" -}}
  {{- end -}}
{{- end -}}
{{- end }}
