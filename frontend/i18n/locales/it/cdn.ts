export default {
  actions: {
    download: 'Scarica file',
    upload: {
      idle: 'Carica',
      multiple: 'Carica {n} file | Carica {n} file',
      progress: 'Caricamento in corso {n} / {total}',
    },
  },

  appdrop: {
    addMore: 'Aggiungi altri file',
    file: 'file | files',
    link: 'clicca qui',
    max: 'Massimo {n} file',
    prompt: 'Trascina e rilascia il tuo file qui o {link} per selezionarlo',
    promptMobile: 'Tocca per selezionare i file dal tuo dispositivo',
    promptPlural: 'Trascina e rilascia i tuoi file qui o {link} per selezionarli',
    removeFile: 'Rimuovi file',
  },
  diagram: 'Editor di diagrammi',
  edit: {
    description: 'Gestisci le risorse e i file sul server. Puoi modificare i metadati ed eliminare i file.',
    title: 'Aggiorna file',
  },

  labels: {
    originalPath: 'Percorso originale',
    path: 'Percorso',
  },

  meta: {
    shortTitle: 'CDN',
    title: 'Gestione file',
  },

  notifications: {
    error: 'Caricamento fallito per {file}: {error}',
    successMsg: '{n} file caricato con successo | {n} file caricati con successo',
    successTitle: 'Caricamento completato',
  },

  orphans: {
    deleteAll: 'Elimina tutto',
    deleted: 'Pulizia completata',
    deletedDetail: '{count} oggetto/i rimossi, {size} liberati',
    deleteFailed: 'Eliminazione non riuscita',
    found: '{count} inutilizzati · {size}',
    hint: 'Cerca oggetti memorizzati che nessun documento referenzia (residui di import falliti o caricamenti interrotti).',
    none: 'Niente di inutilizzato — lo spazio corrisponde ai documenti.',
    scan: 'Controlla',
    scanFailed: 'Controllo non riuscito',
    scanning: 'Controllo…',
    title: 'File inutilizzati',
  },

  page: {
    empty: 'Nessun file ancora caricato.',
  },

  preview: {
    title: 'Anteprima',
    unavailable: 'Anteprima non disponibile per questo tipo di file',
  },

  storageUsed: 'Spazio utilizzato',
};
