<template>
  <div>
    <h2 class="page-title">{{ t('settings.appearance.title') }}</h2>
    <p class="page-subtitle">{{ t('settings.appearance.subtitle') }}</p>

    <AppPreferenceInputSection :options="options" />
  </div>
</template>

<script setup lang="ts">
const appColors = useAppColors();
const { t } = useI18nT();

const colorMode = useColorMode();
const localeOptions = useLocaleOptions();

const options = computed(
  () =>
    [
      {
        label: t('settings.appearance.general'),
        options: [
          {
            label: t('settings.appearance.enableDarkMode'),
            type: 'toggle' as const,
            key: 'darkMode' as const,
            onChange: (option: boolean) => {
              colorMode.preference = option ? 'dark' : 'light';
              document.body.style.colorScheme = colorMode.preference;
            },
          },
          {
            label: t('settings.appearance.codeTheme'),
            description: t('settings.appearance.codeThemeDesc'),
            type: 'select' as const,
            key: 'codeTheme' as const,
            choices: [
              { label: t('settings.appearance.codeThemeAuto'), id: 'auto' },
              { label: 'Palenight', id: 'palenight' },
              { label: 'Dracula', id: 'dracula' },
              { label: 'GitHub Light', id: 'github-light' },
              { label: 'Solarized Light', id: 'solarized-light' },
            ],
          },
          {
            label: t('settings.appearance.appTheme'),
            description: t('settings.appearance.appThemeDesc'),
            type: 'select' as const,
            key: 'appTheme' as const,
            choices: appThemeRegistry.value.map(theme => ({
              label: theme.origin === 'builtin' ? theme.name : `${theme.name} (${theme.originName ?? theme.origin})`,
              id: theme.id,
            })),
          },
          {
            label: t('settings.appearance.setLanguage'),
            description: t('settings.appearance.setLanguageDesc'),
            type: 'select' as const,
            key: 'locale' as const,
            choices: localeOptions.value,
          },
          {
            label: t('settings.appearance.chooseAccentColor'),
            type: 'color' as const,
            key: 'primaryColor' as const,
            onChange: (option: number) => {
              appColors.setAppColor(option);
            },
          },
          {
            label: t('settings.appearance.interfaceStyle'),
            description: t('settings.appearance.interfaceStyleDesc'),
            type: 'radio' as const,
            key: 'style' as const,
            choices: [
              { label: t('settings.appearance.styleDefault'), id: 'default' },
              { label: t('settings.appearance.styleGlassmorphism'), id: 'glassmorphism' },
            ],
          },
          {
            label: t('settings.appearance.displayTabs'),
            description: t('settings.appearance.displayTabsDesc'),
            type: 'toggle' as const,
            key: 'displayTabs' as const,
            tag: t('settings.nav.new'),
          },
          {
            label: t('settings.appearance.displayFeatureTags'),
            description: t('settings.appearance.displayFeatureTagsDesc'),
            type: 'toggle' as const,
            key: 'displayFeatureTags' as const,
          },
        ],
      },
      {
        label: t('settings.appearance.sidebar'),
        options: [
          {
            label: t('settings.appearance.enableCompactMode'),
            description: t('settings.appearance.enableCompactModeDesc'),
            type: 'toggle' as const,
            key: 'compactMode' as const,
          },
          {
            label: t('settings.appearance.normalizeFileIcons'),
            description: t('settings.appearance.normalizeFileIconsDesc'),
            type: 'toggle' as const,
            key: 'normalizeFileIcons' as const,
          },
          {
            label: t('settings.appearance.showItemsInSidebar'),
            type: 'groupCheckbox' as const,
            key: 'sidebarItems' as const,
            items: {
              manageCategories: t('settings.appearance.sidebarItems.manageCategories'),
              teams: t('settings.appearance.sidebarItems.teams'),
              importation: t('settings.appearance.sidebarItems.importation'),
              cdn: t('settings.appearance.sidebarItems.cdn'),
              documents: t('settings.appearance.sidebarItems.documents'),
              settings: t('settings.appearance.sidebarItems.settings'),
              home: t('settings.appearance.sidebarItems.home'),
              newPage: t('settings.appearance.sidebarItems.newPage'),
            },
          },
        ],
      },
      {
        label: t('settings.appearance.navbar'),
        options: [
          {
            label: t('settings.appearance.navbarItemsToDisplay'),
            type: 'groupCheckbox' as const,
            key: 'navbarItems' as const,
            items: {
              breadcrumbNav: t('settings.appearance.navbarItems.breadcrumb'),
              search: t('settings.appearance.navbarItems.search'),
              theme: t('settings.appearance.navbarItems.theme'),
              navigation: t('settings.appearance.navbarItems.navigation'),
            },
          },
        ],
      },
    ] as { label: string; options: Option[] }[],
);
</script>
