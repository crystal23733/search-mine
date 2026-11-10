import { test } from "@playwright/test";
import { pwaInstallationTests } from "./helpers/pwa-installation";
test.use({ baseURL: "http://127.0.0.1:4173" });
pwaInstallationTests();
