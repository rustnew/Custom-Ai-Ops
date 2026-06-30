{{/*
Common labels applied to all resources rendered by model-serving charts.
*/}}
{{- define "bjw-template.labels" -}}
helm.sh/chart: {{ printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
{{ include "bjw-template.selectorLabels" . }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end }}

{{/*
Selector labels for matching pods.
*/}}
{{- define "bjw-template.selectorLabels" -}}
app.kubernetes.io/name: {{ .Chart.Name }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}

{{/*
Full qualified name, truncated to 63 chars.
*/}}
{{- define "bjw-template.fullname" -}}
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
Chart label value.
*/}}
{{- define "bjw-template.chart" -}}
{{- printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Service account name.
*/}}
{{- define "bjw-template.serviceAccountName" -}}
{{- if .Values.serviceAccount.create }}
{{- default (include "bjw-template.fullname" .) .Values.serviceAccount.name }}
{{- else }}
{{- default "default" .Values.serviceAccount.name }}
{{- end }}
{{- end }}

{{/*
Common pod security context.
*/}}
{{- define "bjw-template.podSecurityContext" -}}
runAsNonRoot: true
runAsUser: {{ .Values.global.securityContext.runAsUser | default 1000 }}
fsGroup: {{ .Values.global.securityContext.fsGroup | default 1000 }}
{{- end }}

{{/*
Common container security context.
*/}}
{{- define "bjw-template.securityContext" -}}
allowPrivilegeEscalation: false
readOnlyRootFilesystem: true
capabilities:
  drop:
    - ALL
{{- end }}

{{/*
Standard environment variables shared by all model-serving charts.
*/}}
{{- define "bjw-template.commonEnv" -}}
- name: MODEL_NAME
  value: {{ .Values.model.name | quote }}
- name: POD_NAMESPACE
  valueFrom:
    fieldRef:
      fieldPath: metadata.namespace
- name: POD_NAME
  valueFrom:
    fieldRef:
      fieldPath: metadata.name
{{- range .Values.extraEnv }}
- name: {{ .name }}
  value: {{ .value | quote }}
{{- end }}
{{- end }}

{{/*
GPU tolerations applied by default.
*/}}
{{- define "bjw-template.gpuTolerations" -}}
- key: nvidia.com/gpu
  operator: Exists
  effect: NoSchedule
{{- end }}

{{/*
Standard volume mounts for model storage.
*/}}
{{- define "bjw-template.volumeMounts" -}}
- name: model-storage
  mountPath: /models
  readOnly: false
- name: tmp
  mountPath: /tmp
{{- range .Values.extraVolumeMounts }}
- name: {{ .name }}
  mountPath: {{ .mountPath }}
{{- if .subPath }}
  subPath: {{ .subPath }}
{{- end }}
{{- end }}
{{- end }}

{{/*
Standard volumes for model storage.
*/}}
{{- define "bjw-template.volumes" -}}
- name: tmp
  emptyDir: {}
{{- range .Values.extraVolumes }}
- name: {{ .name }}
{{- if .configMap }}
  configMap:
    name: {{ .configMap.name }}
{{- if .configMap.items }}
    items:
{{ toYaml .configMap.items | indent 4 }}
{{- end }}
{{- else if .secret }}
  secret:
    secretName: {{ .secret.secretName }}
{{- else if .emptyDir }}
  emptyDir: {}
{{- end }}
{{- end }}
{{- end }}

{{/*
Standard liveness probe for HTTP /health endpoints.
*/}}
{{- define "bjw-template.livenessProbe" -}}
httpGet:
  path: /health
  port: http
initialDelaySeconds: {{ .Values.livenessProbe.initialDelaySeconds | default 120 }}
periodSeconds: {{ .Values.livenessProbe.periodSeconds | default 30 }}
timeoutSeconds: {{ .Values.livenessProbe.timeoutSeconds | default 10 }}
failureThreshold: {{ .Values.livenessProbe.failureThreshold | default 5 }}
{{- end }}

{{/*
Standard startup probe for HTTP /health endpoints.
*/}}
{{- define "bjw-template.startupProbe" -}}
httpGet:
  path: /health
  port: http
initialDelaySeconds: {{ .Values.startupProbe.initialDelaySeconds | default 30 }}
periodSeconds: {{ .Values.startupProbe.periodSeconds | default 10 }}
failureThreshold: {{ .Values.startupProbe.failureThreshold | default 60 }}
{{- end }}