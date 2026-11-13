export default {
  actions: {
    download: 'Télécharger le fichier',
    upload: {
      idle: 'Téléverser',
      multiple: 'Téléverser {n} fichier | Téléverser {n} fichiers',
      progress: 'Téléversement de {n} / {total}',
    },
  },

  appdrop: {
    addMore: 'Ajouter plus de fichiers',
    file: 'fichier | fichiers',
    link: 'cliquez ici',
    max: 'Maximum {n} fichiers',
    prompt: 'Glissez-déposez votre fichier ici ou {link} pour sélectionner',
    promptMobile: 'Appuyez pour sélectionner des fichiers depuis votre appareil',
    promptPlural: 'Glissez-déposez vos fichiers ici ou {link} pour sélectionner',
    removeFile: 'Supprimer le fichier',
  },
  cache: {
    clear: 'Vider',
    limitMb: 'Limite (Mo)',
    title: 'Cache local',
    usage: '{used} dans {count} entrées sur cet appareil',
  },
  dedupe: {
    done: '{migrated} migrés, {saved} économisés',
    failed: 'Échec de la migration',
    hint: 'Migrer les objets vers des blobs adressés par contenu (économise les doublons).',
    run: 'Migrer',
    running: 'Migration…',
    title: 'Déduplication',
  },

  diagram: 'Éditeur de diagramme',

  edit: {
    description: 'Gérer les ressources et fichiers sur le serveur. Vous pouvez modifier les métadonnées et supprimer des fichiers du serveur.',
    title: 'Mettre à jour le fichier',
  },

  labels: {
    originalPath: "Chemin d'origine",
    path: 'Chemin',
  },

  meta: {
    shortTitle: 'CDN',
    title: 'Gestionnaire de fichiers',
  },

  notifications: {
    error: 'Échec du téléversement de {file}: {error}',
    successMsg: '{n} fichier téléversé avec succès | {n} fichiers téléversés avec succès',
    successTitle: 'Téléversement terminé',
  },

  orphans: {
    deleteAll: 'Tout supprimer',
    deleted: 'Nettoyage terminé',
    deletedDetail: '{count} objet(s) supprimé(s), {size} libérés',
    deleteFailed: 'Échec de la suppression',
    found: '{count} inutilisé(s) · {size}',
    hint: 'Recherche les objets stockés qu’aucun document ne référence (restes d’imports échoués ou d’envois interrompus).',
    none: 'Rien d’inutilisé — le stockage correspond aux documents.',
    preview: 'Aperçu',
    scan: 'Vérifier',
    scanFailed: 'Échec de la vérification',
    scanning: 'Vérification…',
    title: 'Fichiers inutilisés',
  },

  page: {
    empty: 'Aucun fichier téléversé pour le moment.',
  },
  preview: {
    title: 'Aperçu',
    unavailable: 'Aperçu non disponible pour ce type de fichier',
  },
  storageUsed: 'Espace utilisé',
  unlimited: 'Illimité',
};
