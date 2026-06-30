# Déploiement, Orchestration et Gestion des Modèles ML en Production

## Objectif du document

Maîtriser le cycle de vie complet d'un modèle en production : du packaging jusqu'au service de millions de clients, en passant par l'orchestration Kubernetes, la gestion fine des GPU, l'autoscaling, et les problèmes spécifiques selon les familles de modèles (LLM, vision, audio, multimodal, recommandation).

---

## 1. Les problèmes fondamentaux des modèles en production

### 1.1 Problèmes communs à tous les modèles

- **Cold start** : le chargement des poids (souvent plusieurs Go à centaines de Go) prend du temps, ce qui retarde la disponibilité d'un pod fraîchement démarré.
- **Drift de données (data drift)** : la distribution des entrées en production diverge progressivement de celle d'entraînement, dégradant la qualité des prédictions sans erreur explicite.
- **Drift de concept (concept drift)** : la relation entre entrée et sortie change dans le temps (comportement utilisateur, saisonnalité).
- **Latence p99 vs p50** : la moyenne masque les pires cas ; un modèle peut avoir un p50 excellent et un p99 catastrophique à cause du GC, du batching, ou de la contention GPU.
- **Non-déterminisme** : kernels CUDA non déterministes, ordre de réduction flottante différent selon le batch, rendant le debugging difficile.
- **Memory leaks** : fragmentation mémoire GPU (notamment avec des tailles de batch variables), fuites dans les caches KV pour les LLM.
- **Versioning incohérent** : divergence entre le modèle entraîné, le modèle converti (ONNX, TensorRT), et le modèle réellement servi.
- **Skew entraînement/inférence (training-serving skew)** : pipeline de features ou de preprocessing différent entre offline et online.
- **Pannes silencieuses** : le modèle répond mais avec des valeurs dégradées (NaN, sorties constantes) sans crash, donc sans alerte automatique.
- **Sur-engagement ressources (over-provisioning) ou sous-dimensionnement** : soit gaspillage de coûts GPU, soit famine en pic de trafic.

### 1.2 Problèmes spécifiques par famille de modèles

#### LLM (génératifs, type transformer décodeur)

- Gestion du **KV-cache** : croissance mémoire linéaire avec la longueur de séquence et le batch, source numéro un d'OOM GPU.
- **Batching continu (continuous batching)** complexe à implémenter correctement (vLLM, TGI, TensorRT-LLM) : sans cela, le throughput chute drastiquement.
- Latence **time-to-first-token (TTFT)** vs **time-per-output-token (TPOT)** : deux métriques différentes à monitorer séparément.
- **Quantification** (INT8, INT4, FP8) : compromis qualité/vitesse/mémoire à valider par famille de modèle, pas seulement architecture.
- Context window variable : nécessite du padding/masking efficace, sinon gaspillage de calcul.
- Streaming de tokens : gestion des connexions longues, timeouts, et reprise sur erreur.
- Sécurité : prompt injection, fuite de données via le cache partagé entre requêtes.

#### Vision (CNN, ViT, détection, segmentation)

- Tailles d'image variables : nécessite resize/padding cohérent en pré-traitement, sinon erreurs de shape.
- Pré/post-traitement souvent plus coûteux que l'inférence elle-même (NMS pour la détection d'objets).
- Batching difficile si les résolutions varient (vidéo en particulier).
- GPU sous-utilisé si le pipeline I/O (décodage image/vidéo) est le goulot d'étranglement plutôt que le calcul.

#### Audio / Speech (ASR, TTS)

- Traitement en flux temps réel (streaming) avec contrainte de latence stricte (< 300ms perçu comme acceptable).
- Longueur variable des séquences audio compliquant le batching.
- Modèles TTS autoregressifs lents token par token, similaires aux LLM en termes de problème de cache.

#### Recommandation / Ranking

- Très haut débit de requêtes (QPS) mais modèles souvent petits → le goulot devient le réseau et le feature store, pas le GPU.
- Fraîcheur des features critiques (recommandation basée sur des événements à la seconde près).
- Besoin de cohérence forte entre les features online et offline (feature store partagé).

#### Modèles multimodaux

- Pipelines hétérogènes (encodeur image + encodeur texte + decodeur) avec des besoins GPU différents par composant : difficile à placer efficacement sur un seul type de nœud.
- Synchronisation entre composants si déployés en microservices séparés, ajoutant de la latence réseau interne.

---

## 2. Architecture d'orchestration avec Kubernetes

### 2.1 Principes de base

Le modèle ne doit jamais être déployé comme un simple `Deployment` standard sans adaptation. Les éléments clés :

- **Readiness probe** distincte de la **liveness probe** : le readiness doit vérifier que les poids sont chargés et qu'une inférence test réussit, pas seulement que le process tourne.
- **Resource requests/limits** précis sur `nvidia.com/gpu`, CPU, et mémoire — jamais de limite GPU fractionnaire native sans MIG ou time-slicing configuré.
- **PodDisruptionBudget** pour éviter qu'un rolling update ou un node drain ne tue tous les pods d'un modèle simultanément.
- **Init containers** dédiés au téléchargement des poids depuis un object store (S3/GCS) vers un volume local rapide (NVMe local plutôt que réseau), pour découpler le pull d'image du chargement du modèle.

### 2.2 Gestion fine des GPU dans Kubernetes

- **NVIDIA device plugin** : expose les GPU comme ressource schedulable (`nvidia.com/gpu: 1`), mais par défaut sans partage.
- **MIG (Multi-Instance GPU)** sur GPU type A100/H100 : partitionne un GPU physique en plusieurs instances isolées (mémoire et calcul), utile pour des modèles petits ou moyens ne nécessitant pas un GPU entier.
- **Time-slicing** : partage temporel d'un GPU entre plusieurs pods sans isolation mémoire stricte — adapté pour des charges de travail tolérantes à la contention (dev/test, modèles légers).
- **NVIDIA GPU Operator** : automatise le déploiement des drivers, du device plugin, du DCGM exporter pour les métriques, et du container toolkit.
- **Topologie NUMA et affinité GPU-CPU** : pour les très gros modèles multi-GPU, l'affinité NUMA et la bande passante NVLink/PCIe doivent être prises en compte via `topologyManager` de kubelet.
- **Node pools dédiés** par type de GPU (A100, H100, L4, T4) avec `nodeSelector`/`taints-tolerations`, pour router chaque famille de modèle vers le matériel adapté à son ratio coût/performance.
- **Bin packing vs spreading** : pour maximiser l'utilisation GPU, préférer le bin packing (regrouper les charges sur peu de nœuds) plutôt que le spread par défaut de Kubernetes, via des schedulers custom (Volcano, Kueue) plus adaptés au batch ML que le scheduler par défaut.

### 2.3 Scheduling spécialisé pour l'IA

Le scheduler Kubernetes par défaut n'est pas conçu pour les charges ML batch et gang-scheduling. Solutions à considérer :

- **Kueue** : gestion de quotas et de files d'attente de jobs ML, priorité, et préemption.
- **Volcano** : gang scheduling (tous les pods d'un job distribué démarrent ensemble ou aucun), essentiel pour l'entraînement distribué et certains services d'inférence multi-GPU synchronisés.
- **Karpenter / Cluster Autoscaler** configuré spécifiquement pour provisionner des nœuds GPU à la demande, avec consolidation pour réduire les coûts en heures creuses.

### 2.4 Autoscaling

- **HPA (Horizontal Pod Autoscaler)** basé sur des métriques custom (QPS, latence p99, longueur de queue) plutôt que CPU/mémoire seuls — le CPU est rarement le facteur limitant pour l'inférence GPU.
- **KEDA** : scaling event-driven, utile pour scaler à zéro un modèle peu utilisé et le redémarrer à la demande (au prix d'un cold start à absorber).
- **VPA (Vertical Pod Autoscaler)** : moins pertinent pour le GPU (granularité grossière), plus utile pour ajuster CPU/mémoire des conteneurs auxiliaires (preprocessing, gateway).
- **Predictive scaling** : pour des patterns de trafic connus (heures de bureau, pics régionaux), un scaling proactif basé sur l'historique réduit la latence de cold start par rapport à un scaling purement réactif.
- **Scale-to-zero** : pertinent pour des modèles à faible trafic, mais incompatible avec une exigence de latence stricte sans un mécanisme de pré-chauffage (warm pool).

### 2.5 Service mesh et routage

- **Canary / Blue-Green deployment** pour les nouvelles versions de modèle : router un faible pourcentage du trafic vers la nouvelle version et comparer les métriques business avant bascule complète.
- **Shadow traffic (mirroring)** : dupliquer le trafic réel vers une nouvelle version sans impacter la réponse utilisateur, pour valider en conditions réelles sans risque.
- **Outils dédiés au serving ML** : KServe, Seldon Core, ou Ray Serve, qui ajoutent au-dessus de Kubernetes la gestion de versions de modèles, le batching automatique, le canary natif, et l'autoscaling adapté à l'inférence — préférables à une réimplémentation manuelle de ces mécanismes.

---

## 3. Observabilité et gestion automatique en production

### 3.1 Métriques à instrumenter obligatoirement

| Catégorie | Métriques clés |
|---|---|
| Latence | p50, p90, p99, TTFT et TPOT pour les LLM |
| Throughput | requêtes/s, tokens/s |
| Ressources | utilisation GPU (%), mémoire GPU utilisée/totale, température, power draw |
| Qualité | taux d'erreur, taux de NaN/sorties dégénérées, score de confiance moyen |
| Business | taux de conversion, satisfaction utilisateur, taux d'abandon |
| Coût | coût par requête, coût par token, coût par GPU-heure |

- **DCGM Exporter** (NVIDIA) couplé à Prometheus pour les métriques GPU bas niveau.
- **Distributed tracing** (OpenTelemetry) pour suivre une requête à travers preprocessing → inférence → postprocessing, indispensable pour les pipelines multimodaux.
- **Alerting basé sur des seuils dynamiques** plutôt que statiques, pour s'adapter à la saisonnalité du trafic.

### 3.2 Détection automatique de dérive et de dégradation

- Pipeline de **monitoring de drift** en continu, comparant la distribution des features/sorties en production à une fenêtre de référence (tests statistiques type KS-test, PSI).
- **Réentraînement automatique déclenché** par seuil de drift ou par dégradation de métrique business, avec validation automatique avant promotion.
- **Circuit breaker applicatif** : si le taux d'erreur ou la latence dépasse un seuil, basculer automatiquement vers un modèle de secours plus simple/léger (fallback model) plutôt que de laisser le service entier tomber.

### 3.3 Gestion automatique des incidents

- **Rollback automatique** déclenché par les métriques post-déploiement (pas seulement par échec de healthcheck), intégré au pipeline CI/CD.
- **Auto-healing** : redémarrage automatique des pods détectant un OOM GPU répété, avec backoff exponentiel pour éviter les boucles de crash.
- **Chaos engineering** ciblé GPU (simulation de panne de nœud GPU, de NVLink dégradé) pour valider la résilience avant incident réel.

---

## 4. Servir des millions de clients : considérations à grande échelle

- **Multi-région** : répliquer les modèles dans plusieurs régions pour réduire la latence réseau et assurer la continuité en cas de panne régionale, avec synchronisation de version stricte.
- **Edge caching de réponses** pour les requêtes répétitives (notamment recommandation et certains LLM avec prompts fréquents) afin de réduire la charge GPU.
- **Découplage stricte ingestion/inférence/réponse** via une architecture de queue (Kafka, NATS) pour absorber les pics sans perdre de requêtes.
- **Priorisation de trafic (QoS)** : distinguer les requêtes critiques (SLA garanti) des requêtes best-effort, avec des pools de capacité séparés.
- **Capacity planning basé sur des tests de charge réguliers** simulant le pic prévisible, pas seulement le trafic moyen.
- **Coût à l'échelle** : à des millions de requêtes, une optimisation de 10% sur le coût par requête (quantification, meilleur batching) représente des économies substantielles ; le monitoring de coût doit être traité comme une métrique de production au même titre que la latence.

---

## 5. Synthèse opérationnelle

Pour maîtriser le comportement d'un modèle en production de bout en bout, il faut traiter trois couches indépendamment mais de façon coordonnée :

1. **Couche modèle** : versioning strict, validation de qualité automatisée avant promotion, détection de drift continue, fallback de dégradation gracieuse.
2. **Couche infrastructure (Kubernetes + GPU)** : scheduling adapté à l'IA (Kueue/Volcano), partitionnement GPU (MIG/time-slicing), autoscaling basé sur des métriques métier, node pools dédiés par famille de matériel.
3. **Couche observabilité et automatisation** : métriques bout en bout (technique + business + coût), alerting dynamique, rollback et auto-healing automatiques, tests de charge réguliers.

La bonne pratique consiste à ne jamais traiter le déploiement de modèle comme un déploiement applicatif classique : les contraintes mémoire GPU, la variabilité de latence selon la longueur de séquence/image, et le besoin de validation de qualité continue (au-delà du simple "le service répond") imposent des outils et un design spécifiques au ML serving (KServe, Ray Serve, vLLM/TGI pour les LLM) plutôt qu'une réimplémentation ad hoc au-dessus d'un `Deployment` Kubernetes standard.