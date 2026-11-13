export default {
  actions: {
    download: '파일 다운로드',
    upload: {
      idle: '업로드',
      multiple: '파일 {n}개 업로드 | 파일 {n}개 업로드',
      progress: '업로드 중 {n} / {total}',
    },
  },

  appdrop: {
    addMore: '파일 추가',
    file: '파일 | 파일들',
    link: '여기를 클릭',
    max: '최대 {n}개 파일',
    prompt: '파일을 여기로 드래그 앤 드롭하거나 {link}해서 선택하세요',
    promptMobile: '기기에서 파일을 선택하려면 탭하세요',
    promptPlural: '파일을 여기로 드래그 앤 드롭하거나 {link}해서 선택하세요',
    removeFile: '파일 제거',
  },
  cache: {
    clear: '비우기',
    limitMb: '제한(MB)',
    title: '로컬 캐시',
    usage: '이 기기에 {count}개 항목, {used}',
  },
  dedupe: {
    done: '{migrated}개 마이그레이션, {saved} 절약',
    failed: '마이그레이션 실패',
    hint: '레거시 객체를 콘텐츠 주소 지정 블롭으로 이동 (중복 절약).',
    run: '마이그레이션 실행',
    running: '마이그레이션 중…',
    title: '중복 제거',
  },

  diagram: '다이어그램 편집기',

  edit: {
    description: '서버에 있는 리소스와 파일을 관리합니다. 메타데이터를 수정하거나 파일을 삭제할 수 있습니다.',
    title: '파일 업데이트',
  },

  labels: {
    originalPath: '원본 경로',
    path: '경로',
  },

  meta: {
    shortTitle: 'CDN',
    title: '파일 관리자',
  },

  notifications: {
    error: '{file} 업로드 실패: {error}',
    successMsg: '파일 {n}개가 성공적으로 업로드되었습니다 | 파일 {n}개가 성공적으로 업로드되었습니다',
    successTitle: '업로드 완료',
  },

  orphans: {
    deleteAll: '모두 삭제',
    deleted: '정리 완료',
    deletedDetail: '{count}개 객체 삭제, {size} 확보',
    deleteFailed: '삭제 실패',
    found: '{count}개 미사용 · {size}',
    hint: '문서가 참조하지 않는 저장 객체를 찾습니다(실패한 가져오기나 중단된 업로드의 잔여물).',
    none: '남은 파일이 없습니다 — 저장소와 문서가 일치합니다.',
    preview: '미리 보기',
    scan: '확인',
    scanFailed: '확인 실패',
    scanning: '확인 중…',
    title: '사용되지 않는 파일',
  },

  page: {
    empty: '아직 업로드된 파일이 없습니다.',
  },
  preview: {
    title: '미리보기',
    unavailable: '이 파일 형식은 미리보기를 지원하지 않습니다',
  },
  storageUsed: '사용된 저장 용량',
  unlimited: '무제한',
};
