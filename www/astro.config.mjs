import { defineConfig } from "astro/config";
import vercel from "@astrojs/vercel/static";
import tailwind from "@astrojs/tailwind";
import mdx from "@astrojs/mdx";
import react from "@astrojs/react";
import sitemap from "@astrojs/sitemap";
import compress from "astro-compress";

const integrations = [sitemap(), react(), tailwind(), mdx(), compress()];

// https://astro.build/config
export default defineConfig({
  site: "https://tryopx.com",
  trailingSlash: "never",
  integrations,
  adapter: vercel({
    webAnalytics: {
      enabled: true,
    },
  }),
});
