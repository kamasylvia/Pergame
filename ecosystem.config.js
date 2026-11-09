module.exports = {
  apps: [
    {
      name: "pergame-backend",
      script: "./backend",
      env: {
        PORT: 8201,
        FRONTEND_URL: "https://alexandrie-hub.fr",
        COOKIE_DOMAIN: "alexandrie-hub.fr",
        ADMIN_ACCOUNTS: "152981937240150016",

        S3_ENDPOINT: "cdn.alexandrie-hub.fr",
        S3_PUBLIC_URL: "https://cdn.alexandrie-hub.fr",
        S3_BUCKET: "pergame",
        S3_SECURE: "true",

        DATABASE_HOST: "localhost",
        DATABASE_PORT: 3306,
        DATABASE_NAME: "pergame",

        CPWD: "./backend/",
        GIN_MODE: "release",
      },
    },
    {
      name: "pergame-frontend",
      cwd: "./frontend",
      script: "bun .output/server/index.mjs",
      env: {
        NODE_ENV: "production",
        PORT: 8200,
      },
    },
    {
      name: "pergame-docs",
      cwd: "./docs",
      script: "bun .output/server/index.mjs",
      env: {
        NODE_ENV: "production",
        PORT: 8202,
      },
    },
  ],
};
