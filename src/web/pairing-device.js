// No browser permission or network lookup: use the platform already advertised
// by this client. The name remains editable and uses header-safe characters.
export function defaultDeviceName(navigator = {}, desktop = false) {
  const ua = navigator.userAgent || '', platform = navigator.userAgentData?.platform || navigator.platform || '';
  const os = /iPhone|iPod/.test(ua) ? 'iPhone'
    : /iPad/.test(ua) || /Mac/.test(platform) && navigator.maxTouchPoints > 1 ? 'iPad'
    : /Android/.test(ua) ? 'Android'
    : /CrOS/.test(ua) ? 'ChromeOS'
    : /Win/.test(platform + ua) ? 'Windows'
    : /Mac/.test(platform + ua) ? 'macOS'
    : /Linux/.test(platform + ua) ? 'Linux' : 'Device';
  const browser = /Edg(?:e|A|iOS)?\//.test(ua) ? 'Edge'
    : /OPR\/|Opera/.test(ua) ? 'Opera'
    : /SamsungBrowser\//.test(ua) ? 'Samsung Internet'
    : /Firefox\/|FxiOS\//.test(ua) ? 'Firefox'
    : /Chrome\/|CriOS\//.test(ua) ? 'Chrome'
    : /Safari\//.test(ua) ? 'Safari' : 'Browser';
  return os + ' / ' + (desktop ? 'Desktop' : browser);
}
