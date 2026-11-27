<template>
  <div class="dashboard-root">
    <ClientOnly>
      <BackToTop />
    </ClientOnly>
    <Sidebar />
    <MediumView>
      <Navbar />
      <div id="dashboard-scroll" class="dashboard-scroll">
        <NuxtPage />
      </div>
    </MediumView>
  </div>
</template>
<script setup lang="ts">
const nodesStore = useNodesStore();
const userStore = useUserStore();
const preferences = usePreferencesStore();

const { setAppColor, themeColor } = useAppColors();
const { locale: i18nLocale, loadLocaleMessages } = useI18n();
const { isLoading } = useAppState();
useStyleInjection(); // Custom styles for the app and documents
useAppTheme(); // App chrome theme (built-in or plugin)
const { initGlobalListeners, destroyGlobalListeners } = useCommandCenter();

const primaryColor = preferences.get('primaryColor');
const interfaceStyle = preferences.get('style');
const locale = preferences.get('locale');

/* Watchers to make sure app state reflects preferences */
watch(
  primaryColor,
  color => {
    setAppColor(Number(color));
  },
  { immediate: true },
);

useHead({
  meta: [
    {
      name: 'theme-color',
      content: themeColor,
    },
  ],
});

watch(
  interfaceStyle,
  style => {
    if (import.meta.client) document.documentElement.classList.toggle('glassmorphism', style === 'glassmorphism');
  },
  { immediate: true },
);

watch(
  locale,
  async newLocale => {
    await loadLocaleMessages(newLocale);
    if (newLocale) i18nLocale.value = newLocale;
  },
  { immediate: true },
);

nodesStore.init();
userStore.fetch();
preferences.fetchFromBackend();

onMounted(() => {
  initGlobalListeners();
  isLoading.value = false;
});
onBeforeUnmount(() => {
  destroyGlobalListeners();
});
</script>

<style lang="scss">
.dashboard-root {
  display: flex;
  flex-direction: column;
  height: 100%;
}

// Page scroller: Navbar stays above it and never scrolls. TOC active-heading,
// BackToTop and anchor scroll-padding all target this element (not #app).
.dashboard-scroll {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  scroll-padding-top: 70px;
}

@media print {
  .dashboard-scroll {
    overflow: visible;
  }
}
</style>
