# Méthode Ultime : Gestion Fine des Modèles ML en Production (Bout en Bout → ArgoCD)

## Objectif

Donner une méthode ordonnée, outil par outil, pour faire passer un modèle de l'état "entraîné" à l'état "servi en production, visible et géré via GitOps/ArgoCD", en réglant à chaque étape les problèmes propres à cette étape. Chaque outil est présenté avec son **rôle exact** et **pourquoi il est indispensable** (pas juste une liste).

---

## Vue d'ensemble de la chaîne complète

```
[1. Packaging modèle] → [2. Registry modèle] → [3. Optimisation/Compilation]
→ [4. Conteneurisation] → [5. Registry image] → [6. CI] → [7. Manifeste K8s/Helm]
→ [8. Git (source de vérité)] → [9. ArgoCD (sync GitOps)] → [10. Scheduler GPU-aware]
→ [11. Serving runtime] → [12. Service mesh / routage] → [13. Autoscaling]
→ [14. Observabilité] → [15. Détection de drift / qualité] → [16. Rollback automatique]
```

Chaque étape résout une classe précise de problèmes. Les sauter casse la chaîne de garantie.

---

## Étape 1 — Packaging et versioning du modèle

**Outils** : MLflow Model Registry, DVC (Data Version Control), ou Hugging Face Hub privé.

**Rôle** : capturer le modèle, ses poids, ses hyperparamètres, son dataset de référence et ses métriques d'évaluation comme une unité versionnée et traçable.

**Pourquoi c'est critique** : sans cela, on perd la traçabilité entre "quel modèle tourne en prod" et "quel run d'entraînement l'a produit". C'est la cause numéro un d'incidents non reproductibles. MLflow donne un identifiant unique de version de modèle (`model:/name/version`) que toute la chaîne en aval référence.

---

## Étape 2 — Registre de modèles (Model Registry)

**Outils** : MLflow Registry, Weights & Biases Model Registry, ou un registre interne basé sur un object store (S3 + métadonnées).

**Rôle** : centraliser les versions promues (staging → production → archived), avec un système d'approbation (gate) avant promotion.

**Pourquoi c'est critique** : empêche un modèle non validé d'atteindre la production. C'est le point de contrôle qualité avant que quoi que ce soit ne touche Kubernetes.

---

## Étape 3 — Optimisation et compilation du modèle

**Outils selon la famille de modèle** :
- **TensorRT / TensorRT-LLM** (NVIDIA) : compilation pour GPU NVIDIA, fusion de kernels, quantification.
- **ONNX Runtime** : format intermédiaire portable, utile pour découpler framework d'entraînement et runtime d'inférence.
- **vLLM** ou **TGI (Text Generation Inference)** : runtime spécialisé LLM avec PagedAttention et continuous batching.
- **OpenVINO** : pour déploiement CPU/Intel.

**Rôle** : transformer le modèle brut (PyTorch/TensorFlow) en une forme optimisée pour la latence et le débit réels en production.

**Pourquoi c'est critique** : un modèle non compilé/non quantifié peut coûter 3 à 10x plus cher en GPU et avoir une latence 2 à 5x supérieure. C'est ici que se règlent la majorité des problèmes de KV-cache (vLLM), de fusion de kernels, et de précision numérique (FP16/INT8/FP8).

---

## Étape 4 — Conteneurisation

**Outils** : Docker avec image de base NVIDIA CUDA/cuDNN officielle, ou **BentoML** / **Cog** pour packager automatiquement un modèle en image servable.

**Rôle** : encapsuler le modèle, le runtime d'inférence, et les dépendances système (drivers CUDA compatibles) dans une image immuable.

**Pourquoi c'est critique** : règle le problème de "ça marche sur ma machine" — garantit que la version CUDA/cuDNN utilisée à l'entraînement est compatible avec celle de l'inférence, source fréquente de bugs silencieux (résultats différents) ou de crashs.

**Point d'attention** : ne jamais télécharger les poids du modèle dans l'image Docker elle-même (image trop lourde, rebuild inutile à chaque update de poids). Les poids doivent être chargés au démarrage depuis l'object store via un init container (voir étape 10).

---

## Étape 5 — Registre d'images

**Outils** : Harbor (self-hosted, avec scan de vulnérabilités intégré), ou registres cloud (ECR, GCR, ACR).

**Rôle** : stocker les images versionnées et scannées, avec contrôle d'accès.

**Pourquoi c'est critique** : Harbor ajoute un scan automatique de CVE à chaque push — indispensable pour des images contenant des dépendances Python/CUDA nombreuses, souvent vecteur de vulnérabilités.

---

## Étape 6 — Intégration continue (CI)

**Outils** : GitHub Actions, GitLab CI, ou Jenkins.

**Rôle** : automatiser build de l'image, tests unitaires du code de serving, tests de fumée (smoke test : charger le modèle et faire une inférence test), scan de sécurité, et push vers le registre d'images.

**Pourquoi c'est critique** : garantit qu'aucune image ne part en production sans avoir passé un test d'inférence minimal. C'est le dernier filet avant que le manifeste Kubernetes ne soit mis à jour.

**Étape clé** : le pipeline CI se termine en mettant à jour le tag d'image dans le dépôt Git de manifestes (pas en déployant directement) — c'est ce qui permet le GitOps via ArgoCD.

---

## Étape 7 — Manifestes Kubernetes / Helm

**Outils** : Helm charts, ou Kustomize pour les overlays par environnement (dev/staging/prod).

**Rôle** : définir de façon déclarative le Deployment, les requests/limits GPU, les probes, le HPA, le PodDisruptionBudget, les ConfigMaps de configuration du modèle.

**Pourquoi c'est critique** : Helm permet de templatiser et versionner la configuration d'infrastructure exactement comme le code, avec des valeurs différentes par environnement (ex : 1 GPU en staging, 4 GPU en prod).

**Outils spécialisés à utiliser ici** : **KServe** (CRD `InferenceService`) ou **Seldon Core** au lieu d'un Deployment brut — ils encapsulent déjà readiness probe adaptée au ML, autoscaling basé sur la charge réelle, et support natif du canary.

---

## Étape 8 — Git comme source de vérité

**Outils** : dépôt Git séparé pour les manifestes (pattern "config repo" distinct du "code repo").

**Rôle** : devenir l'unique source de vérité de l'état désiré du cluster. Toute modification de configuration passe par une Pull Request.

**Pourquoi c'est critique** : c'est le fondement du GitOps — sans cela, ArgoCD n'a rien à synchroniser. Cela donne aussi un historique d'audit complet (qui a changé quoi, quand) et permet un rollback par simple `git revert`.

---

## Étape 9 — ArgoCD (synchronisation GitOps)

**Rôle exact d'ArgoCD** : surveiller en continu le dépôt Git de manifestes et synchroniser automatiquement l'état réel du cluster Kubernetes avec l'état déclaré dans Git. Si quelqu'un modifie manuellement une ressource dans le cluster (`kubectl edit`), ArgoCD détecte la dérive (**drift d'infrastructure**, différent du drift de données vu en partie 1) et peut la corriger automatiquement (self-healing) ou alerter.

**Pourquoi c'est l'outil central de visibilité demandé** :
- **Dashboard visuel** : ArgoCD montre l'arbre complet des ressources déployées (Deployment → ReplicaSet → Pods → Services) avec leur état de santé en temps réel (Healthy/Degraded/Progressing).
- **Sync automatique ou manuel** : on choisit si un changement dans Git se déploie automatiquement ou nécessite une approbation manuelle dans l'interface — utile pour la production où on veut un gate humain avant promotion.
- **Rollback en un clic** : ArgoCD garde l'historique des syncs précédents, permettant un retour à une version antérieure du manifeste immédiatement.
- **App of Apps pattern** : pour gérer plusieurs modèles/services comme une hiérarchie d'applications ArgoCD, utile quand on opère plusieurs modèles (UMC, NEURAX, etc. si chacun a son propre service d'inférence).
- **Notifications** (ArgoCD Notifications controller) : alerte Slack/email automatique en cas d'échec de sync ou de dégradation de santé d'une application.

**Configuration recommandée pour le ML** :
- Health checks custom dans ArgoCD pour les CRD KServe `InferenceService` (ArgoCD ne connaît pas nativement l'état de santé d'un CRD custom sans configuration de `resource.customizations`).
- Sync waves pour ordonner le déploiement (ConfigMap de config modèle avant le Deployment, par exemple).
- Projects ArgoCD pour isoler les permissions par équipe/modèle (RBAC).

---

## Étape 10 — Scheduler GPU-aware (au moment où ArgoCD synchronise)

**Outils** : Kueue ou Volcano (mentionnés dans le document précédent), couplés au NVIDIA GPU Operator.

**Rôle** : une fois qu'ArgoCD a appliqué le manifeste, c'est le scheduler Kubernetes (étendu par Kueue/Volcano) qui décide sur quel nœud GPU placer le pod, en respectant les quotas et l'affinité matérielle.

**Pourquoi c'est critique à ce stade** : ArgoCD ne fait que déclarer l'intention ; sans un scheduler GPU-aware, le pod peut rester en `Pending` indéfiniment ou être placé sur un nœud sous-optimal. C'est visible dans ArgoCD comme un état "Progressing" qui ne devient jamais "Healthy" — donc directement diagnostiqué via l'interface ArgoCD.

---

## Étape 11 — Runtime de serving

**Outils par famille** (résumé du document précédent, appliqué concrètement ici) :
- LLM : vLLM, TGI, TensorRT-LLM
- Vision/classique : Triton Inference Server (NVIDIA) — supporte plusieurs frameworks et formats simultanément, avec batching dynamique natif
- Multi-framework généraliste : Triton ou Ray Serve

**Rôle** : exécuter réellement l'inférence avec gestion optimale du batching, du cache, et du GPU.

**Pourquoi Triton en particulier** : il expose nativement des métriques Prometheus, supporte le model ensemble (pipelines multi-modèles), et gère le multi-versioning (plusieurs versions du même modèle servies simultanément) — directement utile pour le canary géré par ArgoCD/KServe.

---

## Étape 12 — Service mesh / routage intelligent

**Outils** : Istio ou Linkerd, souvent intégrés automatiquement par KServe.

**Rôle** : gérer le routage canary (pourcentage de trafic), le mirroring (shadow traffic), le mTLS entre services, et les retries/timeouts au niveau réseau.

**Pourquoi c'est critique** : permet de tester une nouvelle version de modèle sur un faible pourcentage de trafic réel sans risque, ET cette répartition de trafic est elle-même déclarée dans Git et synchronisée par ArgoCD (le pourcentage canary devient une valeur versionnée).

---

## Étape 13 — Autoscaling

**Outils** : KEDA (event-driven) + HPA custom metrics, déjà détaillés précédemment.

**Rôle** : ajuster dynamiquement le nombre de réplicas en fonction de la charge réelle (QPS, longueur de queue).

**Pourquoi visible dans ArgoCD** : le HPA/ScaledObject est lui-même un manifeste géré par Git/ArgoCD — donc sa configuration (seuils, min/max replicas) est versionnée et auditable comme le reste.

---

## Étape 14 — Observabilité

**Outils** : Prometheus (métriques) + Grafana (dashboards) + DCGM Exporter (GPU) + OpenTelemetry (tracing) + Loki (logs).

**Rôle** : donner une vue complète et corrélée de la santé technique et business du modèle en production.

**Pourquoi c'est le complément indispensable d'ArgoCD** : ArgoCD montre que le déploiement correspond à Git (santé d'infrastructure), mais ne dit rien sur la qualité des prédictions ou la performance réelle. Grafana montre l'état runtime ; ArgoCD montre l'état déclaratif. Les deux dashboards doivent être consultés ensemble.

---

## Étape 15 — Détection de drift et qualité continue

**Outils** : Evidently AI, WhyLabs, ou Arize AI.

**Rôle** : surveiller en continu la distribution des inputs/outputs et alerter en cas de dérive significative.

**Pourquoi c'est l'étape souvent oubliée** : un modèle peut rester "Healthy" dans ArgoCD (le pod tourne, répond aux probes) tout en étant qualitativement dégradé. Cette couche comble l'angle mort que ni Kubernetes ni ArgoCD ne couvrent.

---

## Étape 16 — Rollback automatique

**Mécanisme** : intégration entre l'outil de drift/qualité (étape 15) et ArgoCD via un webhook ou un contrôleur custom qui déclenche un `git revert` automatique sur le repo de manifestes en cas de dégradation détectée.

**Rôle** : fermer la boucle complètement automatique — détection de problème → action corrective sans intervention humaine.

**Pourquoi c'est l'aboutissement de la méthode** : cela transforme ArgoCD d'un simple outil de synchronisation en un système de contrôle fermé (closed-loop), où Git redevient la source de vérité même après une correction automatique (traçabilité totale du rollback).

---

## Tableau récapitulatif : rôle et importance de chaque outil

| Outil | Rôle | Sans cet outil, le problème non résolu |
|---|---|---|
| MLflow Registry | Versioning et traçabilité du modèle | Impossible de savoir quel run a produit le modèle en prod |
| TensorRT/vLLM/TGI | Compilation et optimisation runtime | Latence et coût GPU non maîtrisés |
| Docker + image CUDA figée | Reproductibilité environnement | Bugs silencieux liés à des versions CUDA différentes |
| Harbor | Registre d'images sécurisé | Vulnérabilités non détectées avant déploiement |
| CI (GitHub Actions/GitLab CI) | Validation automatique avant déploiement | Images non testées en production |
| Helm/Kustomize | Déclaration d'infrastructure versionnée | Configuration divergente entre environnements |
| KServe/Seldon | Abstraction ML-native sur K8s | Réimplémentation manuelle fragile du canary/autoscaling |
| Git (config repo) | Source de vérité unique | Pas d'audit, pas de rollback fiable |
| **ArgoCD** | **Synchronisation GitOps et visibilité centrale** | **Dérive non détectée entre Git et cluster réel, pas de vue centralisée** |
| Kueue/Volcano | Scheduling GPU-aware | Pods en Pending, mauvais placement GPU |
| Triton/vLLM | Exécution d'inférence optimisée | Sous-utilisation GPU, throughput faible |
| Istio/Linkerd | Routage canary et mTLS | Déploiements risqués sans contrôle de trafic |
| KEDA/HPA | Adaptation dynamique à la charge | Sur-provisionnement coûteux ou sous-capacité en pic |
| Prometheus/Grafana/DCGM | Observabilité technique | Incidents détectés trop tard ou pas du tout |
| Evidently/WhyLabs | Qualité et drift en continu | Dégradation silencieuse de la qualité des prédictions |

---

## Ordre d'implémentation recommandé (priorisation réaliste)

1. Docker + image CUDA figée (base indispensable)
2. Git config repo + Helm charts
3. ArgoCD (visibilité immédiate, gain rapide)
4. CI minimal (build + smoke test)
5. Prometheus/Grafana + DCGM (observabilité de base)
6. KServe ou Seldon (remplace le Deployment brut)
7. Runtime optimisé (vLLM/TensorRT/Triton selon la famille de modèle)
8. Kueue/Volcano (une fois plusieurs modèles en concurrence sur les mêmes GPU)
9. Istio + canary (une fois le rythme de déploiement de nouvelles versions augmente)
10. KEDA (une fois le trafic devient variable/imprévisible)
11. MLflow Registry (formaliser le versioning si pas déjà fait en amont)
12. Evidently/WhyLabs + rollback automatique (maturité finale, closed-loop)

Cet ordre évite de sur-ingénierer prématurément : ArgoCD et l'observabilité de base apportent la valeur la plus immédiate et doivent venir tôt, tandis que le rollback automatique fermé en boucle est la dernière brique, une fois que toute la chaîne de signal (qualité, drift) est fiable.