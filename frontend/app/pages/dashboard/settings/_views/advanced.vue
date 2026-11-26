<template>
  <div>
    <h2 class="page-title">{{ t('settings.advanced.title') }}</h2>
    <p class="page-subtitle">{{ t('settings.advanced.subtitle') }}</p>

    <div>
      <h3>{{ t('settings.advanced.resetPreferences') }}</h3>
      <p>{{ t('settings.advanced.resetPreferencesDesc') }}</p>
      <AppButton type="danger" @click="preferences.reset">{{ t('settings.advanced.resetPreferencesBtn') }}</AppButton>
    </div>
    <div>
      <h3>{{ t('settings.other.developerMode') }}</h3>
      <p>{{ t('settings.other.developerModeDesc') }}</p>
      <AppToggle v-model="developerMode.value" />
    </div>
    <div>
      <h3>{{ t('settings.advanced.resetLocalStorage') }}</h3>
      <p>{{ t('settings.advanced.resetLocalStorageDesc') }}</p>
      <AppButton type="danger" @click="deleteLocalStorage">{{ t('settings.advanced.resetLocalStorageBtn') }}</AppButton>
    </div>
  </div>
</template>

<script setup lang="ts">
import localForage from 'localforage';

const preferences = usePreferencesStore();

const developerMode = preferences.get('developerMode');

const { t } = useI18nT();

const deleteLocalStorage = () => {
  localStorage.clear();
  localForage.clear();
  location.reload();
};
</script>

<style scoped lang="scss">
h3 {
  margin: 3rem 0 0.5rem;
  font-weight: 500;
}
</style>
