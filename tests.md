# Suite de Certification — Validation Rigoureuse du Système de Serving de Modèles

## Objectif

Définir l'ensemble complet des tests que le système doit passer avant d'être approuvé pour la production. Chaque test a un objectif précis, une méthode d'exécution, un critère de réussite/échec strict (binaire, non ambigu), et explique ce qu'il renforce dans le système. Aucun test n'est cosmétique : chaque échec doit bloquer la promotion en production.

Principe directeur : **un système n'est jamais "à peu près prêt"**. Chaque test ci-dessous a un verdict GO / NO-GO. Le système n'est approuvé que lorsque 100% des tests bloquants passent.

---

## Catégorie 1 — Tests de packaging et d'intégrité du modèle

### T1.1 — Intégrité cryptographique des poids
**Objectif** : garantir que les poids déployés sont exactement ceux validés, sans corruption ni substitution.
**Méthode** : calcul de checksum SHA-256 du fichier de poids au moment de la promotion dans le registre, comparaison automatique du checksum au moment du chargement par le moteur de serving.
**Critère de réussite** : 100% de correspondance du checksum, sinon échec bloquant immédiat du démarrage du pod.
**Renforce** : élimine toute possibilité de dérive silencieuse entre "ce qui a été validé" et "ce qui tourne réellement".

### T1.2 — Cohérence format/moteur
**Objectif** : vérifier que le moteur sélectionné correspond réellement au format déclaré du modèle.
**Méthode** : test automatisé exécutant `engine-selector` sur chaque entrée du registre et comparant le moteur proposé au moteur effectivement configuré dans le chart.
**Critère de réussite** : 0 divergence détectée.
**Renforce** : empêche qu'un modèle GGUF se retrouve accidentellement configuré sur un chart vLLM (incompatibilité totale), erreur de configuration manuelle la plus fréquente.

### T1.3 — Validation du budget mémoire avant déploiement
**Objectif** : garantir qu'aucun modèle n'est déployé sans preuve mathématique qu'il tient dans la VRAM cible.
**Méthode** : `vram-budget-calc` exécuté en CI, calcul = VRAM_totale × 0.85 − poids − overhead, doit être strictement positif et couvrir le contexte maximal déclaré.
**Critère de réussite** : budget KV-cache disponible ≥ besoin pour le `maxOutputTokens` et `contextLength` déclarés en gateway.
**Renforce** : élimine la classe d'incidents OOM en production, la plus fréquente et la plus évitable.

### T1.4 — Incompatibilité matérielle bloquée
**Objectif** : empêcher le déploiement de checkpoints incompatibles avec l'architecture GPU cible (ex. FP8 sur GPU sans support natif).
**Méthode** : règle codée en dur testée par cas : tentative de déploiement d'un modèle FP8 sur un nodeSelector Ampere doit être rejetée par la CI.
**Critère de réussite** : rejet systématique à 100% des cas testés (matrice de combinaisons format × architecture GPU).
**Renforce** : transforme une connaissance tribale ("on sait qu'il ne faut pas faire ça") en garde-fou automatique.

---

## Catégorie 2 — Tests d'infrastructure déclarative (Helm/Kustomize/Git)

### T2.1 — Lint strict sur 100% des charts
**Méthode** : `helm lint --strict` exécuté sur chaque chart du dépôt à chaque commit, pas seulement les charts modifiés.
**Critère de réussite** : zéro warning, zéro erreur, sur l'intégralité des charts (y compris les charts dépendants partagés comme `bjw-template`).
**Renforce** : détecte les régressions de dépendances partagées qu'un test ciblé sur le seul chart modifié manquerait.

### T2.2 — Rendu à blanc (dry-run) complet
**Méthode** : `helm template --dry-run` sur chaque combinaison chart × environnement (dev/staging/prod), validation du YAML généré contre le schéma Kubernetes (`kubeconform` ou équivalent).
**Critère de réussite** : 100% des manifestes générés sont syntaxiquement valides et conformes au schéma de l'API Kubernetes ciblée.
**Renforce** : empêche qu'un manifeste cassé n'atteigne ArgoCD, où l'échec serait détecté plus tard et plus coûteux à corriger.

### T2.3 — Cohérence du registre déclaratif
**Méthode** : test vérifiant que chaque entrée de `models/registry.yaml` possède : un chart correspondant, une entrée de passerelle correspondante, un fichier `budget.md` prouvé, un statut valide (LIVE/STAGED/STANDBY).
**Critère de réussite** : 0 entrée orpheline dans un sens comme dans l'autre (modèle sans chart, ou chart sans entrée registre).
**Renforce** : empêche la dérive entre documentation et réalité du déploiement, condition de survie pour un système opéré pendant des années.

### T2.4 — Idempotence du rendu Helm
**Méthode** : exécuter `helm template` deux fois sur le même commit et comparer les sorties.
**Critère de réussite** : sortie strictement identique bit à bit entre les deux exécutions.
**Renforce** : garantit la reproductibilité — un système non idempotent ne peut pas être audité ni reproduit de façon fiable.

### T2.5 — Détection de secrets en clair
**Méthode** : scan automatisé (`gitleaks` ou équivalent) sur chaque commit du repo de charts et du repo de values.
**Critère de réussite** : 0 détection de clé API, token, ou mot de passe en clair.
**Renforce** : empêche la classe de faille la plus dommageable et la plus fréquente dans les dépôts GitOps.

---

## Catégorie 3 — Tests de synchronisation ArgoCD

### T3.1 — Convergence de sync waves
**Objectif** : valider que l'ordre de déploiement (-3 → -2 → -1 → 0 → 1 → 2+) est strictement respecté et que chaque wave atteint l'état "Healthy" avant que la suivante ne démarre.
**Méthode** : déploiement complet sur un cluster de test éphémère, capture des timestamps de transition d'état de chaque wave, vérification de l'ordre chronologique.
**Critère de réussite** : aucune ressource d'une wave N+1 ne devient "Progressing" avant que toutes les ressources de la wave N soient "Healthy".
**Renforce** : empêche les incidents de type "le workload démarre avant que le stockage soit prêt", déjà identifiés comme coûteux dans le pattern existant.

### T3.2 — Self-healing après dérive manuelle
**Objectif** : vérifier qu'ArgoCD détecte et corrige automatiquement toute modification manuelle (`kubectl edit`) d'une ressource gérée.
**Méthode** : modifier manuellement une ressource déployée (ex. changer le nombre de réplicas), mesurer le délai avant correction automatique.
**Critère de réussite** : correction automatique en moins de 3 minutes (intervalle de réconciliation par défaut), sans intervention humaine.
**Renforce** : garantit que Git reste réellement la source de vérité unique, pas seulement en théorie.

### T3.3 — Health check custom des CRD ML
**Méthode** : déployer un `InferenceService` (KServe) ou équivalent et vérifier que le `health.lua` custom rapporte correctement "Healthy" une fois le modèle réellement prêt (pas seulement le pod démarré).
**Critère de réussite** : transition "Progressing" → "Healthy" dans ArgoCD survient au même moment que le `/health` du conteneur retourne 200, à moins de 5 secondes d'écart.
**Renforce** : élimine l'angle mort où ArgoCD affiche un faux "Progressing" indéfini sur des CRD non standard.

### T3.4 — Rollback en un clic, vérifié
**Méthode** : déployer une version cassée intentionnellement (ex. mauvais tag d'image), déclencher un rollback ArgoCD vers le sync précédent, mesurer le temps de retour à l'état sain.
**Critère de réussite** : retour à l'état "Healthy" en moins de 5 minutes après déclenchement du rollback.
**Renforce** : valide que le filet de sécurité de dernier recours fonctionne réellement, pas seulement en théorie sur la documentation.

### T3.5 — App-of-Apps cohérent à l'échelle
**Méthode** : avec N applications gérées (≥ 20, comme dans le pattern actuel à ~21 Applications), vérifier qu'un changement au niveau racine se propage correctement à toutes les applications enfants sans collision de sync wave.
**Critère de réussite** : 100% des applications enfants atteignent "Synced + Healthy" après propagation, sans deadlock de dépendance.
**Renforce** : valide la scalabilité de la structure GitOps elle-même quand le nombre de modèles/services augmente.

---

## Catégorie 4 — Tests de chargement et de démarrage du modèle

### T4.1 — Cold start mesuré et borné
**Méthode** : démarrage d'un pod à froid (pas de cache), chronométrage du temps entre création du pod et premier `200 OK` sur `/health`.
**Critère de réussite** : temps de cold start documenté et inférieur au seuil déclaré dans la fiche modèle (ex. budget `failureThreshold` des probes de démarrage), avec marge de 20%.
**Renforce** : empêche les surprises en production où le cold start réel dépasse ce que les probes tolèrent, causant des redémarrages en boucle.

### T4.2 — Probes différenciées correctement configurées
**Méthode** : vérifier que la liveness probe ne dépend jamais de la disponibilité du modèle (sinon un chargement long déclenche un kill en boucle), et que la readiness probe vérifie réellement une inférence test fonctionnelle.
**Critère de réussite** : liveness reste "OK" pendant tout le chargement ; readiness passe à "OK" seulement après une inférence de validation réussie.
**Renforce** : élimine la classe d'incidents de crash-loop causée par une probe mal conçue, déjà anticipée dans le pattern (tcpSocket pour liveness, httpGet long timeout pour startup).

### T4.3 — Test de chargement concurrent (PVC RWX partagé)
**Méthode** : démarrer simultanément plusieurs réplicas pointant vers le même volume RWX de poids, vérifier l'absence de corruption ou de contention bloquante.
**Critère de réussite** : tous les réplicas démarrent avec succès en parallèle, temps de démarrage du Nème réplica non significativement dégradé par rapport au premier.
**Renforce** : valide que le pattern de volume partagé scale au-delà d'un seul réplica par modèle.

---

## Catégorie 5 — Tests fonctionnels de l'API de serving

### T5.1 — Conformité OpenAI-compatible stricte
**Méthode** : suite de tests de contrat (schema validation) sur `/v1/chat/completions` couvrant tous les moteurs (llama.cpp, vLLM, ONNX Runtime GenAI, Triton), comparant la structure de réponse au schéma OpenAI officiel.
**Critère de réussite** : 100% de conformité de schéma, quel que soit le moteur sous-jacent.
**Renforce** : garantit l'interchangeabilité réelle des moteurs du point de vue client, principe fondamental de l'architecture (section 4.1 du document d'architecture).

### T5.2 — Streaming SSE robuste
**Méthode** : test de connexion longue avec coupure réseau simulée à mi-flux, vérification du comportement de reprise ou d'échec propre côté client.
**Critère de réussite** : aucun token tronqué silencieusement sans signal d'erreur explicite au client ; pas de fuite de connexion côté serveur après coupure.
**Renforce** : évite les réponses corrompues silencieuses, particulièrement critique pour les LLM en production.

### T5.3 — Authentification native vérifiée
**Méthode** : requête sans clé API (attendu 401), requête avec clé invalide (attendu 401), requête avec clé valide (attendu 200), sur chaque backend.
**Critère de réussite** : comportement exact 401/401/200 sur 100% des backends, y compris ceux utilisant un sidecar (ex. Caddy pour vLLM).
**Renforce** : sécurité de base non négociable avant toute exposition publique.

### T5.4 — Test de contexte maximal réel
**Méthode** : envoi d'une requête atteignant exactement le `contextLength` déclaré en gateway, puis une requête le dépassant d'un token.
**Critère de réussite** : la requête à la limite réussit ; la requête en dépassement échoue avec une erreur explicite (pas un crash, pas un troncage silencieux).
**Renforce** : élimine l'écart entre la capacité annoncée et la capacité réelle, source de confusion utilisateur et d'incidents.

### T5.5 — Test de cohérence multi-modèle (registre de modèles)
**Méthode** : pour chaque modèle déclaré actif, exécuter une requête de complétion réelle et vérifier une réponse cohérente (non vide, non NaN, format correct).
**Critère de réussite** : 100% des modèles déclarés "LIVE" répondent correctement ; tout échec bloque le déploiement de l'ensemble du registre.
**Renforce** : empêche qu'un modèle cassé reste invisible parce que l'attention se porte uniquement sur le dernier modèle modifié.

---

## Catégorie 6 — Tests de robustesse GPU et de scheduling

### T6.1 — Placement correct selon nodeSelector/taints
**Méthode** : déployer chaque chart de modèle et vérifier que le pod est effectivement placé sur le node pool attendu (vérification du label du nœud réel).
**Critère de réussite** : 100% de correspondance entre pool déclaré et pool réel d'exécution.
**Renforce** : empêche qu'un modèle coûteux en GPU H100 se retrouve placé sur un nœud L4 par erreur de configuration, ou inversement qu'un modèle léger gaspille un GPU haut de gamme.

### T6.2 — Comportement en pénurie de GPU (Kueue/Volcano)
**Méthode** : simuler une demande de GPU supérieure à la capacité disponible, vérifier que les jobs de priorité supérieure passent avant les jobs de priorité inférieure (préemption correcte).
**Critère de réussite** : ordre de passage strictement conforme aux priorités déclarées, aucun job de priorité basse ne bloque indéfiniment un job de priorité haute (pas de famine).
**Renforce** : valide que le scheduling reste équitable et prévisible même en charge maximale, condition essentielle à grande échelle.

### T6.3 — Détection et éviction de nœud GPU défaillant
**Méthode** : simuler une erreur Xid NVIDIA (via injection de fault ou environnement de test dédié), vérifier la détection par le GPU Operator et le cordon/drain automatique du nœud.
**Critère de réussite** : nœud marqué "non schedulable" en moins de 2 minutes après détection de l'erreur, pods existants migrés sans perte de requête en cours (si possible) ou avec erreur propre signalée au client.
**Renforce** : transforme une panne matérielle silencieuse en incident détecté et géré automatiquement.

### T6.4 — Fragmentation mémoire sous charge prolongée
**Méthode** : test de charge soutenue sur plusieurs heures avec tailles de batch variables, monitoring de la mémoire GPU libre réelle vs théorique.
**Critère de réussite** : pas de croissance monotone de la mémoire utilisée au-delà de ce qu'explique le trafic (signe de fuite/fragmentation) sur une fenêtre de 4 heures minimum.
**Renforce** : détecte les fuites mémoire avant qu'elles ne causent un OOM en production après plusieurs jours d'uptime, scénario classique non couvert par des tests courts.

### T6.5 — Validation MIG/time-slicing
**Méthode** : sur les nœuds configurés en partitionnement GPU, déployer plusieurs pods sur le même GPU physique et vérifier l'isolation (mémoire et/ou performance) effective.
**Critère de réussite** : un pod ne peut pas observer ou impacter la mémoire d'un autre pod sur la même carte au-delà de la dégradation de performance attendue et documentée pour le mode choisi.
**Renforce** : valide qu'un partitionnement mal configuré ne devient pas une faille d'isolation ou un goulot d'étranglement caché.

---

## Catégorie 7 — Tests de charge et de performance

### T7.1 — Test de charge nominale
**Méthode** : k6/Locust simulant le trafic moyen attendu, mesure de p50/p90/p99 de latence, TTFT et TPOT pour les LLM.
**Critère de réussite** : tous les percentiles dans les SLA déclarés en fiche modèle, aucune erreur 5xx sur la durée du test.
**Renforce** : établit la ligne de base de référence pour détecter toute régression future.

### T7.2 — Test de pic (stress test)
**Méthode** : montée en charge jusqu'à 3x le trafic de pointe historique, observation du comportement de dégradation.
**Critère de réussite** : dégradation gracieuse (latence augmente, mais pas d'erreurs en cascade ni de crash) ; l'autoscaling (HPA/KEDA) réagit dans la fenêtre attendue (ex. < 60 secondes pour déclencher un scale-out).
**Renforce** : valide que le système plie sans casser, principe fondamental de robustesse à grande échelle.

### T7.3 — Test de soutenabilité (endurance)
**Méthode** : trafic nominal soutenu sur 24 à 72 heures continues.
**Critère de réussite** : aucune dégradation progressive de latence ou de taux d'erreur sur la durée, mémoire stable (lié à T6.4).
**Renforce** : détecte les problèmes qui n'apparaissent qu'après une longue durée d'exécution (fuites, fragmentation, accumulation de connexions).

### T7.4 — Test de cold start en rafale
**Méthode** : déclenchement simultané du démarrage de N réplicas à froid (scale-out massif soudain), mesure du temps avant que tous deviennent "Ready".
**Critère de réussite** : temps de convergence de l'ensemble du groupe documenté et acceptable pour le SLA de pic, sans contention excessive sur le registre d'images ou le stockage de poids partagé.
**Renforce** : valide le comportement réel lors d'un pic brutal et imprévu, scénario le plus critique pour servir des millions d'utilisateurs.

### T7.5 — Validation du scale-to-zero et réveil
**Méthode** : pour les modèles à faible trafic configurés en scale-to-zero (KEDA), mesurer le délai entre la première requête et la réponse effective après réveil.
**Critère de réussite** : délai de réveil documenté et conforme au SLA déclaré pour ce modèle (différent et plus tolérant que les modèles toujours actifs).
**Renforce** : valide que l'optimisation de coût ne casse pas l'expérience utilisateur au-delà de ce qui est acceptable.

---

## Catégorie 8 — Tests de résilience et de chaos engineering

### T8.1 — Kill aléatoire de pod (chaos basique)
**Méthode** : suppression forcée et aléatoire de pods de serving pendant un trafic actif (type Chaos Mesh ou Litmus).
**Critère de réussite** : aucune requête en cours perdue silencieusement (erreur propre retournée au client si interruption), nouveau pod opérationnel et absorbant le trafic en moins du temps de cold start documenté.
**Renforce** : valide la résilience de base sans attendre un vrai incident pour la découvrir.

### T8.2 — Panne de zone/région simulée
**Méthode** : couper artificiellement l'accès à un cluster de travail entier, vérifier le basculement automatique du trafic gateway vers un backend de secours (autre région, ou fallback SaaS).
**Critère de réussite** : bascule effective en moins du délai annoncé dans le SLA de continuité, perte de requêtes minimisée et mesurée.
**Renforce** : valide la stratégie multi-région/anti-lock-in définie dans l'architecture, qui sinon resterait une intention théorique jamais vérifiée.

### T8.3 — Dégradation du registre de modèles (MLflow indisponible)
**Méthode** : couper l'accès au registre de modèles pendant une opération de déploiement.
**Critère de réussite** : les modèles déjà en production continuent de servir sans interruption (le registre n'est pas un point de défaillance unique pour le runtime, seulement pour les nouvelles promotions).
**Renforce** : valide la séparation correcte entre plan de contrôle (registre, CI/CD) et plan de données (serving réel).

### T8.4 — Panne de la passerelle (Envoy AI Gateway)
**Méthode** : simuler l'indisponibilité totale de la passerelle.
**Critère de réussite** : comportement défini et documenté (ex. fallback DNS direct vers un backend secondaire, ou dégradation contrôlée avec message d'erreur clair) plutôt qu'une coupure silencieuse totale.
**Renforce** : la passerelle étant le point d'entrée unique de tout le système, ce test révèle si elle constitue un SPOF (single point of failure) non géré.

### T8.5 — Corruption de données en transit
**Méthode** : injection de paquets malformés ou de latence réseau artificielle (Toxiproxy) entre la passerelle et les backends.
**Critère de réussite** : aucune réponse corrompue silencieuse délivrée au client ; timeout et retry appliqués correctement selon la configuration déclarée.
**Renforce** : valide la robustesse réseau interne, souvent négligée car testée seulement en conditions de laboratoire idéales.

### T8.6 — Test de drift de données simulé
**Méthode** : injecter artificiellement un changement de distribution dans le trafic de test (ex. requêtes hors domaine d'entraînement), vérifier la détection par l'outil de monitoring de drift (Evidently AI).
**Critère de réussite** : alerte de drift déclenchée dans la fenêtre de détection attendue (ex. < 1 heure), avec déclenchement effectif du circuit breaker applicatif si configuré.
**Renforce** : valide que la dégradation silencieuse de qualité (l'angle mort le plus dangereux car invisible dans les métriques d'infrastructure) est réellement détectée, pas seulement supposée l'être.

---

## Catégorie 9 — Tests de sécurité

### T9.1 — Scan de vulnérabilités d'image
**Méthode** : scan Harbor/Trivy de chaque image avant promotion.
**Critère de réussite** : zéro vulnérabilité CRITICAL non patchée ; vulnérabilités HIGH documentées et acceptées explicitement si non corrigeables immédiatement.
**Renforce** : empêche qu'une dépendance CUDA/Python vulnérable connue n'atteigne la production.

### T9.2 — Isolation réseau (NetworkPolicy)
**Méthode** : vérifier qu'un pod de serving ne peut PAS initier de connexion sortante non autorisée (ex. vers Internet directement, en dehors de ce qui est strictement nécessaire).
**Critère de réussite** : toute tentative de connexion non listée dans la politique d'egress est bloquée et journalisée.
**Renforce** : limite la surface d'attaque en cas de compromission d'un conteneur de serving (exfiltration de données ou de poids de modèle).

### T9.3 — Test de prompt injection (LLM)
**Méthode** : suite de prompts adversariaux connus testant la résistance du modèle/de la couche applicative à l'extraction de instructions système ou au contournement de garde-fous.
**Critère de réussite** : comportement conforme à la politique de sécurité définie (pas de fuite de prompt système, pas de génération de contenu hors politique), avec taux de réussite mesuré et suivi dans le temps (pas de régression silencieuse à chaque changement de modèle).
**Renforce** : sécurité spécifique à la famille LLM, absente des tests d'infrastructure classiques.

### T9.4 — Signature et provenance des images
**Méthode** : vérification cosign de la signature de chaque image avant que `argocd-image-updater` ne propose une mise à jour.
**Critère de réussite** : rejet automatique de toute image non signée ou signée par une identité non autorisée.
**Renforce** : empêche l'injection d'une image malveillante dans la chaîne de déploiement automatisée (supply chain attack).

### T9.5 — Rotation et expiration des secrets
**Méthode** : vérifier que les clés API et secrets gérés via External Secrets Operator sont correctement rafraîchis après rotation côté AWS Secrets Manager/Vault, sans redémarrage manuel requis.
**Critère de réussite** : nouvelle valeur de secret active dans le système en moins du délai de synchronisation déclaré, ancienne valeur révoquée sans interruption de service.
**Renforce** : valide qu'une politique de sécurité (rotation régulière) ne casse pas la disponibilité, motif fréquent pour lequel les équipes désactivent la rotation en pratique.

---

## Catégorie 10 — Tests de coût et de gouvernance économique

### T10.1 — Émission correcte de métrique de coût
**Méthode** : pour chaque requête de complétion, vérifier que la règle CEL de tarification émet une valeur non nulle et cohérente avec le volume de tokens réellement consommés.
**Critère de réussite** : écart entre coût calculé et coût attendu (calcul manuel de référence) inférieur à 1%.
**Renforce** : garantit que le modèle de pricing cost-recovery (ADR-0028 dans le pattern existant) reflète la réalité, condition de la soutenabilité économique du système.

### T10.2 — Alerte de dérive de coût
**Méthode** : simuler un trafic anormalement élevé sur un modèle coûteux, vérifier le déclenchement d'une alerte budgétaire avant que le coût ne devienne incontrôlé.
**Critère de réussite** : alerte déclenchée avant que le coût cumulé ne dépasse un seuil défini (ex. 150% du budget journalier prévu).
**Renforce** : protège contre les incidents de facturation incontrôlée, particulièrement critiques à l'échelle de millions d'utilisateurs.

---

## Catégorie 11 — Tests de bout en bout (synthèse finale)

### T11.1 — Parcours utilisateur complet, multi-moteur
**Méthode** : scénario simulant un utilisateur réel envoyant des requêtes successives routées vers des modèles de moteurs différents (llama.cpp, vLLM, ONNX), vérifiant la cohérence de l'expérience (latence comparable perçue, format de réponse identique).
**Critère de réussite** : aucune différence perceptible côté client entre les moteurs, conformément au principe d'abstraction de la section 4 de l'architecture.
**Renforce** : validation finale que la promesse de modularité est tenue en pratique, pas seulement en théorie de conception.

### T11.2 — Reconstruction complète depuis zéro (disaster recovery total)
**Méthode** : sur un cluster vide, exécuter uniquement `argocd app sync` depuis le repo Git racine, sans aucune intervention manuelle, et mesurer le temps jusqu'à ce que l'ensemble du système (tous modèles, passerelle, observabilité) soit "Healthy".
**Critère de réussite** : reconstruction complète réussie sans intervention manuelle, dans un délai documenté et acceptable (ce délai devient le RTO — Recovery Time Objective — officiel du système).
**Renforce** : c'est le test ultime de la philosophie GitOps — si ce test échoue, Git n'est pas réellement la source de vérité, quoi qu'en dise la documentation.

### T11.3 — Audit de traçabilité complète
**Méthode** : pour un incident simulé (rollback déclenché automatiquement), vérifier qu'il est possible de reconstituer entièrement la chronologie (quel commit, quel test a échoué, quelle action corrective, à quelle heure) uniquement à partir de Git et des logs, sans connaissance tribale.
**Critère de réussite** : reconstitution complète et sans ambiguïté de la chronologie par une personne n'ayant pas participé à l'incident.
**Renforce** : condition de survie du système sur plusieurs années avec rotation d'équipe — un système qui ne peut être compris que par ses créateurs originaux n'est pas durable.

---

## Tableau de synthèse — Critère global d'approbation (GO/NO-GO)

| Catégorie | Nombre de tests | Bloquant pour la mise en production |
|---|---|---|
| 1. Packaging et intégrité du modèle | 4 | Oui — sans exception |
| 2. Infrastructure déclarative | 5 | Oui — sans exception |
| 3. Synchronisation ArgoCD | 5 | Oui — sans exception |
| 4. Chargement et démarrage | 3 | Oui — sans exception |
| 5. API de serving | 5 | Oui — sans exception |
| 6. Robustesse GPU et scheduling | 5 | Oui — sans exception |
| 7. Charge et performance | 5 | Oui pour T7.1/T7.2 ; T7.3/T7.4/T7.5 requis avant montée en charge majeure |
| 8. Résilience et chaos engineering | 6 | Oui pour T8.1/T8.2/T8.6 ; les autres requis avant exposition publique large |
| 9. Sécurité | 5 | Oui — sans exception |
| 10. Coût et gouvernance | 2 | Oui avant ouverture à un trafic facturé |
| 11. Bout en bout | 3 | Oui — sans exception, conditionne l'approbation finale |

**Règle d'approbation finale** : le système n'est certifié prêt pour la production à grande échelle que lorsque l'intégralité des tests bloquants ci-dessus passe simultanément sur un même commit, dans un même run de pipeline CI/CD reproductible. Toute exception doit être documentée comme un ADR explicite avec date de remédiation engagée — jamais comme un oubli silencieux.