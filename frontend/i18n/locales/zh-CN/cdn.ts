export default {
  actions: {
    download: '下载文件',
    upload: {
      idle: '上传',
      multiple: '上传 {n} 个文件',
      progress: '上传中 {n} / {total}',
    },
  },

  appdrop: {
    addMore: '新增更多文件',
    file: '文件 | 文件们',
    link: '点击这里',
    max: '最多 {n} 个文件',
    prompt: '将文件拖拽至此，或{link}选择文件',
    promptMobile: '轻触以选择设备中的文件',
    promptPlural: '将文件拖拽至此，或{link}选择文件',
    removeFile: '移除文件',
  },
  cache: {
    clear: '清空',
    limitMb: '上限(MB)',
    title: '本地缓存',
    usage: '本机 {count} 条，共 {used}',
  },
  dedupe: {
    done: '已迁移 {migrated} 个，节省 {saved}',
    failed: '迁移失败',
    hint: '将旧对象迁移到内容寻址 blob（重复文件省空间）。',
    run: '开始迁移',
    running: '迁移中…',
    title: '去重迁移',
  },

  diagram: '图表编辑器',

  edit: {
    description: '管理服务器上的资源与文件。你可以编辑元数据或删除文件。',
    title: '更新文件',
  },

  labels: {
    originalPath: '原始路径',
    path: '路径',
  },

  meta: {
    shortTitle: 'CDN',
    title: '文件管理器',
  },

  notifications: {
    error: '{file} 上传失败：{error}',
    successMsg: '已成功上传 {n} 个文件',
    successTitle: '上传完成',
  },

  orphans: {
    deleteAll: '全部删除',
    deleted: '清理完成',
    deletedDetail: '已删除 {count} 个对象,释放 {size}',
    deleteFailed: '删除失败',
    found: '{count} 个未使用 · {size}',
    hint: '检查没有文档引用的存储对象(失败导入或中断上传的残留)。',
    none: '没有残留——存储与文档一致。',
    scan: '检查',
    scanFailed: '检查失败',
    scanning: '检查中…',
    title: '未使用文件',
  },

  page: {
    empty: '尚未上传任何文件。',
  },
  preview: {
    title: '预览',
    unavailable: '此文件类型无法预览',
  },
  storageUsed: '已使用的存储空间',
  unlimited: '无限制',
};
