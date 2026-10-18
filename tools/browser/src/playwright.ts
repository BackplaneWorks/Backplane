// playwright-core, loaded only after init.ts has made its `ws` choosable
// (see there and bunws.ts). Import chromium from here, never from
// "playwright-core" directly.
import "./init.ts";
export const { chromium } = require("playwright-core") as typeof import("playwright-core");
