# Architecture de Référence Ultime — Plateforme de Serving de Modèles Multi-Format à l'Échelle Cloud

## Objectif

Définir la structure de projet la plus robuste, modulaire et durable possible pour déployer des modèles ML (tous formats confondus) sur le cloud, capable de servir des millions d'utilisateurs, avec auto-réparation, prévision de charge, et résistance dans le temps (années, pas mois). Ce document synthétise et étend le pattern déjà en place (StatefulSet bjw-template + ArgoCD + Envoy AI Gateway + sync waves) en un système généralisé, multi-moteur, multi-format.

---

## 0. Principe directeur

Le système ne doit **jamais coupler le format du modèle au moteur de serving de façon rigide**. La bonne architecture sépare strictement trois plans :

1. **Plan modèle** (le poids + son format) — interchangeable.
2. **Plan moteur** (le runtime qui exécute ce format) — interchangeable selon le format.
3. **Plan exposition** (l'API OpenAI-compatible exposée à la passerelle) — toujours identique, quel que soit le moteur en dessous.

C'est ce découpage qui rend le système modulaire : on ajoute un nouveau modèle sans toucher à la passerelle, on change de moteur sans toucher au client.

---

## 1. Panorama complet des formats de modèles et de leurs moteurs

Ne pas se limiter à ONNX. Voici la cartographie complète à prévoir dans l'architecture.

| Format | Cas d'usage typique | Moteur open-source recommandé | Pourquoi ce moteur |
|---|---|---|---|
| **GGUF** (quantisé Q4/Q5/Q8) | LLM légers, edge, CPU/GPU modeste | **llama.cpp** | Le plus robuste et le plus léger pour GGUF, aucune dépendance Python, démarrage rapide, idéal pour matériel limité (déjà en LIVE dans ton pattern) |
| **Safetensors / BF16-FP16** | LLM full precision ou semi-précision, gros GPU datacenter | **vLLM** | PagedAttention, continuous batching, throughput le plus élevé sur GPU serveur (A100/H100) |
| **ONNX (INT4 AWQ, INT8, FP16)** | Modèles convertis, portabilité multi-plateforme, intégration native Rust/C++ | **ONNX Runtime GenAI** ou **Triton Inference Server** (backend ONNX) | ORT GenAI pour un serveur custom léger (ton pattern Rust FFI) ; Triton si tu veux du multi-modèle/multi-framework unifié |
| **TensorRT / TensorRT-LLM engines** | Latence minimale sur GPU NVIDIA, production à très grande échelle | **Triton Inference Server** (backend TensorRT-LLM) | Compilation spécifique GPU, fusion de kernels, le plus rapide en pur NVIDIA mais le moins portable |
| **PyTorch natif (.pt/.bin)** | Modèles custom non encore convertis, recherche → prod rapide | **TorchServe** ou **Ray Serve** | Pont rapide avant conversion vers un format optimisé |
| **CoreML / TFLite** | Edge mobile, inference embarquée hors du cloud central | Hors scope serveur — mentionné pour complétude de l'écosystème | Non pertinent pour le cloud central mais à anticiper si la roadmap inclut de l'edge device |
| **GGUF MoE / multi-fichiers (sharded)** | Très gros modèles type Mixtral | **llama.cpp** (support natif) ou **vLLM** (avec tensor parallelism) | Selon la taille — llama.cpp pour un nœud, vLLM pour multi-GPU |
| **AWQ/GPTQ safetensors** | Quantisation différente de GGUF, compatible GPU serveur | **vLLM** (support natif AWQ/GPTQ) | Évite une re-conversion, vLLM lit directement ces formats |

### Règle de décision (arbre de choix moteur)

```
Le modèle est-il en GGUF ?
├── Oui → llama.cpp
└── Non
    ├── Le modèle est-il en ONNX ?
    │   ├── Oui, usage simple/unique → ONNX Runtime GenAI (serveur custom léger)
    │   └── Oui, usage multi-modèle/multi-framework → Triton (backend ONNX)
    ├── Le modèle est-il en safetensors/BF16/AWQ/GPTQ ?
    │   └── Oui → vLLM
    ├── Le modèle a-t-il un engine TensorRT-LLM compilé ?
    │   └── Oui → Triton (backend TensorRT-LLM)
    └── Le modèle est en PyTorch brut non converti ?
        └── Oui → Ray Serve (transitoire, en attendant conversion)
```

Cette règle doit être **codifiée dans un outil interne** (voir section 4.3) plutôt que laissée à une décision humaine ad hoc à chaque nouveau modèle.

---

## 2. Structure de dépôt (monorepo GitOps, inspirée et généralisée du pattern existant)

```
ai-platform/
├── charts/
│   ├── model-serving-llamacpp/        # template générique GGUF
│   ├── model-serving-vllm/            # template générique safetensors/AWQ/GPTQ
│   ├── model-serving-onnx-rust/       # template générique ONNX (serveur Rust custom)
│   ├── model-serving-triton/          # template générique Triton (ONNX/TensorRT-LLM/multi)
│   ├── model-serving-rayserve/        # template transitoire PyTorch brut
│   ├── bjw-template/                  # base commune StatefulSet/PVC/Ingress (dépendance Helm)
│   ├── ai-gateway/                    # Envoy AI Gateway + backends + models + pricing
│   └── apps/                          # App-of-Apps ArgoCD (ApplicationSet par environnement)
├── environments/
│   ├── dev/
│   │   └── values/<app>.yaml          # overrides par appli, par env
│   ├── staging/
│   └── prod/
├── models/
│   ├── registry.yaml                  # registre déclaratif : nom, format, moteur, VRAM budget, statut
│   └── <model-name>/
│       ├── model.md                   # fiche modèle (papier individuel, comme docs/models/onnx.md)
│       ├── budget.md                  # calcul VRAM/CPU prouvé avant déploiement
│       └── eval-report.md             # résultats de validation qualité avant promotion
├── docs/
│   ├── architecture/
│   │   ├── 00-overview.md
│   │   ├── 01-formats-and-engines.md
│   │   ├── 02-gpu-scheduling.md
│   │   ├── 03-gateway-federation.md
│   │   ├── 04-gitops-deployment.md
│   │   ├── 05-observability.md
│   │   ├── 06-resilience-and-dr.md
│   │   └── 07-capacity-forecasting.md
│   ├── adr/                           # Architecture Decision Records (déjà en place dans ton pattern)
│   └── runbooks/                      # procédures d'incident pas à pas
├── tools/
│   ├── engine-selector/               # CLI qui applique l'arbre de décision (section 4.3)
│   ├── vram-budget-calc/              # calculateur automatique du budget mémoire
│   └── model-onboarding/              # scaffold automatique d'un nouveau modèle (génère charts + docs)
├── observability/
│   ├── grafana-dashboards/
│   ├── prometheus-rules/
│   └── alertmanager-routes/
└── tests/
    ├── smoke/                         # tests post-déploiement automatiques par modèle
    ├── load/                          # scripts k6/Locust de test de charge
    └── chaos/                         # scénarios de chaos engineering GPU
```

**Pourquoi cette structure est durable** : chaque format a son propre chart générique réutilisable (pas un chart par modèle dupliqué à l'infini), chaque modèle a sa fiche déclarative dans `models/`, et `tools/` capitalise la connaissance opérationnelle dans du code plutôt que dans la tête d'une personne.

---

## 3. Topologie d'infrastructure (généralisation du pattern deux-clusters)

### 3.1 Séparation plan de contrôle / plan de travail

Reprendre et généraliser le principe déjà validé :

- **Cluster de contrôle** : héberge uniquement ArgoCD et les CRD `Application`/`ApplicationSet`. Jamais de charge de travail GPU ici.
- **Cluster(s) de travail** : un ou plusieurs clusters dédiés à l'exécution réelle des modèles, potentiellement répartis par région ou par fournisseur cloud.

**Pourquoi c'est essentiel à grande échelle** : permet de scaler horizontalement le nombre de clusters de travail (multi-cloud, multi-région) sans jamais toucher à la logique de contrôle GitOps, qui reste unique et centralisée.

### 3.2 Node pools par type de matériel

| Pool | Matériel | Usage |
|---|---|---|
| `gpu-h100-pool` | NVIDIA H100 | LLM haute performance, vLLM/Triton TensorRT-LLM |
| `gpu-a100-pool` | NVIDIA A100 | LLM standard, vLLM |
| `gpu-l4-pool` | NVIDIA L4 | Inference légère, ONNX/llama.cpp, coût optimisé |
| `gpu-edge-pool` | GPU modeste (type A2000, comme dans ton setup home) | GGUF/ONNX petits modèles, PoC |
| `cpu-pool` | CPU uniquement | Preprocessing, gateway, services auxiliaires |

Chaque pool a ses propres taints/tolerations et `nodeSelector`, garantissant que Kueue/Volcano place chaque charge sur le matériel correspondant à son ratio coût/performance.

### 3.3 Outils d'orchestration GPU recommandés (open-source, classés par robustesse)

| Outil | Rôle | Niveau de maturité pour production longue durée |
|---|---|---|
| **NVIDIA GPU Operator** | Driver, device plugin, DCGM exporter, toolkit | Référence industrielle, maintenu activement par NVIDIA |
| **Kueue** (sigs.k8s.io) | Quotas, files d'attente, priorité | Projet officiel Kubernetes SIG, conçu pour durer |
| **Volcano** (CNCF) | Gang scheduling | Projet CNCF incubé, large adoption batch/ML |
| **Karpenter** | Provisioning de nœuds GPU à la demande | Standard de facto AWS, portable via providers |
| **KEDA** (CNCF) | Autoscaling event-driven, scale-to-zero | Projet CNCF graduated, très stable |

Tous ces outils sont des projets CNCF ou maintenus par les fournisseurs matériels eux-mêmes — c'est le critère de choix pour la durabilité (pas de risque d'abandon par une startup).

---

## 4. Couche d'abstraction multi-moteur (le cœur de la modularité)

### 4.1 Contrat d'interface unique

Quel que soit le moteur (llama.cpp, vLLM, ONNX Runtime GenAI, Triton, Ray Serve), chaque service de serving DOIT exposer :

- `POST /v1/chat/completions` (OpenAI-compatible) — pour que la passerelle ne voie jamais de différence
- `GET /health` (503 pendant le chargement, 200 quand prêt)
- Streaming SSE (`text/event-stream`)
- Authentification native par clé API (`--api-key-file` ou équivalent) — évite un sidecar Caddy quand le moteur le supporte nativement

C'est exactement le principe déjà appliqué dans ton pattern (llama.cpp et ONNX Rust ont l'auth native ; vLLM nécessite un sidecar Caddy car il ne le supporte pas nativement — à noter comme dette technique à surveiller si vLLM ajoute le support natif un jour).

### 4.2 Federation à la passerelle (Envoy AI Gateway)

Chaque moteur de serving, une fois exposé via Ingress, est fédéré dans la passerelle exactement comme un backend SaaS externe :

```yaml
backends:
  <model>-local-01:
    schema: OpenAI
    prefix: /v1
    fqdn.hostname: <model>--poc.example.com
    securityType: APIKey
    tlsHostname: <model>--poc.example.com
models:
  <model>-local:
    info:
      displayName: "<Model> (self-hosted)"
    contextLength: <N>
    pricing: { strategy: weighted, ... }
    backends:
      - ref: <model>-local-01
        priority: 0
```

**Pourquoi c'est la décision la plus importante du système** : du point de vue du client final, un modèle self-hosté GGUF, un modèle vLLM safetensors, et un modèle SaaS externe (OpenAI, Anthropic) sont strictement identiques. Cela permet de migrer un modèle d'un moteur à un autre, ou de basculer vers un fournisseur SaaS en cas de panne, sans aucun changement côté client — c'est la base de la robustesse à long terme.

### 4.3 Outil interne `engine-selector`

Un petit outil (CLI Rust, cohérent avec ton stack) qui :

1. Lit le format du modèle (extension, métadonnées HuggingFace, ou config explicite).
2. Applique l'arbre de décision de la section 1.
3. Génère automatiquement le chart Helm approprié à partir du template générique correspondant (`charts/model-serving-<engine>`).
4. Calcule et valide le budget VRAM avant de proposer le déploiement (voir section 4.4).

**Pourquoi cet outil est indispensable pour la durabilité** : élimine la dérive de connaissance tribale ("on sait qu'il faut utiliser tel moteur pour tel format") en la codifiant. Un nouvel ingénieur dans 3 ans peut onboarder un modèle sans connaître l'historique des décisions.

### 4.4 Calcul systématique du budget mémoire (avant tout déploiement)

Reprendre et généraliser le calcul déjà appliqué :

```
Budget utilisable = VRAM_totale × util_factor(0.85–0.90)
                   − taille_poids(format, quantisation)
                   − overhead_fixe(~1 Go)
                   = Budget disponible pour KV-cache / activations
```

Ce calcul doit être un test automatisé (`tools/vram-budget-calc`) exécuté en CI **avant** que le manifeste ne soit mergé — refuser le déploiement si le budget est négatif. Cela évite les OOM en production, qui sont l'incident le plus fréquent et le plus évitable.

**Règle matérielle à coder en dur** : ne jamais déployer un checkpoint FP8 sur une architecture GPU sans support FP8 natif (ex. Ampere) — vérification automatique à intégrer dans l'outil.

---

## 5. Pipeline GitOps complet (CI → CD → ArgoCD)

### 5.1 Flux de livraison continue

```
Merge sur le repo de charts (main)
   → CI : lint + helm template (rendu à blanc) + test de format de valeurs
   → Publication du chart en OCI (registre de charts, versionné en semver automatique)
   → argocd-image-updater détecte une nouvelle image signée (cosign)
   → Commit automatique du tag dans le repo de values (séparé, signé)
   → ArgoCD synchronise (source chart OCI + source values séparée)
```

**Pourquoi séparer le repo de chart et le repo de values** : permet un contrôle d'accès différencié (qui peut changer la structure du déploiement vs qui peut changer quelle version est en prod) et un audit plus clair — pattern déjà validé dans ton ADR-0055.

### 5.2 Sync waves généralisées

| Wave | Contenu | Justification |
|---|---|---|
| -3 | Bootstrap namespace, secrets de base | Rien ne peut démarrer sans ça |
| -2 | Stockage (PVC, bases de données de métriques) | Les pods auront besoin de volumes prêts |
| -1 | Opérateurs et collecteurs (GPU Operator, Prometheus Operator, collecteurs de logs) | Doivent tourner avant les workloads pour ne rater aucune métrique au démarrage |
| 0 | Workloads (les serveurs de modèles eux-mêmes) | Le cœur du système |
| 1 | Contenu (dashboards Grafana, configuration de la passerelle) | Dépend des workloads déjà en place |
| 2+ | Post-sync (tests de fumée automatiques, notifications) | Validation finale |

### 5.3 Health checks custom ArgoCD pour les CRD ML

Indispensable pour KServe/Triton dont les CRD custom ne sont pas nativement compris par ArgoCD :

```yaml
resource.customizations: |
  serving.kserve.io/InferenceService:
    health.lua: |
      hs = {}
      if obj.status and obj.status.conditions then
        for i, condition in ipairs(obj.status.conditions) do
          if condition.type == "Ready" and condition.status == "True" then
            hs.status = "Healthy"
            return hs
          end
        end
      end
      hs.status = "Progressing"
      return hs
```

Sans cela, ArgoCD affichera indéfiniment "Progressing" même quand le modèle est réellement prêt — angle mort critique pour la visibilité demandée.

---

## 6. Observabilité et prévision (le système "qui prévoit et répare")

### 6.1 Stack d'observabilité (open-source, choisi pour la durabilité)

| Couche | Outil | Pourquoi ce choix précis |
|---|---|---|
| Métriques | **Prometheus** + **Mimir** (stockage long terme) | Standard de facto CNCF, Mimir permet une rétention de plusieurs années sans exploser les coûts |
| Logs | **Loki** | Cohérent avec l'écosystème Grafana (LGTM stack), faible coût de stockage |
| Traces | **Tempo** + **OpenTelemetry** | Tracing distribué standard, indispensable pour les pipelines multimodaux |
| Visualisation | **Grafana** | Unifie métriques/logs/traces dans un seul dashboard |
| Métriques GPU bas niveau | **DCGM Exporter** (NVIDIA) | Seul exporter officiel donnant l'utilisation réelle SM/mémoire/température par GPU |
| Collecte | **Grafana Alloy** (successeur de Grafana Agent) | Agent unique pour métriques/logs/traces, réduit la complexité opérationnelle |

C'est exactement la stack LGTM déjà présente dans ton architecture (Mimir/Loki/Tempo/Grafana) — à généraliser comme socle obligatoire pour tout nouveau cluster de travail.

### 6.2 Prévision de charge (capacity forecasting)

- **Prometheus + modèles de séries temporelles simples (Holt-Winters via `prometheus-anomaly-detector` ou recording rules saisonnières)** pour anticiper les pics récurrents (heures de bureau, lancements de campagne).
- **KEDA avec scalers prédictifs** : combiner un scaler basé sur cron (pré-chauffage avant un pic connu) avec un scaler réactif (QPS réel) pour éviter le cold start au moment critique.
- **Tests de charge réguliers automatisés** (k6 ou Locust, dans `tests/load/`) exécutés en CI de façon périodique, pas seulement avant un déploiement majeur — pour détecter une dérive de capacité avant qu'elle ne devienne un incident.

### 6.3 Système de réparation automatique (auto-healing en couches)

| Niveau | Mécanisme | Outil |
|---|---|---|
| Pod | Redémarrage sur échec de liveness probe | Kubernetes natif |
| Nœud GPU défaillant | Détection Xid errors NVIDIA + cordon/drain automatique | **NVIDIA GPU Operator** (node health check intégré) |
| Dérive de configuration | Re-sync automatique vers l'état Git | **ArgoCD self-healing** (déjà natif) |
| Dégradation de qualité du modèle | Bascule automatique vers un modèle de fallback plus simple | Circuit breaker applicatif au niveau de la passerelle (Envoy) |
| Panne de cluster entier | Bascule de trafic vers un autre cluster/région | DNS-based failover ou passerelle multi-backend avec priorité |
| Drift de données | Alerte + déclenchement de pipeline de réévaluation | **Evidently AI** (open-source, self-hosted, pas de dépendance SaaS) |

**Principe clé de durabilité** : chaque mécanisme de réparation doit avoir une **trace dans Git** de son action (même automatique), pour que dans 2 ans on puisse comprendre pourquoi un rollback a eu lieu sans archéologie de logs.

---

## 7. Robustesse multi-année : ce qu'il faut prévoir dès le premier jour

### 7.1 Choix de dépendances pour la longévité

Privilégier systématiquement :
- Les projets **CNCF graduated** (Kubernetes, Prometheus, Envoy, Helm, etc.) plutôt que des outils récents non gouvernés par une fondation neutre.
- Les formats de modèles **avec un écosystème de conversion établi** (GGUF, ONNX, safetensors) plutôt que des formats propriétaires d'un seul fournisseur.
- Les moteurs **activement maintenus par plusieurs contributeurs indépendants** (llama.cpp, vLLM) plutôt que des projets mono-mainteneur.

### 7.2 Documentation vivante comme garde-fou

Reprendre et systématiser le pattern déjà en place :
- **ADR** (Architecture Decision Records) pour chaque décision structurante — pourquoi tel moteur a été choisi pour tel format, pourquoi telle architecture deux-clusters.
- **Fiche par modèle** (`models/<model>/model.md`) documentant le budget VRAM prouvé, le statut (LIVE/STAGED/STANDBY), et l'historique de migration de moteur le cas échéant.
- **Runbooks** d'incident écrits AVANT l'incident, pas après — un système qui doit durer des années aura un turnover d'équipe, et la connaissance doit être dans le dépôt, pas dans une personne.

### 7.3 Tests de non-régression structurels

- `helm lint --strict` + `helm template --dry-run` en CI sur **tous** les charts à chaque commit, pas seulement ceux modifiés (détecte les régressions de dépendances Helm partagées comme `bjw-template`).
- Test automatique de cohérence du registre (`models/registry.yaml`) : chaque modèle déclaré doit avoir un chart correspondant, une entrée gateway correspondante, et un budget VRAM prouvé — sinon échec de CI.
- **Checklist d'onboarding modèle** automatisée par l'outil `model-onboarding` (section 2), qui scaffold tous les fichiers nécessaires et empêche d'oublier une étape (déjà présente sous forme manuelle dans ton document — à transformer en outil exécutable).

### 7.4 Stratégie multi-cloud / anti-lock-in

- Garder la couche Kubernetes comme seule dépendance d'orchestration (pas de service propriétaire cloud non portable type AWS SageMaker endpoints).
- Le pattern de passerelle OpenAI-compatible permet de basculer transparemment entre self-hosted et SaaS externe en cas de panne fournisseur — déjà la base de ton architecture, à documenter explicitement comme stratégie de continuité.
- Stocker les poids de modèles dans un object store compatible S3 (MinIO self-hosted ou S3/GCS/R2) plutôt qu'un service propriétaire non portable.

---

## 8. Synthèse — Pile technologique complète recommandée

| Couche | Outil retenu | Alternative si contrainte différente |
|---|---|---|
| Orchestration | Kubernetes (Talos pour les nœuds, ou k3s pour clusters légers) | — |
| GitOps | ArgoCD | Flux (si préférence pull multi-tenant différente) |
| Moteur GGUF | llama.cpp | — |
| Moteur safetensors/AWQ/GPTQ | vLLM | TGI |
| Moteur ONNX simple | ONNX Runtime GenAI (serveur Rust custom) | — |
| Moteur multi-format avancé | Triton Inference Server | — |
| Scheduling GPU | Kueue + Volcano + NVIDIA GPU Operator | — |
| Autoscaling | KEDA + HPA custom metrics | — |
| Provisioning de nœuds | Karpenter | Cluster Autoscaler |
| Passerelle API | Envoy AI Gateway (OpenAI-compatible) | — |
| Observabilité | Prometheus/Mimir + Loki + Tempo + Grafana + DCGM | — |
| Drift/qualité | Evidently AI (self-hosted) | WhyLabs (si SaaS acceptable) |
| Secrets | External Secrets Operator + AWS Secrets Manager (ou Vault) | — |
| Registre d'images | Harbor (self-hosted, scan CVE intégré) | — |
| Registre de modèles | MLflow Model Registry (self-hosted) | — |
| Object store poids | MinIO (self-hosted, compatible S3) | S3/GCS/R2 directement |
| Tests de charge | k6 ou Locust | — |

---

## 9. Checklist finale d'onboarding d'un nouveau modèle (généralisée, multi-format)

1. Identifier le format natif du modèle (GGUF, safetensors, ONNX, TensorRT engine, PyTorch brut).
2. Lancer `engine-selector` → obtient le moteur recommandé et le chart généré.
3. Lancer `vram-budget-calc` → valide que le budget mémoire est positif sur le pool GPU ciblé ; refuser si négatif ou si incompatibilité matérielle (ex. FP8 sur Ampere).
4. Remplir la fiche modèle (`models/<model>/model.md`) avec budget, statut, contexte.
5. Générer l'entrée de passerelle (`backends` + `models` dans `charts/ai-gateway/values.yaml`), avec pricing et timeout adaptés.
6. Ouvrir une PR sur le repo de values (pas le repo de chart) — déclenche le flux GitOps standard.
7. Vérifier en CI : lint, template dry-run, cohérence du registre.
8. ArgoCD synchronise selon les sync waves définies.
9. Tests de fumée automatiques post-sync (`tests/smoke/`) : auth 401/200, complétion réelle, métrique de coût non nulle.
10. Promotion progressive : `priority` bas en gateway d'abord (canary), montée en charge progressive, puis priorité normale une fois validé sur trafic réel.
11. Ajouter le modèle au dashboard Grafana global et aux règles d'alerting Prometheus.
12. Documenter dans l'ADR si ce modèle introduit un nouveau pattern (nouveau format, nouveau moteur, nouvelle contrainte matérielle).

Cette checklist, une fois entièrement outillée (sections 2 et 4.3), transforme l'ajout d'un modèle d'une opération artisanale en une opération reproductible, testée et auditée — la condition nécessaire pour un système qui doit rester correct et compréhensible pendant des années, avec des équipes qui changent.