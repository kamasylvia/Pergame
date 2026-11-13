export default {
  actions: {
    download: 'Datei herunterladen',
    upload: {
      idle: 'Hochladen',
      multiple: '{n} Datei hochladen | {n} Dateien hochladen',
      progress: 'Lade {n} / {total} Dateien hoch',
    },
  },

  appdrop: {
    addMore: 'Mehr Dateien hinzufügen',
    file: 'Datei | Dateien',
    link: 'hier klicken',
    max: 'Maximal {n} Dateien',
    prompt: 'Ziehe deine Datei hierher oder klicke auf {link}, um sie auszuwählen',
    promptMobile: 'Tippe hier, um Dateien von deinem Gerät auszuwählen',
    promptPlural: 'Ziehe deine Dateien hierher oder klicke auf {link}, um sie auszuwählen',
    removeFile: 'Datei entfernen',
  },
  cache: {
    clear: 'Leeren',
    limitMb: 'Limit (MB)',
    title: 'Lokaler Cache',
    usage: '{used} in {count} Einträgen auf diesem Gerät',
  },
  dedupe: {
    done: '{migrated} migriert, {saved} gespart',
    failed: 'Migration fehlgeschlagen',
    hint: 'Alte Objekte zu inhaltsadressierten Blobs zusammenführen ( spart Speicherplatz bei Duplikaten).',
    run: 'Migration starten',
    running: 'Migriert …',
    title: 'Deduplizierung',
  },

  diagram: 'Diagrammeditor',

  edit: {
    description: 'Verwalte Ressourcen und Dateien auf dem Server. Du kannst Metadaten bearbeiten und Dateien löschen.',
    title: 'Datei aktualisieren',
  },

  labels: {
    originalPath: 'Ursprünglicher Pfad',
    path: 'Pfad',
  },

  meta: {
    shortTitle: 'CDN',
    title: 'Dateimanager',
  },

  notifications: {
    error: 'Das Hochladen von {file} ist fehlgeschlagen: {error}',
    successMsg: '{n} Datei erfolgreich hochgeladen | {n} Dateien erfolgreich hochgeladen',
    successTitle: 'Hochladen abgeschlossen',
  },

  orphans: {
    deleteAll: 'Alle löschen',
    deleted: 'Bereinigung fertig',
    deletedDetail: '{count} Objekt(e) entfernt, {size} freigegeben',
    deleteFailed: 'Löschen fehlgeschlagen',
    found: '{count} ungenutzt · {size}',
    hint: 'Prüft gespeicherte Objekte, auf die kein Dokument verweist (Reste fehlgeschlagener Importe oder abgebrochener Uploads).',
    loading: 'Wird geladen…',
    none: 'Nichts Unbenutztes — der Speicher passt zu den Dokumenten.',
    preview: 'Vorschau',
    scan: 'Prüfen',
    scanFailed: 'Prüfung fehlgeschlagen',
    scanning: 'Prüfe…',
    title: 'Ungenutzte Dateien',
  },

  page: {
    empty: 'Es wurden noch keine Dateien hochgeladen.',
  },
  preview: {
    title: 'Vorschau',
    unavailable: 'Für diesen Dateityp ist keine Vorschau verfügbar',
  },
  storageUsed: 'Belegter Speicherplatz',
  unlimited: 'Unbegrenzt',
};
