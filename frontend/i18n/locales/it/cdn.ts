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
  cache: {
    clear: 'Svuota',
    limitMb: 'Limite (MB)',
    title: 'Cache locale',
    usage: '{used} in {count} voci su questo dispositivo',
  },
  dedupe: {
    done: '{migrated} migrati, {saved} risparmiati',
    failed: 'Migrazione fallita',
    hint: 'Sposta gli oggetti legacy nei blob indirizzati per contenuto (risparmia sui duplicati).',
    run: 'Avvia migrazione',
    running: 'Migrazione…',
    title: 'Deduplicazione',
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
    loading: 'Caricamento…',
    none: 'Niente di inutilizzato — lo spazio corrisponde ai documenti.',
    preview: 'Anteprima',
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
  unlimited: 'Illimitato',
};
