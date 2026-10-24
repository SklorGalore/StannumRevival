import { defineConfig } from '@playwright/test';
import { existsSync } from 'node:fs';

const localChrome='/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const executablePath=process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE || (existsSync(localChrome)?localChrome:undefined);
const production=process.env.NODAL_TEST_PRODUCTION==='1';
const baseURL=`http://127.0.0.1:${production?4173:5173}`;

export default defineConfig({
  testDir: './tests/browser',
  fullyParallel: false,
  timeout: 60_000,
  use: { baseURL, browserName:'chromium', viewport:{width:1440,height:1000}, launchOptions:{executablePath} },
  webServer: { command:production?'npm run preview -- --port 4173 --strictPort':'npm run dev -- --port 5173 --strictPort', url:baseURL, reuseExistingServer:!process.env.CI },
});
