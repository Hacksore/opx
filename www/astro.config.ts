import { defineConfig } from "astro/config";
import vercel from "@astrojs/vercel";
import mdx from "@astrojs/mdx";
import sitemap from "@astrojs/sitemap";
import compress from "astro-compress";

const integrations = [sitemap(), mdx(), compress()];

// https://astro.build/config
export default defineConfig({
  site: "https://tryopx.com",
  output: "static",
  trailingSlash: "never",
  integrations,
  adapter: vercel({
    webAnalytics: {
      enabled: true,
    },
  }),
});
