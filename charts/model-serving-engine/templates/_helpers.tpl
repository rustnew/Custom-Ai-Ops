{{/*
Expand the name of the chart.
*/}}
{{- define "model-serving-engine.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Create a default fully qualified app name.
*/}}
{{- define "model-serving-engine.fullname" -}}
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
Chart label values.
*/}}
{{- define "model-serving-engine.chart" -}}
{{- printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Common labels.
*/}}
{{- define "model-serving-engine.labels" -}}
helm.sh/chart: {{ include "model-serving-engine.chart" . }}
{{ include "model-serving-engine.selectorLabels" . }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- with .Values.commonLabels }}
{{ toYaml . }}
{{- end }}
{{- end }}

{{/*
Selector labels.
*/}}
{{- define "model-serving-engine.selectorLabels" -}}
app.kubernetes.io/name: {{ include "model-serving-engine.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}

{{/*
Common annotations.
*/}}
{{- define "model-serving-engine.annotations" -}}
{{- with .Values.commonAnnotations }}
{{ toYaml . }}
{{- end }}
{{- end }}

{{/*
Service account name.
*/}}
{{- define "model-serving-engine.serviceAccountName" -}}
{{- if .Values.serviceAccount.create }}
{{- default (include "model-serving-engine.fullname" .) .Values.serviceAccount.name }}
{{- else }}
{{- default "default" .Values.serviceAccount.name }}
{{- end }}
{{- end }}

{{/*
Engine image config for the selected engine type.
*/}}
{{- define "model-serving-engine.engineImage" -}}
{{- $type := .Values.engine.type -}}
{{- if eq $type "vllm" -}}
{{- .Values.engine.vllm.image | toJson -}}
{{- else if eq $type "llamacpp" -}}
{{- .Values.engine.llamacpp.image | toJson -}}
{{- else if eq $type "onnxGenai" -}}
{{- .Values.engine.onnxGenai.image | toJson -}}
{{- end -}}
{{- end }}

{{/*
Engine args for the selected engine type.
*/}}
{{- define "model-serving-engine.engineArgs" -}}
{{- $type := .Values.engine.type -}}
{{- if eq $type "vllm" -}}
{{- .Values.engine.vllm.args | toJson -}}
{{- else if eq $type "llamacpp" -}}
{{- .Values.engine.llamacpp.args | toJson -}}
{{- else if eq $type "onnxGenai" -}}
{{- .Values.engine.onnxGenai.args | toJson -}}
{{- end -}}
{{- end }}

{{/*
Engine command for the selected engine type.
*/}}
{{- define "model-serving-engine.engineCommand" -}}
{{- $type := .Values.engine.type -}}
{{- if eq $type "vllm" -}}
{{- .Values.engine.vllm.command | toJson -}}
{{- else if eq $type "llamacpp" -}}
{{- .Values.engine.llamacpp.command | toJson -}}
{{- else if eq $type "onnxGenai" -}}
{{- .Values.engine.onnxGenai.command | toJson -}}
{{- end -}}
{{- end }}

{{/*
Engine resource limits/requests for the selected engine type.
*/}}
{{- define "model-serving-engine.engineResources" -}}
{{- $type := .Values.engine.type -}}
{{- if eq $type "vllm" -}}
{{- .Values.engine.vllm.resources | toJson -}}
{{- else if eq $type "llamacpp" -}}
{{- .Values.engine.llamacpp.resources | toJson -}}
{{- else if eq $type "onnxGenai" -}}
{{- .Values.engine.onnxGenai.resources | toJson -}}
{{- end -}}
{{- end }}

{{/*
Engine container port for the selected type.
*/}}
{{- define "model-serving-engine.enginePort" -}}
{{- $type := .Values.engine.type -}}
{{- if eq $type "vllm" -}}
8000
{{- else if eq $type "llamacpp" -}}
8080
{{- else if eq $type "onnxGenai" -}}
8080
{{- end -}}
{{- end }}