export default {
  actions: {
    download: 'Download file',
    upload: {
      idle: 'Upload',
      multiple: 'Upload {n} file | Upload {n} files',
      progress: 'Uploading {n} / {total}',
    },
  },

  appdrop: {
    addMore: 'Add more files',
    file: 'file | files',
    link: 'click here',
    max: 'Maximum {n} files',
    prompt: 'Drag and drop your file here or {link} to select',
    promptMobile: 'Tap to select files from your device',
    promptPlural: 'Drag and drop your files here or {link} to select',
    removeFile: 'Remove file',
  },
  cache: {
    clear: 'Clear',
    limitMb: 'Limit (MB)',
    title: 'Local cache',
    usage: '{used} in {count} entries on this device',
  },
  dedupe: {
    done: '{migrated} migrated, {saved} saved',
    failed: 'Migration failed',
    hint: 'Move legacy objects to content-addressed blobs (saves space on duplicates).',
    run: 'Run migration',
    running: 'Migrating…',
    title: 'Deduplication',
  },

  diagram: 'Diagram editor',

  edit: {
    description: 'Manage resources and files on the server. You can edit metadata and delete files.',
    title: 'Update file',
  },

  labels: {
    originalPath: 'Original path',
    path: 'Path',
  },

  meta: {
    shortTitle: 'CDN',
    title: 'File manager',
  },

  notifications: {
    error: 'Upload failed for {file}: {error}',
    successMsg: '{n} file uploaded successfully | {n} files uploaded successfully',
    successTitle: 'Upload complete',
  },

  orphans: {
    deleteAll: 'Delete all',
    deleted: 'Cleanup done',
    deletedDetail: '{count} object(s) removed, {size} freed',
    deleteFailed: 'Delete failed',
    found: '{count} unused · {size}',
    hint: 'Check for stored objects no document points at (leftovers from failed imports or interrupted uploads).',
    loading: 'Loading…',
    none: 'Nothing unused — storage matches your documents.',
    preview: 'Preview',
    scan: 'Check',
    scanFailed: 'Scan failed',
    scanning: 'Checking…',
    title: 'Unused files',
  },

  page: {
    empty: 'No files uploaded yet.',
  },
  preview: {
    title: 'Preview',
    unavailable: 'Preview not available for this file type',
  },
  storageUsed: 'Storage used',
  unlimited: 'Unlimited',
};
