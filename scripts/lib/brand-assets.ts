export const BRAND_ASSET_PATHS = {
  developmentIconComposerProject: "assets/dev/app-icon.icon",
  developmentIosIconPng: "assets/dev/backplane-ios-1024.png",
  developmentUniversalIconPng: "assets/dev/backplane-universal-1024.png",

  productionIconComposerProject: "assets/prod/app-icon.icon",
  productionIosIconPng: "assets/prod/backplane-ios-1024.png",
  productionMacIconPng: "assets/prod/backplane-macos-1024.png",
  productionLinuxIconPng: "assets/prod/backplane-universal-1024.png",
  productionWindowsIconIco: "assets/prod/backplane-windows.ico",
  productionWebFaviconIco: "assets/prod/backplane-web-favicon.ico",
  productionWebFavicon16Png: "assets/prod/backplane-web-favicon-16x16.png",
  productionWebFavicon32Png: "assets/prod/backplane-web-favicon-32x32.png",
  productionWebAppleTouchIconPng: "assets/prod/backplane-web-apple-touch-180.png",

  nightlyIconComposerProject: "assets/nightly/app-icon.icon",
  nightlyIosIconPng: "assets/nightly/backplane-ios-1024.png",
  nightlyMacIconPng: "assets/nightly/backplane-macos-1024.png",
  nightlyLinuxIconPng: "assets/nightly/backplane-universal-1024.png",
  nightlyWindowsIconIco: "assets/nightly/backplane-windows.ico",
  nightlyWebFaviconIco: "assets/nightly/backplane-web-favicon.ico",
  nightlyWebFavicon16Png: "assets/nightly/backplane-web-favicon-16x16.png",
  nightlyWebFavicon32Png: "assets/nightly/backplane-web-favicon-32x32.png",
  nightlyWebAppleTouchIconPng: "assets/nightly/backplane-web-apple-touch-180.png",

  developmentDesktopIconPng: "assets/dev/backplane-macos-1024.png",
  developmentWindowsIconIco: "assets/dev/backplane-windows.ico",
  developmentWebFaviconIco: "assets/dev/backplane-web-favicon.ico",
  developmentWebFavicon16Png: "assets/dev/backplane-web-favicon-16x16.png",
  developmentWebFavicon32Png: "assets/dev/backplane-web-favicon-32x32.png",
  developmentWebAppleTouchIconPng: "assets/dev/backplane-web-apple-touch-180.png",
} as const;

export type WebAssetBrand = "development" | "nightly" | "production";

export const WEB_ASSET_CHANNELS = ["latest", "nightly"] as const;

export type WebAssetChannel = (typeof WEB_ASSET_CHANNELS)[number];

export function resolveWebAssetBrandForChannel(channel: WebAssetChannel): WebAssetBrand {
  return channel === "nightly" ? "nightly" : "production";
}

export function resolveWebAssetBrandForPackageVersion(version: string): WebAssetBrand {
  return version.includes("-nightly.") ? "nightly" : "production";
}

export interface IconOverride {
  readonly sourceRelativePath: string;
  readonly targetRelativePath: string;
}

const WEB_ICON_TARGET_FILENAMES = {
  faviconIco: "favicon.ico",
  favicon16Png: "favicon-16x16.png",
  favicon32Png: "favicon-32x32.png",
  appleTouchIconPng: "apple-touch-icon.png",
} as const;

const WEB_ICON_SOURCE_PATHS_BY_BRAND = {
  development: {
    faviconIco: BRAND_ASSET_PATHS.developmentWebFaviconIco,
    favicon16Png: BRAND_ASSET_PATHS.developmentWebFavicon16Png,
    favicon32Png: BRAND_ASSET_PATHS.developmentWebFavicon32Png,
    appleTouchIconPng: BRAND_ASSET_PATHS.developmentWebAppleTouchIconPng,
  },
  nightly: {
    faviconIco: BRAND_ASSET_PATHS.nightlyWebFaviconIco,
    favicon16Png: BRAND_ASSET_PATHS.nightlyWebFavicon16Png,
    favicon32Png: BRAND_ASSET_PATHS.nightlyWebFavicon32Png,
    appleTouchIconPng: BRAND_ASSET_PATHS.nightlyWebAppleTouchIconPng,
  },
  production: {
    faviconIco: BRAND_ASSET_PATHS.productionWebFaviconIco,
    favicon16Png: BRAND_ASSET_PATHS.productionWebFavicon16Png,
    favicon32Png: BRAND_ASSET_PATHS.productionWebFavicon32Png,
    appleTouchIconPng: BRAND_ASSET_PATHS.productionWebAppleTouchIconPng,
  },
} as const satisfies Record<WebAssetBrand, Record<keyof typeof WEB_ICON_TARGET_FILENAMES, string>>;

export function resolveWebIconOverrides(
  brand: WebAssetBrand,
  targetDirectory: string,
): ReadonlyArray<IconOverride> {
  const sourcePaths = WEB_ICON_SOURCE_PATHS_BY_BRAND[brand];
  return [
    {
      sourceRelativePath: sourcePaths.faviconIco,
      targetRelativePath: `${targetDirectory}/${WEB_ICON_TARGET_FILENAMES.faviconIco}`,
    },
    {
      sourceRelativePath: sourcePaths.favicon16Png,
      targetRelativePath: `${targetDirectory}/${WEB_ICON_TARGET_FILENAMES.favicon16Png}`,
    },
    {
      sourceRelativePath: sourcePaths.favicon32Png,
      targetRelativePath: `${targetDirectory}/${WEB_ICON_TARGET_FILENAMES.favicon32Png}`,
    },
    {
      sourceRelativePath: sourcePaths.appleTouchIconPng,
      targetRelativePath: `${targetDirectory}/${WEB_ICON_TARGET_FILENAMES.appleTouchIconPng}`,
    },
  ];
}

export const DEVELOPMENT_ICON_OVERRIDES = resolveWebIconOverrides("development", "dist/client");

export const DEVELOPMENT_PUBLIC_ICON_OVERRIDES = resolveWebIconOverrides(
  "development",
  "apps/web/public",
);
