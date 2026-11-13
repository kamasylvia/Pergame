export default {
  actions: {
    download: '下載檔案',
    upload: {
      idle: '上傳',
      multiple: '上傳 {n} 個檔案',
      progress: '上傳中 {n} / {total}',
    },
  },

  appdrop: {
    addMore: '新增更多檔案',
    file: '檔案 | 檔案們',
    link: '按這裡',
    max: '最多 {n} 個檔案',
    prompt: '將檔案拖曳至此，或{link}選擇檔案',
    promptMobile: '輕觸以選擇裝置中的檔案',
    promptPlural: '將檔案拖曳至此，或{link}選擇檔案',
    removeFile: '移除檔案',
  },
  cache: {
    clear: '清空',
    limitMb: '上限(MB)',
    title: '本機快取',
    usage: '本機 {count} 條，共 {used}',
  },
  dedupe: {
    done: '已遷移 {migrated} 個，節省 {saved}',
    failed: '遷移失敗',
    hint: '將舊物件遷移到內容定址 blob（重複檔案省空間）。',
    run: '開始遷移',
    running: '遷移中…',
    title: '去重遷移',
  },

  diagram: '圖表編輯器',

  edit: {
    description: '管理伺服器上的資源與檔案。你可以編輯中繼資料或刪除檔案。',
    title: '更新檔案',
  },

  labels: {
    originalPath: '原始路徑',
    path: '路徑',
  },

  meta: {
    shortTitle: 'CDN',
    title: '檔案管理員',
  },

  notifications: {
    error: '{file} 上傳失敗：{error}',
    successMsg: '已成功上傳 {n} 個檔案',
    successTitle: '上傳完成',
  },

  orphans: {
    deleteAll: '全部刪除',
    deleted: '清理完成',
    deletedDetail: '已刪除 {count} 個物件,釋放 {size}',
    deleteFailed: '刪除失敗',
    found: '{count} 個未使用 · {size}',
    hint: '檢查沒有文件引用的儲存物件(失敗匯入或中斷上傳的殘留)。',
    none: '沒有殘留——儲存與文件一致。',
    preview: '預覽',
    scan: '檢查',
    scanFailed: '檢查失敗',
    scanning: '檢查中…',
    title: '未使用檔案',
  },

  page: {
    empty: '尚未上傳任何檔案。',
  },
  preview: {
    title: '預覽',
    unavailable: '此檔案類型無法預覽',
  },
  storageUsed: '已使用的儲存空間',
  unlimited: '無限制',
};
