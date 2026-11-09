export default defineAppConfig({
  ui: {
    colors: {
      primary: "blue",
      neutral: "slate",
    },
    footer: {
      slots: {
        root: "border-t border-default",
        left: "text-sm text-muted",
      },
    },
  },
  seo: {
    siteName: "Pergame Documentation",
  },
  header: {
    title: "Pergame",
    to: "/",
    logo: {
      alt: "",
      light: "",
      dark: "",
    },
    search: true,
    colorMode: true,
    links: [
      {
        icon: "i-simple-icons-github",
        to: "https://github.com/kamasylvia/Pergame",
        target: "_blank",
        "aria-label": "GitHub",
      },
    ],
  },
  footer: {
    credits: `Pergame • © ${new Date().getFullYear()}`,
    colorMode: false,
    links: [
      {
        icon: "i-simple-icons-discord",
        to: "https://discord.gg/UPsEg6egPj",
        target: "_blank",
        "aria-label": "Nuxt on Discord",
      },
      {
        icon: "i-simple-icons-github",
        to: "https://github.com/kamasylvia/Pergame",
        target: "_blank",
        "aria-label": "Pergame on GitHub",
      },
    ],
  },
  toc: {
    title: "On this page",
    bottom: {
      title: "Pergame",
      edit: "https://github.com/kamasylvia/Pergame/edit/main/docs/content",
      links: [
        {
          icon: "i-lucide-star",
          label: "Star on GitHub",
          to: "https://github.com/kamasylvia/Pergame",
          target: "_blank",
        },
        {
          icon: "i-lucide-book-open",
          label: "Join the community",
          to: "https://discord.gg/UPsEg6egPj",
          target: "_blank",
        },
      ],
    },
  },
});
